/** 准备情况: what a new Mac still needs before the first agent runs (docs/ONBOARDING.md, 第 5 阶段). tmux, the
 *  agent CLIs found on the PATH (`clis_detect`, file checks only) and, for agents whose CLI keeps its sign-in in a
 *  known file, whether it is there. Every missing piece comes with its command to copy and its install page. Shown
 *  on the empty page and in the first-run guide until it is ready, and from the app menu (准备情况…) any time. */
import { type CliFound, type CliReport, clipboardWrite, pathOpen } from "./api";
import { getClis, onClisChange, refreshClis, splitAgents } from "./models";
import { missingLine } from "./newpopover";
import { reportError } from "./store";

export type CheckState = "ok" | "missing" | "note";

export interface CheckItem {
  label: string;
  state: CheckState;
  detail: string;
  /** The exact command to run, shown with a copy button. */
  command?: string;
  url?: string;
}

const TMUX_URL = "https://github.com/tmux/tmux/wiki/Installing";

const agentsOf = (report: CliReport): CliFound[] => report.agents.filter((a) => a.kind !== "terminal");

/** Ready for a first agent: tmux and at least one agent CLI. Sign-in does not count: each agent offers it when it
 *  starts. Before the first detection answers nothing is known, so nothing is flagged. */
export function setupReady(report: CliReport | null): boolean {
  return report === null || (report.tmux !== null && agentsOf(report).some((a) => a.path !== null));
}

function agentItem(agent: CliFound): CheckItem {
  if (agent.path === null) {
    return {
      label: agent.name,
      state: "missing",
      detail: agent.install_command ? "没装。在终端里运行这一行，或看安装说明：" : "没装。按安装说明装好：",
      command: agent.install_command ?? undefined,
      url: agent.install_url,
    };
  }
  if (agent.signed_in === false) {
    return {
      label: agent.name,
      state: "note",
      detail: agent.login_command
        ? "已装，没找到登录信息。第一次开对话时按它的提示登录，或先运行："
        : "已装，没找到登录信息。第一次开对话时按它的提示登录。",
      command: agent.login_command ?? undefined,
    };
  }
  return { label: agent.name, state: "ok", detail: agent.signed_in ? "已装，已登录" : "已装" };
}

/** The rows: tmux, then the installed agents (with sign-in where it can be told), then, only when none is
 *  installed, every core agent with how to install it. When some are installed the others are only a line of links
 *  (`missingLine`), since one agent is enough. */
export function setupChecks(report: CliReport): CheckItem[] {
  const items: CheckItem[] = [
    report.tmux
      ? { label: "tmux", state: "ok", detail: "已装" }
      : {
          label: "tmux",
          state: "missing",
          detail: "没装。Shifu 的每个对话都在 tmux 里运行。装了 Homebrew 的话运行：",
          command: "brew install tmux",
          url: TMUX_URL,
        },
  ];
  const agents = agentsOf(report);
  const installed = agents.filter((a) => a.path !== null);
  if (installed.length > 0) return [...items, ...installed.map(agentItem)];
  items.push({ label: "Agent", state: "missing", detail: "还没找到 Agent 命令行工具，装好下面任意一个就行：" });
  // Extra plain agents and the user's own are offered once found, not suggested for installing.
  return [...items, ...agents.filter((a) => !a.extra).map(agentItem)];
}

const MARKS: Record<CheckState, string> = { ok: "✓", missing: "✕", note: "!" };

function button(text: string, className: string, onClick: () => void): HTMLButtonElement {
  const b = Object.assign(document.createElement("button"), { type: "button", className, textContent: text });
  b.addEventListener("click", onClick);
  return b;
}

function row(item: CheckItem): HTMLElement {
  const li = document.createElement("li");
  li.className = `setup-item ${item.state}`;
  const head = document.createElement("div");
  head.className = "setup-item-head";
  head.append(
    Object.assign(document.createElement("span"), { className: "setup-mark", textContent: MARKS[item.state], ariaHidden: "true" }),
    Object.assign(document.createElement("span"), { className: "setup-label", textContent: item.label }),
    Object.assign(document.createElement("span"), { className: "setup-detail", textContent: item.detail }),
  );
  if (item.url) {
    const url = item.url;
    const link = button("安装说明", "link-btn", () => pathOpen(url).catch(reportError));
    link.title = url;
    head.append(link);
  }
  li.append(head);
  if (item.command) {
    const command = item.command;
    const line = document.createElement("div");
    line.className = "setup-command";
    const copy = button("复制", "btn", () => {
      clipboardWrite(command).then(() => (copy.textContent = "已复制")).catch(reportError);
    });
    line.append(Object.assign(document.createElement("code"), { textContent: command }), copy);
    li.append(line);
  }
  return li;
}

/** The checklist for `report`, with the line of other agents' install pages when one is already installed. */
export function checklistElement(report: CliReport): HTMLElement {
  const box = document.createElement("div");
  box.className = "setup-check";
  const list = document.createElement("ul");
  list.className = "setup-list";
  list.append(...setupChecks(report).map(row));
  box.append(list);
  if (agentsOf(report).some((a) => a.path !== null)) {
    const line = missingLine(splitAgents(report).missing);
    if (line) box.append(line);
  }
  return box;
}

/** For the empty page: the checklist until ready, then just what was found. */
export function setupNotes(report: CliReport | null): HTMLElement[] {
  if (!report) return [];
  if (!setupReady(report)) return [checklistElement(report)];
  const note = document.createElement("div");
  note.className = "empty-note";
  note.textContent = `这台 Mac 上找到：${agentsOf(report).filter((a) => a.path !== null).map((a) => a.name).join("、")}`;
  const notes: HTMLElement[] = [note];
  const line = missingLine(splitAgents(report).missing);
  if (line) notes.push(line);
  return notes;
}

let openClose: (() => void) | null = null;

/** The checklist over the window, from the menu or at start while something is missing. Repainted whenever the
 *  detection answers again (重新检查, or coming back to the window after installing). */
export function showSetupChecklist(): void {
  if (openClose) return;
  const overlay = document.createElement("div");
  overlay.className = "guide setup-overlay";
  const card = document.createElement("div");
  card.className = "guide-card";
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-label", "准备情况");
  const body = document.createElement("div");
  body.className = "guide-body";
  const lead = Object.assign(document.createElement("p"), { className: "guide-lead" });
  const paint = () => {
    const report = getClis();
    lead.textContent = setupReady(report)
      ? "都准备好了，可以开始第一个对话。"
      : "Shifu 需要 tmux 和至少一个 Agent 命令行工具。装好后点「重新检查」，回到这个窗口时也会自动检查。";
    body.replaceChildren(...(report ? [checklistElement(report)] : []));
  };
  const stop = onClisChange(paint);
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape" && !e.isComposing) close();
  };
  const close = () => {
    stop();
    window.removeEventListener("keydown", onKey, true);
    overlay.remove();
    openClose = null;
  };
  const recheck = button("重新检查", "btn", () => void refreshClis().catch(reportError));
  const done = button("关闭", "btn primary", close);
  const foot = document.createElement("div");
  foot.className = "guide-foot";
  foot.append(recheck, done);
  card.append(Object.assign(document.createElement("h1"), { className: "guide-title", textContent: "准备情况" }), lead, body, foot);
  overlay.append(card);
  window.addEventListener("keydown", onKey, true);
  paint();
  document.body.append(overlay);
  openClose = close;
  done.focus();
}
