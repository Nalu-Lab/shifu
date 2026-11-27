import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { approvalSummary, clip, type AgentView } from "./format";
import { type Task, WAITING_GROUPS, waitingGroup, waitingList, waitingSummary } from "./tasks";
import { recordWaitingOpened } from "./waitingseen";

const agent = (name: string, overrides: Partial<AgentView> = {}): AgentView => ({
  name, kind: "claude", cwd: "/w", dir_name: "w", branch: null, title: null,
  status: "needs_reply", status_since: 1, last_message: null, exited: false, session_id: `s-${name}`, ...overrides,
});
const approval = (name: string, since: number, what = "Bash：git push") => agent(name, { status: "needs_approval", status_since: since, waiting_for: what });
const question = (name: string, since: number) => agent(name, { status_since: since, mark: "new", waiting_for: "要合并吗？", last_message: "改好了。\n要合并吗？" });
const finished = (name: string, since: number) => agent(name, { status_since: since, mark: "new", waiting_for: "改好了。", last_message: "改好了。" });
const task: Task = { id: "t", project: "/w", name: "T", auto_name: false, created_at: 1, agents: ["a", "b", "c", "d", "e", "f", "g"] };
const names = (agents: AgentView[]) => waitingList([task], agents).map((w) => w.agent.name);

describe("在等你 order", () => {
  it("puts approvals first, then questions, then finished results; the longest wait first in each", () => {
    const agents = [finished("a", 10), question("b", 50), approval("c", 90), finished("d", 5), question("e", 20), approval("f", 30)];
    expect(names(agents)).toEqual(["f", "c", "e", "b", "d", "a"]);
    expect(waitingList([task], agents).map((w) => waitingGroup(w.agent))).toEqual(["needs_approval", "needs_approval", "asked", "asked", "new", "new"]);
    expect(WAITING_GROUPS.map((g) => g.label)).toEqual(["等你批准", "待回复", "做完了"]);
  });

  it("does not depend on the order the backend lists agents in, so rows do not swap between polls", () => {
    const agents = [finished("a", 10), question("b", 10), approval("c", 10), finished("d", 10), question("e", 10)];
    const first = names(agents);
    expect(first).toEqual(["c", "b", "e", "a", "d"]);
    for (const shuffled of [[...agents].reverse(), [agents[2], agents[0], agents[4], agents[1], agents[3]]]) {
      expect(names(shuffled)).toEqual(first);
    }
  });

  it("moves only the row whose state changed", () => {
    const before = [finished("a", 10), finished("b", 20), finished("c", 30)];
    expect(names(before)).toEqual(["a", "b", "c"]);
    // 'c' now asks for an approval: it jumps to the top, the others keep their places.
    expect(names([before[0], before[1], approval("c", 40)])).toEqual(["c", "a", "b"]);
  });

  it("counts a running Codex conversation's open question as a question, and a read question still as one", () => {
    const codex = agent("a", { kind: "codex", status: "running", status_since: 1, pending_questions: ["先做哪个？"] });
    const readQuestion = { ...question("b", 5), mark: "parked" as const };
    expect(names([finished("c", 1), codex, readQuestion, approval("d", 100)])).toEqual(["d", "a", "b", "c"]);
  });

  it("an opened approval with a new question waiting is grouped by the question", () => {
    const both = { ...approval("a", 5, "Bash：ls"), kind: "codex", pending_questions: ["选哪个分支？"], pending_question_ids: ["q1"] };
    const opened = recordWaitingOpened({}, { ...both, pending_questions: [], pending_question_ids: [] });
    const rows = waitingList([task], [both, approval("b", 50)], opened);
    expect(rows.map((w) => [w.agent.name, waitingGroup(w.agent)])).toEqual([["b", "needs_approval"], ["a", "asked"]]);
  });
});

