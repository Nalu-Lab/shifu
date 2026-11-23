import { invoke } from "@tauri-apps/api/core";
import { present } from "./newpopover";
import "./remote.css";
import "./accounts.css";
import googleMark from "./assets/google-sign-in.png";

type View =
  | { status: "signed_out" | "invite_required" }
  | { status: "signed_in"; email: string; current: string; devices: { id: string; name: string }[] };
type Action = { kind: "refresh" | "cancel" | "sign_out" } | { kind: "sign_in"; name: string }
  | { kind: "redeem"; code: string } | { kind: "remove"; id: string };

const tr = (en: string, zh: string) => navigator.language.toLowerCase().startsWith("zh") ? zh : en;
const messages: Record<string, () => string> = {
  invalid_invite: () => tr("Check the invite code. It may have expired or already been used.", "请核对邀请码，它可能已过期或已使用。"),
  awaiting_approval: () => tr("Your invite was accepted. Shifu will work on this Mac once your account is approved.", "邀请码已通过，账号获批后即可在这台 Mac 上使用。"),
  login_expired: () => tr("Sign in with Google again to continue.", "请重新用 Google 登录。"),
  google_cancelled: () => tr("Google sign-in was cancelled.", "已取消 Google 登录。"),
  google_sign_in_failed: () => tr("Google could not confirm this account. Try signing in again.", "Google 未能确认此账号，请重试。"),
  keychain_failed: () => tr("Shifu needs access to its entry in the Mac Keychain.", "请允许 Shifu 访问它在 Mac 钥匙串中的条目。"),
  rate_limited: () => tr("Too many attempts. Try again in a minute.", "尝试次数过多，请稍后再试。"),
  invalid_service: () => tr("Check the Shifu account server configuration.", "请检查 Shifu 账号服务配置。"),
  invalid_device: () => tr("Enter a name for this Mac, up to 80 characters.", "请输入本机名称，最多 80 个字符。"),
  browser_failed: () => tr("Shifu could not open your web browser.", "无法打开系统浏览器。"),
  callback_unavailable: () => tr("Shifu could not receive the browser response. Try signing in again.", "未能收到浏览器回调，请重试。"),
  unknown_mac: () => tr("This Mac has already been removed. Refresh the list.", "该 Mac 已被移除，请刷新列表。"),
};

let outstanding: Promise<View> | undefined;

function perform(action: Action): Promise<View> {
  if (!outstanding) outstanding = invoke<View>("account_action", { action }).finally(() => { outstanding = undefined; });
  return outstanding;
}

export function showAccounts(anchor: HTMLElement): void {
  const pop = document.createElement("section");
  pop.className = "popup remote-pop account-pop";
  pop.setAttribute("role", "dialog");
  const title = tr("My Macs", "我的 Mac");
  pop.setAttribute("aria-label", title);
  const heading = Object.assign(document.createElement("strong"), { textContent: title });
  const body = document.createElement("div");
  const info = document.createElement("p");
  info.className = "remote-note";
  info.setAttribute("role", "status");
  const error = document.createElement("p");
  error.className = "remote-error";
  error.setAttribute("role", "alert");
  error.hidden = true;
  pop.append(heading, body, info, error);
  if (!present(anchor, "accounts", pop, { suspended: true })) return;

  function button(text: string, click: () => void): HTMLButtonElement {
    const element = Object.assign(document.createElement("button"), { type: "button", className: "btn", textContent: text });
    element.addEventListener("click", click);
    return element;
  }
  function note(text: string): HTMLElement {
    return Object.assign(document.createElement("p"), { className: "remote-state", textContent: text });
  }
  function form(label: string, submit: string, max: number, send: (value: string) => void): HTMLButtonElement {
    const element = document.createElement("form");
    element.className = "remote-form";
    const field = document.createElement("label");
    const input = Object.assign(document.createElement("input"), {
      required: true, maxLength: max, spellcheck: false, autocomplete: "off",
    });
    field.append(label, input);
    const go = Object.assign(document.createElement("button"), { type: "submit", className: "btn", textContent: submit });
    element.addEventListener("submit", event => {
      event.preventDefault();
      if (input.value.trim()) send(input.value.trim());
      else input.focus();
    });
    element.append(field, go);
    body.append(element);
    return go;
  }
  function paint(view: View): void {
    body.replaceChildren();
    if (view.status === "signed_out") {
      body.append(note(tr("First time here? Have your invite code ready.", "首次登录需要邀请码。")));
      const google = form(tr("Name for this Mac", "本机名称"), tr("Sign in with Google", "使用 Google 登录"), 80,
        name => void update({ kind: "sign_in", name }));
      google.className = "account-google";
      google.prepend(Object.assign(document.createElement("img"), { src: googleMark, alt: "", width: 20, height: 20 }));
    } else if (view.status === "invite_required") {
      body.append(note(tr("Enter your invite to join the Shifu pilot.", "输入邀请码以加入 Shifu 内测。")));
      form(tr("Invite code", "邀请码"), tr("Redeem invite", "使用邀请码"), 80,
        code => void update({ kind: "redeem", code }));
      body.append(button(tr("Use another Google account", "使用其他 Google 账号"), () => void update({ kind: "cancel" })));
    } else if (view.status === "signed_in") {
      body.append(note(view.email));
      for (const mac of view.devices) {
        const row = document.createElement("div");
        row.className = "remote-device";
        const name = document.createElement("span");
        name.textContent = mac.id === view.current ? tr(`${mac.name} (this Mac)`, `${mac.name}（本机）`) : mac.name;
        const remove = button(tr("Remove", "移除"), () => {
          row.replaceChildren(note(tr(`Sign ${mac.name} out and remove it?`, `退出并移除 ${mac.name}？`)),
            button(tr("Remove", "移除"), () => void update({ kind: "remove", id: mac.id })),
            button(tr("Cancel", "取消"), () => paint(view)));
        });
        remove.setAttribute("aria-label", tr(`Remove ${mac.name}`, `移除 ${mac.name}`));
        row.append(name, remove);
        body.append(row);
      }
      body.append(button(tr("Refresh", "刷新"), () => void update({ kind: "refresh" })),
        button(tr("Sign out", "退出登录"), () => void update({ kind: "sign_out" })));
    }
  }
  async function update(action: Action): Promise<void> {
    error.hidden = true;
    info.textContent = action.kind === "sign_in" ? tr("Continue in your browser…", "请在浏览器中继续…") : tr("Loading…", "正在读取…");
    body.inert = true;
    try {
      const next = await perform(action);
      if (pop.isConnected) paint(next);
    } catch (cause) {
      if (!pop.isConnected) return;
      const code = String(cause);
      if (code === "login_expired" || code === "signed_out") paint({ status: "signed_out" });
      if (!body.childElementCount) body.append(button(tr("Try again", "重试"), () => void update({ kind: "refresh" })));
      error.textContent = messages[code]?.() ?? tr("Could not reach the account service. Try again.", "无法访问账号服务，请重试。");
      error.hidden = false;
    } finally {
      info.textContent = "";
      body.inert = false;
    }
  }
  void update({ kind: "refresh" });
}
