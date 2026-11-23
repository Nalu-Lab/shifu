import { accountApi } from './api.mjs';
import { googleIdentity } from './google.mjs';
import { AccountStore } from './store.mjs';

process.umask(0o077);
const google = googleIdentity(process.env.SHIFU_GOOGLE_CLIENT_ID, process.env.SHIFU_GOOGLE_CLIENT_SECRET);
const store = new AccountStore(process.env.SHIFU_ACCOUNTS_DB || 'data/accounts.sqlite');
const app = await accountApi(store, google);
app.addHook('onClose', async () => store.db.close());
const address = await app.listen({ host: '127.0.0.1', port: Number(process.env.PORT || 3845) });
console.log('Shifu accounts listening at', address);
for (const signal of ['SIGTERM', 'SIGINT'])
  process.once(signal, () => {
    void app.close();
  });
