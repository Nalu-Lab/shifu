/** 准备情况: what a new Mac still needs (docs/ONBOARDING.md, 第 5 阶段), one row each with one action: tmux (shipped
 *  inside Shifu.app, so normally just a tick), the agent CLIs found on the PATH (`clis_detect`, file checks only)
 *  with Install or Sign in, browser use, Computer Use and notifications. Install, Sign in and browser setup type
 *  their command into a visible Shifu terminal tab (setup.rs), so the person sees it run and can answer it; the
 *  checklist steps aside meanwhile and comes back by itself once the detection sees the result. Nothing runs
 *  without a click. Shown on the empty page and in the first-run guide until ready, and from the app menu any
 *  time. */
import { type CliFound, type CliReport, type SetupExtras, clipboardWrite, pathOpen, setupExtras, setupOpen } from "./api";
import { getClis, onClisChange, refreshClis, splitAgents } from "./models";
import { missingLine } from "./newpopover";
import { reportError } from "./store";
import { tr } from "./i18n";

export type CheckState = "ok" | "missing" | "note" | "info";

/** One click: install or sign in to agent `kind`, set up browser use, set up Computer Use or allow it once set up
 *  (both install the helper Shifu carries if needed and open its permission window), open notification settings. */
export type SetupDo = "install" | "login" | "browser" | "computer" | "allow" | "notifications";
export interface SetupAction {
  do: SetupDo;
  kind: string;
  name: string;
}

export interface CheckItem {
  label: string;
  state: CheckState;
  detail: string;
  /** The button for this row. */
  action?: SetupAction;
  /** A command to copy, only where Shifu cannot run it (tmux in a build that does not carry it). */
  command?: string;
  url?: string;
}

const TMUX_URL = "https://github.com/tmux/tmux/wiki/Installing";
const NODE_URL = "https://nodejs.org/en/download";

const agentsOf = (report: CliReport): CliFound[] => report.agents.filter((a) => a.kind !== "terminal");

/** Ready for a first agent: tmux and at least one agent CLI. Sign-in does not count: each agent offers it when it
 *  starts. Before the first detection answers nothing is known, so nothing is flagged. */
export function setupReady(report: CliReport | null): boolean {
  return report === null || (report.tmux !== null && agentsOf(report).some((a) => a.path !== null));
}

/** Its one-line installer needs npm, which comes with Node. */
const needsNode = (agent: CliFound): boolean => /^\s*(npm|npx)\s/.test(agent.install_command ?? "");

/** Shifu can run its one-line installer here: it has one, and npm is there when it needs npm. */
const canInstall = (report: CliReport, agent: CliFound): boolean =>
  Boolean(agent.install_command) && !(needsNode(agent) && !report.npm);

/** A row for an agent that is not installed: Install when Shifu can run its installer here. */
export function missingItem(report: CliReport, agent: CliFound): CheckItem {
  const base = { label: agent.name, state: "missing" as const, detail: tr("没装", "Not installed"), url: agent.install_url || undefined };
  return canInstall(report, agent) ? { ...base, action: { do: "install", kind: agent.kind, name: agent.name } } : base;
}

function agentItem(report: CliReport, agent: CliFound): CheckItem {
  if (agent.path === null) return missingItem(report, agent);
  if (agent.signed_in === false) {
    return { label: agent.name, state: "note", detail: tr("需登录", "Sign-in needed"), action: { do: "login", kind: agent.kind, name: agent.name } };
  }
  return { label: agent.name, state: "ok", detail: agent.signed_in ? tr("已装，已登录", "Installed, signed in") : tr("已装", "Installed") };
}

/** The agents not installed that are offered (extra plain agents and the user's own only once found). */
const missingAgents = (report: CliReport): CliFound[] => agentsOf(report).filter((a) => a.path === null && !a.extra);

/** Sign In rows for the installed agents whose sign-in is known to be missing. */
export const signInItems = (report: CliReport): CheckItem[] =>
  agentsOf(report).filter((a) => a.path !== null && a.signed_in === false).map((a) => agentItem(report, a));

/** Install rows for the agents Shifu can install right now, then, without Node, one row for Node.js naming how many
 *  agents wait for it. The agents with only an install page are `pageOnly`. */
