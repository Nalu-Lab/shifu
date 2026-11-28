/** The first-run guide (docs/ONBOARDING.md): one screen shown on first open, while there are no tasks and it was
 *  neither finished nor skipped. Scan, show what was found, tick, go: the agents found on this Mac (one is the
 *  default), the projects found in the usual code folders (the newest few ticked), one 权限 toggle, and 开始, which
 *  creates a task per ticked project with the default agent and opens the first. Nothing is saved on 跳过 except
 *  that the guide was seen; 权限 and the default agent are saved on 开始 only. Words are kept to a minimum: what a
 *  new user reads before 开始 stays under about 40 Chinese characters. */
import { open } from "@tauri-apps/plugin-dialog";
import {
  type CliReport,
  type FoundProject,
  type Permissions,
  clipboardWrite,
  guideDoneSet,
  permissionsGet,
  permissionsSet,
  projectsScan,
} from "./api";
import { shortPath } from "./format";
import { tr } from "./i18n";
import { type AgentSpec, getClis, onClisChange, splitAgents } from "./models";
import { currentDefaultKind, setDefaultKind } from "./newpopover";
import { reportError } from "./store";
import { installItems, pageLinks, pageOnly, rowsElement, signInItems } from "./setupcheck";
import { projectName } from "./tasks";
import { markFor, vendorIcon } from "./vendoricons";

/** Shown only on a fresh start: nothing created yet and the guide not done. */
export const shouldShowGuide = (taskCount: number, done: boolean): boolean => taskCount === 0 && !done;

/** The agents to pick a default from: the installed ones (agents.json ones included), without the plain terminal. */
export const guideAgents = (report: CliReport | null): AgentSpec[] =>
  splitAgents(report).installed.filter((a) => a.kind !== "terminal");

/** The agent the tasks start with: the default while it is installed, else the first found. */
export function guideKind(agents: readonly AgentSpec[], current: string): string | null {
  return agents.find((a) => a.kind === current)?.kind ?? agents[0]?.kind ?? null;
}

/** How many of the found projects start ticked: the most recently changed ones. */
export const PRESELECT = 5;

/** The projects ticked before the person touches anything: the newest `PRESELECT` (the scan sorts newest first). */
export const preselected = (found: readonly FoundProject[]): string[] =>
  [...found].sort((a, b) => b.modified - a.modified).slice(0, PRESELECT).map((p) => p.path);

/** Found projects joined with the ones seen before; a path is listed once, the earlier listing wins. */
export function mergeProjects(have: readonly FoundProject[], more: readonly FoundProject[]): FoundProject[] {
  const seen = new Set(have.map((p) => p.path));
  return [...have, ...more.filter((p) => !seen.has(p.path))];
}

/** 开始 needs a ticked project, an installed agent and tmux (every conversation runs in it). Before detection
 *  answers nothing is known, so it waits. */
export function canStart(ticked: number, agents: readonly AgentSpec[], report: CliReport | null): boolean {
  return ticked > 0 && agents.length > 0 && report !== null && report.tmux !== null;
}

/** Creates one task per folder with `kind`, in order, and opens the first. Resolves with how many were made. */
export type CreateTasks = (projects: string[], kind: string) => Promise<number>;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className: string, text = ""): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  e.className = className;
  if (text) e.textContent = text;
  return e;
}

function btn(text: string, className: string, onClick: () => void): HTMLButtonElement {
  const b = el("button", className, text);
  b.type = "button";
  b.addEventListener("click", onClick);
  return b;
}

