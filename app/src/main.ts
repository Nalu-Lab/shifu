import { listen } from "@tauri-apps/api/event";
import { open as openPanel } from "@tauri-apps/plugin-dialog";
import { pickedPaths } from "./filekind";
import { applyStoredFontSizes, stepTerminal } from "./fontsize";
import "@xterm/xterm/css/xterm.css";
import "dockview/dist/styles/dockview.css";
import "./style.css";
import { showAppearancePopover } from "./appearance";
import { checkForUpdates, startUpdateChecks } from "./update";
import { applyStoredTheme } from "./theme";
import { ProgressPage } from "./progress";
import { progressReader } from "./progressrefresh";
import { EndNotices } from "./endnotices";
import { getClis, onClisChange, pickArgs, pickFor, refreshClis, refreshModels, setClis } from "./models";
import {
  agentKill,
  agentResume,
  agentsList,
  botList,
  botJoin,
  botLeave,
  botOpen,
  botSave,
  type ResidentBot,
  goalsGet,
  jevLogTail,
  notifyRemove,
  notifySend,
  pathReveal,
  cliMismatch,
  pmDir,
  pmOpen,
  dailyDir,
  dailyOpen,
  fullscreenInset,
  type FullscreenInset,
  projectReorder,
  seenLoad,
  seenSave,
  setAttention,
  taskAddAgent,
  taskDelete,
  taskMoveAgent,
  taskNewWithAgent,
  taskNew,
  taskRename,
  taskReorder,
  agentReorder,
  guideDoneGet,
  taskGoalsGet,
  taskSetCurrent,
  tasksLoad,
  type CliReport,
  setBeforeAgentStart,
  setupRun,
  uiChinese,
} from "./api";
import { setChinese, tr } from "./i18n";
import { baselineSeen } from "./attention";
import { agentLabel, type AgentView } from "./format";
import { closeMenu } from "./menu";
import { waitForVerdict } from "./jev";
import { currentlyNeeding, nextNotices, type Notice, type Notified, staleNotices } from "./notify";
import { closeNewPopover, currentDefaultKind, showAddAgentPopover, showNewTaskPopover } from "./newpopover";
import { shouldShowGuide, showGuide } from "./onboarding";
import { ensurePermissionsChosen } from "./permissions";
import { setSetupTabOpener, setupReady, showSetupChecklist } from "./setupcheck";
import { startPtyBus } from "./ptybus";
import { Sidebar, type Role } from "./sidebar";
import { botKindKey } from "./botpicker";
import { MemoryBar } from "./memory";
import { LimitsBar } from "./limitsbar";
import { PowerButton } from "./power";
import { RemoteButton } from "./remote";
import { handleRemoteControlMenu, leaveRemoteController } from "./remotecontrol";
import {
  getActiveAgent,
  getWaitingList,
  dismissWaitingFor,
  dropPickedFolder,
  getPickedFolders,
  initWaitingOpened,
  initPinnedTasks,
  onWaitingOpenedChange,
  getAgents,
  getCurrentId,
  getCurrentTask,
  getProjectOrder,
  getTasks,
  findAgent,
  initSeen,
  onAttentionChange,
  onSeenChange,
  reportError,
  setActiveAgent,
  setAgents,
  setCurrent,
  setTaskFile,
  setWindowFocused,
  subscribe,
} from "./store";
import { type TaskFile, projectsOf, taskOfAgent, withPickedFolders } from "./tasks";
import { loadWaitingSeen, WAITING_SEEN_KEY } from "./waitingseen";

const PINNED_TASKS_KEY = "shifu.pinnedTasks";
/** Saved by an older or interrupted write, or absent: then nothing is pinned. */
function loadPinnedTasks(storage: Pick<Storage, "getItem">): string[] {
  try {
    const ids: unknown = JSON.parse(storage.getItem(PINNED_TASKS_KEY) ?? "[]");
    return Array.isArray(ids) ? ids.filter((id): id is string => typeof id === "string") : [];
  } catch {
    return [];
  }
}
import { Workspace } from "./workspace";
import type { DockviewGroupPanel } from "dockview";

