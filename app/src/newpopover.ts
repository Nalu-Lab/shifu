import { open } from "@tauri-apps/plugin-dialog";
import { type Permissions, pathOpen, permissionsGet, permissionsSet, recentDirs } from "./api";
import { pickFolder, projectName } from "./tasks";
import { shortPath } from "./format";
import { addPickedFolder, reportError } from "./store";
import {
  AGENTS,
  type AgentSpec,
  defaultKind,
  effortLabel,
  effortsFor,
  getClis,
  installUrl,
  modelsOf,
  pickFor,
  savePick,
  specOf,
  splitAgents,
} from "./models";
import { vendorIcon } from "./vendoricons";

let kind = "claude";
let dismiss: (() => void) | null = null;
let anchorOpen: { anchor: HTMLElement; which: string } | null = null;

export function closeNewPopover(): void {
  dismiss?.();
}

function button(text: string, className: string): HTMLButtonElement {
  const b = document.createElement("button");
  b.className = className;
  b.textContent = text;
  return b;
}

function label(text: string): HTMLElement {
  const l = document.createElement("div");
  l.className = "pop-label";
  l.textContent = text;
  return l;
}

/** Mark and name, as on the agent buttons and the "+" menu. */
function agentLabel(spec: AgentSpec): Element[] {
  const name = document.createElement("span");
  name.textContent = spec.name;
  return [vendorIcon(spec.kind, "agent-mark"), name];
}

/** A native select (one arrow drawn by CSS); the first option, "", is the CLI's own default. */
function select(options: { id: string; label: string }[], value: string, onChange: (v: string) => void): HTMLSelectElement {
  const sel = document.createElement("select");
  sel.className = "pick-select";
  for (const o of [{ id: "", label: "默认" }, ...options]) sel.append(new Option(o.label, o.id, false, o.id === value));
  sel.addEventListener("change", () => onChange(sel.value));
  return sel;
}

/** "未安装：Kimi · Qwen Code": each name opens its install page. Empty (null) when everything is installed. */
export function missingLine(missing: readonly AgentSpec[]): HTMLElement | null {
  if (missing.length === 0) return null;
  const line = document.createElement("div");
  line.className = "missing-agents";
  line.append("未安装：");
  missing.forEach((spec, i) => {
    if (i > 0) line.append(" · ");
    const link = button(spec.name, "link-btn");
    const url = installUrl(getClis(), spec.kind);
    link.title = url ? `安装说明：${url}` : "";
    link.disabled = !url;
    link.addEventListener("click", () => {
      if (url) pathOpen(url).catch(reportError);
    });
    line.append(link);
  });
  return line;
}

/** One button showing the chosen agent's mark and name; it drops down the others, each with its mark. Eight
 *  agents no longer fit a row of buttons (Eddie 11-22: 下拉选择). */
export function agentDropdown(specs: readonly AgentSpec[], current: string, onPick: (kind: string) => void): HTMLElement {
  const box = document.createElement("div");
  box.className = "agent-dropdown";
  const toggle = button("", "agent-dropdown-button");
  toggle.type = "button";
  toggle.setAttribute("aria-haspopup", "listbox");
  const list = document.createElement("div");
  list.className = "agent-dropdown-list";
  list.setAttribute("role", "listbox");
  list.hidden = true;
  let chosen = current;
  const setOpen = (open: boolean) => {
    list.hidden = !open;
    toggle.setAttribute("aria-expanded", String(open));
    if (open) list.querySelector<HTMLElement>('[aria-selected="true"]')?.focus();
  };
  const paint = () => {
    const spec = specs.find((s) => s.kind === chosen) ?? specs[0];
    toggle.replaceChildren(...(spec ? agentLabel(spec) : []));
    list.replaceChildren(...specs.map((option) => {
      const item = button("", "agent-dropdown-item");
      item.type = "button";
      item.setAttribute("role", "option");
      item.setAttribute("aria-selected", String(option.kind === spec?.kind));
      item.append(...agentLabel(option));
      item.addEventListener("click", () => {
        chosen = option.kind;
        setOpen(false);
        paint();
        toggle.focus();
        onPick(option.kind);
      });
      return item;
    }));
  };
  toggle.addEventListener("click", () => setOpen(list.hidden));
  box.addEventListener("keydown", (event) => {
    if (list.hidden) return;
    const items = Array.from(list.querySelectorAll<HTMLElement>(".agent-dropdown-item"));
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (event.key === "Escape") {
      // Closes the list only, not the popover around it.
      event.stopPropagation();
      setOpen(false);
      toggle.focus();
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      items[(at + (event.key === "ArrowDown" ? 1 : items.length - 1)) % items.length]?.focus();
    }
  });
  box.addEventListener("focusout", (event) => {
    if (!box.contains(event.relatedTarget as Node | null)) setOpen(false);
  });
  paint();
  setOpen(false);
  box.append(toggle, list);
  return box;
}

