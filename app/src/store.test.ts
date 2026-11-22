import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentView } from "./format";
import { getWaitingList, initSeen, setActiveAgent, setAgents, setTaskFile, setWindowFocused } from "./store";

const agent = (name: string): AgentView => ({
  name, kind: "codex", cwd: "/w", dir_name: "w", branch: null, title: name, status: "needs_reply",
  status_since: 100, last_message: "改好了。", waiting_for: null,
} as AgentView);

describe("which conversation counts as seen", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    initSeen({});
    setWindowFocused(true);
    setActiveAgent(null);
    setTaskFile({ current: null, tasks: [{ id: "career", project: "/w", name: "Career", auto_name: false, created_at: 1, agents: ["a", "b"] }] });
    setAgents([agent("a"), agent("b")]);
  });
  afterEach(() => vi.useRealTimers());

  it("a panel passed through while another opens keeps its 在等你 row", () => {
    // Opening 'a' restores the task's layout, whose front panel was 'b', then brings 'a' forward (Eddie 11-22).
    setActiveAgent("b");
    setActiveAgent("a");
    vi.advanceTimersByTime(5000);
    expect(getWaitingList().map((w) => w.agent.name)).toEqual(["b"]);
  });

  it("a panel left in front is seen", () => {
    setActiveAgent("b");
    vi.advanceTimersByTime(5000);
    expect(getWaitingList()).toEqual([]);
  });
});
