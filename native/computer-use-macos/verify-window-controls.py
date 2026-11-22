#!/usr/bin/env python3
"""Test discovery, capture, and postconditions against disposable background windows."""
from contextlib import contextmanager
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

output = Path(sys.argv[1]).resolve()
output.mkdir(parents=True, exist_ok=False)
fixture = Path(sys.argv[2]).resolve()
cli = os.environ['SHIFU_CLI']
home = Path(os.environ['SHIFU_COMPUTER_HOME']).resolve()
assert home != Path.home() / '.shifu/computer', 'use an isolated broker'
counter = output / 'counter.txt'
checks = {}


def command(*args):
    run = subprocess.run([cli, 'computer', *map(str, args)], capture_output=True, text=True, timeout=45)
    assert run.stdout, run.stderr
    return json.loads(run.stdout)


@contextmanager
def launch(**settings):
    env = dict(os.environ, SHIFU_FIXTURE_COUNTER_PATH=str(counter), **settings)
    with (output / 'fixture.log').open('a') as log, subprocess.Popen([fixture], env=env, stderr=log) as child:
        def call(method, *args, ok=True):
            result = command(method, '--app', f'pid:{child.pid}', *args)
            assert ok is None or result['ok'] == ok, result.get('error', result)
            return result

        try:
            yield call
        finally:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()


def wait_windows(call, predicate):
    end = time.monotonic() + 8
    while True:
        result = call('list-windows', '--background-only', ok=None)
        if result['ok']:
            windows = result['result']['windows']
        else:
            assert result['error']['code'] == 'app_not_found', result
            windows = []
        if predicate(windows):
            return windows
        assert time.monotonic() < end, windows
        time.sleep(.2)


def button(snapshot, title):
    match = re.search(r'^\s*(\d+) button ' + re.escape(title) + r'\s*$', snapshot['treeText'], re.M)
    assert match, snapshot['treeText']
    return int(match[1])


def verify_dimensions(shot):
    probe = subprocess.check_output(['sips', '-g', 'pixelWidth', '-g', 'pixelHeight', shot['path']], text=True, timeout=5)
    assert f"pixelWidth: {shot['width']}\n" in probe and f"pixelHeight: {shot['height']}\n" in probe, probe


try:
    with launch(SHIFU_FIXTURE_OBSERVATION='1', SHIFU_FIXTURE_WINDOW_CONTROLS='1') as call:
        main, existing = wait_windows(call, lambda windows: len(windows) == 2)
        assert main['title'] == 'Shifu Computer Use 测试窗口'
        by_id = ('--window-id', main['id'])
        by_index = ('--window-index', main['index'])
        response = call('get-app-state', *by_index, '--background-only', '--screenshot-format', 'jpeg', '--screenshot-max-edge', 256)
        index_snapshot = response['result']['snapshot']
        jpeg = response['result']['screenshot']
        assert jpeg['format'] == 'jpeg' and Path(jpeg['path']).suffix == '.jpg'
        assert Path(jpeg['path']).read_bytes().startswith(bytes.fromhex('ffd8ff'))
        assert max(jpeg['width'], jpeg['height']) <= 256
        verify_dimensions(jpeg)
        png = call('get-app-state', *by_id, '--background-only', '--screenshot-scale', .5)['result']['screenshot']
        assert png['format'] == 'png' and Path(png['path']).read_bytes().startswith(bytes.fromhex('89504e470d0a1a0a'))
        assert png['scaleX'] <= .5 and png['scaleY'] <= .5
        verify_dimensions(png)
        checks['jpeg_max_edge'] = [jpeg['width'], jpeg['height']]
        checks['png_max_scale'] = [png['scaleX'], png['scaleY']]

        disabled = call('get-app-state', *by_id, '--wait-for-enabled', '延迟完成', '--timeout-ms', 0,
                        '--background-only', '--no-screenshot', ok=False)
        assert disabled['error']['code'] == 'postcondition_timeout'

        # A second app can have the same window index in the same CLI session.
        with launch() as other_app:
            wait_windows(other_app, lambda windows: len(windows) == 1)
            second = other_app('get-app-state', *by_index, '--background-only', '--no-screenshot')['result']['snapshot']
            assert second['app']['pid'] != index_snapshot['app']['pid']

        increment = button(index_snapshot, '增加计数')
        old = call('click', *by_index, '--element-index', increment, '--wait-for-window', existing['title'],
                   '--timeout-ms', 0, '--no-screenshot', ok=False)
        assert old['error']['code'] == 'postcondition_timeout' and counter.read_text() == '1'
        assert old['result']['action']['targetWindowId'] == main['id']
        checks['same_index_in_another_app_does_not_replace_the_original'] = True
        wait_windows(call, lambda windows: len(windows) == 2 and windows[0]['id'] == existing['id'])
        stale = call('click', *by_index, '--element-index', increment, '--no-screenshot', ok=False)
        assert stale['error']['code'] == 'window_stale' and counter.read_text() == '1'
        checks['existing_window_and_stale_index_rejected'] = True

        state = call('get-app-state', *by_id, '--no-screenshot')['result']['snapshot']
        result = call('click', *by_id, '--element-index', button(state, '打开结果'),
                      '--wait-for-enabled', '延迟完成', '--wait-for-window', existing['title'],
                      '--timeout-ms', 5000, '--no-screenshot')['result']
        assert result['action']['targetWindowId'] == main['id'] and counter.read_text() == '2'
        enabled, created = result['postcondition']['checks']
        assert enabled['met'] and enabled['windowId'] == main['id']
        assert created['met'] and created['window']['id'] not in (main['id'], existing['id'])
        assert result['postcondition']['foregroundFallbackUsed'] is False
        checks['async_window_and_enabled_control_action_count'] = 1

    for visibility in ('minimized', 'app_hidden'):
        with launch(SHIFU_FIXTURE_VISIBILITY=visibility) as call:
            window = wait_windows(call, lambda windows: any(w['visibility'] == visibility for w in windows))[0]
            assert window['isOffscreen'] and window['isMinimized'] == (visibility == 'minimized')
            result = call('get-app-state', '--window-id', window['id'], '--background-only')['result']
            snapshot = result['snapshot']
            assert snapshot['window']['id'] == window['id'] and snapshot['window']['visibility'] == visibility
            assert snapshot['foreground']['state'] == 'other_app' and '测试输入框' in snapshot['treeText']
            assert result['screenshot'] is None and result['screenshotStatus']['state'] == 'failed'
            blocked = call('click', '--window-id', window['id'], '--x', 60, '--y', 160, '--no-screenshot', ok=False)
            assert blocked['error']['code'] == 'background_unavailable' and counter.read_text() == '0'
            checks[visibility] = {'read_by_id': True, 'screenshot_failure_reported': True, 'coordinate_click_rejected': True}
finally:
    command('stop')
    (output / 'result.json').write_text(json.dumps(checks, indent=2))
print(json.dumps(checks, indent=2))