export function installItems(report: CliReport): CheckItem[] {
  const missing = missingAgents(report);
  const items = missing.filter((a) => canInstall(report, a)).map((a) => missingItem(report, a));
  const waiting = missing.filter((a) => a.install_command && !canInstall(report, a));
  if (waiting.length > 0) {
    const first = waiting[0].name;
    const detail = waiting.length === 1
      ? tr(`${first} 需要它`, `${first} needs it`)
      : tr(`${first} 等 ${waiting.length} 个 Agent 需要它`, `Needed by ${first} and ${waiting.length - 1} more`);
    items.push({ label: "Node.js", state: "note", detail, url: NODE_URL });
  }
  return items;
}

/** The agents not installed that have only an install page, for a line of links. */
export const pageOnly = (report: CliReport): CliFound[] => missingAgents(report).filter((a) => !a.install_command);

/** "Not installed: Kimi · Antigravity": each opens its install page; null when there is none. */
export const pageLinks = (report: CliReport): HTMLElement | null => missingLine(pageOnly(report).map((a) => ({ kind: a.kind, name: a.name })));

function tmuxItem(report: CliReport): CheckItem {
  if (report.tmux) return { label: "tmux", state: "ok", detail: report.tmux_bundled ? tr("内置", "Built in") : tr("已装", "Installed") };
  return { label: "tmux", state: "missing", detail: tr("必需。在终端里运行：", "Required. Run in Terminal:"), command: "brew install tmux", url: TMUX_URL };
}

/** Browser use, Computer Use and notifications; `null` extras while they are being checked. */
export function extraItems(report: CliReport, extras: SetupExtras | null): CheckItem[] {
  const browser: CheckItem = { label: tr("浏览器", "Browser use"), state: "info", detail: tr("正在检查…", "Checking…") };
  const computer: CheckItem = { label: tr("电脑操作", "Computer use"), state: "info", detail: tr("正在检查…", "Checking…") };
  if (extras) {
    if (extras.browser) Object.assign(browser, { state: "ok", detail: tr("可用", "Ready") });
    else if (report.npm) Object.assign(browser, { state: "note", detail: tr("未设置", "Not set up"), action: { do: "browser", kind: "", name: "" } });
    else Object.assign(browser, { state: "note", detail: tr("需要 Node.js", "Needs Node.js"), url: NODE_URL });
    if (extras.computer === "ready") Object.assign(computer, { state: "ok", detail: tr("可用", "Ready") });
    else if (extras.computer === "permissions") Object.assign(computer, { state: "note", detail: tr("需授权", "Needs permission"), action: { do: "allow", kind: "", name: "" } });
    else if (extras.computer === "install") Object.assign(computer, { state: "note", detail: tr("未设置", "Not set up"), action: { do: "computer", kind: "", name: "" } });
  }
  const notifications: CheckItem = {
    label: tr("通知", "Notifications"),
    state: "info",
    detail: tr("后台时提醒", "When in background"),
    action: { do: "notifications", kind: "", name: "" },
  };
  // A build without the helper (a local one) has no Computer Use row.
  return extras?.computer === "missing" ? [browser, notifications] : [browser, computer, notifications];
}

/** The rows: tmux, the installed agents (with Sign in where it can be told) or, when none is, the agents to
 *  install from; then browser use, Computer Use and notifications. When some agent is installed the others are only
 *  a line of links (`missingLine`), since one agent is enough. */
export function setupChecks(report: CliReport, extras: SetupExtras | null = null): CheckItem[] {
  const items: CheckItem[] = [tmuxItem(report)];
  const installed = agentsOf(report).filter((a) => a.path !== null);
  if (installed.length > 0) items.push(...installed.map((a) => agentItem(report, a)));
  else items.push({ label: "Agent", state: "missing", detail: tr("没找到，装一个：", "None found. Install one:") }, ...installItems(report));
  return [...items, ...extraItems(report, extras)];
}

const MARKS: Record<CheckState, string> = { ok: "✓", missing: "✕", note: "!", info: "·" };

function button(text: string, className: string, onClick: () => void): HTMLButtonElement {
  const b = Object.assign(document.createElement("button"), { type: "button", className, textContent: text });
  b.addEventListener("click", onClick);
  return b;
}

