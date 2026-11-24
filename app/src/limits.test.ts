import { describe, expect, it } from "vitest";
import { ACCENT_AT, arrangePanel, codebuddySegments, creditsText, isHot, isReset, limitLine, limitLines, resetCountdown, resetText, windowName, type CodeBuddyUse, type VendorLimits } from "./limits";
import { isInstalled } from "./models";
import type { CliReport } from "./api";

/** Local-time seconds, so the tests do not depend on the machine's time zone. */
const at = (y: number, mo: number, d: number, h: number, mi: number) => new Date(y, mo - 1, d, h, mi).getTime() / 1000;

describe("windowName", () => {
  it("names the known windows per vendor and falls back to minutes", () => {
    expect(windowName(300, "claude")).toBe("5小时");
    expect(windowName(300, "codex")).toBe("5小时");
    expect(windowName(10080, "codex")).toBe("本周");
    expect(windowName(10080, "kimi")).toBe("7天");
    expect(windowName(10080, "claude")).toBe("7天");
    expect(windowName(43200, "qwen")).toBe("30天");
    expect(windowName(60, "codex")).toBe("60分钟");
  });
});

describe("resetText", () => {
  const now = at(2026, 11, 6, 9, 30);
  it("includes the date beyond today, even for a reset on the same weekday", () => {
    expect(resetText(at(2026, 11, 6, 13, 11), now)).toBe("13:11");
    expect(resetText(at(2026, 11, 6, 23, 59), now)).toBe("23:59");
    expect(resetText(at(2026, 11, 13, 13, 11), now)).toBe("11/13 13:11");
    expect(resetText(at(2026, 11, 7, 0, 5), now)).toBe("11/7 00:05");
    expect(resetText(at(2026, 12, 6, 13, 11), now)).toBe("12/6 13:11");
  });
});

describe("resetCountdown", () => {
  it("counts whole days and hours left, so the reset never looks further away than it is", () => {
    const now = at(2026, 11, 22, 14, 27);
    // Codex on 11-22: the week resets 11-28 21:05, 6 days 6 hours away.
    expect(resetCountdown(at(2026, 11, 28, 21, 5), now)).toBe("6d6h");
    // Claude on 11-22: the week resets 11-23 03:00, 12 hours 33 minutes away.
    expect(resetCountdown(at(2026, 11, 23, 3, 0), now)).toBe("12h");
    expect(resetCountdown(now + 86400, now)).toBe("1d");
    // A day or more keeps its hours (Eddie 11-22: Codex showed only "6d").
    expect(resetCountdown(now + 2 * 86400 + 5 * 3600 + 59 * 60, now)).toBe("2d5h");
    expect(resetCountdown(now + 86399, now)).toBe("23h");
    expect(resetCountdown(now + 3600, now)).toBe("1h");
    expect(resetCountdown(now + 3599, now)).toBe("59m");
  });
  it("never shows 0m while the reset is still ahead", () => {
    const now = at(2026, 11, 22, 14, 27);
    expect(resetCountdown(now + 59, now)).toBe("1m");
    expect(resetCountdown(now + 1, now)).toBe("1m");
  });
});

describe("reset and accent", () => {
  it("a window is reset when its time is now or earlier", () => {
    const w = { used_percent: 100, window_minutes: 10080, resets_at: 1000 };
    expect(isReset(w, 999)).toBe(false);
    expect(isReset(w, 1000)).toBe(true);
  });
  it("90 percent and above is hot", () => {
    expect(ACCENT_AT).toBe(90);
    expect(isHot({ used_percent: 89.9, window_minutes: 300, resets_at: 1 })).toBe(false);
    expect(isHot({ used_percent: 90, window_minutes: 300, resets_at: 1 })).toBe(true);
  });
});

describe("creditsText", () => {
  it("drops the decimals and groups thousands", () => {
    expect(creditsText("29855.1005840000")).toBe("29,855");
    expect(creditsText("0")).toBe("0");
    expect(creditsText("n/a")).toBeNull();
  });
});

