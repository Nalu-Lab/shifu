import '@desktop/src/style.css';
import { showAccounts } from '@desktop/src/accounts.ts';
import { closeNewPopover } from '@desktop/src/newpopover.ts';

const machine = document.querySelector('#mac');
const user = document.querySelector('#user');
const open = document.querySelector('#open');
const storageKey = () => 'shifu-account-preview.' + machine.value;
async function api(method, path, body, session) {
  const headers = {};
  if (body) headers['Content-Type'] = 'application/json';
  if (session) headers.Authorization = 'Bearer ' + session;
  const reply = await fetch(path, { method, headers, body: body && JSON.stringify(body) });
  const data = await reply.json();
  if (!reply.ok) throw data.error;
  return data;
}

async function desktop(action) {
  const key = storageKey();
  const device = machine.value.repeat(32);
  const member = user.value;
  let { session, ticket } = JSON.parse(sessionStorage.getItem(key) || '{}');
  if (action.kind === 'sign_in') {
    const { flow } = await api('POST', '/v1/login/start', { port: 45000, device, name: action.name });
    ({ session, ticket } = await api('POST', '/v1/login/finish', { flow, code: member }));
  } else if (action.kind === 'redeem') {
    try {
      ({ session } = await api('POST', '/v1/login/invite', { ticket, code: action.code }));
      ticket = undefined;
    } catch (error) {
      if (error === 'login_expired') sessionStorage.removeItem(key);
      throw error;
    }
  } else if (action.kind === 'cancel') {
    ticket = undefined;
  } else if (action.kind === 'remove') {
    await api('DELETE', '/v1/macs/' + action.id, undefined, session);
  } else if (action.kind === 'sign_out') {
    if (session) await api('POST', '/v1/logout', undefined, session);
    session = undefined;
    ticket = undefined;
  }
  sessionStorage.setItem(key, JSON.stringify({ session, ticket }));
  if (ticket) return { status: 'invite_required' };
  if (!session) return { status: 'signed_out' };
  try {
    return { status: 'signed_in', ...(await api('GET', '/v1/me', undefined, session)) };
  } catch (error) {
    if (error !== 'signed_out') throw error;
    sessionStorage.removeItem(key);
    return { status: 'signed_out' };
  }
}

window.__TAURI_INTERNALS__ = {
  invoke(command, { action }) {
    if (command !== 'account_action') throw new Error('Unsupported preview action');
    return desktop(action);
  }
};
open.addEventListener('click', () => showAccounts(open));
machine.addEventListener('change', closeNewPopover);
user.addEventListener('change', closeNewPopover);
