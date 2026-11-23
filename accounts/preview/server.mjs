// Local rehearsal only. The production entry point never imports this module.
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { accountApi } from '../api.mjs';
import { AccountStore } from '../store.mjs';

const app = fileURLToPath(new URL('../../app/', import.meta.url));
const requireApp = createRequire(new URL('../../app/package.json', import.meta.url));
const { createServer } = await import(pathToFileURL(requireApp.resolve('vite')).href);
const store = new AccountStore(':memory:');
const users = new Map([
  ['alex', 'alex@example.test'],
  ['sam', 'sam@example.test']
]);
const service = await accountApi(store, {
  authorization: () => 'about:blank',
  async identify(code) {
    if (!users.has(code)) throw new Error('Choose a sample identity.');
    return { subject: 'local-preview/' + code, email: users.get(code) };
  }
});
const api = await service.listen({ host: '127.0.0.1', port: 0 });
const vite = await createServer({
  configFile: false,
  root: fileURLToPath(new URL('./', import.meta.url)),
  cacheDir: fileURLToPath(new URL('../node_modules/.vite-preview', import.meta.url)),
  resolve: { alias: { '@desktop': app } },
  server: {
    host: '127.0.0.1',
    port: 0,
    cors: false,
    fs: { allow: [app, fileURLToPath(new URL('./', import.meta.url))] },
    proxy: { '/v1': api }
  }
});
try {
  await vite.listen();
} catch (error) {
  await service.close();
  throw error;
}
console.log('Local Shifu pilot:', 'http://127.0.0.1:' + vite.httpServer.address().port);
for (const user of users.keys()) console.log('Single-use sample invite for', user + ':', store.issueInvite());
console.log('Sample accounts and invites last until you stop this preview.');
for (const signal of ['SIGTERM', 'SIGINT']) {
  process.once(signal, async () => {
    await vite.close();
    await service.close();
    store.db.close();
  });
}