export function actionText(action: SetupAction): string {
  switch (action.do) {
    case "install":
      return tr("安装", "Install");
    case "login":
      return tr("登录", "Sign In");
    case "allow":
      return tr("授权", "Allow");
    case "browser":
    case "computer":
      return tr("设置", "Set Up");
    case "notifications":
      return tr("设置…", "Settings…");
  }
}

/** The tab (and, with no task yet, the task) a terminal action opens in. */
export function tabLabel(action: SetupAction): string {
  if (action.do === "install") return tr(`安装 ${action.name}`, `Install ${action.name}`);
  if (action.do === "login") return tr(`登录 ${action.name}`, `Sign in to ${action.name}`);
  return tr("设置浏览器", "Browser Setup");
}

/** Opens a terminal action's tab (main.ts: setup_run, then shows it). */
type OpenTab = (kind: string, action: "install" | "login" | "browser", label: string) => Promise<void>;
let openTab: OpenTab = () => Promise.reject(new Error("not ready"));
export function setSetupTabOpener(open: OpenTab): void {
  openTab = open;
}

/** Whether the detection now shows what `action` was for. */
function done(action: SetupAction, extras: SetupExtras | null): boolean {
  const agent = getClis()?.agents.find((a) => a.kind === action.kind);
  if (action.do === "install") return agent?.path != null;
  if (action.do === "login") return agent?.signed_in !== false;
  return extras?.browser === true;
}

const POLL_MS = 3000;
const POLL_FOR_MS = 20 * 60 * 1000;
/** Which action is under way, shown on its row. */
let running: SetupAction | null = null;

/** Runs `action`. Terminal ones hide `overlay` behind a small 返回 button and check every few seconds; once the
 *  agent is found (signed in, the runtime installed) the overlay comes back by itself. */
export async function runAction(action: SetupAction, overlay: HTMLElement, repaint: () => void): Promise<void> {
  if (action.do === "computer" || action.do === "allow" || action.do === "notifications") {
    await setupOpen(action.do === "notifications" ? "notifications" : "computer");
    repaint();
    return;
  }
  await openTab(action.kind, action.do, tabLabel(action));
  running = action;
  overlay.hidden = true;
  const back = button(tr("返回准备情况", "Back to Setup"), "btn primary setup-back", () => finish());
  document.body.append(back);
  const started = Date.now();
  const timer = window.setInterval(() => {
    if (Date.now() - started > POLL_FOR_MS) return window.clearInterval(timer);
    const extras = action.do === "browser" ? setupExtras().catch(() => null) : Promise.resolve(null);
    void Promise.all([refreshClis(), extras]).then(([, e]) => {
      if (done(action, e)) finish();
    }).catch(() => undefined);
  }, POLL_MS);
  function finish(): void {
    window.clearInterval(timer);
    back.remove();
    running = null;
    overlay.hidden = false;
    repaint();
  }
}

function row(item: CheckItem, overlay: HTMLElement | null, repaint: () => void): HTMLElement {
  const li = document.createElement("li");
  li.className = `setup-item ${item.state}`;
  const head = document.createElement("div");
  head.className = "setup-item-head";
  const busy = item.action && running?.do === item.action.do && running.kind === item.action.kind;
  head.append(
    Object.assign(document.createElement("span"), { className: "setup-mark", textContent: MARKS[item.state], ariaHidden: "true" }),
    Object.assign(document.createElement("span"), { className: "setup-label", textContent: item.label }),
    Object.assign(document.createElement("span"), { className: "setup-detail", textContent: busy ? tr("进行中…", "In progress…") : item.detail }),
  );
  if (item.action && overlay) {
    const action = item.action;
    const go = button(actionText(action), "btn setup-go", () => {
      go.disabled = true;
      runAction(action, overlay, repaint).catch(reportError).finally(() => (go.disabled = false));
    });
    head.append(go);
  } else if (item.url) {
    const url = item.url;
    const link = button(item.url === NODE_URL ? "nodejs.org" : tr("安装说明", "Install guide"), "link-btn", () => pathOpen(url).catch(reportError));
    link.title = url;
    head.append(link);
  }
  li.append(head);
  if (item.command) {
    const command = item.command;
    const line = document.createElement("div");
    line.className = "setup-command";
    const copy = button(tr("复制", "Copy"), "btn", () => {
      clipboardWrite(command).then(() => (copy.textContent = tr("已复制", "Copied"))).catch(reportError);
    });
    line.append(Object.assign(document.createElement("code"), { textContent: command }), copy);
    li.append(line);
  }
  return li;
}

