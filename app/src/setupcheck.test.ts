import { describe, expect, it } from "vitest";
import type { CliFound, CliReport } from "./api";
import { actionText, installItems, pageOnly, setupChecks, setupReady, signInItems, tabLabel } from "./setupcheck";

const agent = (kind: string, found: boolean, extra: Partial<CliFound> = {}): CliFound => ({
  kind,
  name: kind,
  path: found ? `/bin/${kind}` : null,
  install_url: `https://example.com/${kind}`,
  install_command: `npm i -g ${kind}`,
  login_command: null,
  signed_in: null,
  ...extra,
});
const report = (tmux: boolean, agents: CliFound[], more: Partial<CliReport> = {}): CliReport => ({
  agents,
  tmux: tmux ? "/bin/tmux" : null,
  npm: "/bin/npm",
  shell_read: true,
  ...more,
});
const head = (r: CliReport) => setupChecks(r).slice(0, -3);

describe("setup checklist", () => {
  it("is ready with tmux and one agent; sign-in does not hold it back", () => {
    expect(setupReady(null)).toBe(true);
    expect(setupReady(report(true, [agent("claude", true, { signed_in: false })]))).toBe(true);
    expect(setupReady(report(false, [agent("claude", true)]))).toBe(false);
    expect(setupReady(report(true, [agent("claude", false)]))).toBe(false);
  });

  it("ticks the tmux Shifu carries; only a build without it asks for brew", () => {
    expect(setupChecks(report(true, [], { tmux_bundled: true }))[0]).toMatchObject({ label: "tmux", state: "ok", detail: "内置" });
    expect(setupChecks(report(true, []))[0]).toMatchObject({ label: "tmux", state: "ok", detail: "已装" });
    const [tmux] = setupChecks(report(false, [agent("claude", true)]));
    expect(tmux).toMatchObject({ label: "tmux", state: "missing", command: "brew install tmux" });
    expect(tmux.url).toMatch(/^https:/);
  });

  it("offers one-click Install where the installer can run; without Node, one Node.js row instead of the npm ones", () => {
    const claude = agent("claude", false, { install_command: "curl -fsSL https://claude.ai/install.sh | bash" });
    const kimi = agent("kimi", false, { install_command: null });
    const r = report(true, [agent("codex", false), claude, agent("gemini", false), kimi], { npm: null });
    expect(head(r).map((i) => [i.label, i.detail, i.action?.do])).toEqual([
      ["tmux", "已装", undefined],
      ["Agent", "没找到，装一个：", undefined],
      ["claude", "没装", "install"],
      ["Node.js", "codex 等 2 个 Agent 需要它", undefined],
    ]);
    expect(head(r)[3].url).toContain("nodejs.org");
    expect(pageOnly(r).map((a) => a.kind)).toEqual(["kimi"]);
    const withNode = head(report(true, [agent("codex", false), claude, kimi]));
    expect(withNode.slice(2).map((i) => [i.label, i.action?.do])).toEqual([["codex", "install"], ["claude", "install"]]);
    expect(installItems(report(true, [agent("codex", false)], { npm: null }))[0].detail).toBe("codex 需要它");
  });

  it("with no agent installed, extra agents are not suggested; once found they count", () => {
    expect(installItems(report(true, [agent("claude", false), agent("aider", false, { extra: true })])).map((i) => i.label)).toEqual(["claude"]);
    const found = head(report(true, [agent("claude", false), agent("mybot", true, { extra: true })]));
    expect(found.map((i) => [i.label, i.state])).toEqual([["tmux", "ok"], ["mybot", "ok"]]);
  });

  it("lists the installed agents once one is there, with Sign In where sign-in is missing", () => {
    const r = report(true, [
      agent("claude", true, { signed_in: true }),
      agent("codex", true, { signed_in: false, login_command: "codex login" }),
      agent("gemini", true, { signed_in: false }),
      agent("kimi", true),
      agent("qwen", false),
      agent("terminal", true),
    ]);
    const items = head(r);
    expect(items.map((i) => [i.label, i.state, i.action?.do])).toEqual([
      ["tmux", "ok", undefined],
      ["claude", "ok", undefined],
      ["codex", "note", "login"],
      ["gemini", "note", "login"],
      ["kimi", "ok", undefined],
    ]);
    expect(items[1].detail).toContain("已登录");
    expect(items[4].detail).not.toContain("登录");
    expect(signInItems(r).map((i) => i.label)).toEqual(["codex", "gemini"]);
  });

  it("ends with browser use, Computer Use and notifications, one action each", () => {
    const r = report(true, [agent("claude", true)]);
    expect(setupChecks(r).slice(-3).map((i) => [i.state, i.action?.do])).toEqual([["info", undefined], ["info", undefined], ["info", "notifications"]]);
    const set = setupChecks(r, { browser: false, computer: "install" }).slice(-3);
    expect(set.map((i) => [i.state, i.action?.do])).toEqual([["note", "browser"], ["note", "computer"], ["info", "notifications"]]);
    const allow = setupChecks(r, { browser: true, computer: "permissions" }).slice(-3);
    expect(allow.map((i) => [i.state, i.action?.do])).toEqual([["ok", undefined], ["note", "allow"], ["info", "notifications"]]);
    expect(setupChecks(r, { browser: true, computer: "ready" }).slice(-3).map((i) => i.state)).toEqual(["ok", "ok", "info"]);
    // No Node: browser use says so; a build without the helper has no Computer Use row.
    const bare = setupChecks({ ...r, npm: null }, { browser: false, computer: "missing" }).slice(-2);
    expect(bare.map((i) => [i.detail, i.action?.do])).toEqual([["需要 Node.js", undefined], ["后台时提醒", "notifications"]]);
  });

  it("names buttons and tabs in a few words", () => {
    const claude = { do: "install" as const, kind: "claude", name: "Claude Code" };
    expect(actionText(claude)).toBe("安装");
    expect(actionText({ ...claude, do: "login" })).toBe("登录");
    expect(actionText({ ...claude, do: "allow" })).toBe("授权");
    expect(tabLabel(claude)).toBe("安装 Claude Code");
    expect(tabLabel({ ...claude, do: "login" })).toBe("登录 Claude Code");
  });
});
