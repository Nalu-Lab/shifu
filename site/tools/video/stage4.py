#!/usr/bin/env python3
"""Stage the isolated demo for the hero video. Usage: stage4.py zh|en
Panes run small bash scripts that print crafted terminal output on cue (files under /tmp/demo/cue/);
no agent CLI or model runs. The binary sits outside any .app with an unregistered bundle id, so the
app's notification call fails quietly instead of posting as the real Shifu."""
import json, os, shutil, subprocess, sys, time

LANG = sys.argv[1]
D = "/tmp/demo"; H = f"{D}/home"; SOCK = "shifu-app-demo"; CUE = f"{D}/cue"
BS = f"{D}/code/billing-service"; DR = f"{D}/code/data-reports"; NT = f"{D}/notes"
APP = f"{D}/ShifuDemo/Contents"
for p in (f"{H}/agents", f"{H}/layouts", f"{D}/scripts", CUE, f"{APP}/MacOS", BS, DR, NT):
    os.makedirs(p, exist_ok=True)
shutil.copy("/Users/rico/Desktop/Workspace/shifu-site/target/debug/shifu", f"{D}/shifu")
shutil.copy("/Users/rico/Desktop/Workspace/shifu/target/shifu-build-cache.noindex/desktop-4b4bf7cb3a32971a206f/"
            "debug/bundle/macos/Shifu.app/Contents/MacOS/shifu-app", f"{APP}/MacOS/shifu-demo")
os.chmod(f"{APP}/MacOS/shifu-demo", 0o755); os.chmod(f"{D}/shifu", 0o755)
open(f"{APP}/Info.plist", "w").write("""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>invalid.shifu-site-demo</string>
<key>CFBundleExecutable</key><string>shifu-demo</string>
<key>CFBundleName</key><string>ShifuDemo</string>
</dict></plist>
""")
open(f"{H}/tmux.conf", "w").write(
    'set -g status off\nset -s escape-time 10\nset -g default-terminal "tmux-256color"\n'
    'set -g window-size latest\nsetw -g aggressive-resize on\nsetw -g remain-on-exit on\n'
    'set -as terminal-features ",*:RGB"\n')

E = "\x1b"
R, BD, DIM = f"{E}[0m", f"{E}[1m", f"{E}[2m"
OR, GR, RD, BL, YL, GY, PU, CY = (f"{E}[38;5;{n}m" for n in (173, 114, 174, 111, 221, 245, 141, 80))
GD = f"{E}[38;2;190;235;195;48;2;22;58;32m"; RDD = f"{E}[38;2;240;190;190;48;2;72;26;30m"

zh = LANG == "zh"
T = dict(
    rl_prompt="给 /api/* 加限流，每分钟 120 次" if zh else "Rate-limit /api/*: 120 req/min per key",
    rl_look="先看中间件是怎么挂的。" if zh else "First, how the middleware is wired.",
    rl_wire="正在接入路由…" if zh else "Wiring it into the router…",
    rl_done=("已完成：限流接在 /api/* 上，超出返回 429，\n  并带 Retry-After。改动在 src/middleware/。" if zh else
             "Done: /api/* now returns 429 with\n  Retry-After past the limit."),
    gm_prompt="给限流写个压测脚本。" if zh else "Load-test the limiter at 120/min.",
    gm_look="先看看仓库里已有的压测工具。" if zh else "Checking which load tools the repo uses.",
    gm_run="压测进行中：每分钟 150 次请求" if zh else "Running burst scenario: 150 req/min",
    cx_prompt="升级依赖，测试通过后推送。" if zh else "Upgrade deps; push once tests pass.",
    cx_after="已推送。正在开 PR…" if zh else "Pushed. Opening a pull request…",
    labels=dict(rl="滑动窗口限流" if zh else "Sliding-window limiter", gm="限流压测" if zh else "Load test",
                cx="升级依赖" if zh else "Dependency upgrade", login="登录重定向" if zh else "Login redirect",
                orders="10 月订单分析" if zh else "October orders", vec="向量数据库调研" if zh else "Vector DB research",
                weekly="第 47 周周报" if zh else "Week 47 report"),
    waits=dict(login="session cookie 要设 SameSite=Lax 还是 Strict？" if zh else "Should the session cookie use SameSite=Lax or Strict?",
               orders="分销渠道退款率 6.9%，明细在 reports/refunds_2026_10.csv。" if zh else "Resellers refund 6.9%; details in reports/refunds_2026_10.csv.",
               vec="先用 pgvector，笔记在 research/vector-db.md。" if zh else "Start with pgvector; notes in research/vector-db.md.",
               rl="限流已接好，18 个测试通过。" if zh else "Rate limit is in; 18 tests pass."),
    tasks=["修复登录重定向", "给 API 加限流", "分析上月订单数据", "调研三个开源向量数据库", "整理会议纪要成周报"] if zh else
          ["Fix login redirect", "Add API rate limiting", "Analyze last month's orders", "Compare 3 vector databases",
           "Turn meeting notes into a weekly report"],
)


