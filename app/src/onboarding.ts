/** The first-run guide (docs/ONBOARDING.md, 第 3 阶段): one screen shown on first open, while there are no tasks
 *  and it was neither finished nor skipped. Default agent, 权限, a test notification, then the first project
 *  folder: the task is created with the default agent, which opens right away. */
import { open } from "@tauri-apps/plugin-dialog";
import { type CliReport, type Permissions, guideDoneSet, notifySend, permissionsSet } from "./api";
import { type AgentSpec, getClis, onClisChange, splitAgents } from "./models";
import { agentDropdown, currentDefaultKind, permissionPicker, setDefaultKind, setupNotes } from "./newpopover";
import { reportError } from "./store";
import { projectName } from "./tasks";

/** Shown only on a fresh start: nothing created yet and the guide not done. */
export const shouldShowGuide = (taskCount: number, done: boolean): boolean => taskCount === 0 && !done;

/** The agents to pick a default from: the installed ones, without the plain terminal. */
export const guideAgents = (report: CliReport | null): AgentSpec[] =>
  splitAgents(report).installed.filter((a) => a.kind !== "terminal");

/** 开始 needs a folder and an installed agent to start in it. */
export const canStart = (folder: string | null, agents: readonly AgentSpec[]): boolean =>
  folder !== null && agents.length > 0;

/** The agent the first task starts with: the default while it is installed, else the first found. */
export function guideKind(agents: readonly AgentSpec[], current: string): string | null {
  return agents.find((a) => a.kind === current)?.kind ?? agents[0]?.kind ?? null;
}

const TEST_AGENT = "shifu-guide-test";

type CreateTask = (project: string, name: string, kind: string) => Promise<void>;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className: string, text = ""): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  e.className = className;
  if (text) e.textContent = text;
  return e;
}

function step(n: number, title: string, ...body: HTMLElement[]): HTMLElement {
  const box = el("section", "guide-step");
  box.append(el("div", "guide-step-title", `${n}. ${title}`), ...body);
  return box;
}

/** Puts the guide over the window. Finishing or skipping saves `guide_done`, so it does not come back. */
export function showGuide(create: CreateTask): void {
  const overlay = el("div", "guide");
  const card = el("div", "guide-card");
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-label", "开始使用 Shifu");
  overlay.append(card);

  let folder: string | null = null;
  let busy = false;
  const close = () => {
    stop();
    overlay.remove();
    guideDoneSet().catch(reportError);
  };

  // 1. Agent: repainted when the detection answers (it can come after the guide opened) and on return to the window.
  const agentBox = el("div", "guide-body");
  const paintAgents = () => {
    const agents = guideAgents(getClis());
    const kind = guideKind(agents, currentDefaultKind());
    agentBox.replaceChildren();
    if (kind) {
      setDefaultKind(kind);
      agentBox.append(agentDropdown(agents, kind, (picked) => setDefaultKind(picked)));
    }
    agentBox.append(...setupNotes(getClis()));
    paintStart();
  };

  // 3. Notifications: a test one, which also makes macOS ask for permission the first time.
  const notifyStatus = el("div", "guide-note");
  const notifyButton = el("button", "btn", "发一条测试通知");
  notifyButton.addEventListener("click", () => {
    notifyStatus.textContent = "";
    notifySend(TEST_AGENT, "Shifu", "通知可以用了：Agent 等你回复时会这样提醒你。")
      .then(() => {
        notifyStatus.textContent = "已发出。没看到的话，到「系统设置 › 通知 › Shifu」里打开。";
      })
      .catch((e) => {
        notifyStatus.textContent = `没发出去：${e instanceof Error ? e.message : String(e)}`;
      });
  });
  const notifyBody = el("div", "guide-body");
  notifyBody.append(notifyButton, notifyStatus);

  // 4. The first project folder.
  const folderLabel = el("span", "guide-folder", "还没选");
  const folderButton = el("button", "btn", "选择文件夹…");
  folderButton.addEventListener("click", async () => {
    try {
      const dir = await open({ directory: true, multiple: false, title: "选择第一个项目文件夹" });
      if (typeof dir === "string") {
        folder = dir;
        folderLabel.textContent = projectName(dir);
        folderLabel.title = dir;
        paintStart();
      }
    } catch (e) {
      reportError(e);
    }
  });
  const folderBody = el("div", "guide-body guide-row");
  folderBody.append(folderButton, folderLabel);

  const skip = el("button", "btn", "跳过");
  skip.addEventListener("click", close);
  const start = el("button", "btn primary", "建立任务并开始");
  function paintStart(): void {
    start.disabled = busy || !canStart(folder, guideAgents(getClis()));
    start.title = guideAgents(getClis()).length ? "" : "先装好一个 Agent";
  }
  start.addEventListener("click", async () => {
    const kind = guideKind(guideAgents(getClis()), currentDefaultKind());
    if (!folder || !kind) return;
    busy = true;
    paintStart();
    try {
      // What step 2 shows is the answer to the first-run permission question, so it is not asked again.
      const shown = permissions.querySelector("select");
      if (shown && !shown.disabled) await permissionsSet(shown.value as Permissions);
      await create(folder, "", kind);
      close();
    } catch (e) {
      reportError(e);
      busy = false;
      paintStart();
    }
  });
  const foot = el("div", "guide-foot");
  foot.append(skip, start);
  const permissions = permissionPicker();

  card.append(
    el("h1", "guide-title", "开始使用 Shifu"),
    el("p", "guide-lead", "四步设好，也可以先跳过，以后在新建任务里都能改。"),
    step(1, "默认 Agent", agentBox),
    step(2, "权限（所有对话）", permissions),
    step(3, "通知", notifyBody),
    step(4, "第一个项目", folderBody),
    foot,
  );
  const stop = onClisChange(paintAgents);
  paintAgents();
  document.body.append(overlay);
}
