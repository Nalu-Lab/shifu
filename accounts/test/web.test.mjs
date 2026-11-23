import assert from 'node:assert/strict';
import test from 'node:test';
import { accountApi } from '../api.mjs';
import { AccountStore } from '../store.mjs';
import { accountWeb } from '../web.mjs';

const origin = 'https://accounts.example.test';
const device = 'c'.repeat(32);

async function fixture(t) {
  const store = new AccountStore(':memory:');
  const google = {
    authorization: ({ state }) => 'https://accounts.google.com/o/oauth2/v2/auth?state=' + state,
    identify: async (code) => {
      if (!['alice', 'bob'].includes(code)) throw new Error();
      return { subject: code, email: code + '@example.test' };
    }
  };
  const app = await accountApi(store, google);
  await accountWeb(app, store, google, { origin });
  t.after(async () => {
    await app.close();
    store.db.close();
  });
  const jar = () => {
    const cookies = {};
    const keep = (reply) => {
      for (const line of [reply.headers['set-cookie'] ?? []].flat()) {
        const [pair] = line.split(';');
        const [name, value] = pair.split('=');
        if (value) cookies[name] = value;
        else delete cookies[name];
      }
      return reply;
    };
    const header = () => Object.entries(cookies).map(([k, v]) => `${k}=${v}`).join('; ');
    return {
      cookies,
      get: async (url) => keep(await app.inject({ method: 'GET', url, headers: { cookie: header() } })),
      post: async (url, form, from = origin) =>
        keep(
          await app.inject({
            method: 'POST',
            url,
            payload: new URLSearchParams(form).toString(),
            headers: { cookie: header(), origin: from, 'content-type': 'application/x-www-form-urlencoded' }
          })
        )
    };
  };
  // Sign in through the site as `who`; returns the browser and the last response.
  const signIn = async (who) => {
    const browser = jar();
    const start = await browser.get('/auth/google');
    assert.equal(start.statusCode, 303);
    const state = new URL(start.headers.location).searchParams.get('state');
    const back = await browser.get(`/auth/callback?code=${who}&state=${state}`);
    return { browser, back };
  };
  return { app, store, jar, signIn };
}

test('a new Google account must enter a valid invite on the site, and a spent code admits no one else', async (t) => {
  const { store, signIn } = await fixture(t);
  const code = store.issueInvite();

  const { browser, back } = await signIn('alice');
  assert.equal(back.statusCode, 303);
  assert.equal(back.headers.location, '/join');
  const form = await browser.get('/join');
  assert.match(form.body, /alice@example\.test/);
  assert.match(form.body, /Invite code/);

  const wrong = await browser.post('/join', { code: 'AAAA-BBBB-CCCC-DDDD-EEEE' });
  assert.equal(wrong.statusCode, 403);
  assert.match(wrong.body, /not valid/);

  const ok = await browser.post('/join', { code: code.toLowerCase() });
  assert.equal(ok.statusCode, 200);
  assert.match(ok.body, /You're in/);
  assert.equal(browser.cookies.shifu_ticket, undefined);
  // Joining on the web registers no Mac and issues no session.
  assert.equal(store.db.prepare('SELECT count(*) AS n FROM macs').get().n, 0);

  const other = await signIn('bob');
  const reused = await other.browser.post('/join', { code });
  assert.equal(reused.statusCode, 403);
  assert.equal(store.db.prepare('SELECT count(*) AS n FROM accounts').get().n, 1);
});

test('a member signs in with Google again on every visit and on the Mac, without a second invite', async (t) => {
  const { app, store, signIn } = await fixture(t);
  const code = store.issueInvite();
  const first = await signIn('alice');
  await first.browser.post('/join', { code });

  const again = await signIn('alice');
  assert.equal(again.back.statusCode, 200);
  assert.match(again.back.body, /Welcome back/);
  assert.deepEqual(Object.keys(again.browser.cookies), []);

  const start = await app.inject({ method: 'POST', url: '/v1/login/start', payload: { port: 45900, device, name: 'Studio' } });
  const finish = await app.inject({ method: 'POST', url: '/v1/login/finish', payload: { flow: start.json().flow, code: 'alice' } });
  assert.equal(finish.json().status, 'signed_in');
});

test('the callback refuses a mismatched state, a missing flow cookie, a desktop flow, and cross-site posts', async (t) => {
  const { app, store, jar, signIn } = await fixture(t);
  const browser = jar();
  const start = await browser.get('/auth/google');
  const forged = await browser.get('/auth/callback?code=alice&state=' + 'x'.repeat(43));
  assert.equal(forged.statusCode, 400);

  const stranger = jar();
  const state = new URL(start.headers.location).searchParams.get('state');
  assert.equal((await stranger.get(`/auth/callback?code=alice&state=${state}`)).statusCode, 400);

  // A desktop flow handle cannot finish on the website.
  const desktop = (await app.inject({ method: 'POST', url: '/v1/login/start', payload: { port: 45900, device, name: 'Studio' } })).json();
  const mixed = jar();
  mixed.cookies.shifu_flow = desktop.flow;
  assert.equal((await mixed.get(`/auth/callback?code=alice&state=${desktop.flow}`)).statusCode, 400);

  const code = store.issueInvite();
  const { browser: pending } = await signIn('alice');
  const cross = await pending.post('/join', { code }, 'https://evil.example');
  assert.equal(cross.statusCode, 403);
  assert.equal(store.db.prepare('SELECT count(*) AS n FROM accounts').get().n, 0);
});

test('pages send a strict content policy and serve the Google button', async (t) => {
  const { app } = await fixture(t);
  const home = await app.inject({ method: 'GET', url: '/' });
  assert.equal(home.statusCode, 200);
  assert.match(home.headers['content-security-policy'], /default-src 'none'/);
  assert.equal(home.headers['cache-control'], 'no-store');
  assert.equal(home.headers['referrer-policy'], 'same-origin');
  const img = await app.inject({ method: 'GET', url: '/google-sign-in.png' });
  assert.equal(img.headers['content-type'], 'image/png');
});