def sh_quote(s):
    return "'" + s.replace("'", "'\\''") + "'"


def script(name, body):
    path = f"{D}/scripts/{name}.sh"
    head, rest = body.split("#GO\n", 1)
    open(path, "w").write("#!/bin/bash\nprintf '\\033[?25l'\n"
                          # print the opening lines only once the app has attached and sized the pane
                          "while [ \"$(tput cols)\" -ge 120 ]; do sleep 0.1; done; sleep 0.3; clear\n" + head +
                          f"while [ ! -e {CUE}/go ]; do sleep 0.05; done\n" + rest + "\nexec sleep 864000\n")
    os.chmod(path, 0o755)
    return path


def p(text, delay=0.0):
    out = f"sleep {delay}\n" if delay else ""
    return out + f"printf '%s\\n' {sh_quote(text)}\n"


def wait(cue):
    return f"while [ ! -e {CUE}/{cue} ]; do sleep 0.05; done\n"


claude = (p(f"{OR}✻{R} {BD}Claude Code{R}  {GY}~/code/billing-service{R}") + p("") +
          p(f"{GY}>{R} {T['rl_prompt']}") + p("") + "#GO\n" +
          p(f"{GR}⏺{R} {T['rl_look']}", 0.5) +
          p(f"{GR}⏺{R} {BD}Read{R}(src/middleware/index.ts)", 0.7) + p(f"  {GY}⎿  Read 48 lines{R}", 0.3) +
          p(f"{GR}⏺{R} {BD}Update{R}(src/middleware/rateLimit.ts)", 0.8) +
          p(f"  {RDD}- const max = 600;                    {R}", 0.25) +
          p(f"  {GD}+ const limiter = new SlidingWindow({{ {R}", 0.25) +
          p(f"  {GD}+   windowMs: 60_000, max: 120,     {R}", 0.25) +
          p(f"  {GD}+   key: (req) => req.get(\"x-api-key\"),{R}", 0.25) +
          p(f"  {GD}+ }});                                 {R}", 0.25) +
          p(f"{GR}⏺{R} {BD}Bash{R}(npm test -- rateLimit)", 0.8) +
          "i=0; while [ ! -e " + CUE + "/claude-done ]; do printf '\\r" + OR + "✻ " + T['rl_wire'] + R + " " + GY +
          "(%ss)" + R + "' $((41+i/7)); i=$((i+1)); sleep 0.15; done\n"
          "printf '\\r\\033[2K'\n" +
          p(f"  {GY}⎿{R}  {GR}✓ 18 passed{R} {GY}(1.9s){R}") +
          p(f"{GR}⏺{R} {T['rl_done']}"))
gemini = (p(f"{PU}Gemini CLI{R}  {GY}~/code/billing-service{R}") + p("") +
          p(f"{BL}>{R} {T['gm_prompt']}") + p("") + "#GO\n" +
          p(f"{PU}✦{R} {T['gm_look']}", 0.6) +
          p(f" {GR}✔{R}  {BD}ReadFile{R} package.json", 0.6) +
          p(f" {GR}✔{R}  {BD}WriteFile{R} bench/rate-limit.k6.js", 0.7) +
          p(f"    {GR}+ scenarios: {{ burst: {{ rate: 150,{R}", 0.2) +
          p(f"    {GR}+   timeUnit: \"1m\", duration: \"3m\" }} }}{R}", 0.2) +
          p(f" {YL}⊷{R}  {BD}Shell{R} k6 run bench/rate-limit.k6.js", 0.6) + p("") +
          "f=(⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏); i=0; while true; do printf '\\r" + PU + "%s" + R + " " + GY + T['gm_run'] +
          " (%ss)" + R + "' \"${f[$((i%10))]}\" $((12+i/7)); i=$((i+1)); sleep 0.15; done\n")
codex = (p(f"{BD}>_ OpenAI Codex{R}  {GY}~/code/billing-service{R}") + p("") +
         p(f"{GY}›{R} {T['cx_prompt']}") + p("") + "#GO\n" +
         p(f"{GY}•{R} {BD}Ran{R} npm outdated", 0.5) + p(f"  {GY}└ 14 packages have newer versions{R}", 0.3) +
         p(f"{GY}•{R} {BD}Edited{R} package.json {GR}(+14{R} {RD}-14){R}", 0.7) +
         p(f"{GY}•{R} {BD}Ran{R} npm test", 0.6) + p(f"  {GY}└{R} {GR}✓ 186 passed{R} {GY}(12.4s){R}", 0.6) +
         p(f"{GY}•{R} {BD}Ran{R} git commit -m \"chore: upgrade deps\"", 0.5) +
         p(f"  {GY}└ [chore/upgrade-deps 3f9c2a1] 2 files{R}", 0.3) + p("") +
         wait("codex-ask") +
         p(f"{BD}Would you like to run the following command?{R}") + p("") +
         p(f"  {GY}${R} {CY}git push origin chore/upgrade-deps{R}") + p("") +
         p(f"{CY}› 1. Yes, proceed{R}") +
         p("  2. Yes, and don't ask again for this command") +
         p(f"  3. No, and tell Codex what to do differently") + p("") +
         f"printf '{GY}Press enter to confirm or esc to cancel{R}'\n"
         "read -r _\n"
         "printf '\\r\\033[9A\\033[J'\n" +
         p(f"{GY}✔{R} {GY}You approved{R} {CY}git push origin chore/upgrade-deps{R}") +
         p(f"{GY}•{R} {BD}Ran{R} git push origin chore/upgrade-deps", 0.4) +
         p(f"  {GY}└ * [new branch] chore/upgrade-deps{R}", 0.5) +
         p(f"{GY}•{R} {T['cx_after']}", 0.6))
