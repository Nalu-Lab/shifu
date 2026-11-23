import assert from 'node:assert/strict';
import { generateKeyPairSync, sign } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { OAuth2Client } from 'google-auth-library';
import { accountApi } from '../api.mjs';
import { googleIdentity } from '../google.mjs';
import { AccountStore, digest } from '../store.mjs';

const first = 'a'.repeat(32);
const second = 'b'.repeat(32);
const id = (who) => ({ subject: who, email: who + '@example.test' });
async function fixture(t) {
  let time = Date.now();
  const store = new AccountStore(':memory:', () => time);
  const google = {
    authorization: ({ nonce }) => 'https://accounts.google.com/o/oauth2/v2/auth?nonce=' + nonce,
    identify: async (code) => {
      if (!['alice', 'bob'].includes(code)) throw new Error();
      return id(code);
    }
  };
  const app = await accountApi(store, google);
  t.after(async () => {
    await app.close();
    store.db.close();
  });
  const call = async (method, url, payload, session) => {
    const reply = await app.inject({
      method,
      url,
      payload,
      headers: session ? { authorization: 'Bearer ' + session } : {}
    });
    return { http: reply.statusCode, ...reply.json() };
  };
  const login = async (subject, device = first) => {
    const start = await call('POST', '/v1/login/start', {
      port: 45900,
      device,
      name: device === first ? 'Studio' : 'Laptop'
    });
    return call('POST', '/v1/login/finish', { flow: start.flow, code: subject });
  };
  const redeem = (ticket, code) => call('POST', '/v1/login/invite', { ticket, code });
  return {
    store,
    call,
    login,
    redeem,
    advance: (ms) => {
      time += ms;
    }
  };
}

test('Google alone grants a new identity no account, session, or device until a valid invite is redeemed', async (t) => {
  const { store, call, login, redeem } = await fixture(t);
  const invite = store.issueInvite();
  const user = await login('alice');
  assert.equal(user.status, 'invite_required');
  assert.equal((await call('GET', '/v1/me', undefined, user.ticket)).http, 401);
  for (const code of ['', 'not-a-code'])
    assert.equal((await redeem(user.ticket, code)).error, 'invalid_invite');
  for (const table of ['accounts', 'macs'])
    assert.equal(store.db.prepare('SELECT count(*) AS n FROM ' + table).get().n, 0);
  const member = await redeem(user.ticket, invite.toLowerCase().replaceAll('-', ' '));
  assert.equal(member.status, 'signed_in');
  assert.equal((await redeem(user.ticket, invite)).error, 'login_expired');
  assert.equal((await redeem((await login('bob')).ticket, invite)).error, 'invalid_invite');
  assert.equal(store.db.prepare('SELECT hash FROM invites').get().hash, digest(invite.replaceAll('-', '')));
  assert.notEqual(store.db.prepare('SELECT session_hash FROM macs').get().session_hash, member.session);
});

test('a returning Google account registers a second Mac without another code and can revoke it', async (t) => {
  const { store, call, login, redeem } = await fixture(t);
  const alice = await redeem((await login('alice')).ticket, store.issueInvite());
  const laptop = await login('alice', second);
  assert.equal(laptop.status, 'signed_in');
  const me = await call('GET', '/v1/me', undefined, laptop.session);
  assert.equal(me.email, 'alice@example.test');
  assert.equal(me.current, second);
  assert.deepEqual(me.devices, [
    { id: second, name: 'Laptop' },
    { id: first, name: 'Studio' }
  ]);
  const bob = await redeem((await login('bob')).ticket, store.issueInvite());
  assert.equal((await call('DELETE', '/v1/macs/' + second, undefined, bob.session)).http, 404);
  assert.deepEqual((await call('GET', '/v1/me', undefined, bob.session)).devices, [
    { id: first, name: 'Studio' }
  ]);
  assert.equal((await call('DELETE', '/v1/macs/' + second, undefined, alice.session)).http, 200);
  assert.equal((await call('GET', '/v1/me', undefined, laptop.session)).http, 401);
  await call('POST', '/v1/logout', undefined, alice.session);
  assert.equal((await call('GET', '/v1/me', undefined, alice.session)).http, 401);
  assert.equal((await login('alice')).status, 'signed_in');
});

test('competing first users cannot redeem the same code twice, and a duplicate login does not consume a spare invite', async (t) => {
  const { store, login, redeem } = await fixture(t);
  const [alice, bob, anotherAlice] = await Promise.all([
    login('alice'),
    login('bob'),
    login('alice', second)
  ]);
  const code = store.issueInvite();
  const [a, b] = await Promise.all([redeem(alice.ticket, code), redeem(bob.ticket, code)]);
  assert.deepEqual([a.http, b.http].sort(), [200, 403]);
  const spare = store.issueInvite();
  if (a.http === 200) {
    assert.equal((await redeem(anotherAlice.ticket, spare)).status, 'signed_in');
    assert.equal((await redeem(bob.ticket, spare)).status, 'signed_in');
  } else {
    assert.equal((await redeem(anotherAlice.ticket, spare)).status, 'signed_in');
    assert.equal((await redeem(alice.ticket, '')).status, 'signed_in');
  }
  assert.equal(store.db.prepare('SELECT count(*) AS n FROM accounts').get().n, 2);
});