/** Puts the guide over the window. Finishing or skipping saves `guide_done`, so it does not come back. */
export function showGuide(createTasks: CreateTasks): void {
  const overlay = el("div", "guide");
  const card = el("div", "guide-card guide-scan");
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-label", tr("开始使用 Shifu", "Welcome to Shifu"));
  overlay.append(card);

  let kind: string | null = null;
  let found: FoundProject[] = [];
  const ticked = new Set<string>();
  let touched = false;
  let scanning = true;
  let protectedLeft: string[] = [];
  let busy = false;
  let moreOpen = false;
  const close = () => {
    stop();
    overlay.remove();
    guideDoneSet().catch(reportError);
  };

  // Agents: repainted when detection answers (it can come after the guide opened) and on return to the window.
  const agentBox = el("div", "guide-body");
  const paintAgents = () => {
    const report = getClis();
    const agents = guideAgents(report);
    kind = guideKind(agents, kind ?? currentDefaultKind());
    agentBox.replaceChildren();
    if (!report) {
      agentBox.append(el("div", "guide-note", tr("正在查找…", "Looking…")));
      paintStart();
      return;
    }
    if (report.tmux === null) {
      const line = el("div", "guide-tmux");
      const command = "brew install tmux";
      const copy = btn(tr("复制", "Copy"), "btn", () => {
        clipboardWrite(command).then(() => (copy.textContent = tr("已复制", "Copied"))).catch(reportError);
      });
      line.append(el("span", "guide-warn", tr("需要 tmux：", "Needs tmux:")), el("code", "", command), copy);
      agentBox.append(line);
    }
    // Install and Sign In open a terminal tab and step the guide aside until it is done (setupcheck.ts).
    if (agents.length === 0) {
      agentBox.append(el("div", "guide-warn", tr("没找到 Agent，装一个：", "No agent found. Install one:")), rowsElement(installItems(report), overlay, paintAgents));
      const links = pageLinks(report);
      if (links) agentBox.append(links);
    } else {
      const chips = el("div", "guide-chips");
      chips.setAttribute("role", "radiogroup");
      for (const spec of agents) {
        const chip = btn("", "guide-chip", () => {
          kind = spec.kind;
          paintAgents();
        });
        chip.setAttribute("role", "radio");
        chip.setAttribute("aria-checked", String(spec.kind === kind));
        // A missing sign-in has its own Sign In row below.
        chip.append(vendorIcon(markFor(spec.kind), "agent-mark"), el("span", "", spec.name));
        chips.append(chip);
      }
      agentBox.append(chips);
      const signIn = signInItems(report);
      if (signIn.length > 0) agentBox.append(rowsElement(signIn, overlay, paintAgents));
      const more = installItems(report);
      if (more.length > 0 || pageOnly(report).length > 0) {
        const details = el("details", "guide-more");
        details.open = moreOpen;
        details.addEventListener("toggle", () => (moreOpen = details.open));
        details.append(el("summary", "", tr("更多 Agent", "More agents")), rowsElement(more, overlay, paintAgents));
        const links = pageLinks(report);
        if (links) details.append(links);
        agentBox.append(details);
      }
    }
    paintStart();
  };

  // Projects: what the scan found, newest first, the newest few ticked, plus any folder picked by hand.
  const projectList = el("div", "guide-projects");
  const projectFoot = el("div", "guide-row");
  const paintProjects = () => {
    projectList.replaceChildren();
    if (scanning) projectList.append(el("div", "guide-note", tr("正在查找…", "Looking…")));
    for (const p of found) {
      const row = el("label", "guide-project");
      const box = Object.assign(document.createElement("input"), { type: "checkbox", checked: ticked.has(p.path) });
      box.addEventListener("change", () => {
        touched = true;
        if (box.checked) ticked.add(p.path);
        else ticked.delete(p.path);
        paintStart();
      });
      const where = el("span", "guide-tag", shortPath(p.path.replace(/\/[^/]+\/?$/, ""), 2));
      row.title = p.path;
      row.append(box, el("span", "guide-project-name", p.name || projectName(p.path)), where);
      projectList.append(row);
    }
    projectFoot.replaceChildren(other);
    if (protectedLeft.length > 0 && !scanning) projectFoot.append(searchProtected);
    paintStart();
  };
  const runScan = async (protectedToo: boolean) => {
    scanning = true;
    paintProjects();
    try {
      const result = await projectsScan(protectedToo);
      found = mergeProjects(found, result.projects);
      protectedLeft = result.protected_left;
      if (!touched) {
        ticked.clear();
        for (const path of preselected(found)) ticked.add(path);
      }
    } catch (e) {
      reportError(e);
    } finally {
      scanning = false;
      paintProjects();
    }
  };
  const other = btn(tr("其他文件夹…", "Other Folder…"), "btn", async () => {
    try {
      const dir = await open({ directory: true, multiple: false, title: tr("选择项目文件夹", "Choose a Project Folder") });
      if (typeof dir !== "string") return;
      touched = true;
      found = [{ path: dir, name: projectName(dir), git: false, modified: Date.now() / 1000 }, ...found.filter((p) => p.path !== dir)];
      ticked.add(dir);
      paintProjects();
    } catch (e) {
      reportError(e);
    }
  });
  // Desktop, Documents and Downloads are guarded by macOS, which asks before an app may look inside: only on a tap.
  const searchProtected = btn(tr("查找桌面和文稿", "Search Desktop & Documents"), "link-btn", () => void runScan(true));

  // 权限: one toggle, saved on 开始 only. Off is 「按 Agent 自己的设置」, the recommended and fresh-install default.
  const permBox = Object.assign(document.createElement("input"), { type: "checkbox", disabled: true });
  let permKnown = false;
  permissionsGet()
    .then((mode) => {
      permBox.checked = mode === "bypass";
      permBox.disabled = false;
      permKnown = true;
    })
    .catch(reportError);
  const perm = el("label", "guide-perm");
  perm.append(
    permBox,
    el("span", "guide-perm-label", tr("免确认", "No confirmation")),
    el("span", "guide-tag", tr("改文件、跑命令不再问你", "agents edit and run commands without asking")),
  );

  const skip = btn(tr("跳过", "Skip"), "btn", close);
  const start = btn(tr("开始", "Start"), "btn primary", async () => {
    const report = getClis();
    const chosen = guideKind(guideAgents(report), kind ?? "");
    const projects = found.map((p) => p.path).filter((path) => ticked.has(path));
    if (!chosen || !canStart(projects.length, guideAgents(report), report)) return;
    busy = true;
    paintStart();
    try {
      // The toggle is the answer to the first-run permission question, so it is not asked again.
      if (permKnown) await permissionsSet((permBox.checked ? "bypass" : "own") as Permissions);
      setDefaultKind(chosen);
      if ((await createTasks(projects, chosen)) > 0) close();
      else {
        busy = false;
        paintStart();
      }
    } catch (e) {
      reportError(e);
      busy = false;
      paintStart();
    }
  });
  function paintStart(): void {
    start.disabled = busy || !canStart(ticked.size, guideAgents(getClis()), getClis());
  }
  const foot = el("div", "guide-foot");
  foot.append(skip, start);

  card.append(
    el("h1", "guide-title", tr("开始使用 Shifu", "Welcome to Shifu")),
    el("div", "guide-step-title", "Agent"),
    agentBox,
    el("div", "guide-step-title", tr("项目", "Projects")),
    projectList,
    projectFoot,
    perm,
    foot,
  );
  const stop = onClisChange(paintAgents);
  paintAgents();
  void runScan(false);
  document.body.append(overlay);
}
