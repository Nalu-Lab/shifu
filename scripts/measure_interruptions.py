#!/usr/bin/env python3
"""Count what a person at this Mac would notice while a command or scenario runs.

    python3 scripts/measure_interruptions.py NEW_DIR -- shifu browser open http://127.0.0.1:8000

As a module: `with Monitor(work_dir, 'name') as m: ...` then read `m.report`.
Counts app activations, frontmost-window owner changes, Space switches, pointer movements and
clipboard writes (see interruption-monitor.swift). Events while the person was typing or moving the
mouse are flagged `personActive`; the scenario should be rerun if any appear.
"""
import json
import os
import pathlib
import signal
import subprocess
import sys
import time

SOURCE = pathlib.Path(__file__).with_name('interruption-monitor.swift')


def build(work_dir):
    binary = pathlib.Path(work_dir) / 'interruption-monitor'
    if not binary.exists():
        subprocess.run(['swiftc', '-O', str(SOURCE), '-o', str(binary)], check=True)
    return binary


class Monitor:
    def __init__(self, work_dir, name):
        self.work_dir = pathlib.Path(work_dir)
        self.out = self.work_dir / f'{name}.interruptions.json'
        self.report = None

    def __enter__(self):
        binary = build(self.work_dir)
        ready = pathlib.Path(str(self.out) + '.ready')
        self.process = subprocess.Popen([str(binary), str(self.out)])
        deadline = time.time() + 10
        while not ready.exists():
            if time.time() > deadline or self.process.poll() is not None:
                raise RuntimeError('interruption monitor did not start')
            time.sleep(0.05)
        ready.unlink()
        time.sleep(0.2)
        return self

    def __exit__(self, *exc):
        # Let late activations and restores land before stopping.
        time.sleep(0.8)
        self.process.send_signal(signal.SIGTERM)
        self.process.wait(timeout=10)
        self.report = json.loads(self.out.read_text())
        return False


def seconds_since_person_input():
    """Seconds since the person's last keyboard or mouse input (hardware event types only)."""
    import ctypes
    cg = ctypes.CDLL('/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics')
    since = cg.CGEventSourceSecondsSinceLastEventType
    since.restype, since.argtypes = ctypes.c_double, [ctypes.c_int, ctypes.c_uint32]
    # keyDown, flagsChanged, mouseMoved, left/right/other down, leftDragged, scrollWheel; 1 = HID system state.
    return min(since(1, kind) for kind in (10, 12, 5, 1, 3, 25, 6, 22))


def main():
    if len(sys.argv) < 4 or sys.argv[2] != '--':
        sys.exit(__doc__)
    work = pathlib.Path(sys.argv[1])
    work.mkdir(parents=True, exist_ok=False)
    with Monitor(work, 'command') as monitor:
        status = subprocess.run(sys.argv[3:]).returncode
    summary = {key: monitor.report[key] for key in ('interruptions', 'counts', 'personActiveEvents', 'restored')}
    print(json.dumps({'exitStatus': status, **summary}, ensure_ascii=False, indent=2))
    return status


if __name__ == '__main__':
    sys.exit(main())
