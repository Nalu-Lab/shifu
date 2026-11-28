#!/usr/bin/env python3
"""Count interruptions caused by shifu browser / shifu computer on disposable targets only.

    python3 scripts/verify-quiet.py NEW_DIR --cli /abs/shifu [--fixture /abs/compiled-Fixture]
        [--scenarios browser-default,browser-headless,browser-show,computer-background,computer-paste,
                     computer-foreground,computer-gate]

Every scenario runs under measure_interruptions.Monitor (app activations, frontmost-window owner,
Space switches, pointer movement, clipboard writes). Targets: a localhost page this script serves,
an isolated SHIFU_BROWSER_HOME, and the AppKit Fixture process it starts (addressed by PID only).
Foreground scenarios run only after the person has been idle for --idle-seconds (default 15); otherwise
they are reported as not_run. Use a candidate helper with SHIFU_COMPUTER_HELPER; SHIFU_COMPUTER_HOME is
always a fresh short directory so the person's own helper connection is untouched.
"""
import argparse
import functools
import http.server
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from measure_interruptions import Monitor, seconds_since_person_input  # noqa: E402

PAGE = '<!doctype html><title>Quiet fixture</title><h1>Quiet fixture</h1><input aria-label=q><button onclick="document.title=\'clicked\'">Go</button>'


def serve(root):
    root.mkdir()
    (root / 'index.html').write_text(PAGE)
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(root))
    handler.log_message = lambda *a: None
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, f'http://127.0.0.1:{server.server_port}/index.html'


class Run:
    def __init__(self, args):
        self.out = pathlib.Path(args.out).resolve()
        self.out.mkdir(parents=True, exist_ok=False)
        self.cli = args.cli
        self.idle = args.idle_seconds
        self.step = 0
        self.env = dict(os.environ)
        self.env['SHIFU_BROWSER_HOME'] = str(self.out / 'browser-home')
        self.env.setdefault('SHIFU_BROWSER_RUNTIME', str(pathlib.Path.home() / '.shifu/browser/runtime'))
        self.computer_home = pathlib.Path(tempfile.mkdtemp(prefix='shq-', dir='/tmp'))
        self.env['SHIFU_COMPUTER_HOME'] = str(self.computer_home)
        self.env.pop('SHIFU_COMPUTER_FOREGROUND', None)
        self.env.pop('SHIFU_COMPUTER_IDLE_SECONDS', None)
        self.report = {'cli': self.cli, 'helper': self.env.get('SHIFU_COMPUTER_HELPER', 'installed'), 'scenarios': {}}

    def call(self, tool, *args, env=None, timeout=90):
        self.step += 1
        p = subprocess.run([self.cli, tool, *map(str, args)], capture_output=True, text=True, timeout=timeout,
                           env=dict(self.env, **(env or {})), cwd=self.out)
        (self.out / f'{self.step:02d}-{tool}-{args[0] if args else ""}.json').write_text(p.stdout + '\n' + p.stderr)
        try:
            return json.loads(p.stdout)
        except ValueError:
            return {'ok': False, 'error': {'code': 'no_json', 'message': p.stderr.strip()[-400:]}}

    def scenario(self, name, body, foreground=False):
        if foreground:
            idle = seconds_since_person_input()
            if idle < self.idle:
                self.report['scenarios'][name] = {'state': 'not_run', 'reason': f'person active {idle:.1f}s ago'}
                return
        steps = []
        with Monitor(self.out, name) as monitor:
            try:
                body(lambda tool, *a, **k: steps.append(self._summarize(a, self.call(tool, *a, **k))) or steps[-1])
                state = 'ran'
            except Exception as error:  # keep measuring the others
                state = f'error: {error}'
        r = monitor.report
        self.report['scenarios'][name] = {
            'state': state, 'interruptions': r['interruptions'], 'counts': r['counts'],
            'personActiveEvents': r['personActiveEvents'], 'restored': r['restored'],
            'events': r['events'], 'steps': steps,
        }

    @staticmethod
    def _summarize(args, response):
        result = response.get('result') if isinstance(response.get('result'), dict) else {}
        return {'args': [str(a) for a in args][:4], 'ok': response.get('ok'),
                'error': (response.get('error') or {}).get('code'),
                'delivery': (result.get('action') or {}).get('delivery'),
                'foregroundRestore': result.get('foregroundRestore'),
                'foregroundGate': result.get('foregroundGate'), 'focus': result.get('focus'),
                'response': response}