/** The agent dropdown for the agents installed here, then model and effort for the chosen one; the missing ones are
 *  named under them with their install pages. Choices are remembered per agent. */
function agentPicker(): HTMLElement {
  const box = document.createElement("div");
  const picks = document.createElement("div");
  picks.className = "pick-row";
  const { installed, missing } = splitAgents(getClis());
  kind = defaultKind(getClis(), kind);
  const field = (text: string, control: HTMLElement) => {
    const f = document.createElement("label");
    f.className = "pick-field";
    const t = document.createElement("span");
    t.textContent = text;
    const wrap = document.createElement("span");
    wrap.className = "select-wrap";
    wrap.append(control);
    f.append(t, wrap);
    return f;
  };
  const paint = () => {
    const spec = specOf(kind) ?? AGENTS[0];
    picks.hidden = spec.kind === "terminal";
    if (picks.hidden) {
      picks.replaceChildren();
      return;
    }
    const models = modelsOf(spec.kind);
    const pick = pickFor(spec.kind);
    const efforts = effortsFor(models, pick.model).map((id) => ({ id, label: effortLabel(id) }));
    picks.replaceChildren(
      field("模型", select(models, pick.model, (model) => {
        savePick(spec.kind, { ...pickFor(spec.kind), model });
        paint(); // the levels on offer depend on the model
      })),
      field("思考", select(efforts, pick.effort, (effort) => savePick(spec.kind, { ...pickFor(spec.kind), effort }))),
    );
  };
  paint();
  const type = document.createElement("div");
  type.className = "agent-type-row";
  type.append(agentDropdown(installed, kind, (picked) => {
    kind = picked;
    paint();
  }));
  box.append(type, picks);
  const line = missingLine(missing);
  if (line) box.append(line);
  return box;
}

/** Shows `pop` under `anchor` until Esc, a click elsewhere, or the window losing focus. Clicking the anchor again
 *  closes it (`which` tells popovers of one anchor apart). Returns false when it just closed that popover. */
