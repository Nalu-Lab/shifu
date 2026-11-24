import type { BotId, BotProfile, ResidentBot } from "./api";
import { residentPortrait } from "./botportraits";
import { catPortrait } from "./catart";
import { catOptions, randomAppearance, type CatAppearance } from "./cattraits";
import { getClis, splitAgents } from "./models";
import { agentDropdown, closeNewPopover, present } from "./newpopover";
import "./botpicker.css";

/** A built-in resident role (项目经理, 每日分析): it can be taken off 我的 Bot and put back, never edited. */
export interface PickerRole {
  id: string;
  name: string;
  purpose: string;
  /** SVG markup of the role's sidebar icon. */
  icon: string;
  joined: boolean;
}

interface BotPickerActions {
  join(id: BotId): Promise<readonly ResidentBot[]>;
  leave(id: BotId): Promise<readonly ResidentBot[]>;
  setRole(id: string, joined: boolean): readonly PickerRole[];
  openRole(id: string, anchor: HTMLElement): void;
  save(id: BotId, profile: BotProfile): Promise<readonly ResidentBot[]>;
  chat(id: BotId, anchor: HTMLElement): void;
}

/** Which agent a bot's conversation runs; asked on first open when unset. */
export const botKindKey = (id: BotId) => `shifu.botKind.${id}`;

interface Draft {
  id: BotId;
  joined: boolean;
  /** The agent its conversation runs, saved with the profile. */
  kind: string;
  fixedPortrait?: string;
  profile: BotProfile;
}

function button(text: string, className = "btn"): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  element.className = className;
  element.textContent = text;
  return element;
}

/** Choose what is on 我的 Bot: join or remove a companion or role, or edit a cat while keeping its conversation.
 *  `adding` opens on 添加 Bot. */
