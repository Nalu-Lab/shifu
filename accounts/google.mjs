import { OAuth2Client } from 'google-auth-library';
import { digest } from './store.mjs';

const loopback = (port) => `http://127.0.0.1:${port}/callback`;

// The desktop client returns to a loopback port; the web client passes a fixed HTTPS callback.
export function googleIdentity(clientId, clientSecret, redirect = loopback) {
  if (!clientId || !clientSecret) throw new Error('Configure the Google Desktop OAuth client ID and secret.');
  const client = new OAuth2Client({ clientId, clientSecret, transporterOptions: { timeout: 12_000 } });
  return {
    authorization({ port, verifier, nonce, state }) {
      return client.generateAuthUrl({
        redirect_uri: redirect(port),
        scope: ['openid', 'email'],
        state,
        nonce,
        code_challenge_method: 'S256',
        code_challenge: digest(verifier),
        prompt: 'select_account'
      });
    },
    async identify(code, { port, verifier, nonce }) {
      const { tokens } = await client.getToken({
        code,
        redirect_uri: redirect(port),
        codeVerifier: verifier
      });
      const ticket = await client.verifyIdToken({ idToken: tokens.id_token, audience: clientId });
      const claims = ticket.getPayload();
      if (!claims?.sub || claims.nonce !== nonce || !claims.email || claims.email_verified !== true) {
        throw new Error('Invalid Google identity');
      }
      return { subject: claims.sub, email: claims.email };
    }
  };
}
