"""Shared helpers: launch the demo app without keeping focus, find its window, quit, tear down."""
import json, os, re, subprocess, time
SC = os.path.expanduser("~/.cargo/bin/shifu")
ENV = dict(os.environ, SHIFU_APP_HOME="/tmp/demo/home", SHIFU_APP_SOCKET="shifu-app-demo", SHIFU_APP_SHIFU="/tmp/demo/shifu")
EXE = "/tmp/demo/ShifuDemo/Contents/MacOS/shifu-demo"

def sh(*a, **k):
    return subprocess.run(a, capture_output=True, text=True, **k)

def front_bundle():
    asn = sh("lsappinfo", "front").stdout.strip()
    m = re.search(r'(?:"CFBundleIdentifier"|bundleID)="([^"]+)"', sh("lsappinfo", "info", "-only", "bundleid", asn).stdout)
    return m.group(1) if m else None

def win_id():
    r = sh(SC, "computer", "list-windows", "--app", "invalid.shifu-site-demo", "--json")
    try:
        ws = json.loads(r.stdout)["result"]["windows"]
        return ws[0]["id"] if ws else None
    except Exception:
        return None

def launch():
    prev = front_bundle()
    subprocess.Popen([EXE], cwd="/tmp/demo", env=ENV, stdout=open("/tmp/demo/app.log", "a"),
                     stderr=subprocess.STDOUT, start_new_session=True)
    wid = None
    for _ in range(80):
        time.sleep(0.1)
        wid = win_id()
        if wid:
            break
    if prev and prev != "invalid.shifu-site-demo":
        sh("open", "-b", prev)
    print("window", wid, "focus back to", prev, "now", front_bundle())
    return wid

def teardown():
    sh("pkill", "-f", EXE); time.sleep(1)
    sh("tmux", "-L", "shifu-app-demo", "kill-server")
    sh("bash", "-c", "rm -rf /tmp/demo ~/Library/WebKit/shifu-demo ~/Library/Caches/shifu-demo "
       "~/Library/WebKit/invalid.shifu-site-demo ~/Library/Caches/invalid.shifu-site-demo")
