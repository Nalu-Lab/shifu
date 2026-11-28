/** 权限: the one choice for how every agent starts (`launch::Permissions`), and the first-run question. A fresh
 *  install is asked once, before its first agent starts, with 「按 Agent 自己的设置」 preselected; a user of an
 *  earlier version already has a saved choice and is never asked (see `settle_permissions_in` in src/launch.rs). */
import { type Permissions, permissionsChosen, permissionsSet } from "./api";
import { tr } from "./i18n";

export interface PermissionOption {
  id: Permissions;
  label: string;
  hint: string;
}

/** Recommended first: it is also what an unanswered fresh install gets. In the system language (i18n.ts). */
export const permissionOptions = (): readonly PermissionOption[] => [
  {
    id: "own",
    label: tr("按 Agent 自己的设置", "Agent's own settings"),
    hint: tr("推荐。不加免确认参数，按各家自己的配置决定何时问你。", "Recommended. No bypass flags: each agent's own settings decide when it asks you."),
  },
  {
    id: "bypass",
    label: tr("免确认（全权）", "No confirmation (full access)"),
    hint: tr(
      "风险：改文件、跑命令（包括删除和推送）都不再逐项问你。只在信任的项目里用。",
      "Risk: editing files and running commands (including deletes and pushes) no longer ask you. Use only in projects you trust.",
    ),
  },
];

export const RECOMMENDED: Permissions = "own";

export const hintOf = (id: string): string => permissionOptions().find((o) => o.id === id)?.hint ?? "";

export interface GateDeps {
  chosen: () => Promise<boolean>;
  /** Shows the question; resolves with the answer. */
  ask: () => Promise<Permissions>;
  save: (mode: Permissions) => Promise<void>;
}

/** What runs before an agent starts: asks once when nothing was chosen yet, saves the answer, then lets every
 *  later start through without another look. Starts that arrive while the question is open wait for the same
 *  answer. The plain terminal has nothing to approve, so it never asks. */
export function permissionGate(deps: GateDeps): (kind?: string) => Promise<void> {
  let settled = false;
  let pending: Promise<void> | null = null;
  return (kind) => {
    if (settled || kind === "terminal") return Promise.resolve();
    pending ??= (async () => {
      // Not readable: start anyway; the backend gives an unanswered install "own".
      if (!(await deps.chosen().catch(() => true))) await deps.save(await deps.ask());
      settled = true;
    })().finally(() => {
      pending = null;
    });
    return pending;
  };
}

/** The two choices as radio buttons, each with its line; `onPick` gets every change. */
export function permissionChoice(initial: Permissions, onPick: (mode: Permissions) => void): HTMLElement {
  const box = document.createElement("div");
  box.className = "perm-choice";
  box.setAttribute("role", "radiogroup");
  for (const o of permissionOptions()) {
    const row = document.createElement("label");
    row.className = "perm-option";
    const radio = Object.assign(document.createElement("input"), {
      type: "radio",
      name: "shifu-permissions",
      value: o.id,
      checked: o.id === initial,
    });
    radio.addEventListener("change", () => {
      if (radio.checked) onPick(o.id);
    });
    const text = document.createElement("span");
    const title = Object.assign(document.createElement("span"), { className: "perm-option-label", textContent: o.label });
    const hint = Object.assign(document.createElement("span"), { className: "perm-hint", textContent: o.hint });
    text.append(title, hint);
    row.append(radio, text);
    box.append(row);
  }
  return box;
}

/** The first-run question, over the whole window. Only 继续 closes it: there is no way to start an agent around it. */
export function askPermissions(): Promise<Permissions> {
  return new Promise((resolve) => {
    let picked: Permissions = RECOMMENDED;
    const overlay = document.createElement("div");
    overlay.className = "guide perm-ask";
    const card = document.createElement("div");
    card.className = "guide-card";
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.setAttribute("aria-label", tr("Agent 的权限", "Agent permissions"));
    const go = Object.assign(document.createElement("button"), { type: "button", className: "btn primary", textContent: tr("继续", "Continue") });
    go.addEventListener("click", () => {
      overlay.remove();
      resolve(picked);
    });
    const foot = document.createElement("div");
    foot.className = "guide-foot";
    foot.append(go);
    card.append(
      Object.assign(document.createElement("h1"), { className: "guide-title", textContent: tr("Agent 改文件、跑命令前要问你吗？", "Should agents ask before editing files or running commands?") }),
      Object.assign(document.createElement("p"), {
        className: "guide-lead",
        textContent: tr("对之后所有对话有效，随时可在新建任务的「权限」里改。", "Applies to every conversation from now on. Change it any time under Permissions in New Task."),
      }),
      permissionChoice(picked, (mode) => (picked = mode)),
      foot,
    );
    overlay.append(card);
    document.body.append(overlay);
    go.focus();
  });
}

/** The gate the app installs (main.ts → `setBeforeAgentStart`). */
export const ensurePermissionsChosen = permissionGate({ chosen: permissionsChosen, ask: askPermissions, save: permissionsSet });
