import { describe, expect, it } from "vitest";
import { AGENTS, specOf, defaultKind, effortsFor, installUrl, isInstalled, mergeCatalog, splitAgents, validPick, type ModelInfo } from "./models";
import type { CliReport } from "./api";
import { markFor } from "./vendoricons";

const m = (id: string, efforts: string[]): ModelInfo => ({ id, label: id, efforts });
const models = [m("opus", ["low", "high", "max"]), m("sonnet", ["low", "medium", "high"]), m("haiku-4-5", [])];

describe("models", () => {
  it("offers Qwen Code once in the shared new-task and add-agent picker", () => {
    expect(specOf("qwen")).toEqual({ kind: "qwen", name: "Qwen Code" });
    expect(AGENTS.filter(a => a.kind === "qwen")).toHaveLength(1);
  });
  it("offers Antigravity in both new-task and add-agent pickers", () => {
    expect(specOf("antigravity")).toEqual({ kind: "antigravity", name: "Antigravity" });
    expect(AGENTS.filter(a => a.kind === "antigravity")).toHaveLength(1);
  });
  it("levels follow the chosen model; with the default every level appears once, in order", () => {
    expect(effortsFor(models, "haiku-4-5")).toEqual([]);
    expect(effortsFor(models, "sonnet")).toEqual(["low", "medium", "high"]);
    expect(effortsFor(models, "")).toEqual(["low", "high", "max", "medium"]);
  });

  it("a saved pick no longer on offer falls back to the default; with no list yet it is kept", () => {
    expect(validPick(models, { model: "opus", effort: "max" })).toEqual({ model: "opus", effort: "max" });
    expect(validPick(models, { model: "sonnet", effort: "max" })).toEqual({ model: "sonnet", effort: "" });
    expect(validPick(models, { model: "gone", effort: "low" })).toEqual({ model: "", effort: "low" });
    expect(validPick([], { model: "opus", effort: "max" })).toEqual({ model: "opus", effort: "max" });
  });

  it("a failed probe keeps last time's list", () => {
    const old = { claude: [m("opus", [])], codex: [m("gpt", [])] };
    const next = mergeCatalog(old, [
      { kind: "claude", models: [m("fable", [])], error: null },
      { kind: "codex", models: [], error: "读不到" },
      { kind: "codebuddy", models: [m("hy4free", [])], error: null },
    ]);
    expect(Object.fromEntries(Object.entries(next).map(([k, v]) => [k, v.map((x) => x.id)]))).toEqual({
      claude: ["fable"],
      codex: ["gpt"],
      codebuddy: ["hy4free"],
    });
  });
});

describe("installed agents", () => {
  const report = (installed: string[]): CliReport => ({
    agents: AGENTS.filter((a) => a.kind !== "terminal").map((a) => ({
      kind: a.kind,
      name: a.name,
      path: installed.includes(a.kind) ? `/bin/${a.kind}` : null,
      install_url: `https://example.com/${a.kind}`,
    })),
    tmux: "/opt/homebrew/bin/tmux",
    shell_read: true,
  });

  it("before the first answer nothing is hidden", () => {
    expect(splitAgents(null).missing).toEqual([]);
    expect(defaultKind(null, "kimi")).toBe("kimi");
  });

  it("offers what is installed, keeps the terminal, and points at the rest", () => {
    const { installed, missing } = splitAgents(report(["codex", "qwen", "gemini"]));
    expect(installed.map((a) => a.kind)).toEqual(["codex", "qwen", "gemini", "terminal"]);
    expect(missing.map((a) => a.kind)).toEqual(["claude", "codebuddy", "kimi", "antigravity", "stepfun", "opencode", "pi"]);
    expect(installUrl(report([]), "kimi")).toBe("https://example.com/kimi");
    expect(splitAgents(report(["antigravity"])).missing).toContainEqual({ kind: "gemini", name: "Gemini CLI" });
  });

  it("keeps the remembered agent while it is installed, else takes the first installed one", () => {
    expect(defaultKind(report(["claude", "codex"]), "codex")).toBe("codex");
    expect(defaultKind(report(["codex", "qwen"]), "claude")).toBe("codex");
    expect(defaultKind(report([]), "claude")).toBe("terminal");
  });

  it("extra and user-defined agents are offered only when found, before the terminal", () => {
    const base = report(["codex"]);
    const more = (kind: string, name: string, path: string | null, source: string) =>
      ({ kind, name, path, install_url: "", source, hooks: "none", extra: true });
    const r: CliReport = {
      ...base,
      agents: [...base.agents, more("aider", "Aider", "/bin/aider", "plain"), more("droid", "Factory Droid", null, "plain"),
        more("mybot", "My Bot", "/opt/mybot", "agents.json"), more("gone", "Gone", null, "agents.json")],
    };
    const { installed, missing } = splitAgents(r);
    expect(installed.map((a) => a.kind)).toEqual(["codex", "aider", "mybot", "terminal"]);
    expect(missing.map((a) => a.kind)).not.toContain("droid");
    expect(missing.map((a) => a.kind)).not.toContain("gone");
    expect(missing.map((a) => a.kind)).toContain("claude");
    expect(installed.find((a) => a.kind === "mybot")?.name).toBe("My Bot");
    expect(defaultKind({ ...r, agents: r.agents.map((a) => (a.kind === "codex" ? { ...a, path: null } : a)) }, "claude")).toBe("aider");
    expect(markFor("aider")).toBe("plain");
    expect(markFor("codex")).toBe("codex");
  });

  it("an agent the report does not list is not hidden", () => {
    expect(isInstalled({ agents: [], tmux: null, shell_read: false }, "claude")).toBe(true);
  });
});
