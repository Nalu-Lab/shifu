import { createCloseButton, DefaultTab } from "dockview";
import type { ITabRenderer, TabPartInitParameters } from "dockview";
import { agentRename, agentsList } from "./api";
import { type AgentView, agentLabel, displayKey, STARTUP_CHECK_SECS, statusHint, statusLabel } from "./format";
import { type MenuItem, showMenu } from "./menu";
import { specOf } from "./models";
import { pinItem, unreadItem } from "./marks";
import { findAgent, getTasks, reportError, setAgents, subscribe } from "./store";
import { taskOfAgent } from "./tasks";

/** What ending does, said the same way here and in the sidebar. */
export const ENDS = "5 秒后结束并移除，期间可撤销";

/** 收起 is offered only where the agent has an open panel. */
export interface AgentMenuHooks {
  rename(): void;
  putAway?: () => void;
  end(): void;
}

/** A working agent takes two clicks to end; putting a panel away never ends it. */
function endItems(agent: AgentView, putAway: (() => void) | undefined, end: () => void): MenuItem[] {
  const who = specOf(agent.kind)?.name ?? "它";
  const working = agent.status === "running" || agent.status === "needs_approval";
  const endIt: MenuItem = working
    ? { label: `结束 ${who}`, hint: "它还在干活", confirmLabel: "确认结束", confirmHint: ENDS, danger: true, run: end }
    : { label: `结束 ${who}`, hint: ENDS, danger: true, run: end };
  return putAway ? [putAwayItem(putAway), endIt] : [endIt];
}

function putAwayItem(run: () => void): MenuItem {
  return { label: "收起，继续运行", hint: "会话留在原任务中，可以撤销收起", run };
}

/** One right-click menu for an agent, the same on its tab and its sidebar row (Eddie 11-23).
 *  Splitting a tab off is done by dragging it (Eddie 11-22 dropped 移到下面/移到右边). */
export function agentMenuItems(name: string, hooks: AgentMenuHooks): MenuItem[] {
  const agent = findAgent(name);
  const items: MenuItem[] = [{ label: "重命名", run: hooks.rename }];
  items.push(...unreadItem(agent), ...pinItem(taskOfAgent(getTasks(), name)?.id));
  if (agent) items.push(...endItems(agent, hooks.putAway, hooks.end));
  else if (hooks.putAway) items.push(putAwayItem(hooks.putAway));
  return items;
}

/** Closing always offers to put the panel away or end its agent .
 *  Cmd+W only drops the panel.
 *  Right-click or double-click renames it; the name is kept on the agent, so the sidebar shows it too. */
export class AgentTab implements ITabRenderer {
  readonly element = document.createElement("div");
  private readonly dot = document.createElement("span");
  private readonly title = document.createElement("div");
  private unsubscribe: (() => void) | null = null;
  private startupTimer: number | undefined;
  private editing = false;

  constructor(private readonly closePanel: (id: string) => void, private readonly endAgent: (name: string) => void) {
    // dv-default-tab brings dockview's own hover/active styling for the close button.
    this.element.className = "dv-default-tab agent-tab";
    this.dot.className = "dot";
    this.title.className = "dv-default-tab-content";
    const close = document.createElement("div");
    close.className = "dv-default-tab-action";
    close.setAttribute("role", "button");
    close.setAttribute("tabindex", "-1");
    close.setAttribute("aria-label", "收起或结束会话");
    close.setAttribute("title", "收起或结束会话");
    close.append(createCloseButton());
    this.element.append(this.dot, this.title, close);
  }

  init(params: TabPartInitParameters): void {
    const name: string = params.params?.agent ?? params.api.id;
    const close = this.element.querySelector(".dv-default-tab-action")!;
    close.addEventListener("pointerdown", (e) => e.preventDefault());
    const end = () => this.endAgent(name);
    const putAway = () => this.closePanel(name);
    close.addEventListener("click", (event) => {
      const e = event as MouseEvent;
      e.preventDefault();
      const agent = findAgent(name);
      if (!agent) return params.api.close();
      showMenu(e.clientX, e.clientY, endItems(agent, putAway, end));
    });
    this.element.addEventListener("dblclick", () => this.startEdit(name));
    this.element.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      showMenu(e.clientX, e.clientY, agentMenuItems(name, { rename: () => this.startEdit(name), putAway, end }));
    });
    const paint = () => {
      window.clearTimeout(this.startupTimer);
      const agent = findAgent(name);
      const status = agent ? displayKey(agent) : "exited";
      this.dot.className = `dot s-${status}`;
      this.title.textContent = agent ? agentLabel(agent) : name;
      // The session name stays reachable for anyone who needs it (`shifu attach`, logs).
      const hint = statusHint(status);
      this.element.title = hint ? `${name}\n${statusLabel(status)}：${hint}` : name;
      // The backend only emits when agents change; this deadline must also paint without a new callback.
      if (agent && status === "starting")
        this.startupTimer = window.setTimeout(paint, (agent.status_since + STARTUP_CHECK_SECS) * 1000 - Date.now());
    };
    this.unsubscribe = subscribe(paint);
    paint();
  }

  /** Rename in place. Enter confirms, Esc cancels; the form lets WebKit tell a Pinyin pick from a plain Enter. */
  private startEdit(name: string): void {
    if (this.editing) return;
    this.editing = true;
    const shown = this.title.textContent ?? "";
    const form = document.createElement("form");
    form.className = "inline-form";
    const input = document.createElement("input");
    input.className = "task-edit";
    input.value = shown;
    form.append(input);
    // While typing, the tab must not be dragged or let dockview take the pointer (selecting text in the input).
    const dragHost = this.element.closest<HTMLElement>("[draggable]");
    const wasDraggable = dragHost?.draggable ?? false;
    if (dragHost) dragHost.draggable = false;
    for (const type of ["pointerdown", "mousedown", "dblclick", "contextmenu"])
      input.addEventListener(type, (e) => e.stopPropagation());
    this.title.replaceWith(form);
    input.focus();
    input.select();
    let done = false;
    const finish = (save: boolean) => {
      if (done) return;
      done = true;
      this.editing = false;
      if (dragHost) dragHost.draggable = wasDraggable;
      const label = input.value.trim();
      form.replaceWith(this.title);
      if (save && label && label !== shown) {
        this.title.textContent = label;
        void renameAgent(name, label);
      }
    };
    form.addEventListener("submit", (e) => {
      e.preventDefault();
      finish(true);
    });
    input.addEventListener("keydown", (e) => {
      e.stopPropagation();
      if (e.key === "Escape" && !e.isComposing) finish(false);
    });
    input.addEventListener("blur", () => finish(true));
  }

  dispose(): void {
    this.unsubscribe?.();
    window.clearTimeout(this.startupTimer);
  }
}

/** Keep Dockview's file tab, routing its close button through the same undo history. */
export class FileTab extends DefaultTab {
  constructor(private readonly closePanel: (id: string) => void) {
    super();
  }

  override init(params: TabPartInitParameters): void {
    super.init(params);
    this.element.querySelector(".dv-default-tab-action")!.addEventListener("click", (event) => {
      event.preventDefault();
      this.closePanel(params.api.id);
    }, true);
  }
}

/** Saves the name on the agent and reloads the list, so every place that shows the agent changes at once. */
export async function renameAgent(name: string, label: string): Promise<void> {
  try {
    await agentRename(name, label);
    setAgents(await agentsList());
  } catch (e) {
    reportError(e);
  }
}

