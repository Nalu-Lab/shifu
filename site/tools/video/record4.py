#!/usr/bin/env python3
"""Record the story from the running isolated demo. Usage: record4.py <out-dir> <lang>
Frames come from `screencapture -l <window>` (reads the demo window only, never raises it). State changes go
through the demo's own agent records, cue files and tmux send-keys on the demo socket; no input reaches the desktop."""
import json, os, subprocess, sys, threading, time
OUT, LANG = sys.argv[1], sys.argv[2]
os.makedirs(OUT, exist_ok=True)
WID = open("/tmp/demo/winid").read().strip()
H = "/tmp/demo/home/agents"
zh = LANG == "zh"
DONE = "限流已接好，18 个测试通过。" if zh else "Rate limit is in; 18 tests pass."

SINCE = json.load(open("/tmp/demo/since.json"))
def setrec(name, **kw):
    p = f"{H}/{name}.json"; d = json.load(open(p)); d.update(kw)
    d = {k: v for k, v in d.items() if v is not None}
    json.dump(d, open(p + ".tmp", "w"), ensure_ascii=False); os.replace(p + ".tmp", p)

stop = False
t0 = None
def worker(k):
    while not stop:
        ts = time.time() - t0
        subprocess.run(["screencapture", "-x", "-o", "-l", WID, f"{OUT}/f_{ts:07.3f}_{k}.png"])

events = [
    (4.6, "codex asks", lambda: (open("/tmp/demo/cue/codex-ask", "w").close(),
                                 setrec("codex-deps", status="needs_approval", status_since=SINCE["codex"],
                                        waiting_for="git push origin chore/upgrade-deps"))),
    (8.4, "approve", lambda: (subprocess.run(["tmux", "-L", "shifu-app-demo", "send-keys", "-t", "=codex-deps:", "Enter"]),
                              setrec("codex-deps", status="running", waiting_for=None))),
    (11.2, "claude done", lambda: (open("/tmp/demo/cue/claude-done", "w").close(),
                                   setrec("claude-ratelimit", status="needs_reply", status_since=SINCE["claude"], waiting_for=DONE))),
]
t0 = time.time() + 0.6
ths = [threading.Thread(target=worker, args=(k,)) for k in range(3)]
for i, th in enumerate(ths):
    time.sleep(0.2 if i == 0 else 0.09)
    th.start()
while time.time() < t0: time.sleep(0.01)
open("/tmp/demo/cue/go", "w").close()
log = []
for at, name, fn in events:
    while time.time() - t0 < at: time.sleep(0.01)
    fn(); log.append((round(time.time() - t0, 2), name))
while time.time() - t0 < 15.5: time.sleep(0.05)
stop = True
for th in ths: th.join()
json.dump(log, open(f"{OUT}/events.json", "w"))
print(log, len([f for f in os.listdir(OUT) if f.endswith('.png')]), "frames")