export function showBotPicker(anchor: HTMLElement, bots: readonly ResidentBot[], builtIn: readonly PickerRole[],
  actions: BotPickerActions, adding = false): void {
  const pop = document.createElement("div");
  pop.className = "popup bot-picker";
  pop.setAttribute("role", "dialog");
  pop.setAttribute("aria-label", "我的 Bot");
  pop.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !event.isComposing) anchor.focus();
  });
  let roster = bots;
  let roles = builtIn;
  let showingJoined = !adding && (roster.some((bot) => bot.joined) || roles.some((role) => role.joined));
  let draft: Draft | null = null;
  let busy = false;
  let error = "";
  const tabs = document.createElement("div");
  tabs.className = "bot-picker-tabs";
  const content = document.createElement("div");
  content.className = "bot-picker-content";
  const feedback = document.createElement("div");
  feedback.className = "bot-picker-error";
  feedback.setAttribute("role", "alert");

  const showError = () => {
    feedback.textContent = error;
    feedback.hidden = !error;
  };
  const open = (id: BotId) => {
    closeNewPopover();
    actions.chat(id, anchor);
  };
  const begin = (bot: ResidentBot) => {
    draft = {
      id: bot.id,
      joined: bot.joined,
      kind: localStorage.getItem(botKindKey(bot.id)) ?? "claude",
      fixedPortrait: bot.appearance === null ? residentPortrait(bot) : undefined,
      profile: {
        name: bot.name,
        purpose: bot.purpose,
        appearance: bot.appearance && { ...bot.appearance },
      },
    };
    error = "";
    render();
    content.querySelector<HTMLInputElement>("input")?.focus();
  };

  const renderEditor = (current: Draft) => {
    const form = document.createElement("form");
    form.className = "cat-editor";
    const toolbar = document.createElement("div");
    toolbar.className = "cat-editor-toolbar";
    const back = button("返回", "bot-picker-tab");
    back.addEventListener("click", () => {
      draft = null;
      error = "";
      render();
      tabs.querySelector<HTMLButtonElement>('[aria-pressed="true"]')?.focus();
    });
    const title = document.createElement("span");
    title.className = "cat-editor-title";
    title.textContent = `${current.profile.name}的设置`;
    toolbar.append(back, title);
    const portrait = document.createElement("img");
    portrait.className = "cat-editor-portrait";
    portrait.alt = current.fixedPortrait ? `${current.profile.name}的头像` : "猫咪外观";
    portrait.draggable = false;
    if (current.fixedPortrait) portrait.src = current.fixedPortrait;
    const hero = document.createElement("div");
    hero.className = "cat-editor-hero";
    const fields = document.createElement("div");
    fields.className = "cat-editor-fields";
    const nameLabel = document.createElement("label");
    nameLabel.className = "cat-editor-field";
    nameLabel.append("名字");
    const name = document.createElement("input");
    name.type = "text";
    name.name = "cat-name";
    name.value = current.profile.name;
    name.required = true;
    name.maxLength = 48;
    name.autocomplete = "off";
    name.addEventListener("input", () => { name.setCustomValidity(""); current.profile.name = name.value; });
    nameLabel.append(name);
    const purposeLabel = document.createElement("label");
    purposeLabel.className = "cat-editor-field";
    purposeLabel.append("职责");
    const purpose = document.createElement("textarea");
    purpose.name = "cat-purpose";
    purpose.value = current.profile.purpose;
    purpose.required = true;
    purpose.maxLength = 320;
    purpose.rows = 2;
    purpose.addEventListener("input", () => { purpose.setCustomValidity(""); current.profile.purpose = purpose.value; });
    purposeLabel.append(purpose);
    const agentLabel = document.createElement("div");
    agentLabel.className = "cat-editor-field";
    agentLabel.append("使用的 agent");
    const agents = splitAgents(getClis()).installed.filter((spec) => spec.kind !== "terminal");
    agentLabel.append(agentDropdown(agents, current.kind, (picked) => { current.kind = picked; }));
    fields.append(nameLabel, purposeLabel, agentLabel);
    hero.append(portrait, fields);
    form.append(toolbar, hero);
    const footer = document.createElement("div");
    footer.className = "cat-editor-footer";
    const appearance = current.profile.appearance;
    if (appearance) {
      const refreshPortrait = () => { portrait.src = catPortrait(appearance, "full"); };
      refreshPortrait();
      const options = document.createElement("div");
      options.className = "cat-editor-options";
      const choices: { key: keyof CatAppearance; label: string }[] = [
        { key: "coat", label: "毛色" }, { key: "ears", label: "耳朵" },
        { key: "tail", label: "尾巴" }, { key: "eyes", label: "眼睛" },
      ];
      const updateOptions = () => {
        options.querySelectorAll<HTMLButtonElement>("button[data-trait]").forEach((choice) => {
          const key = choice.dataset.trait as keyof CatAppearance;
          choice.setAttribute("aria-pressed", String(appearance[key] === choice.dataset.value));
        });
      };
      for (const { key, label } of choices) {
        const group = document.createElement("fieldset");
        group.className = "cat-editor-choice";
        const legend = document.createElement("legend");
        legend.textContent = label;
        group.append(legend);
        const row = document.createElement("div");
        row.className = "cat-editor-choice-row";
        for (const option of catOptions[key]) {
          const choice = button(option.label, "cat-trait-option");
          if (key === "coat" || key === "eyes") {
            const swatch = document.createElement("span");
            swatch.className = "cat-trait-swatch";
            swatch.dataset.trait = key;
            swatch.dataset.value = option.value;
            swatch.setAttribute("aria-hidden", "true");
            choice.prepend(swatch);
          }
          choice.dataset.trait = key;
          choice.dataset.value = option.value;
          choice.addEventListener("click", () => {
            // Each option comes from the schema-owned list for this trait.
            Object.assign(appearance, { [key]: option.value });
            updateOptions();
            refreshPortrait();
          });
          row.append(choice);
        }
        group.append(row);
        options.append(group);
      }
      updateOptions();
      const random = button("随机搭配", "bot-picker-tab");
      random.addEventListener("click", () => {
        Object.assign(appearance, randomAppearance(appearance));
        updateOptions();
        refreshPortrait();
      });
      form.append(options);
      footer.append(random);
    }
    const cancel = button("取消", "bot-picker-tab");
    cancel.addEventListener("click", () => back.click());
    const save = button(current.joined ? "保存" : "加入");
    save.type = "submit";
    save.classList.add("cat-editor-save");
    footer.append(cancel, save);
    if (current.joined) {
      const chat = button("打开对话", "btn bot-picker-action");
      chat.type = "button";
      chat.addEventListener("click", () => {
        localStorage.setItem(botKindKey(current.id), current.kind);
        open(current.id);
      });
      footer.append(chat);
    }
    form.append(footer);
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      if (busy) return;
      name.setCustomValidity(name.value.trim().length === 0 || [...name.value.trim()].length > 24 ? "名字需要 1–24 个字" : "");
      purpose.setCustomValidity(purpose.value.trim().length === 0 || [...purpose.value.trim()].length > 160 ? "职责需要 1–160 个字" : "");
      if (!form.reportValidity()) return;
      current.profile.name = name.value.trim();
      current.profile.purpose = purpose.value.trim();
      busy = true;
      error = "";
      showError();
      pop.querySelectorAll<HTMLButtonElement | HTMLInputElement | HTMLTextAreaElement>("button, input, textarea").forEach((control) => { control.disabled = true; });
      save.textContent = current.joined ? "保存中…" : "加入中…";
      try {
        roster = await actions.save(current.id, current.profile);
        localStorage.setItem(botKindKey(current.id), current.kind);
        // Keep a newly added bot's settings open so the user can start a conversation.
        draft = current.joined ? null : { ...current, joined: true };
        showingJoined = true;
      } catch (reason) {
        error = String(reason);
      } finally {
        busy = false;
        render();
        if (pop.isConnected) {
          if (draft) content.querySelector<HTMLInputElement>("input")?.focus();
          else content.querySelector<HTMLButtonElement>(`[data-bot="${current.id}"] button`)?.focus();
        }
      }
    });
    content.append(form);
  };

  const render = () => {
    tabs.replaceChildren();
    for (const [title, joined] of [["我的 Bot", true], ["添加 Bot", false]] as const) {
      const tab = button(title, "bot-picker-tab");
      tab.setAttribute("aria-pressed", String(showingJoined === joined));
      tab.disabled = busy;
      tab.addEventListener("click", () => {
        showingJoined = joined;
        draft = null;
        error = "";
        render();
        tabs.querySelector<HTMLButtonElement>('[aria-pressed="true"]')?.focus();
      });
      tabs.append(tab);
    }
    content.replaceChildren();
    pop.classList.toggle("editing", draft !== null);
    if (draft) {
      renderEditor(draft);
      showError();
      return;
    }
    const visible = roster.filter((bot) => bot.joined === showingJoined);
    if (!showingJoined && visible.length > 0) {
      const create = document.createElement("div");
      create.className = "bot-picker-create";
      const random = button("随机动物");
      random.disabled = busy;
      random.addEventListener("click", () => begin(visible[Math.floor(Math.random() * visible.length)]));
      create.append(random);
      content.append(create);
    }
    const list = document.createElement("div");
    list.className = "bot-picker-list";
    const visibleRoles = roles.filter((role) => role.joined === showingJoined);
    for (const role of visibleRoles) {
      const row = document.createElement("div");
      row.className = "bot-picker-row";
      row.dataset.bot = role.id;
      const icon = document.createElement("span");
      icon.className = "bot-picker-role-icon";
      icon.innerHTML = role.icon;
      const description = document.createElement("div");
      description.className = "bot-picker-description";
      const name = document.createElement("div");
      name.className = "bot-picker-name";
      name.textContent = role.name;
      const purpose = document.createElement("div");
      purpose.className = "bot-picker-purpose";
      purpose.textContent = role.purpose;
      description.append(name, purpose);
      const controls = document.createElement("div");
      controls.className = "bot-picker-row-actions";
      const toggle = button(role.joined ? "移除" : "加入", role.joined ? "bot-picker-tab" : "btn bot-picker-action");
      toggle.disabled = busy;
      toggle.setAttribute("aria-label", `${role.joined ? "移除" : "加入"}${role.name}`);
      toggle.addEventListener("click", () => {
        roles = actions.setRole(role.id, !role.joined);
        render();
      });
      controls.append(toggle);
      if (role.joined) {
        const chat = button("打开对话", "btn bot-picker-action");
        chat.disabled = busy;
        chat.addEventListener("click", () => {
          closeNewPopover();
          actions.openRole(role.id, anchor);
        });
        controls.append(chat);
      }
      row.append(icon, description, controls);
      list.append(row);
    }
    if (showingJoined && visible.length === 0 && visibleRoles.length === 0) {
      const empty = document.createElement("p");
      empty.className = "bot-picker-empty";
      empty.textContent = "还没有 Bot";
      const add = button("添加 Bot");
      add.addEventListener("click", () => {
        showingJoined = false;
        render();
        content.querySelector<HTMLButtonElement>("button")?.focus();
      });
      list.append(empty, add);
    }
    if (!showingJoined && visible.length === 0 && visibleRoles.length === 0) {
      const empty = document.createElement("p");
      empty.className = "bot-picker-empty";
      empty.textContent = "暂时没有可添加的 Bot";
      list.append(empty);
    }
    for (const bot of visible) {
      const row = document.createElement("div");
      row.className = "bot-picker-row";
      row.dataset.bot = bot.id;
      const portrait = document.createElement("img");
      portrait.src = residentPortrait(bot);
      portrait.alt = "";
      portrait.draggable = false;
      const description = document.createElement("div");
      description.className = "bot-picker-description";
      const name = document.createElement("div");
      name.className = "bot-picker-name";
      name.textContent = bot.name;
      const purpose = document.createElement("div");
      purpose.className = "bot-picker-purpose";
      purpose.textContent = bot.purpose;
      description.append(name, purpose);
      const controls = document.createElement("div");
      controls.className = "bot-picker-row-actions";
      const edit = button(bot.joined ? "编辑" : "自定义", "bot-picker-tab");
      edit.disabled = busy;
      edit.setAttribute("aria-label", `${bot.joined ? "编辑" : "自定义"}${bot.name}`);
      edit.addEventListener("click", () => begin(bot));
      const action = button(bot.joined ? "打开对话" : "加入", "btn bot-picker-action");
      action.disabled = busy;
      if (bot.joined) {
        action.addEventListener("click", () => open(bot.id));
      } else {
        action.addEventListener("click", async () => {
          if (busy) return;
          busy = true;
          error = "";
          render();
          let joined: ResidentBot | undefined;
          try {
            roster = await actions.join(bot.id);
            showingJoined = true;
            joined = roster.find((candidate) => candidate.id === bot.id);
          } catch (reason) {
            error = String(reason);
          }
          busy = false;
          if (joined) begin(joined);
          else render();
        });
      }
      controls.append(edit);
      if (bot.joined) {
        const remove = button("移除", "bot-picker-tab");
        remove.disabled = busy;
        remove.setAttribute("aria-label", `移除${bot.name}`);
        remove.addEventListener("click", async () => {
          if (busy) return;
          busy = true;
          error = "";
          render();
          try {
            roster = await actions.leave(bot.id);
          } catch (reason) {
            error = String(reason);
          } finally {
            busy = false;
            render();
          }
        });
        controls.append(remove);
      }
      controls.append(action);
      row.append(portrait, description, controls);
      list.append(row);
    }
    content.append(list);
    showError();
  };
  render();
  pop.append(tabs, content, feedback);
  if (present(anchor, "resident-bots", pop, { suspended: false })) {
    tabs.querySelector<HTMLButtonElement>('[aria-pressed="true"]')?.focus();
  }
}
