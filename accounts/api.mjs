import Fastify from 'fastify';
import rateLimit from '@fastify/rate-limit';
import { secret } from './store.mjs';

const handle = (value) => typeof value === 'string' && /^[A-Za-z0-9_-]{43}$/.test(value);
const deviceId = (value) => typeof value === 'string' && /^[a-f0-9]{32}$/.test(value);
const fail = (reply, status, error) => reply.code(status).send({ error });

export async function accountApi(store, google) {
  const app = Fastify({ bodyLimit: 8192, trustProxy: 'loopback' });
  await app.register(rateLimit, { max: 120, timeWindow: '1 minute' });
  app.addHook('onSend', async (_request, reply) => {
    reply.header('Cache-Control', 'no-store');
  });
  app.setErrorHandler((error, request, reply) => {
    if (error.statusCode === 429) return fail(reply, 429, 'rate_limited');
    if (error.statusCode >= 400 && error.statusCode < 500)
      return fail(reply, error.statusCode, 'invalid_request');
    request.log.error({ code: error.code }, 'Account request failed');
    return fail(reply, 503, 'service_unavailable');
  });
  const loginLimit = { config: { rateLimit: { max: 25, timeWindow: '1 minute' } } };

  app.post('/v1/login/start', loginLimit, (request, reply) => {
    const { port, device, name } = request.body ?? {};
    if (
      !Number.isInteger(port) ||
      port < 1024 ||
      port > 65535 ||
      !deviceId(device) ||
      typeof name !== 'string' ||
      !name.trim() ||
      name.trim().length > 80 ||
      /[\u0000-\u001f\u007f]/u.test(name)
    ) {
      return fail(reply, 400, 'invalid_device');
    }
    const state = secret();
    const { flow, verifier, nonce } = store.start(port, device, name.trim());
    return { flow, state, url: google.authorization({ port, verifier, nonce, state }) };
  });

  app.post('/v1/login/finish', loginLimit, async (request, reply) => {
    const { flow, code } = request.body ?? {};
    if (!handle(flow) || typeof code !== 'string' || !code || code.length > 2048)
      return fail(reply, 400, 'invalid_login');
    const entry = store.takeFlow(flow);
    if (!entry) return fail(reply, 401, 'login_expired');
    let identity;
    try {
      identity = await google.identify(code, entry);
    } catch {
      return fail(reply, 401, 'google_sign_in_failed');
    }
    return store.identified(identity, entry);
  });

  app.post('/v1/login/invite', loginLimit, (request, reply) => {
    const { ticket, code } = request.body ?? {};
    if (!handle(ticket) || typeof code !== 'string' || code.length > 80)
      return fail(reply, 400, 'invalid_request');
    const result = store.redeem(ticket, code.trim());
    if (result.status === 'login_expired') return fail(reply, 401, result.status);
    if (result.status === 'invalid_invite') return fail(reply, 403, result.status);
    return result;
  });

  await app.register(async (routes) => {
    routes.decorateRequest('member', null);
    routes.addHook('preHandler', async (request, reply) => {
      const session = /^Bearer ([A-Za-z0-9_-]{43})$/.exec(request.headers.authorization ?? '')?.[1];
      request.member = session && store.authenticate(session);
      if (!request.member) return fail(reply, 401, 'signed_out');
    });
    routes.get('/v1/me', (request) => store.directory(request.member));
    routes.delete('/v1/macs/:id', (request, reply) => {
      if (!deviceId(request.params.id) || !store.remove(request.member, request.params.id))
        return fail(reply, 404, 'unknown_mac');
      return { ok: true };
    });
    routes.post('/v1/logout', (request, reply) => {
      store.signOut(request.member, request.headers.authorization.slice(7));
      return reply.send({ ok: true });
    });
  });
  return app;
}