describe("limitLine", () => {
  const now = at(2026, 11, 6, 22, 0);
  const codex: VendorLimits = {
    windows: [{ used_percent: 100, window_minutes: 10080, resets_at: at(2026, 11, 9, 13, 11) }],
    credits: "29855.1005840000",
    updated_at: at(2026, 11, 6, 21, 50),
  };

  it("real Codex data: one window at 100 percent, credits, reset on Monday", () => {
    const line = limitLine("codex", codex, now);
    expect(line.label).toBe("Codex");
    expect(line.segments).toEqual([
      { label: "本周", value: "100%", hot: true, fill: 100, resetsAt: codex.windows[0].resets_at },
      { label: "credits", value: "29,855", credits: true },
    ]);
    expect(line.title).toContain("本周：已用 100%，11月9日 周一 13:11 重置");
    expect(line.title).toContain("credits 余额 29,855");
    expect(line.title).toContain("数据记于 21:50");
  });

  it("keeps the reported credits balance visible before the quota is near its limit", () => {
    const line = limitLine("codex", { ...codex, windows: [{ ...codex.windows[0], used_percent: 36 }] }, now);
    expect(line.segments).toEqual([
      { label: "本周", value: "36%", hot: false, fill: 36, resetsAt: codex.windows[0].resets_at },
      { label: "credits", value: "29,855", credits: true },
    ]);
    expect(line.title).toContain("credits 余额 29,855");
    expect(limitLine("codex", { ...codex, credits: "0" }, now).title).toContain("credits 余额 0");
    const none = limitLine("codex", { ...codex, credits: null }, now);
    expect(none.segments.map((s) => s.label)).toEqual(["本周"]);
  });

  it("a past reset time waits for a new reading, with no old percentage or inferred recovery", () => {
    const line = limitLine("codex", codex, at(2026, 11, 9, 14, 0));
    expect(line.segments).toEqual([
      { label: "本周", value: "待更新", fill: 0 },
      { label: "credits", value: "29,855", credits: true },
    ]);
    expect(line.label).toContain("旧数据");
  });

  it("preserves each live window's reset and skips elapsed times", () => {
    const claude: VendorLimits = {
      windows: [
        { used_percent: 42.4, window_minutes: 300, resets_at: at(2026, 11, 6, 23, 30) },
        { used_percent: 63, window_minutes: 10080, resets_at: at(2026, 11, 9, 9, 0) },
      ],
      credits: null,
      updated_at: at(2026, 11, 6, 21, 0),
    };
    expect(limitLine("claude", claude, now).segments).toEqual([
      { label: "5小时", value: "42%", hot: false, fill: 42.4, resetsAt: claude.windows[0].resets_at },
      { label: "7天", value: "63%", hot: false, fill: 63, resetsAt: claude.windows[1].resets_at },
    ]);
    const later = limitLine("claude", claude, at(2026, 11, 7, 0, 0));
    expect(later.segments).toEqual([
      { label: "5小时", value: "待更新", fill: 0 },
      { label: "7天", value: "63%", hot: false, fill: 63, resetsAt: claude.windows[1].resets_at },
    ]);
  });

  it("keeps a failed reading's percentage and timestamp with its reason", () => {
    const line = limitLine("codex", { ...codex, error: "Codex 查询超时" }, now);
    expect(line.label).toBe("Codex（旧数据）");
    expect(line.segments[0].value).toBe("100%");
    expect(line.note).toContain("数据截至");
    expect(line.note).toContain("Codex 查询超时");
    expect(limitLine("codex", { ...codex, updated_at: now - 601 }, now).label).toContain("旧数据");
    const empty = limitLine("claude", { windows: [], credits: null, updated_at: 0, error: "查询失败" }, now);
    expect(empty.message).toBe("暂无数据");
    expect(empty.note).toContain("查询失败");
  });
});

describe("limitLines", () => {
  const antigravity = { groups: [], updated_at: 0, error: null };
  const none = { claude: null, codex: null, kimi: null, codebuddy: null, antigravity, qwen: null, qwen_quota: null };
  it("a vendor without data has no line", () => {
    const data: VendorLimits = { windows: [{ used_percent: 1, window_minutes: 300, resets_at: 5000 }], credits: null, updated_at: 1 };
    expect(limitLines(none, 0)).toEqual([]);
    expect(limitLines({ ...none, codex: data }, 0).map((l) => l.label)).toEqual(["Codex"]);
    expect(limitLines({ ...none, claude: data, codex: data, kimi: data }, 0).map((l) => l.label)).toEqual(["Claude", "Codex", "Kimi"]);
  });

  it("Qwen shows the Token Plan's month, and the tokens its transcripts recorded only in the title", () => {
    // What `bl usage token-plan --output json` answered on 11-17: one 30-day window, 1.5592% of it used.
    const quota: VendorLimits = { windows: [{ used_percent: 1.5592, window_minutes: 43200, resets_at: 5000 }], credits: null, updated_at: 100 };
    const [line] = limitLines({ ...none, qwen_quota: quota }, 0);
    expect(line.label).toBe("Qwen");
    expect(line.segments[0]).toEqual({ label: "30天", value: "2%", hot: false, fill: 1.5592, resetsAt: 5000 });
    expect(line.title).not.toContain("tokens");
    const today = { calls: 3, input: 100, output: 20, cached: 80, total: 120 };
    const spent = limitLines({ ...none, qwen_quota: quota, qwen: { today, week: today, updated_at: 100 } }, 0)[0];
    expect(spent.title).toContain("今天 120 tokens（3 次调用）");
  });

  it("keeps Qwen's usage when the Token Plan did not report a reset time", () => {
    const quota: VendorLimits = { windows: [{ used_percent: 12, window_minutes: 43200, resets_at: 0 }], credits: null, updated_at: 100 };
    const [line] = limitLines({ ...none, qwen_quota: quota }, 100);
    expect(line.label).toBe("Qwen");
    expect(line.segments[0].value).toBe("12%");
    expect(line.segments[0].resetsAt).toBeUndefined();
    expect(line.title).toContain("暂无重置时间");
  });

  it("a plan it could not read keeps the row and says why", () => {
    const quota: VendorLimits = { windows: [], credits: null, updated_at: 0, error: "百炼控制台未登录，终端里跑一次 bl auth login --console" };
    const [line] = limitLines({ ...none, qwen_quota: quota }, 0);
    expect(line.message).toBe("暂无数据");
    expect(line.note).toContain("bl auth login --console");
  });
});

