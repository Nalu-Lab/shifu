import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentView } from "./format";
import { findAgent, getPinnedTasks, getWaitingList, initPinnedTasks, initSeen, initWaitingOpened, markUnread, setActiveAgent, setAgents, setTaskFile, setTaskPinned, setWindowFocused } from "./store";

const agent = (name: string): AgentView => ({
  name, kind: "codex", cwd: "/w", dir_name: "w", branch: null, title: name, status: "needs_reply",
  status_since: 100, last_message: "改好了。", waiting_for: null,
} as AgentView);

describe("which conversation counts as seen", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    initSeen({});
    initWaitingOpened({});
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

  it("标为未读 brings a seen reply back, and it stays unread while it is still in front", () => {
    setActiveAgent("b");
    vi.advanceTimersByTime(5000);
    expect(findAgent("b")?.mark).toBe("parked");
    markUnread("b");
    vi.advanceTimersByTime(5000);
    expect(findAgent("b")?.mark).toBe("new");
    expect(getWaitingList().map((w) => w.agent.name)).toContain("b");
    // Looking at it again after switching away reads it as usual.
    setActiveAgent("a");
    setActiveAgent("b");
    vi.advanceTimersByTime(5000);
    expect(findAgent("b")?.mark).toBe("parked");
  });
});

describe("pinned tasks", () => {
  it("pin and unpin a task, telling whoever saves them", () => {
    const saved: string[][] = [];
    initPinnedTasks(["old"], (ids) => saved.push(ids));
    setTaskPinned("career", true);
    expect([...getPinnedTasks()].sort()).toEqual(["career", "old"]);
    setTaskPinned("old", false);
    expect([...getPinnedTasks()]).toEqual(["career"]);
    expect(saved).toEqual([["old", "career"], ["career"]]);
  });
});
