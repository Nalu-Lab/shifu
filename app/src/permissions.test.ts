import { describe, expect, it } from "vitest";
import type { Permissions } from "./api";
import { permissionOptions, RECOMMENDED, permissionGate } from "./permissions";

function fakes(chosen: boolean | Error, answer: Permissions = "own") {
  const calls = { chosen: 0, ask: 0, saved: [] as Permissions[] };
  let release: () => void = () => {};
  const asked = new Promise<void>((r) => (release = r));
  const gate = permissionGate({
    chosen: async () => {
      calls.chosen++;
      if (chosen instanceof Error) throw chosen;
      return chosen;
    },
    ask: async () => {
      calls.ask++;
      await asked;
      return answer;
    },
    save: async (mode) => {
      calls.saved.push(mode);
    },
  });
  return { gate, calls, release };
}

describe("first-run permission question", () => {
  it("recommends and preselects the agent's own settings, with a risk line on bypass", () => {
    expect(RECOMMENDED).toBe("own");
    expect(permissionOptions()[0].id).toBe("own");
    expect(permissionOptions().find((o) => o.id === "bypass")?.hint).toMatch(/^风险/);
  });

  it("does not ask a user who already has a choice, and stops checking after that", async () => {
    const { gate, calls } = fakes(true);
    await gate("claude");
    await gate("codex");
    expect(calls).toEqual({ chosen: 1, ask: 0, saved: [] });
  });

  it("asks a fresh install once and saves the answer; starts meanwhile wait for it", async () => {
    const { gate, calls, release } = fakes(false, "bypass");
    const first = gate("claude");
    const second = gate("codex");
    let done = false;
    void Promise.all([first, second]).then(() => (done = true));
    await Promise.resolve();
    expect(done).toBe(false);
    release();
    await Promise.all([first, second]);
    await gate(undefined);
    expect(calls).toEqual({ chosen: 1, ask: 1, saved: ["bypass"] });
  });

  it("never asks for the plain terminal", async () => {
    const { gate, calls } = fakes(false);
    await gate("terminal");
    expect(calls.chosen).toBe(0);
  });

  it("lets the start through when the choice cannot be read (the backend then uses own)", async () => {
    const { gate, calls } = fakes(new Error("no backend"));
    await gate("claude");
    expect(calls.ask).toBe(0);
  });
});
