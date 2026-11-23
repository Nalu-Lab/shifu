// Website sign-up: Google sign-in, then a one-time invite for new accounts.
// The site keeps no login. Each visit and each Mac signs in with Google again.
import { readFileSync } from 'node:fs';

const DOWNLOAD = 'https://github.com/Nalu-Lab/shifu/releases';
const FLOW = 'shifu_flow';
const TICKET = 'shifu_ticket';
const handle = (value) => typeof value === 'string' && /^[A-Za-z0-9_-]{43}$/.test(value);
const googleButton = readFileSync(new URL('./web/google-sign-in.png', import.meta.url));

const escape = (s) =>
  String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

function cookies(request) {
  const out = {};
  for (const part of (request.headers.cookie ?? '').split(';')) {
    const at = part.indexOf('=');
    if (at > 0) out[part.slice(0, at).trim()] = part.slice(at + 1).trim();
  }
  return out;
}

const setCookie = (name, value, seconds) =>
  `${name}=${value}; Path=/; Max-Age=${seconds}; HttpOnly; Secure; SameSite=Lax`;

function page(title, body) {
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>${escape(title)} · Shifu</title>
<style>
:root{color-scheme:light dark;--fg:#1d1d1f;--muted:#6e6e73;--bg:#f5f5f7;--card:#fff;--line:#d2d2d7;--accent:#0b57d0}
@media (prefers-color-scheme:dark){:root{--fg:#f5f5f7;--muted:#a1a1a6;--bg:#111;--card:#1c1c1e;--line:#3a3a3c;--accent:#7cacf8}}
*{box-sizing:border-box}body{margin:0;min-height:100vh;display:grid;place-items:center;background:var(--bg);color:var(--fg);font:16px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;padding:24px}
main{width:100%;max-width:420px;background:var(--card);border:1px solid var(--line);border-radius:16px;padding:32px}
h1{font-size:22px;margin:0 0 8px}p{margin:0 0 16px;color:var(--muted)}p strong{color:var(--fg)}
.brand{font-weight:600;letter-spacing:.02em;color:var(--muted);font-size:13px;text-transform:uppercase;margin-bottom:20px}
a.google{display:inline-flex;align-items:center;gap:10px;padding:10px 16px;border:1px solid var(--line);border-radius:999px;color:var(--fg);text-decoration:none;font-weight:500}
a.google img{width:20px;height:20px}a.google:focus-visible,button:focus-visible,input:focus-visible,a:focus-visible{outline:3px solid var(--accent);outline-offset:2px}
label{display:block;font-weight:500;margin-bottom:6px}
input{width:100%;font:inherit;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;letter-spacing:.05em;padding:10px 12px;border:1px solid var(--line);border-radius:10px;background:transparent;color:var(--fg);text-transform:uppercase}
button,a.button{display:inline-block;margin-top:16px;font:inherit;font-weight:500;padding:10px 18px;border:0;border-radius:10px;background:var(--accent);color:#fff;text-decoration:none;cursor:pointer}
.error{color:#c5221f}@media (prefers-color-scheme:dark){.error{color:#f28b82}}
ol{padding-left:20px;color:var(--muted)}ol li{margin-bottom:6px}
</style></head><body><main><div class="brand">Shifu · private beta</div>${body}</main></body></html>`;
}

const signInButton = `<a class="google" href="/auth/google"><img src="/google-sign-in.png" alt="">Sign in with Google</a>`;

const joined = (email, fresh) =>
  page(
    "You're in",
    `<h1>${fresh ? "You're in" : 'Welcome back'}</h1>
<p><strong>${escape(email)}</strong> ${fresh ? 'has joined' : 'is already a member of'} the Shifu beta.</p>
<ol><li>Download and install Shifu.</li><li>Open <strong>Shifu → My Macs…</strong> and sign in with this Google account again. You won't need the code again.</li></ol>
<a class="button" href="${DOWNLOAD}">Download Shifu</a>`
  );

function invitePage(email, error) {
  return page(
    'Enter your invite',
    `<h1>Enter your invite</h1>
<p>Signed in as <strong>${escape(email)}</strong>. New accounts need a one-time invite code.</p>
<form method="post" action="/join">
<label for="code">Invite code</label>
<input id="code" name="code" autocomplete="off" spellcheck="false" maxlength="80" required autofocus placeholder="XXXX-XXXX-XXXX-XXXX-XXXX"${error ? ' aria-describedby="err" aria-invalid="true"' : ''}>
${error ? `<p class="error" id="err" role="alert">${escape(error)}</p>` : ''}
<button type="submit">Join</button>
</form>`
  );
}

const startOver = (message) =>
  page('Sign in', `<h1>Sign in again</h1><p>${escape(message)}</p>${signInButton}`);

export async function accountWeb(app, store, google, { origin }) {
  app.addContentTypeParser('application/x-www-form-urlencoded', { parseAs: 'string', bodyLimit: 1024 }, (_req, body, done) =>
    done(null, Object.fromEntries(new URLSearchParams(body)))
  );
  const limit = { config: { rateLimit: { max: 25, timeWindow: '1 minute' } } };
  const html = (reply, body, status = 200) =>
    reply
      .code(status)
      .header('Content-Type', 'text/html; charset=utf-8')
      .header(
        'Content-Security-Policy',
        "default-src 'none'; style-src 'unsafe-inline'; img-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'"
      )
      // Not no-referrer: Chrome then sends Origin: null on form posts and the /join check would refuse them.
      .header('Referrer-Policy', 'same-origin')
      .header('X-Content-Type-Options', 'nosniff')
      .send(body);

  app.get('/', (_request, reply) =>
    html(
      reply,
      page(
        'Join the beta',
        `<h1>Join the Shifu beta</h1><p>Shifu is invite-only for now. Sign in with Google, then enter the invite code you were given.</p>${signInButton}`
      )
    )
  );
  app.get('/google-sign-in.png', (_request, reply) =>
    reply.header('Content-Type', 'image/png').header('Cache-Control', 'public, max-age=86400').send(googleButton)
  );

  app.get('/auth/google', limit, (_request, reply) => {
    const { flow, verifier, nonce } = store.startWeb();
    // The flow handle doubles as OAuth state; the cookie binds the callback to this browser.
    const url = google.authorization({ verifier, nonce, state: flow });
    return reply.header('Set-Cookie', setCookie(FLOW, flow, 600)).redirect(url, 303);
  });

  app.get('/auth/callback', limit, async (request, reply) => {
    const { code, state, error } = request.query ?? {};
    const flow = cookies(request)[FLOW];
    reply.header('Set-Cookie', setCookie(FLOW, '', 0));
    if (error) return html(reply, startOver('Google sign-in was cancelled.'), 400);
    if (!handle(flow) || state !== flow || typeof code !== 'string' || !code || code.length > 2048)
      return html(reply, startOver('This sign-in link has expired or was opened in another browser.'), 400);
    const entry = store.takeFlow(flow);
    if (!entry || entry.port !== 0) return html(reply, startOver('This sign-in took too long.'), 400);
    let identity;
    try {
      identity = await google.identify(code, entry);
    } catch {
      return html(reply, startOver('Google sign-in could not be verified.'), 401);
    }
    const result = store.identifiedWeb(identity);
    if (result.status === 'member') return html(reply, joined(result.email, false));
    reply.header('Set-Cookie', [setCookie(FLOW, '', 0), setCookie(TICKET, result.ticket, 600)]);
    return reply.redirect('/join', 303);
  });

  app.get('/join', (request, reply) => {
    const ticket = cookies(request)[TICKET];
    const pending = handle(ticket) && store.pendingEmail(ticket);
    if (!pending) return html(reply, startOver('Sign in with Google to continue.'), 401);
    return html(reply, invitePage(pending));
  });

  app.post('/join', limit, (request, reply) => {
    if (request.headers.origin !== origin) return html(reply, startOver('Please try again from this page.'), 403);
    const ticket = cookies(request)[TICKET];
    const code = request.body?.code;
    if (!handle(ticket) || typeof code !== 'string' || code.length > 80)
      return html(reply, startOver('Sign in with Google to continue.'), 400);
    const email = store.pendingEmail(ticket);
    const result = store.redeem(ticket, code.trim(), { web: true });
    if (result.status === 'invalid_invite')
      return html(reply, invitePage(email, 'That code is not valid, has expired, or was already used.'), 403);
    reply.header('Set-Cookie', setCookie(TICKET, '', 0));
    if (result.status !== 'member') return html(reply, startOver('Too many tries, or this page expired.'), 401);
    return html(reply, joined(result.email, true));
  });
}
