import { describe, expect, it } from "vitest";
import type { CliFound, CliReport } from "./api";
import { setupChecks, setupReady } from "./setupcheck";

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
const report = (tmux: boolean, agents: CliFound[]): CliReport => ({ agents, tmux: tmux ? "/bin/tmux" : null, shell_read: true });

describe("setup checklist", () => {
  it("is ready with tmux and one agent; sign-in does not hold it back", () => {
    expect(setupReady(null)).toBe(true);
    expect(setupReady(report(true, [agent("claude", true, { signed_in: false })]))).toBe(true);
    expect(setupReady(report(false, [agent("claude", true)]))).toBe(false);
    expect(setupReady(report(true, [agent("claude", false)]))).toBe(false);
  });

  it("gives the tmux command and install page when tmux is missing", () => {
    const [tmux] = setupChecks(report(false, [agent("claude", true)]));
    expect(tmux).toMatchObject({ label: "tmux", state: "missing", command: "brew install tmux" });
    expect(tmux.url).toMatch(/^https:/);
    expect(setupChecks(report(true, [agent("claude", true)]))[0]).toMatchObject({ label: "tmux", state: "ok" });
  });

  it("with no agent installed, extra agents are not suggested; once found they count", () => {
    const missing = setupChecks(report(true, [agent("claude", false), agent("aider", false, { extra: true })]));
    expect(missing.map((i) => i.label)).toEqual(["tmux", "Agent", "claude"]);
    const found = setupChecks(report(true, [agent("claude", false), agent("mybot", true, { extra: true })]));
    expect(found.map((i) => [i.label, i.state])).toEqual([["tmux", "ok"], ["mybot", "ok"]]);
  });

  it("with no agent installed, lists every agent with its install command or page", () => {
    const items = setupChecks(report(true, [agent("claude", false), agent("kimi", false, { install_command: null })]));
    expect(items.map((i) => [i.label, i.state])).toEqual([
      ["tmux", "ok"],
      ["Agent", "missing"],
      ["claude", "missing"],
      ["kimi", "missing"],
    ]);
    expect(items[2]).toMatchObject({ command: "npm i -g claude", url: "https://example.com/claude" });
    expect(items[3].command).toBeUndefined();
    expect(items[3].url).toBe("https://example.com/kimi");
  });

  it("lists only the installed agents once one is there, with sign-in where it can be told", () => {
    const items = setupChecks(
      report(true, [
        agent("claude", true, { signed_in: true }),
        agent("codex", true, { signed_in: false, login_command: "codex login" }),
        agent("gemini", true, { signed_in: false }),
        agent("kimi", true),
        agent("qwen", false),
        agent("terminal", true),
      ]),
    );
    expect(items.map((i) => [i.label, i.state, i.command])).toEqual([
      ["tmux", "ok", undefined],
      ["claude", "ok", undefined],
      ["codex", "note", "codex login"],
      ["gemini", "note", undefined],
      ["kimi", "ok", undefined],
    ]);
    expect(items[1].detail).toContain("已登录");
    expect(items[4].detail).not.toContain("登录");
  });
});