export function present(anchor: HTMLElement, which: string, pop: HTMLElement, blurGuard: { suspended: boolean }): boolean {
  const wasOpenHere = anchorOpen?.anchor === anchor && anchorOpen.which === which;
  closeNewPopover();
  if (wasOpenHere) return false;
  anchorOpen = { anchor, which };
  document.body.append(pop);
  // Placed again whenever it changes size: the new-task list fills in after it opens, and a placement measured
  // before that pushed the 建立 button below the window (Eddie 11-23).
  const place = () => {
    const rect = anchor.getBoundingClientRect();
    const { width, height } = pop.getBoundingClientRect();
    pop.style.left = `${Math.max(4, Math.min(rect.left, window.innerWidth - width - 4))}px`;
    pop.style.top = `${Math.max(4, Math.min(rect.bottom + 4, window.innerHeight - height - 4))}px`;
  };
  place();
  const resized = new ResizeObserver(place);
  resized.observe(pop);
  window.addEventListener("resize", place);

  const onPointer = (e: PointerEvent) => {
    if (!pop.contains(e.target as Node) && !anchor.contains(e.target as Node)) closeNewPopover();
  };
  const onKey = (e: KeyboardEvent) => {
    // Esc while picking a Pinyin candidate cancels the candidate, not the popover.
    if (e.key === "Escape" && !e.isComposing) closeNewPopover();
  };
  const onBlur = () => {
    if (!blurGuard.suspended) closeNewPopover();
  };
  window.addEventListener("pointerdown", onPointer, true);
  window.addEventListener("keydown", onKey, true);
  window.addEventListener("blur", onBlur);
  dismiss = () => {
    window.removeEventListener("pointerdown", onPointer, true);
    window.removeEventListener("keydown", onKey, true);
    window.removeEventListener("blur", onBlur);
    window.removeEventListener("resize", place);
    resized.disconnect();
    pop.remove();
    anchorOpen = null;
    dismiss = null;
  };
  return true;
}

const PERMISSION_OPTIONS: { id: Permissions; label: string; hint: string }[] = [
  { id: "bypass", label: "免确认（全权）", hint: "改文件、跑命令都不逐项问你。只在信任的项目里用。" },
  { id: "own", label: "用 Agent 自己的设置", hint: "不加免确认参数，按各家自己的配置决定何时问你。" },
];

/** 权限: one choice for every agent started or resumed from now on (`launch::Permissions`). Filled in once the
 *  saved choice is read; a change is saved at once. */
function permissionPicker(): HTMLElement {
  const box = document.createElement("div");
  box.className = "perm-picker";
  const sel = document.createElement("select");
  sel.className = "pick-select";
  sel.disabled = true;
  for (const o of PERMISSION_OPTIONS) sel.append(new Option(o.label, o.id));
  const wrap = document.createElement("span");
  wrap.className = "select-wrap";
  wrap.append(sel);
  const hint = document.createElement("div");
  hint.className = "perm-hint";
  const paint = () => {
    hint.textContent = PERMISSION_OPTIONS.find((o) => o.id === sel.value)?.hint ?? "";
  };
  sel.addEventListener("change", () => {
    paint();
    permissionsSet(sel.value as Permissions).catch(reportError);
  });
  permissionsGet()
    .then((mode) => {
      sel.value = mode;
      sel.disabled = false;
      paint();
    })
    .catch(reportError);
  box.append(wrap, hint);
  return box;
}

type AddAgent = (kind: string) => Promise<void>;

/** Cmd+T / the "+" on a group: the supported agent CLIs, started in the current task's folder. */
export function showAddAgentPopover(anchor: HTMLElement, add: AddAgent, options: { agentsOnly?: boolean } = {}): void {
  const pop = document.createElement("div");
  pop.className = "popup add-pop";
  const { installed, missing } = splitAgents(getClis());
  for (const spec of installed) {
    if (options.agentsOnly && spec.kind === "terminal") continue;
    const item = button("", "popup-item agent-item");
    const model = modelsOf(spec.kind).find((m) => m.id === pickFor(spec.kind).model)?.label ?? "默认模型";
    const hint = document.createElement("span");
    hint.className = "agent-hint";
    hint.textContent = spec.kind === "terminal" ? "" : model;
    item.append(...agentLabel(spec), hint);
    item.addEventListener("click", () => {
      closeNewPopover();
      add(spec.kind).catch(reportError);
    });
    pop.append(item);
  }
  const line = missingLine(missing.filter((spec) => !(options.agentsOnly && spec.kind === "terminal")));
  if (line) pop.append(line);
  present(anchor, "add-agent", pop, { suspended: false });
}

type CreateTask = (project: string, name: string, kind: string) => Promise<void>;

export interface NewTaskOptions {
  /** Folders that already have tasks, then those picked earlier that have none yet. */
  projects: readonly string[];
  /** Folder to start with: the "+" of a project row, else the current task's. */
  preselect?: string;
  create: CreateTask;
}