static = {
    "claude-login": f"{OR}✻{R} {BD}Claude Code{R}\n",
    "codex-orders": f"{BD}>_ OpenAI Codex{R}\n",
    "claude-vectordb": f"{OR}✻{R} {BD}Claude Code{R}\n",
    "kimi-weekly": f"{BL}Kimi Code{R}\n",
}
scripts = {"claude-ratelimit": script("claude-ratelimit", claude), "gemini-ratelimit": script("gemini-ratelimit", gemini),
           "codex-deps": script("codex-deps", codex)}
for name, text in static.items():
    open(f"{D}/scripts/{name}.txt", "w").write(text)

tmux = ["tmux", "-L", SOCK, "-f", f"{H}/tmux.conf"]
now = int(time.time())
L, W = T["labels"], T["waits"]
agents = [  # name, kind, command, cwd, status, label, seconds ago, waiting_for
    ("claude-ratelimit", "claude", "claude", BS, "running", L["rl"], 300, None),
    ("gemini-ratelimit", "other", "gemini", BS, "running", L["gm"], 140, None),
    ("codex-deps", "codex", "codex", BS, "running", L["cx"], 200, None),
    ("claude-login", "claude", "claude", BS, "needs_reply", L["login"], 240, W["login"]),
    ("codex-orders", "codex", "codex", DR, "needs_reply", L["orders"], 420, W["orders"]),
    ("claude-vectordb", "claude", "claude", NT, "needs_reply", L["vec"], 780, W["vec"]),
    ("kimi-weekly", "kimi", "kimi", NT, "running", L["weekly"], 180, None),
]
for name, kind, cmd, cwd, status, label, ago, waiting in agents:
    shell = scripts.get(name) or f"printf '\\033[?25l'; clear; cat {D}/scripts/{name}.txt; exec sleep 864000"
    out = subprocess.run(tmux + ["new-session", "-d", "-P", "-F", "#{pane_id} #{window_id}", "-s", name, "-c", cwd,
                                 "-x", "120", "-y", "40", shell], capture_output=True, text=True, check=True).stdout.split()
    rec = dict(name=name, kind=kind, command=cmd, cwd=cwd, created_at=now - 5400, status=status, status_since=now - ago,
               window_id=out[1], pane_id=out[0], session_id=None, last_message=None, label=label)
    if waiting:
        rec["waiting_for"] = waiting
    json.dump(rec, open(f"{H}/agents/{name}.json", "w"), ensure_ascii=False)
tn = T["tasks"]
tasks = [("t1", BS, tn[0], ["claude-login"]), ("t3", BS, tn[1], ["claude-ratelimit", "gemini-ratelimit", "codex-deps"]),
         ("t4", DR, tn[2], ["codex-orders"]), ("t6", NT, tn[3], ["claude-vectordb"]), ("t5", NT, tn[4], ["kimi-weekly"])]
json.dump({"current": "t3", "project_order": [BS, DR, NT],
           "tasks": [dict(id=i, project=pr, name=n, auto_name=False, created_at=now - 9000 + k * 1000, agents=a)
                     for k, (i, pr, n, a) in enumerate(tasks)]},
          open(f"{H}/tasks.json", "w"), ensure_ascii=False)


def panel(name):
    return {"id": name, "contentComponent": "terminal", "tabComponent": "agent", "params": {"agent": name}, "title": name}


layout = {
    "grid": {
        "root": {"type": "branch", "size": 760, "data": [
            {"type": "branch", "size": 476, "data": [
                {"type": "leaf", "size": 400, "data": {"views": ["claude-ratelimit"], "activeView": "claude-ratelimit", "id": "1"}},
                {"type": "leaf", "size": 360, "data": {"views": ["gemini-ratelimit"], "activeView": "gemini-ratelimit", "id": "2"}},
            ]},
            {"type": "leaf", "size": 476, "data": {"views": ["codex-deps"], "activeView": "codex-deps", "id": "3"}},
        ]},
        "width": 952, "height": 760, "orientation": "HORIZONTAL",
    },
    "panels": {n: panel(n) for n in ("claude-ratelimit", "gemini-ratelimit", "codex-deps")},
    "activeGroup": "1",
}
json.dump(layout, open(f"{H}/layouts/t3.json", "w"))
print("staged", LANG)