test('the callback, admission ticket, invite, and session have independent lifetimes', async (t) => {
  const { store, call, login, redeem, advance } = await fixture(t);
  const start = await call('POST', '/v1/login/start', { port: 45000, device: first, name: 'Mac' });
  const finish = (code) => call('POST', '/v1/login/finish', { flow: start.flow, code });
  assert.equal((await finish('unverified')).http, 401);
  assert.equal((await finish('alice')).error, 'login_expired');
  assert.equal((await call('POST', '/v1/login/start', { port: 80, device: first, name: 'Mac' })).http, 400);
  const early = store.issueInvite();
  const pending = await login('alice');
  advance(10 * 60_000);
  assert.equal((await redeem(pending.ticket, early)).error, 'login_expired');
  advance(7 * 86_400_000);
  const later = await login('alice');
  assert.equal((await redeem(later.ticket, early)).error, 'invalid_invite');
  const verified = await redeem(later.ticket, store.issueInvite());
  advance(30 * 86_400_000);
  assert.equal((await call('GET', '/v1/me', undefined, verified.session)).http, 401);
});

test('a second SQLite connection and a restarted server both observe the redeemed invite', () => {
  const folder = mkdtempSync(join(tmpdir(), 'shifu-accounts-'));
  const file = join(folder, 'accounts.sqlite');
  const db = new AccountStore(file);
  const other = new AccountStore(file);
  try {
    const code = db.issueInvite();
    const user = { device: first, name: 'Mac' };
    const a = db.identified(id('alice'), user);
    const b = other.identified(id('bob'), user);
    assert.equal(other.redeem(b.ticket, code).status, 'signed_in');
    assert.equal(db.redeem(a.ticket, code).status, 'invalid_invite');
    const reopened = new AccountStore(file);
    try {
      assert.equal(reopened.identified(id('bob'), user).status, 'signed_in');
    } finally {
      reopened.db.close();
    }
  } finally {
    db.db.close();
    other.db.close();
    rmSync(folder, { recursive: true, force: true });
  }
});

test('Google verification binds the authorization code, signature, audience and nonce to this login', async (t) => {
  const { publicKey, privateKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const cert = publicKey.export({ format: 'pem', type: 'spki' });
  t.mock.method(OAuth2Client.prototype, 'getFederatedSignonCertsAsync', async () => ({
    certs: { fixture: cert }
  }));
  const now = Math.floor(Date.now() / 1000);
  const claims = {
    iss: 'https://accounts.google.com',
    aud: 'desktop-id',
    sub: 'stable-subject',
    email: 'alice@example.test',
    email_verified: true,
    nonce: 'bound-nonce',
    iat: now,
    exp: now + 600
  };
  const encode = (value) => Buffer.from(JSON.stringify(value)).toString('base64url');
  let signingKey = privateKey;
  t.mock.method(OAuth2Client.prototype, 'getToken', async (options) => {
    assert.deepEqual(options, {
      code: 'provided-code',
      codeVerifier: 'verifier',
      redirect_uri: 'http://127.0.0.1:45123/callback'
    });
    const content = encode({ alg: 'RS256', kid: 'fixture' }) + '.' + encode(claims);
    return {
      tokens: {
        id_token: content + '.' + sign('sha256', Buffer.from(content), signingKey).toString('base64url')
      }
    };
  });
  const google = googleIdentity('desktop-id', 'secret');
  const flow = { port: 45123, verifier: 'verifier', nonce: 'bound-nonce', state: 'state' };
  const url = new URL(google.authorization(flow));
  assert.equal(url.origin, 'https://accounts.google.com');
  assert.equal(url.searchParams.get('code_challenge_method'), 'S256');
  assert.equal(url.searchParams.get('code_challenge'), digest(flow.verifier));
  assert.equal(url.searchParams.get('nonce'), flow.nonce);
  assert.equal(url.searchParams.get('redirect_uri'), 'http://127.0.0.1:45123/callback');
  assert.deepEqual(await google.identify('provided-code', flow), {
    subject: 'stable-subject',
    email: claims.email
  });
  await assert.rejects(google.identify('provided-code', { ...flow, nonce: 'another-login' }));
  claims.aud = 'other-client';
  await assert.rejects(google.identify('provided-code', flow));
  claims.aud = 'desktop-id';
  signingKey = generateKeyPairSync('rsa', { modulusLength: 2048 }).privateKey;
  await assert.rejects(google.identify('provided-code', flow));
});

test('membership follows the verified Google subject even when email addresses change or match', () => {
  const store = new AccountStore(':memory:');
  try {
    const mac = { device: first, name: 'Studio' };
    const one = store.identified({ subject: 'member', email: 'shared@example.test' }, mac);
    store.redeem(one.ticket, store.issueInvite());
    const other = store.identified({ subject: 'different', email: 'shared@example.test' }, mac);
    assert.equal(other.status, 'invite_required');
    const returning = store.identified({ subject: 'member', email: 'changed@example.test' }, mac);
    assert.equal(returning.status, 'signed_in');
    assert.equal(store.authenticate(returning.session).email, 'changed@example.test');
  } finally {
    store.db.close();
  }
});
