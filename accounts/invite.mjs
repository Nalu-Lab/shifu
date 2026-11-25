import { AccountStore } from './store.mjs';

process.umask(0o077);
const store = new AccountStore(process.env.SHIFU_ACCOUNTS_DB || 'data/accounts.sqlite');
try {
  console.log(store.issueInvite());
} finally {
  store.db.close();
}
