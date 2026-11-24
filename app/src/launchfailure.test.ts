import { describe, expect, it, vi } from "vitest";

vi.mock("./api", () => ({}));
vi.mock("./store", () => ({ reportError: () => undefined }));

import { bannerView } from "./launchfailure";

describe("bannerView", () => {
  it("is the plain exited banner with resume when the agent did not end at launch", () => {
    expect(bannerView(null)).toEqual({ message: "已退出", action: "resume", actionLabel: "恢复会话", command: null, url: null, output: [] });
  });

  it("explains a launch failure and offers its command and a fresh retry", () => {
    const view = bannerView({
      reason: "not_found",
      message: "没找到 codex 命令，Codex 可能还没装。在终端里运行下面的命令装好，再点「重试」。",
      command: "npm install -g @openai/codex",
      install_url: "https://github.com/openai/codex",
      output: ["zsh:1: command not found: codex"],
    });
    expect(view.action).toBe("retry");
    expect(view.actionLabel).toBe("重试");
    expect(view.command).toBe("npm install -g @openai/codex");
    expect(view.url).toBe("https://github.com/openai/codex");
    expect(view.output).toEqual(["zsh:1: command not found: codex"]);
  });
});
