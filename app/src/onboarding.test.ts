import { describe, expect, it } from "vitest";
import type { CliReport } from "./api";
import { AGENTS } from "./models";
import { PRESELECT, canStart, guideAgents, guideKind, mergeProjects, preselected, shouldShowGuide } from "./onboarding";

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

  it("starts only with a ticked project, an agent and tmux", () => {
    const agents = guideAgents(report(["claude"]));
    expect(canStart(0, agents, report(["claude"]))).toBe(false);
    expect(canStart(1, [], report([]))).toBe(false);
    expect(canStart(1, agents, null)).toBe(false);
    expect(canStart(1, agents, { ...report(["claude"]), tmux: null })).toBe(false);
    expect(canStart(2, agents, report(["claude"]))).toBe(true);
  });

  it("ticks the most recently changed projects first", () => {
    const found = Array.from({ length: 8 }, (_, i) => ({ path: `/p${i}`, name: `p${i}`, git: true, modified: i }));
    expect(preselected(found)).toEqual(["/p7", "/p6", "/p5", "/p4", "/p3"].slice(0, PRESELECT));
    expect(preselected([])).toEqual([]);
  });

  it("lists a folder found twice once", () => {
    const a = { path: "/a", name: "a", git: true, modified: 1 };
    const b = { path: "/b", name: "b", git: false, modified: 2 };
    expect(mergeProjects([a], [{ ...a, modified: 9 }, b])).toEqual([a, b]);
  });
});