describe("codebuddySegments", () => {
  // Local noon of some day, and that day's midnight in ms.
  const noon = new Date(2026, 10, 9, 12, 0, 0);
  const now = Math.floor(noon.getTime() / 1000);
  const ms = (h: number) => new Date(2026, 10, 9, h, 0, 0).getTime();

  it("adds up only today's credits and shows the free quota until it resets", () => {
    const use: CodeBuddyUse = { spends: [[ms(0) - 60_000, 9], [ms(9), 15.06], [ms(11), 1.28]], free: { used_percent: 9, resets_at: now + 3600 } };
    expect(codebuddySegments(use, now)).toEqual([
      { label: "免费", value: "9%", hot: false, fill: 9, resetsAt: now + 3600 },
      { label: "今天", value: "16.3 积分", credits: true },
    ]);
  });

  it("has no rows when nothing was spent today and the free quota has reset", () => {
    expect(codebuddySegments({ spends: [[ms(0) - 1, 4]], free: { used_percent: 50, resets_at: now - 1 } }, now)).toEqual([]);
  });
});

describe("arrangePanel", () => {
  const report: CliReport = {
    agents: [
      { kind: "claude", name: "Claude", path: "/bin/claude", install_url: "" },
      { kind: "codex", name: "Codex", path: "/bin/codex", install_url: "" },
      { kind: "codebuddy", name: "CodeBuddy", path: null, install_url: "" },
      { kind: "kimi", name: "Kimi", path: "/bin/kimi", install_url: "" },
      { kind: "antigravity", name: "Antigravity", path: null, install_url: "" },
      { kind: "qwen", name: "Qwen Code", path: "/bin/qwen", install_url: "" },
    ],
    tmux: "/bin/tmux",
    shell_read: true,
  };
  const installed = (kind: string) => isInstalled(report, kind);
  const items = [
    { key: "claude", vendor: "claude" },
    { key: "codex", vendor: "codex" },
    { key: "kimi", vendor: "kimi", empty: true },
    { key: "qwen", vendor: "qwen", empty: true },
    { key: "codebuddy", vendor: "codebuddy", empty: true },
    { key: "antigravity:Gemini Models", vendor: "antigravity" },
    { key: "antigravity:Claude and GPT models", vendor: "antigravity" },
  ];
  const keys = (list: { key: string }[]) => list.map((item) => item.key);

  it("leaves out vendors whose CLI is not installed, with or without numbers", () => {
    const { full, empty } = arrangePanel(items, installed, null);
    expect(keys(full)).toEqual(["claude", "codex"]);
    expect(keys(empty)).toEqual(["kimi", "qwen"]);
  });

  it("keeps vendors without numbers apart, to be listed only when asked for", () => {
    const { full, empty } = arrangePanel(items, () => true, null);
    expect(full.every((item) => !item.empty)).toBe(true);
    expect(keys(empty)).toEqual(["kimi", "qwen", "codebuddy"]);
  });

  it("puts the active conversation's vendor first and keeps the others in order", () => {
    expect(keys(arrangePanel(items, installed, "codex").full)).toEqual(["codex", "claude"]);
    expect(keys(arrangePanel(items, () => true, "antigravity").full))
      .toEqual(["antigravity:Gemini Models", "antigravity:Claude and GPT models", "claude", "codex"]);
    expect(keys(arrangePanel(items, installed, "qwen").empty)).toEqual(["qwen", "kimi"]);
    expect(keys(arrangePanel(items, installed, "terminal").full)).toEqual(["claude", "codex"]);
  });

  it("turns through the vendors with numbers, or shows the first one without while none has any", () => {
    expect(keys(arrangePanel(items, installed, null).turning)).toEqual(["claude", "codex"]);
    const none = items.map((item) => ({ ...item, empty: true }));
    expect(keys(arrangePanel(none, installed, "qwen").turning)).toEqual(["qwen"]);
    expect(arrangePanel(none, () => false, null).turning).toEqual([]);
  });

  it("hides nothing before the first CLI report arrives", () => {
    const { full, empty } = arrangePanel(items, (kind) => isInstalled(null, kind), null);
    expect(full.length + empty.length).toBe(items.length);
  });
});
