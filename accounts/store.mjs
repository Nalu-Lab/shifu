import { createHash, randomBytes } from 'node:crypto';
import { mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { DatabaseSync } from 'node:sqlite';

export const digest = (value) => createHash('sha256').update(value).digest('base64url');
export const secret = () => randomBytes(32).toString('base64url');
const minute = 60_000;
const day = 24 * 60 * minute;

export class AccountStore {
  constructor(path, now = Date.now) {
    this.now = now;
    if (path !== ':memory:') mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
    this.db = new DatabaseSync(path);
    this.db.exec(`
      PRAGMA journal_mode = WAL;
      PRAGMA foreign_keys = ON;
      PRAGMA busy_timeout = 5000;
      CREATE TABLE IF NOT EXISTS accounts (subject TEXT PRIMARY KEY, email TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS invites (
        hash TEXT PRIMARY KEY, expires INTEGER NOT NULL, claimed_by TEXT UNIQUE REFERENCES accounts(subject)
      );
      CREATE TABLE IF NOT EXISTS macs (
        owner TEXT NOT NULL REFERENCES accounts(subject), id TEXT NOT NULL, name TEXT NOT NULL,
        session_hash TEXT UNIQUE, expires INTEGER NOT NULL, PRIMARY KEY (owner, id)
      );
      CREATE TABLE IF NOT EXISTS flows (
        hash TEXT PRIMARY KEY, verifier TEXT NOT NULL, nonce TEXT NOT NULL, port INTEGER NOT NULL,
        device TEXT NOT NULL, name TEXT NOT NULL, expires INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS admissions (
        hash TEXT PRIMARY KEY, subject TEXT NOT NULL, email TEXT NOT NULL, device TEXT NOT NULL,
        name TEXT NOT NULL, attempts INTEGER NOT NULL, expires INTEGER NOT NULL
      );
      CREATE INDEX IF NOT EXISTS flows_expiry ON flows(expires);
      CREATE INDEX IF NOT EXISTS admissions_expiry ON admissions(expires);
    `);
  }

  transaction(work) {
    this.db.exec('BEGIN IMMEDIATE');
    try {
      const result = work();
      this.db.exec('COMMIT');
      return result;
    } catch (error) {
      this.db.exec('ROLLBACK');
      throw error;
    }
  }

  issueInvite() {
    const code = randomBytes(10).toString('hex').toUpperCase();
    this.db
      .prepare('INSERT INTO invites(hash, expires) VALUES (?, ?)')
      .run(digest(code), this.now() + 7 * day);
    return code.match(/.{4}/g).join('-');
  }

  start(port, device, name) {
    const flow = secret();
    const verifier = secret();
    const nonce = secret();
    const now = this.now();
    this.db.prepare('DELETE FROM flows WHERE expires <= ?').run(now);
    this.db.prepare('DELETE FROM admissions WHERE expires <= ? OR attempts <= 0').run(now);
    this.db
      .prepare('INSERT INTO flows VALUES (?, ?, ?, ?, ?, ?, ?)')
      .run(digest(flow), verifier, nonce, port, device, name, now + 5 * minute);
    return { flow, verifier, nonce };
  }

  takeFlow(flow) {
    return this.db
      .prepare('DELETE FROM flows WHERE hash = ? AND expires > ? RETURNING *')
      .get(digest(flow), this.now());
  }

  // Called only after verifying Google's ID token. A pending admission grants no account access.
  identified({ subject, email }, { device, name }) {
    return this.transaction(() => {
      if (this.db.prepare('SELECT 1 FROM accounts WHERE subject = ?').get(subject)) {
        return this.signIn({ subject, email, device, name });
      }
      const ticket = secret();
      this.db
        .prepare('INSERT INTO admissions VALUES (?, ?, ?, ?, ?, 5, ?)')
        .run(digest(ticket), subject, email, device, name, this.now() + 10 * minute);
      return { status: 'invite_required', ticket };
    });
  }

  redeem(ticket, code) {
    const now = this.now();
    const hash = digest(ticket);
    return this.transaction(() => {
      const user = this.db
        .prepare('SELECT * FROM admissions WHERE hash = ? AND expires > ? AND attempts > 0')
        .get(hash, now);
      if (!user) return { status: 'login_expired' };
      // The same Google account may already have joined from another Mac while this prompt was open.
      if (!this.db.prepare('SELECT 1 FROM accounts WHERE subject = ?').get(user.subject)) {
        const codeHash = digest(code.toUpperCase().replace(/[\s-]/g, ''));
        const invite = this.db
          .prepare('SELECT 1 FROM invites WHERE hash = ? AND expires > ? AND claimed_by IS NULL')
          .get(codeHash, now);
        if (!invite) {
          this.db.prepare('UPDATE admissions SET attempts = attempts - 1 WHERE hash = ?').run(hash);
          return { status: user.attempts === 1 ? 'login_expired' : 'invalid_invite' };
        }
        this.db.prepare('INSERT INTO accounts VALUES (?, ?)').run(user.subject, user.email);
        this.db.prepare('UPDATE invites SET claimed_by = ? WHERE hash = ?').run(user.subject, codeHash);
      }
      this.db.prepare('DELETE FROM admissions WHERE hash = ?').run(hash);
      return this.signIn(user);
    });
  }

  signIn({ subject, email, device, name }) {
    const session = secret();
    this.db.prepare('UPDATE accounts SET email = ? WHERE subject = ?').run(email, subject);
    this.db
      .prepare(
        `INSERT INTO macs(owner, id, name, session_hash, expires) VALUES (?, ?, ?, ?, ?)
      ON CONFLICT(owner, id) DO UPDATE SET name=excluded.name, session_hash=excluded.session_hash, expires=excluded.expires`
      )
      .run(subject, device, name, digest(session), this.now() + 30 * day);
    return { status: 'signed_in', session };
  }

  authenticate(session) {
    return this.db
      .prepare(
        `SELECT owner, id, email FROM macs JOIN accounts ON accounts.subject = macs.owner
      WHERE session_hash = ? AND expires > ?`
      )
      .get(digest(session), this.now());
  }

  directory({ owner, id, email }) {
    const devices = this.db
      .prepare('SELECT id, name FROM macs WHERE owner = ? ORDER BY name COLLATE NOCASE, id')
      .all(owner);
    return { email, current: id, devices };
  }

  remove({ owner }, id) {
    return this.db.prepare('DELETE FROM macs WHERE owner = ? AND id = ?').run(owner, id).changes === 1;
  }

  signOut({ owner, id }, session) {
    this.db
      .prepare(
        'UPDATE macs SET session_hash = NULL, expires = 0 WHERE owner = ? AND id = ? AND session_hash = ?'
      )
      .run(owner, id, digest(session));
  }
}
