import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { DatabaseSync } from 'node:sqlite';
import { accountApi } from '../api.mjs';
import { AccountStore } from '../store.mjs';
import { accountWeb } from '../web.mjs';

const origin = 'https://accounts.example.test';
const device = 'd'.repeat(32);

async function fixture(t) {
  const store = new AccountStore(':memory:');
  const google = {
    authorization: ({ state }) => 'https://accounts.google.com/o/oauth2/v2/auth?state=' + (state ?? ''),
    identify: async (code) => ({ subject: code, email: code + '@example.test' })
  };
  const app = await accountApi(store, google);
  await accountWeb(app, store, google, { origin });
  t.after(async () => {
    await app.close();
    store.db.close();
  });
  const mac = async (who) => {
    const start = await app.inject({ method: 'POST', url: '/v1/login/start', payload: { port: 45900, device, name: 'Studio' } });
    return app.inject({ method: 'POST', url: '/v1/login/finish', payload: { flow: start.json().flow, code: who } });
  };
  const site = async (who) => {
    const start = await app.inject({ method: 'GET', url: '/auth/google' });
    const flow = /shifu_flow=([^;]+)/.exec(start.headers['set-cookie'])[1];
    return app.inject({ method: 'GET', url: `/auth/callback?code=${who}&state=${flow}`, headers: { cookie: `shifu_flow=${flow}` } });
  };
  return { app, store, mac, site };
}

test('a redeemed invite leaves the account waiting until it is approved, on the Mac and on the site', async (t) => {
  const { app, store, mac, site } = await fixture(t);
  const invite = store.issueInvite();
  const admission = (await mac('alice')).json();
  assert.equal(admission.status, 'invite_required');
  const redeemed = await app.inject({ method: 'POST', url: '/v1/login/invite', payload: { ticket: admission.ticket, code: invite } });
  assert.equal(redeemed.statusCode, 403);
  assert.equal(redeemed.json().error, 'awaiting_approval');
  assert.equal(store.db.prepare('SELECT count(*) AS n FROM macs').get().n, 0);

  const again = await mac('alice');
  assert.equal(again.statusCode, 403);
  assert.equal(again.json().error, 'awaiting_approval');
  assert.match((await site('alice')).body, /Waiting for approval/);
  assert.deepEqual(store.pending(), ['alice@example.test']);

  assert.ok(store.approve('alice@example.test'));
  assert.deepEqual(store.pending(), []);
  assert.equal((await mac('alice')).json().status, 'signed_in');
  assert.match((await site('alice')).body, /Welcome back/);
});

test('a rejected account cannot sign in, cannot be approved later, and its invite stays spent', async (t) => {
  const { app, store, mac } = await fixture(t);
  const invite = store.issueInvite();
  const ticket = (await mac('bob')).json().ticket;
  await app.inject({ method: 'POST', url: '/v1/login/invite', payload: { ticket, code: invite } });
  assert.ok(store.reject('bob@example.test'));
  assert.equal(store.approve('bob@example.test'), false);
  assert.equal((await mac('bob')).json().error, 'awaiting_approval');
  const carol = (await mac('carol')).json().ticket;
  const reused = await app.inject({ method: 'POST', url: '/v1/login/invite', payload: { ticket: carol, code: invite } });
  assert.equal(reused.json().error, 'invalid_invite');
});

test('approval is required for existing sessions too, and accounts from before review keep access', () => {
  const dir = mkdtempSync(join(tmpdir(), 'shifu-review-'));
  try {
    const file = join(dir, 'accounts.sqlite');
    const legacy = new DatabaseSync(file);
    legacy.exec(`CREATE TABLE accounts (subject TEXT PRIMARY KEY, email TEXT NOT NULL);
      INSERT INTO accounts VALUES ('eddie', 'eddie@example.test');`);
    legacy.close();
    const store = new AccountStore(file);
    assert.equal(store.approved('eddie'), true);
    const { session } = store.signIn({ subject: 'eddie', email: 'eddie@example.test', device, name: 'Mac' });
    assert.ok(store.authenticate(session));
    store.db.prepare('UPDATE accounts SET approved = 0').run();
    assert.equal(store.authenticate(session), undefined);
    store.db.close();
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
