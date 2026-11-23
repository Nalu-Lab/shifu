#!/usr/bin/env python3
"""Two-start setup, then record. Usage: run4.py zh|en
Start 1 records every waiting row as seen. Before start 2 the codex and claude records are set to the waiting
states they reach later in the story (with fixed timestamps), so start 2 counts them as already announced;
they are flipped back to running, then the story flips them again with the same timestamps. The app therefore
never tries to post a notification, and the finished row still reads 做完了."""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__))
import demo4
LANG = sys.argv[1]
S = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
H = "/tmp/demo/home/agents"
zh = LANG == "zh"
DONE = "限流已接好，18 个测试通过。" if zh else "Rate limit is in; 18 tests pass."

def setrec(name, **kw):
    p = f"{H}/{name}.json"; d = json.load(open(p)); d.update(kw)
    d = {k: v for k, v in d.items() if v is not None}
    json.dump(d, open(p + ".tmp", "w"), ensure_ascii=False); os.replace(p + ".tmp", p)

demo4.teardown()
print(subprocess.run(["python3", f"{S}/tools/stage4.py", LANG], capture_output=True, text=True).stdout.strip())
demo4.launch(); time.sleep(6)
subprocess.run(["pkill", "-TERM", "-f", demo4.EXE]); time.sleep(2)
subprocess.run(["pkill", "-KILL", "-f", demo4.EXE]); time.sleep(0.5)
now = int(time.time())
since = {"codex": now - 70, "claude": now - 40}
setrec("codex-deps", status="needs_approval", status_since=since["codex"], waiting_for="git push origin chore/upgrade-deps")
setrec("claude-ratelimit", status="needs_reply", status_since=since["claude"], waiting_for=DONE)
json.dump(since, open("/tmp/demo/since.json", "w"))
wid = demo4.launch(); time.sleep(4)
setrec("codex-deps", status="running", waiting_for=None)
setrec("claude-ratelimit", status="running", waiting_for=None)
open("/tmp/demo/winid", "w").write(str(wid))
time.sleep(3)
out = f"{S}/rec-{LANG}"
subprocess.run(["rm", "-rf", out])
subprocess.run(["python3", f"{S}/tools/record4.py", out, LANG])
print(open("/tmp/demo/app.log").read()[-600:])
demo4.teardown()
print("front app now:", demo4.front_bundle(), "| demo procs:", subprocess.run(["pgrep", "-fl", "ShifuDemo|shifu-demo"], capture_output=True, text=True).stdout.strip() or "none")
