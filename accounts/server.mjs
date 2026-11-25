import { accountApi } from './api.mjs';
import { googleIdentity } from './google.mjs';
import { AccountStore } from './store.mjs';
import { accountWeb } from './web.mjs';

process.umask(0o077);
const google = googleIdentity(process.env.SHIFU_GOOGLE_CLIENT_ID, process.env.SHIFU_GOOGLE_CLIENT_SECRET);
const store = new AccountStore(process.env.SHIFU_ACCOUNTS_DB || 'data/accounts.sqlite');
const app = await accountApi(store, google);
// The website needs its own Google client of type Web application, with
// <SHIFU_ACCOUNTS_ORIGIN>/auth/callback as its redirect URI. Without it, only the desktop API runs.
const origin = process.env.SHIFU_ACCOUNTS_ORIGIN;
if (process.env.SHIFU_GOOGLE_WEB_CLIENT_ID && origin) {
  const web = googleIdentity(
    process.env.SHIFU_GOOGLE_WEB_CLIENT_ID,
    process.env.SHIFU_GOOGLE_WEB_CLIENT_SECRET,
    () => `${origin}/auth/callback`
  );
  await accountWeb(app, store, web, { origin });
}
app.addHook('onClose', async () => store.db.close());
const address = await app.listen({ host: '127.0.0.1', port: Number(process.env.PORT || 3845) });
console.log('Shifu accounts listening at', address);
for (const signal of ['SIGTERM', 'SIGINT'])
  process.once(signal, () => {
    void app.close();
  });