async function main() {
  // A fresh install is asked how agents may act before the first one starts (permissions.ts).
  setBeforeAgentStart(ensurePermissionsChosen);
  // The first-run screens, the new-task form and the empty page follow the system language (i18n.ts).
  setChinese(await uiChinese().catch(() => true));
  applyStoredTheme();
  applyStoredFontSizes();
  // The picker uses last launch's list until this answers (a couple of seconds).
  void refreshModels().catch(reportError);
  // Which agent CLIs are installed (file checks); again when the login shell's PATH arrives and on every return
  // to the window, so one installed meanwhile shows up.
  void refreshClis().catch(reportError);
  void listen<CliReport>("clis", ({ payload }) => setClis(payload));
  await startPtyBus();
  const initial = await agentsList();
  // First start with this version: everything that exists counts as seen. The baseline is written right away,
  // so a restart before any reply is seen does not make them all new.
  const stored = await seenLoad();
  const seen = stored ?? baselineSeen(initial);
  if (!stored) await seenSave(seen).catch(reportError);
  initSeen(seen);
  initWaitingOpened(loadWaitingSeen(localStorage));
  initPinnedTasks(loadPinnedTasks(localStorage), (ids) => {
    try { localStorage.setItem(PINNED_TASKS_KEY, JSON.stringify(ids)); } catch (error) { reportError(error); }
  });
  onWaitingOpenedChange((next) => {
    try { localStorage.setItem(WAITING_SEEN_KEY, JSON.stringify(next)); } catch (error) { reportError(error); }
  });
  onSeenChange((next) => void seenSave(next).catch(reportError));
  onAttentionChange((count) => void setAttention(count).catch(reportError));
  setWindowFocused(document.hasFocus());
  window.addEventListener("focus", () => {
    setWindowFocused(true);
    void refreshClis().catch(reportError);
  });
  window.addEventListener("blur", () => setWindowFocused(false));
  setAgents(initial);
  // Notifications (APP-SPEC-B4): what already needs the user at start-up is not announced.
  let notified: Notified = currentlyNeeding(getAgents());
  // A new Claude reply waits (at most 3 s) for Jev's verdict; when Jev says it was a report there is no notification.
  const deliver = async (n: Notice) => {
    if (n.jev) {
      const verdict = await waitForVerdict(n.jev.session, n.jev.since, {
        readTail: jevLogTail,
        sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
      });
      if (verdict === "report") return;
    }
    await notifySend(n.agent, n.title, n.body);
    shown.add(n.agent);
  };
  // Agents with a notification in Notification Center. One comes out when its agent no longer needs the user or
  // is the one in front of them; the other agents' stay (each left there also keeps a timer busy on the main
  // thread, see `notify::remove`, so they do not linger past their use).
  const shown = new Set<string>();
  subscribe(() => {
    const agents = getAgents();
    const next = nextNotices(notified, agents, document.hasFocus());
    notified = next.notified;
    for (const n of next.notices) void deliver(n).catch(reportError);
    const stale = staleNotices(shown, agents, getActiveAgent(), document.hasFocus());
    if (stale.length === 0) return;
    stale.forEach((name) => shown.delete(name));
    void notifyRemove(stale).catch(reportError);
  });
  const file = await tasksLoad();
  setTaskFile(file);

  const app = document.getElementById("app")!;
  const dock = document.createElement("div");
  dock.className = "dock";

  let showingTask: Promise<void> = Promise.resolve();
  /** Make `id` the current task: its layout (or its agents as tabs) replaces the one on screen. */
  const showTask = async (id: string) => {
    const task = getTasks().find((t) => t.id === id);
    if (!task) return;
    setCurrent(id);
    taskSetCurrent(id).catch(reportError);
    showingTask = workspace.show(task);
    await showingTask;
  };
  const selectTask = async (id: string) => {
    if (getCurrentId() !== id) await showTask(id);
    // The sidebar selects immediately, but its saved layout may still be loading.
    else await showingTask;
  };
  /** The conversation the user just opened from the sidebar. One whose process is gone (Mac restarted, tmux quit)
   *  comes back on its own here; the others wait until they are opened (Eddie 11-22). */
  const openedByUser = (name: string) => {
    dismissWaitingFor(name);
    if (findAgent(name)?.exited) agentResume(name).catch(reportError);
  };
  const focusAgent = async (taskId: string, name: string) => {
    hideProgress();
    await selectTask(taskId);
    workspace.open(name);
    if (getCurrentId() === taskId && workspace.activeAgent() === name) openedByUser(name);
  };

  // The setup checklist's Install / Sign In / browser setup: a terminal tab in the current task (or a new one).
  setSetupTabOpener(async (kind, action, label) => {
    const { file, agent } = await setupRun(kind, action, getCurrentId(), label);
    setAgents(await agentsList());
    setTaskFile(file);
    if (file.current && agent) await focusAgent(file.current, agent);
  });

  const createTask = async (project: string, name: string, kind: string) => {
    const { file } = await taskNew(project, name, kind, pickArgs(pickFor(kind)));
    dropPickedFolder(project);
    // The new agent has to be in the list before its task is shown, or the panel would be dropped as stale.
    setAgents(await agentsList());
    setTaskFile(file);
    if (file.current) await showTask(file.current);
  };

  /** The first-run guide's 开始: one task per ticked folder with `kind`, then the first one opened. A folder that
   *  fails is reported and the rest still go ahead. */
  const createTasks = async (projects: string[], kind: string): Promise<number> => {
    let first: string | null = null;
    let last: TaskFile | null = null;
    for (const project of projects) {
      try {
        const { file } = await taskNew(project, "", kind, pickArgs(pickFor(kind)));
        first ??= file.current;
        last = file;
        dropPickedFolder(project);
      } catch (e) {
        reportError(e);
      }
    }
    if (!last) return 0;
    setAgents(await agentsList());
    setTaskFile(last);
    if (first) await showTask(first);
    return projects.length;
  };

  /** 添加项目 on the empty page: the folder dialog, then a task there with the default agent. */
  const addProject = async () => {
    const dir = await openPanel({ directory: true, multiple: false, title: tr("添加项目", "Add Project") });
    if (typeof dir === "string") await createTask(dir, "", currentDefaultKind());
  };

  /** Add an agent to the current task and open it (in `group` if given, or split off it toward `direction`). */
  const addAgent = async (kind: string, group?: DockviewGroupPanel, direction?: "below" | "right") => {
    const id = getCurrentId();
    if (!id) throw new Error("先新建一个任务");
    const { file, agent } = await taskAddAgent(id, kind, pickArgs(pickFor(kind)));
    setTaskFile(file);
    if (agent && getCurrentId() === id) workspace.open(agent, { group, direction });
  };

  /** Which agent each pinned role is (its model is remembered per agent, like everywhere else). Asked the first
   *  time it starts; later starts, after it was ended, use the same without asking . */
  const ROLES: Record<Role, { kindKey: string; dir: () => Promise<string>; open: typeof pmOpen }> = {
    pm: { kindKey: "shifu.pmKind", dir: pmDir, open: pmOpen },
    daily: { kindKey: "shifu.dailyKind", dir: dailyDir, open: dailyOpen },
  };
  const startRole = async (role: Role, kind: string) => {
    const { file, agent } = await ROLES[role].open(kind, pickArgs(pickFor(kind)));
    // A just-started agent has to be in the list before its task is shown, or its panel is dropped as stale.
    if (agent) setAgents(await agentsList());
    setTaskFile(file);
    const dir = await ROLES[role].dir();
    const task = file.tasks.find((t) => t.project === dir);
    if (task) await selectTask(task.id);
  };
  const openRole = async (role: Role, anchor: HTMLElement) => {
    const dir = await ROLES[role].dir();
    const running = getTasks().find((t) => t.project === dir && t.agents.length > 0);
    if (running) {
      await selectTask(running.id);
      const active = workspace.activeAgent();
      if (getCurrentId() === running.id && active) openedByUser(active);
      return;
    }
    const kind = localStorage.getItem(ROLES[role].kindKey);
    if (kind) return startRole(role, kind);
    showAddAgentPopover(anchor, async (picked) => {
      localStorage.setItem(ROLES[role].kindKey, picked);
      await startRole(role, picked);
    }, { agentsOnly: true });
  };

  const openBot = async (bot: ResidentBot, anchor: HTMLElement) => {
    const existing = getTasks().find((task) => task.project === bot.dir);
    if (existing?.agents.length) return focusAgent(existing.id, existing.agents[0]);

    const kindKey = botKindKey(bot.id);
    const start = async (kind: string) => {
      const { file, agent } = await botOpen(bot.id, kind, pickArgs(pickFor(kind)));
      if (agent) setAgents(await agentsList());
      setTaskFile(file);
      const task = file.tasks.find((item) => item.project === bot.dir);
      const name = agent ?? task?.agents[0];
      if (!task || !name) throw new Error(`无法打开${bot.name}的对话`);
      localStorage.setItem(kindKey, kind);
      await focusAgent(task.id, name);
    };
    const kind = localStorage.getItem(kindKey);
    if (kind) await start(kind);
    else showAddAgentPopover(anchor, start, { agentsOnly: true });
  };

  const deleteTask = async (id: string) => {
    const task = getTasks().find((t) => t.id === id);
    const wasCurrent = getCurrentId() === id;
    // Its panels go first; its layout must not be saved again after the file is deleted.
    if (wasCurrent) await workspace.detach();
    try {
      const file = await taskDelete(id);
      setTaskFile(file);
      setAgents(await agentsList());
      if (wasCurrent) {
        setCurrent(file.current);
        if (file.current) await showTask(file.current);
      }
    } catch (e) {
      reportError(e);
      if (wasCurrent && task) await workspace.show(task);
    }
  };

  const requestNewTask = (anchor: HTMLElement, project?: string) =>
    void showNewTaskPopover(anchor, {
      projects: withPickedFolders(projectsOf(getTasks(), getProjectOrder()), getPickedFolders()).map((p) => p.path),
      preselect: project ?? getCurrentTask()?.project,
      create: createTask,
    });

  const sidebar = new Sidebar({
    selectTask: (id) => {
      hideProgress();
      void selectTask(id).then(() => {
        const active = workspace.activeAgent();
        if (getCurrentId() === id && active) openedByUser(active);
      }).catch(reportError);
    },
    focusAgent: (taskId, name) => void focusAgent(taskId, name).catch(reportError),
    putAway: (name) => (workspace.api.getPanel(name) ? () => workspace.closeTab(name) : undefined),
    moveAgent: (name, toId) =>
      void taskMoveAgent(name, toId)
        .then((file) => {
          setTaskFile(file);
          return workspace.moved(name, toId, file.tasks.map((task) => task.id));
        })
        .catch(reportError),
    moveAgentToFolder: (name, project) =>
      void taskNewWithAgent(project, name)
        .then((file) => {
          setTaskFile(file);
          dropPickedFolder(project);
          const made = file.tasks.find((task) => task.agents.includes(name));
          return made ? workspace.moved(name, made.id, file.tasks.map((task) => task.id)) : undefined;
        })
        .catch(reportError),
    removeEmptyFolder: (project) => dropPickedFolder(project),
    killAgent: endAgent,
    reorderProject: (paths) => void projectReorder(paths).then(setTaskFile).catch(reportError),
    reorderTask: (project, ids) => void taskReorder(project, ids).then(setTaskFile).catch(reportError),
    reorderAgents: (task, names) => void agentReorder(task, names).then(setTaskFile).catch(reportError),
    renameTask: (id, name) => void taskRename(id, name).then(setTaskFile).catch(reportError),
    revealPath: (path) => void pathReveal(path).catch(reportError),
    deleteTask: (id) => void deleteTask(id),
    requestNewTask,
    showProgress: () => showProgress(),
    openRole: (role, anchor) => {
      hideProgress();
      void openRole(role, anchor).catch(reportError);
    },
    repickRole: (role) => localStorage.removeItem(ROLES[role].kindKey),
    listBots: botList,
    joinBot: botJoin,
    leaveBot: botLeave,
    saveBot: botSave,
    openBot: (bot, anchor) => void openBot(bot, anchor).catch(reportError),
  });
  try {
    const bots = await botList();
    if (bots) sidebar.setBots(bots);
  } catch (error) {
    reportError(error);
    // The cat retries the catalog if the initial read failed.
  }
  void pmDir().then((dir) => sidebar.setRoleDir("pm", dir)).catch(reportError);
  void cliMismatch().then((why) => why && sidebar.setNotice(why)).catch(reportError);
  startUpdateChecks((why) => sidebar.setNotice(why));
  // Preview mocks answer unknown commands with null; the 每日分析 row then stays hidden.
  void dailyDir().then((dir) => dir && sidebar.setRoleDir("daily", dir)).catch(reportError);
  const body = document.createElement("div");
  body.className = "app-body";
  const statusbar = document.createElement("footer");
  statusbar.className = "statusbar";
  statusbar.setAttribute("aria-label", "用量与进程监控");
  const usage = document.createElement("div");
  usage.className = "status-usage";
  const limitsBar = new LimitsBar(() => setSidebarHidden(false));
  body.append(sidebar.element, dock);
  usage.append(limitsBar.element);
  statusbar.append(usage, new MemoryBar({
    focusAgent: (id, name) => void focusAgent(id, name).catch(reportError),
    openAgent: (name) => { hideProgress(); workspace.open(name); },
  }).element, new RemoteButton().element, new PowerButton().element);
  app.append(body, statusbar);

  // 进度 covers the task layout without touching it; picking any task or agent brings the layout back.
  const progress: ProgressPage = new ProgressPage(
    (name) => {
      const task = taskOfAgent(getTasks(), name);
      if (task) void focusAgent(task.id, name).catch(reportError);
    },
    () => sidebar.shownProjects().flatMap((p) => p.tasks),
    () => void refreshTaskGoals(true),
  );
  const refreshTaskGoals = progressReader(taskGoalsGet, (goals) => progress.setTaskGoals(goals), reportError);
  const refreshProjectGoals = progressReader(goalsGet, (goals) => progress.setGoals(goals), reportError);
  dock.append(progress.element);
  subscribe(() => progress.setAgents(getAgents()));
  let progressTimer: number | undefined;
  const refreshGoals = () => {
    void refreshProjectGoals();
    void refreshTaskGoals();
  };
  function showProgress(): void {
    progress.element.hidden = false;
    progress.setAgents(getAgents());
    sidebar.setProgressActive(true);
    refreshGoals();
    window.clearInterval(progressTimer);
    progressTimer = window.setInterval(refreshGoals, 5000);
  }
  function hideProgress(): void {
    if (progress.element.hidden) return;
    progress.element.hidden = true;
    sidebar.setProgressActive(false);
    window.clearInterval(progressTimer);
  }

  const undoNotice = document.createElement("div");
  undoNotice.className = "undo-close";
  undoNotice.hidden = true;
  const undoMessage = document.createElement("span");
  undoMessage.setAttribute("role", "status");
  const undoButton = document.createElement("button");
  undoButton.textContent = "撤销";
  undoButton.title = "重新打开关闭的标签（⇧⌘T）";
  const dismissUndo = document.createElement("button");
  dismissUndo.textContent = "×";
  dismissUndo.className = "undo-dismiss";
  dismissUndo.setAttribute("aria-label", "关闭撤销提示");
  undoNotice.append(undoMessage, undoButton, dismissUndo);
  const endNotices = new EndNotices(async (name) => {
    const file = await agentKill(name);
    setTaskFile(file);
    workspace.closePanel(name);
  }, () => workspace.api.activePanel?.view.content.focus?.());
  function endAgent(name: string): void {
    const agent = findAgent(name);
    if (agent) endNotices.schedule(name, agentLabel(agent));
  }
  const notices = document.createElement("div");
  notices.className = "undo-notices";
  notices.append(undoNotice, endNotices.element);
  document.body.append(notices);
  let undoTimer: number | undefined;
  const hideUndo = () => {
    window.clearTimeout(undoTimer);
    undoNotice.hidden = true;
  };
  const expireUndo = () => {
    window.clearTimeout(undoTimer);
    undoTimer = window.setTimeout(hideUndo, 8000);
  };
  undoNotice.addEventListener("mouseenter", () => window.clearTimeout(undoTimer));
  undoNotice.addEventListener("mouseleave", expireUndo);
  undoNotice.addEventListener("focusin", () => window.clearTimeout(undoTimer));
  undoNotice.addEventListener("focusout", expireUndo);
  dismissUndo.addEventListener("click", hideUndo);
  /** File > Open File (Cmd+O): the picked files open as tabs beside the current conversation, or alone with no task. */
  const openPickedFiles = async () => {
    const picked = await openPanel({ multiple: true, directory: false, title: "打开文件", defaultPath: getCurrentTask()?.project });
    const paths = pickedPaths(picked);
    if (paths.length) hideProgress();
    for (const path of paths) workspace.openFile(path, undefined);
  };
  let reopening: Promise<void> = Promise.resolve();
  const reopenClosed = () => {
    hideUndo();
    hideProgress();
    reopening = reopening.then(() => workspace.reopenClosed()).catch(reportError);
  };
  undoButton.addEventListener("click", reopenClosed);

  const workspace = new Workspace(dock, {
    requestAddAgent: (anchor, group) => showAddAgentPopover(anchor, (kind) => addAgent(kind, group)),
    requestSplit: (anchor, group, direction) => split(direction, group, anchor),
    addAgent: (kind) => void addAgent(kind).catch(reportError),
    requestNewTask: (anchor) => requestNewTask(anchor),
    addProject: () => void addProject().catch(reportError),
    activeChanged: (name) => {
      sidebar.setActive(name);
      setActiveAgent(name);
    },
    selectTask,
    endAgent,
    tabClosed: (message) => {
      undoMessage.textContent = message;
      undoNotice.hidden = false;
      expireUndo();
    },
  });

  /** Split a group (the active one by default): its front tab moves over, or a new agent opens on that side. */
  function split(direction: "below" | "right", group = workspace.api.activeGroup, anchor?: HTMLElement): void {
    if (!group || workspace.split(direction, group)) return;
    const at = anchor ?? workspace.addButtonOf(group) ?? sidebar.newButton;
    showAddAgentPopover(at, (kind) => addAgent(kind, group, direction));
  }

  // The sidebar can be hidden (button by the traffic lights, Cmd+B); the choice survives a restart.
  const SIDEBAR_KEY = "shifu.sidebarHidden";
  const sideToggle = document.createElement("button");
  sideToggle.className = "icon-btn side-toggle";
  sideToggle.innerHTML =
    '<svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.3"><rect x="1.5" y="2.5" width="12" height="10" rx="1.8"/><path d="M5.5 2.5v10"/></svg>';
  const lightsDrag = document.createElement("div");
  lightsDrag.className = "lights-drag";
  lightsDrag.dataset.tauriDragRegion = "";
  document.body.append(lightsDrag, sideToggle);
  const setSidebarHidden = (hidden: boolean) => {
    if (hidden) limitsBar.close();
    document.body.classList.toggle("side-hidden", hidden);
    sideToggle.title = hidden ? "显示侧栏  ⌘B" : "隐藏侧栏  ⌘B";
    sideToggle.setAttribute("aria-label", hidden ? "显示侧栏" : "隐藏侧栏");
    localStorage.setItem(SIDEBAR_KEY, hidden ? "1" : "0");
  };
  sideToggle.addEventListener("click", () => setSidebarHidden(!document.body.classList.contains("side-hidden")));
  // With the sidebar hidden the traffic lights and this button sit over the top-left group's tab bar; that group
  // (the first in document order) leaves them room.
  const markCorner = () => {
    dock.querySelectorAll(".under-lights").forEach((g) => g.classList.remove("under-lights"));
    dock.querySelector(".dv-groupview")?.classList.add("under-lights");
  };
  workspace.api.onDidLayoutChange(markCorner);
  markCorner();
  setSidebarHidden(localStorage.getItem(SIDEBAR_KEY) === "1");
  // In full screen macOS draws the menu bar and the title bar strip (traffic lights) over the top of the page, and
  // slides them in and out; the backend reports how much they cover so the sidebar head and tab bars stay visible.
  const applyFullscreen = ({ fullscreen, top }: FullscreenInset) => {
    document.body.classList.toggle("fullscreen", fullscreen);
    document.documentElement.style.setProperty("--fullscreen-top", `${top}px`);
  };
  await listen<FullscreenInset>("fullscreen-inset", ({ payload }) => applyFullscreen(payload));
  fullscreenInset().then(applyFullscreen, reportError);
  // The statusbar is already visible while restoring a task; its sidebar action must be ready first.
  if (file.current) await showTask(file.current);
  // First open: the one-screen guide (docs/ONBOARDING.md, 第 3 阶段). A failed read shows nothing.
  if (shouldShowGuide(file.tasks.length, await guideDoneGet().catch(() => true))) showGuide(createTasks);
  // Otherwise, while tmux or every agent CLI is missing, the setup checklist (the guide has it in step 1).
  else {
    const check = () => {
      const report = getClis();
      if (!report) return;
      stopCheck();
      if (!setupReady(report)) showSetupChecklist();
    };
    const stopCheck = onClisChange(check);
    check();
  }

  await listen<AgentView[]>("agents", async ({ payload }) => {
    // Sessions created by the remote helper join their project's existing task without changing focus.
    if (payload.some(agent => !taskOfAgent(getTasks(), agent.name))) {
      try {setTaskFile(await tasksLoad());} catch (error) {reportError(error);}
    }
    setAgents(payload);
    workspace.reconcile();
  });

  // The backend brings the window forward when a notification is clicked, then names the agent to jump to.
  await listen<{ agent: string }>("notify-jump", ({ payload }) => {
    if (!leaveRemoteController()) return;
    closeMenu();
    closeNewPopover();
    const task = taskOfAgent(getTasks(), payload.agent);
    if (task) void focusAgent(task.id, payload.agent).catch(reportError);
  });
  await listen<string>("notify-error", ({ payload }) => reportError(payload));

  // Cmd+N / Cmd+T / Cmd+W / Cmd+J / Cmd+O come from the native menu (the default Cmd+W would close the window).
  await listen<string>("menu", ({ payload }) => {
    if (handleRemoteControlMenu(payload)) return;
    closeMenu();
    if (payload === "new-task") requestNewTask(sidebar.newButton);
    else if (payload === "new")
      showAddAgentPopover(sidebar.newButton, (kind) => addAgent(kind, workspace.api.activeGroup));
    else if (payload === "close") workspace.closeActive();
    else if (payload === "reopen-closed") reopenClosed();
    else if (payload === "open-file") void openPickedFiles().catch(reportError);
    else if (payload === "split-down") split("below");
    else if (payload === "split-right") split("right");
    else if (payload === "sidebar") sideToggle.click();
    else if (payload === "font-bigger") stepTerminal(1);
    else if (payload === "font-smaller") stepTerminal(-1);
    else if (payload === "font-default") stepTerminal(0);
    else if (payload === "search") {
      setSidebarHidden(false);
      sidebar.focusSearch();
    }
    else if (payload === "appearance") showAppearancePopover(sidebar.newButton);
    else if (payload === "check-update") checkForUpdates();
    else if (payload === "setup-check") {
      void refreshClis().catch(reportError);
      showSetupChecklist();
    }
    else if (payload === "jump") {
      closeNewPopover();
      const first = getWaitingList()[0];
      if (first) void focusAgent(first.task.id, first.agent.name).catch(reportError);
    }
  });
}

main().catch((e) => {
  document.body.textContent = `Shifu 没能启动: ${e instanceof Error ? e.message : String(e)}`;
});
