/** Updating Shifu from inside the app: a quiet check soon after launch (at most once a day, `update_check: false`
 *  in settings.json turns it off) and 「检查更新…」 in the Shifu menu. Text follows the system language (i18n.ts). */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isChinese, tr } from "./i18n";

export interface UpdateCheck {
  status: "available" | "latest" | "skipped" | "unsupported";
  current: string;
  version: string | null;
  notes: string | null;
  date: string | null;
  message: string | null;
}

type Progress = { phase: "download"; downloaded: number; total: number | null } | { phase: "install" } | { phase: "restart" };

const LAUNCH_DELAY_MS = 20_000;

/** Release notes are written Chinese first, then English after a `---` line; each language gets its half. */
export function notesFor(notes: string | null, zh: boolean): string {
  if (!notes) return "";
  const parts = notes.split(/\r?\n-{3,}\r?\n/);
  const text = parts.length > 1 ? (zh ? parts[0] : parts.slice(1).join("\n")) : notes;
  return text.trim();
}

export function progressText(p: Progress): string {
  const t = tr;
  if (p.phase === "install") return t("正在安装…", "Installing…");
  if (p.phase === "restart") return t("正在重启…", "Restarting…");
  const mb = (n: number) => (n / 1048576).toFixed(1);
  if (p.total) return t(`正在下载 ${Math.floor((p.downloaded / p.total) * 100)}%（${mb(p.downloaded)} / ${mb(p.total)} MB）`,
    `Downloading ${Math.floor((p.downloaded / p.total) * 100)}% (${mb(p.downloaded)} / ${mb(p.total)} MB)`);
  return t(`正在下载 ${mb(p.downloaded)} MB`, `Downloading ${mb(p.downloaded)} MB`);
}

let card: HTMLElement | null = null;
let installing = false;

function closeCard(): void {
  if (installing) return;
  card?.remove();
  card = null;
}

function button(label: string, primary: boolean, run: () => void): HTMLButtonElement {
  const b = document.createElement("button");
  b.className = primary ? "btn primary" : "btn";
  b.textContent = label;
  b.addEventListener("click", run);
  return b;
}

/** A small card in the bottom-right corner; nothing else is blocked while it shows. */
function showCard(fill: (body: HTMLElement) => void): void {
  card?.remove();
  card = document.createElement("div");
  card.className = "update-card";
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-live", "polite");
  fill(card);
  document.body.append(card);
}

function showMessage(text: string): void {
  showCard((body) => {
    const p = document.createElement("p");
    p.className = "update-message";
    p.textContent = text;
    const row = document.createElement("div");
    row.className = "update-actions";
    row.append(button(tr("好", "OK"), true, closeCard));
    body.append(p, row);
  });
}

function showAvailable(check: UpdateCheck): void {
  const t = tr;
  showCard((body) => {
    const title = document.createElement("div");
    title.className = "update-title";
    title.textContent = t(`Shifu ${check.version} 可以更新了`, `Shifu ${check.version} is available`);
    const sub = document.createElement("div");
    sub.className = "update-sub";
    sub.textContent = t(`现在是 ${check.current}`, `You have ${check.current}`) + (check.date ? ` · ${check.date}` : "");
    body.append(title, sub);
    const notes = notesFor(check.notes, isChinese());
    if (notes) {
      const pre = document.createElement("div");
      pre.className = "update-notes";
      pre.textContent = notes;
      body.append(pre);
    }
    const safe = document.createElement("p");
    safe.className = "update-hint";
    safe.textContent = t("更新时 Shifu 会重启。Agent 对话在 tmux 里继续运行，重启后照常接上，不会中断。",
      "Shifu restarts to update. Agent conversations keep running in tmux and reconnect after the restart.");
    const status = document.createElement("p");
    status.className = "update-status";
    status.hidden = true;
    const bar = document.createElement("progress");
    bar.className = "update-progress";
    bar.hidden = true;
    const row = document.createElement("div");
    row.className = "update-actions";
    const later = button(t("以后", "Later"), false, closeCard);
    const go = button(t("更新并重启", "Update and Restart"), true, () => void install());
    row.append(later, go);
    body.append(safe, bar, status, row);

    const install = async () => {
      installing = true;
      later.disabled = go.disabled = true;
      status.hidden = bar.hidden = false;
      status.classList.remove("error");
      status.textContent = t("正在连接…", "Connecting…");
      const stop = await listen<Progress>("update-progress", ({ payload }) => {
        status.textContent = progressText(payload);
        if (payload.phase === "download" && payload.total) bar.value = payload.downloaded / payload.total;
        else if (payload.phase !== "download") bar.removeAttribute("value");
      });
      try {
        await invoke("update_install");
      } catch (error) {
        installing = false;
        later.disabled = go.disabled = false;
        bar.hidden = true;
        status.classList.add("error");
        status.textContent = t(`更新没有完成：${String(error)}。Shifu 还是原来的版本，可以再试一次。`,
          `The update did not finish: ${String(error)}. Shifu is unchanged; you can try again.`);
        go.textContent = t("再试一次", "Try Again");
      } finally {
        stop();
      }
    };
  });
}

async function check(manual: boolean): Promise<void> {
  if (installing) return;
  const result = await invoke<UpdateCheck | null>("update_check", { manual });
  if (!result) return; // preview mocks
  if (result.status === "available") showAvailable(result);
  else if (manual && result.status === "latest")
    showMessage(tr(`Shifu ${result.current} 已是最新版本。`, `Shifu ${result.current} is up to date.`));
  else if (manual && result.message) showMessage(result.message);
}

/** The menu item: always asks, and says so when there is nothing new or the check failed. */
export function checkForUpdates(): void {
  void check(true).catch((error) => showMessage(String(error)));
}

/** Once after launch; failures stay quiet (the menu item reports them). The Computer Use helper inside this app
 *  replaces an installed older one; `notice` gets a message when that should have happened and could not. */
export function startUpdateChecks(notice: (text: string) => void): void {
  void invoke<string | null>("computer_helper_sync").then((why) => why && notice(why)).catch(() => {});
  window.setTimeout(() => void check(false).catch((error) => console.warn("update check:", error)), LAUNCH_DELAY_MS);
}