/** Project, task name (may stay empty), the supported agent CLIs. Enter or "建立" creates. */
export async function showNewTaskPopover(anchor: HTMLElement, opts: NewTaskOptions): Promise<void> {
  const guard = { suspended: false };
  const pop = document.createElement("div");
  pop.className = "popup new-pop";

  let folders = [...opts.projects];
  let selected = opts.preselect ?? folders[0] ?? null;
  // The "+" of a folder that is not in the list (one picked earlier, with no task yet) still starts selected.
  if (selected !== null && !folders.includes(selected)) folders.unshift(selected);
  // A dropdown, not a list: the list of recent folders pushed the rest of the form down (Eddie 11-23).
  const CHOOSE = "\u0000choose";
  const list = document.createElement("select");
  list.className = "pick-select";
  list.setAttribute("aria-label", "项目");
  const paintList = () => {
    const names = folders.map(projectName);
    list.replaceChildren(
      ...folders.map((dir, i) => {
        // Two folders with one name are told apart by their path.
        const clash = names.indexOf(names[i]) !== names.lastIndexOf(names[i]);
        const option = new Option(clash ? `${names[i]}（${shortPath(dir)}）` : names[i], dir, false, dir === selected);
        option.title = dir;
        return option;
      }),
      new Option("选择文件夹…", CHOOSE),
    );
    list.value = selected ?? CHOOSE;
    list.title = selected ?? "";
  };
  const listWrap = document.createElement("span");
  listWrap.className = "select-wrap dir-select";
  listWrap.append(list);

  const chooseFolder = async () => {
    // The system dialog takes focus, which would close the popover through the blur handler.
    guard.suspended = true;
    try {
      const dir = await open({ directory: true, multiple: false, title: "选择文件夹" });
      if (typeof dir === "string") {
        // The picked folder becomes the task's folder, first in the list, and shows in the sidebar from now on.
        ({ folders, selected } = pickFolder(folders, dir));
        addPickedFolder(dir);
      }
    } catch (e) {
      reportError(e);
    } finally {
      guard.suspended = false;
      paintList();
      input.focus();
    }
  };
  list.addEventListener("change", () => {
    if (list.value === CHOOSE) chooseFolder();
    else {
      selected = list.value;
      list.title = selected;
    }
  });

  const input = document.createElement("input");
  input.className = "text-input";
  input.placeholder = "不填就叫「任务 N」";
  input.setAttribute("aria-label", "任务名");

  const submit = document.createElement("button");
  submit.className = "btn primary";
  submit.textContent = "建立";
  const create = () => {
    if (!selected) return reportError("先选一个项目文件夹");
    const [project, name] = [selected, input.value];
    closeNewPopover();
    opts.create(project, name, kind).catch(reportError);
  };
  submit.addEventListener("click", create);
  // Enter in the name field submits this form; WebKit does not submit for the Enter that picks a Pinyin candidate.
  const nameForm = document.createElement("form");
  nameForm.className = "inline-form";
  nameForm.append(input);
  nameForm.addEventListener("submit", (e) => {
    e.preventDefault();
    create();
  });

  const foot = document.createElement("div");
  foot.className = "pop-foot";
  foot.append(submit);
  pop.append(
    label("项目"),
    listWrap,
    label("任务名"),
    nameForm,
    label("类型"),
    agentPicker(),
    label("权限（所有对话）"),
    permissionPicker(),
    foot,
  );

  paintList();
  if (!present(anchor, "new-task", pop, guard)) return;
  input.focus();

  // Folders used before that are not projects yet.
  try {
    const recent = await recentDirs();
    if (dismiss === null) return;
    for (const dir of recent) if (!folders.includes(dir)) folders.push(dir);
    selected ??= folders[0] ?? null;
    paintList();
  } catch (e) {
    reportError(e);
  }
}