/** A list of rows; buttons act and hide `overlay` while a terminal action runs. */
export function rowsElement(items: CheckItem[], overlay: HTMLElement | null, repaint: () => void): HTMLUListElement {
  const list = document.createElement("ul");
  list.className = "setup-list";
  list.append(...items.map((item) => row(item, overlay, repaint)));
  return list;
}

/** The checklist for `report`, with the line of other agents' install pages when one is already installed. */
export function checklistElement(report: CliReport, overlay: HTMLElement | null, extras: SetupExtras | null, repaint: () => void): HTMLElement {
  const box = document.createElement("div");
  box.className = "setup-check";
  box.append(rowsElement(setupChecks(report, extras), overlay, repaint));
  const line = agentsOf(report).some((a) => a.path !== null) ? missingLine(splitAgents(report).missing) : pageLinks(report);
  if (line) box.append(line);
  return box;
}

/** For the empty page: a button to the checklist until ready, then just what was found. */
export function setupNotes(report: CliReport | null): HTMLElement[] {
  if (!report) return [];
  if (!setupReady(report)) {
    const open = button(tr("准备情况…", "Setup Checklist…"), "btn primary", () => showSetupChecklist());
    return [Object.assign(document.createElement("div"), { className: "empty-note", textContent: tr("还没有 Agent。", "No agent yet.") }), open];
  }
  const note = document.createElement("div");
  note.className = "empty-note";
  note.textContent = `${tr("找到：", "Found: ")}${agentsOf(report).filter((a) => a.path !== null).map((a) => a.name).join(tr("、", ", "))}`;
  const notes: HTMLElement[] = [note];
  const line = missingLine(splitAgents(report).missing);
  if (line) notes.push(line);
  return notes;
}

let openClose: (() => void) | null = null;

/** The checklist over the window, from the menu or at start while something is missing. Repainted whenever the
 *  detection answers again (重新检查, coming back to the window, an action finishing). */
export function showSetupChecklist(): void {
  if (openClose) return;
  const overlay = document.createElement("div");
  overlay.className = "guide setup-overlay";
  const card = document.createElement("div");
  card.className = "guide-card";
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-label", tr("准备情况", "Setup Checklist"));
  const body = document.createElement("div");
  body.className = "guide-body";
  const lead = Object.assign(document.createElement("p"), { className: "guide-lead" });
  let extras: SetupExtras | null = null;
  const paint = () => {
    const report = getClis();
    lead.textContent = setupReady(report)
      ? tr("可以开始了。", "Ready to go.")
      : report?.tmux === null
        ? tr("需要 tmux 和一个 Agent。", "Needs tmux and one agent.")
        : tr("装一个 Agent 就能开始。", "Install one agent to start.");
    body.replaceChildren(...(report ? [checklistElement(report, overlay, extras, refresh)] : []));
  };
  const refresh = () => {
    paint();
    checkExtras();
  };
  const checkExtras = () => {
    if (overlay.hidden) return;
    setupExtras().then((e) => {
      extras = e;
      paint();
    }).catch(reportError);
  };
  const stop = onClisChange(paint);
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape" && !e.isComposing && !overlay.hidden) close();
  };
  // Permissions are granted in System Settings: look again on coming back.
  window.addEventListener("focus", checkExtras);
  const close = () => {
    stop();
    window.removeEventListener("keydown", onKey, true);
    window.removeEventListener("focus", checkExtras);
    overlay.remove();
    openClose = null;
  };
  const recheck = button(tr("重新检查", "Check Again"), "btn", () => {
    void refreshClis().catch(reportError);
    checkExtras();
  });
  const doneBtn = button(tr("关闭", "Close"), "btn primary", close);
  const foot = document.createElement("div");
  foot.className = "guide-foot";
  foot.append(recheck, doneBtn);
  card.append(Object.assign(document.createElement("h1"), { className: "guide-title", textContent: tr("准备情况", "Setup Checklist") }), lead, body, foot);
  overlay.append(card);
  window.addEventListener("keydown", onKey, true);
  paint();
  checkExtras();
  document.body.append(overlay);
  openClose = close;
  doneBtn.focus();
}
