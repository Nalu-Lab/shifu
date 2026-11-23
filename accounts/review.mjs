// Review new accounts: `review` lists who is waiting; `review approve <email>` or `review reject <email>`.
import { AccountStore } from './store.mjs';

process.umask(0o077);
const store = new AccountStore(process.env.SHIFU_ACCOUNTS_DB || 'data/accounts.sqlite');
const [action, email] = process.argv.slice(2);
try {
  if (!action) {
    const waiting = store.pending();
    console.log(waiting.length ? waiting.join('\n') : 'No one is waiting.');
  } else if ((action === 'approve' || action === 'reject') && email) {
    if (!store[action](email)) {
      console.error(`No single waiting account for ${email}: none, or more than one has this email.`);
      process.exitCode = 1;
    } else console.log(`${action === 'approve' ? 'Approved' : 'Rejected'} ${email}.`);
  } else {
    console.error('Usage: review [approve|reject <email>]');
    process.exitCode = 2;
  }
} finally {
  store.db.close();
}