def element(snapshot, role, text=''):
    pattern = r'^\s*(\d+) ' + re.escape(role) + (r'\b.*' + re.escape(text) if text else r'\b')
    match = re.search(pattern, snapshot['treeText'], re.M)
    if not match:
        raise RuntimeError(f'{role} {text} not in tree')
    return int(match[1])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('out')
    parser.add_argument('--cli', required=True)
    parser.add_argument('--fixture')
    parser.add_argument('--idle-seconds', type=float, default=15)
    parser.add_argument('--scenarios', default='browser-default,browser-headless,computer-background,computer-paste,computer-foreground')
    args = parser.parse_args()
    run = Run(args)
    wanted = args.scenarios.split(',')
    server, url = serve(run.out / 'site')
    fixture = None
    try:
        for name, flags in (('browser-default', []), ('browser-headless', ['--headless']), ('browser-show', ['--show'])):
            if name not in wanted:
                continue
            session = f'quiet-{name}-{os.getpid()}'

            def browse(call, flags=flags, session=session):
                call('browser', '--session', session, 'open', url, *flags)
                call('browser', '--session', session, 'snapshot')
                call('browser', '--session', session, 'click', 'e3')
                call('browser', '--session', session, 'tab-new', url)
                call('browser', '--session', session, 'close')
            run.scenario(name, browse, foreground=name != 'browser-headless')

        if any(s.startswith('computer') for s in wanted):
            if not args.fixture:
                sys.exit('computer scenarios need --fixture')
            fixture = subprocess.Popen([str(pathlib.Path(args.fixture).resolve())], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                       env=dict(os.environ, SHIFU_FIXTURE_PASTE='1'))
            app = f'pid:{fixture.pid}'
            deadline = time.time() + 10
            while True:
                windows = run.call('computer', 'list-windows', '--app', app).get('result', {}).get('windows', [])
                if windows or time.time() > deadline:
                    break
                time.sleep(0.2)
            window = windows[0]['id']
            state = run.call('computer', 'get-app-state', '--app', app, '--window-id', window, '--no-screenshot')
            snapshot = state['result']['snapshot']
            button, field = element(snapshot, 'button'), element(snapshot, 'text field')
            base = ['--app', app, '--window-id', window, '--no-screenshot']

            def background(call):
                call('computer', 'get-app-state', *base)
                call('computer', 'click', *base, '--element-index', button)
                call('computer', 'click', *base, '--x', 60, '--y', 70)
                call('computer', 'set-value', *base, '--element-index', field, '--value', 'quiet')
                call('computer', 'type-text', *base, '--element-index', field, '--text', ' typed')
                call('computer', 'press-key', *base, '--key', 'Tab')
                call('computer', 'scroll', *base, '--element-index', field, '--direction', 'down')

            def paste(call):
                call('computer', 'paste-text', *base, '--text', 'pasted')

            def foreground(call):
                call('computer', 'click', *base, '--element-index', button, '--restore-window')
                call('computer', 'click', *base, '--x', 60, '--y', 70, '--restore-window')
                call('computer', 'press-key', *base, '--key', 'Tab', '--restore-window')
                call('computer', 'get-app-state', *base, '--wait-for', 'NEVER-PRESENT-PROBE', '--timeout-ms', 1200,
                     '--foreground-fallback')

            def gate(call):
                call('computer', 'click', *base, '--element-index', button, '--restore-window',
                     env={'SHIFU_COMPUTER_IDLE_SECONDS': '86400'})
                call('computer', 'paste-text', *base, '--text', 'x', env={'SHIFU_COMPUTER_FOREGROUND': 'never'})
                call('computer', 'get-app-state', *base, '--wait-for', 'NEVER-PRESENT-PROBE', '--timeout-ms', 1000,
                     '--foreground-fallback', env={'SHIFU_COMPUTER_FOREGROUND': 'never'})

            for name, body, fg in (('computer-background', background, False), ('computer-paste', paste, True),
                                   ('computer-foreground', foreground, True), ('computer-gate', gate, False)):
                if name in wanted:
                    run.scenario(name, body, foreground=fg)
    finally:
        if fixture:
            fixture.terminate()
            fixture.wait(timeout=10)
        run.call('computer', 'stop')
        shutil.rmtree(run.computer_home, ignore_errors=True)
        server.shutdown()
        (run.out / 'report.json').write_text(json.dumps(run.report, ensure_ascii=False, indent=2))
    for name, scenario in run.report['scenarios'].items():
        print(name, json.dumps({k: scenario.get(k) for k in ('state', 'interruptions', 'counts', 'personActiveEvents', 'restored', 'reason')}, ensure_ascii=False))
        for step in scenario.get('steps', []):
            print('   ', ' '.join(step['args']), 'ok' if step['ok'] else f"FAIL {step['error']}",
                  step['delivery'] or '', json.dumps(step['foregroundRestore']) if step['foregroundRestore'] else '')


if __name__ == '__main__':
    main()
