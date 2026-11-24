import { agentResume, agentRetry, clipboardWrite, pathOpen, type LaunchFailure } from "./api";
import { reportError } from "./store";

/** What the exited banner says and offers: plain "已退出" + resume, or why it ended at launch + retry. */
export interface BannerView {
  message: string;
  action: "resume" | "retry";
  actionLabel: string;
  command: string | null;
  url: string | null;
  output: string[];
}

export function bannerView(failure: LaunchFailure | null): BannerView {
  if (!failure) return { message: "已退出", action: "resume", actionLabel: "恢复会话", command: null, url: null, output: [] };
  return {
    message: failure.message,
    action: "retry",
    actionLabel: "重试",
    command: failure.command,
    url: failure.install_url,
    output: failure.output,
  };
}

function button(text: string, className: string, onClick: () => void): HTMLButtonElement {
  const b = Object.assign(document.createElement("button"), { type: "button", className, textContent: text });
  b.addEventListener("click", onClick);
  return b;
}

/** Fills `banner` for agent `name`; `failure` is what `agent_launch_failure` found (null: it did not end at launch). */
export function paintBanner(banner: HTMLElement, name: string, failure: LaunchFailure | null): void {
  const view = bannerView(failure);
  banner.replaceChildren();
  banner.classList.toggle("launch-failed", failure !== null);
  const head = document.createElement("div");
  head.className = "term-banner-head";
  const message = Object.assign(document.createElement("span"), { textContent: view.message });
  head.append(message);
  if (view.url) {
    const url = view.url;
    const link = button("安装说明", "link-btn", () => pathOpen(url).catch(reportError));
    link.title = url;
    head.append(link);
  }
  head.append(button(view.actionLabel, "btn", () => (view.action === "retry" ? agentRetry(name) : agentResume(name)).catch(reportError)));
  banner.append(head);
  if (view.command) {
    const command = view.command;
    const row = document.createElement("div");
    row.className = "term-banner-command";
    const code = Object.assign(document.createElement("code"), { textContent: command });
    const copy = button("复制", "btn", () => {
      clipboardWrite(command).then(() => (copy.textContent = "已复制")).catch(reportError);
    });
    row.append(code, copy);
    banner.append(row);
  }
  if (view.output.length > 0) {
    banner.append(Object.assign(document.createElement("pre"), { className: "term-banner-output", textContent: view.output.join("\n") }));
  }
}
