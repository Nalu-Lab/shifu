import { describe, expect, it } from "vitest";
import type { CliReport } from "./api";
import { AGENTS } from "./models";
import { canStart, guideAgents, guideKind, shouldShowGuide } from "./onboarding";

const report = (found: string[]): CliReport => ({
  agents: AGENTS.filter((a) => a.kind !== "terminal").map(({ kind }) => ({
    kind,
    name: kind,
    path: found.includes(kind) ? `/bin/${kind}` : null,
    install_url: `https://example.com/${kind}`,
  })),
  tmux: "/bin/tmux",
  shell_read: true,
});

describe("first-run guide", () => {
  it("shows only on a fresh start that was neither finished nor skipped", () => {
    expect(shouldShowGuide(0, false)).toBe(true);
    expect(shouldShowGuide(0, true)).toBe(false);
    expect(shouldShowGuide(1, false)).toBe(false);
  });

  it("offers the installed agents but not the plain terminal", () => {
    expect(guideAgents(report(["codex", "kimi"])).map((a) => a.kind)).toEqual(["codex", "kimi"]);
    expect(guideAgents(report([]))).toEqual([]);
  });

  it("keeps the default while it is installed, else takes the first found", () => {
    const agents = guideAgents(report(["codex", "kimi"]));
    expect(guideKind(agents, "kimi")).toBe("kimi");
    expect(guideKind(agents, "claude")).toBe("codex");
    expect(guideKind([], "claude")).toBeNull();
  });

  it("starts only with a folder and an agent", () => {
    const agents = guideAgents(report(["claude"]));
    expect(canStart(null, agents)).toBe(false);
    expect(canStart("/p", [])).toBe(false);
    expect(canStart("/p", agents)).toBe(true);
  });
});