describe("one line on what happened", () => {
  it("says what an approval is for in words", () => {
    expect(approvalSummary("Bash：git push origin main")).toBe("要运行：git push origin main");
    expect(approvalSummary("shell：bash -lc npm publish")).toBe("要运行：npm publish");
    expect(approvalSummary("exec_command：/bin/zsh -lc 'rm -rf build'")).toBe("要运行：'rm -rf build'");
    expect(approvalSummary("Edit：config.toml")).toBe("要改：config.toml");
    expect(approvalSummary("apply_patch")).toBe("要改文件");
    expect(approvalSummary("Read：hosts")).toBe("要读：hosts");
    expect(approvalSummary("WebFetch：https://example.com/a")).toBe("要访问：https://example.com/a");
    expect(approvalSummary("mcp__github__create_pr：Fix it")).toBe("要用 github 的 create_pr：Fix it");
    expect(approvalSummary("ExitPlanMode")).toBe("计划写好了，等你批准");
    expect(approvalSummary("Skill")).toBe("要用 Skill");
    // Already a sentence (Kimi, opencode's own words, older records): kept as it is.
    expect(approvalSummary("运行检查")).toBe("运行检查");
    expect(approvalSummary("bash：echo shifu-mock")).toBe("要运行：echo shifu-mock");
  });

  it("clips to one short line: commands keep their start, sentences their end", () => {
    expect(clip("a\n  b\tc", 10, "start")).toBe("a b c");
    expect(clip("x".repeat(20), 10, "start")).toBe(`${"x".repeat(9)}…`);
    expect(clip(`${"前文".repeat(20)}要合并吗？`, 10, "end")).toBe("…前文前文要合并吗？");
    expect([...clip("字".repeat(500), 72, "end")].length).toBe(72);
  });

  it("shows the approval, the question or the closing line, and keeps the whole text for the tooltip", () => {
    const long = `npm run build && ${"x".repeat(200)}`;
    const blocked = waitingSummary(approval("a", 1, `Bash：${long}`));
    expect(blocked.status).toBe("needs_approval");
    expect(blocked.line.startsWith("要运行：npm run build &&") && blocked.line.endsWith("…")).toBe(true);
    expect([...blocked.line].length).toBe(72);
    expect(blocked.full).toBe(`要运行：${long}`);
    expect(waitingSummary(question("b", 1))).toEqual({ status: "asked", line: "要合并吗？", full: "要合并吗？" });
    expect(waitingSummary(finished("c", 1))).toEqual({ status: "new", line: "改好了。", full: "改好了。" });
  });

  it("falls back to the saved reply, or to a plain approval, when the record says nothing more", () => {
    const old = agent("a", { mark: "new", last_message: `${"很长的回复".repeat(80)}`, waiting_for: null });
    const summary = waitingSummary(old);
    expect(summary.line.startsWith("…") && [...summary.line].length === 72).toBe(true);
    expect(waitingSummary(agent("b", { status: "needs_approval", waiting_for: null })).line).toBe("要你批准一个操作");
    expect(waitingSummary(agent("c", { mark: "new", last_message: null, waiting_for: null })).line).toBe("");
  });

  it("puts the approval above a Codex conversation's open questions, each on its own line", () => {
    const both = { ...approval("a", 1, "shell：bash -lc cargo test"), kind: "codex", pending_questions: ["选哪个分支？"] };
    expect(waitingSummary(both)).toEqual({ status: "needs_approval", line: "要运行：cargo test\n选哪个分支？", full: "要运行：cargo test\n选哪个分支？" });
  });
});

describe("the conversation in front, and a restart", () => {
  let store: typeof import("./store");
  beforeEach(async () => {
    vi.useFakeTimers();
    store = await import("./store");
    store.initSeen({});
    store.initWaitingOpened({});
    store.setWindowFocused(true);
    store.setActiveAgent(null);
    store.setTaskFile({ current: "t", tasks: [task] });
  });
  afterEach(() => vi.useRealTimers());

  it("an approval or a question in front stays in 在等你; a finished result in front is seen", () => {
    store.setActiveAgent("a");
    store.setAgents([approval("a", 10)]);
    vi.advanceTimersByTime(5000);
    expect(store.getWaitingList().map((w) => w.agent.name)).toEqual(["a"]);
    store.setAgents([question("a", 20)]);
    vi.advanceTimersByTime(5000);
    expect(store.getWaitingList().map((w) => w.agent.name)).toEqual(["a"]);
    store.setAgents([finished("a", 30)]);
    vi.advanceTimersByTime(5000);
    expect(store.getWaitingList()).toEqual([]);
    // Approved and working again: gone; the next approval is back at once.
    store.setAgents([agent("a", { status: "running", status_since: 40 })]);
    expect(store.getWaitingList()).toEqual([]);
    store.setAgents([approval("a", 41)]);
    expect(store.getWaitingList().map((w) => w.agent.name)).toEqual(["a"]);
  });

  it("Cmd+J's target, the first row, is the most urgent one", () => {
    store.setAgents([finished("a", 1), question("b", 2), approval("c", 3)]);
    expect(store.getWaitingList()[0].agent.name).toBe("c");
    store.setAgents([finished("a", 1), question("b", 2), agent("c", { status: "running", status_since: 4 })]);
    expect(store.getWaitingList()[0].agent.name).toBe("b");
  });

  it("after a restart, what still waits is listed again from the records; what was opened stays dismissed", () => {
    const opened = recordWaitingOpened({}, finished("b", 20));
    // A fresh app: preferences come back from storage, then the first agent list.
    store.initSeen({});
    store.initWaitingOpened(opened);
    store.setAgents([approval("a", 10), finished("b", 20), question("c", 30)]);
    expect(store.getWaitingList().map((w) => w.agent.name)).toEqual(["a", "c"]);
    // The opened one comes back when something new happens in it.
    store.setAgents([approval("a", 10), approval("b", 40), question("c", 30)]);
    expect(store.getWaitingList().map((w) => w.agent.name)).toEqual(["a", "b", "c"]);
  });
});
