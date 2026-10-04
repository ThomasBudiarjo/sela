#!/usr/bin/env python3
"""Bounded native spike supervisor; no service or display changes."""
import os
import pathlib
import subprocess
import time
import json
import sys

out = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else '.amp/in/artifacts/output-spike')
out.mkdir(parents=True, exist_ok=True)
binary = os.environ.get('CARGO_TARGET_DIR', '/home/user/workspace/repo/target') + '/debug/examples/output_spike'
env = dict(os.environ, DISPLAY=':99', XDG_RUNTIME_DIR='/tmp/sela-runtime', VK_DRIVER_FILES='/dev/null', SELA_SPIKE_BACKEND=os.environ.get('SELA_SPIKE_BACKEND', 'gl'))
children = []
files = []
def cmd(*args):
    return subprocess.check_output(args, env=env, timeout=5, text=True).strip()
def window(name):
    for _ in range(50):
        try:
            return cmd('xdotool', 'search', '--name', name).splitlines()[-1]
        except subprocess.CalledProcessError:
            time.sleep(.1)
    raise RuntimeError('window missing: ' + name)
def capture(name):
    # Root capture only; retain complete operator/audience view.
    subprocess.run(['import', '-window', 'root', str(out / (name + '.png'))], env=env, check=True, timeout=5)
def launch(name, *args):
    f = open(out / (name + '.log'), 'w')
    files.append(f)
    p = subprocess.Popen([binary, *args], env=env, stdout=f, stderr=f)
    children.append(p)
    return p
try:
    audience = launch('audience', '--audience')
    aw = window('^Sela audience spike$')
    operator = launch('operator')
    ow = window('^Sela output operator spike$')
    cmd('xdotool', 'windowactivate', '--sync', ow)
    time.sleep(1)
    capture('baseline')
    for key, delay in [('ctrl+1', .4), ('ctrl+2', .8), ('ctrl+3', 2.4)]:
        cmd('xdotool', 'key', '--clearmodifiers', key)
        if key == 'ctrl+3':
            time.sleep(.25)
            capture('stall-a')
            time.sleep(.5)
            capture('stall-b')
            # Queue an action behind the stall to distinguish control responsiveness.
            cmd('xdotool', 'key', '--clearmodifiers', 'ctrl+1')
        time.sleep(delay)
    cmd('xdotool', 'windowsize', aw, '600', '320')
    time.sleep(.5)
    cmd('xdotool', 'key', '--clearmodifiers', 'ctrl+q')
    operator.wait(timeout=5)
    exit_us = time.time_ns() // 1000
    time.sleep(1)
    capture('after-exit-a')
    time.sleep(.5)
    capture('after-exit-b')
    # A second operator is forcibly terminated; parent supervisor remains owner.
    operator = launch('operator-killed')
    window('^Sela output operator spike$')
    time.sleep(.5)
    operator.kill()
    operator.wait(timeout=5)
    killed_us = time.time_ns() // 1000
    time.sleep(1)
    capture('after-kill')
    cmd('xdotool', 'windowactivate', '--sync', aw)
    cmd('xdotool', 'key', '--clearmodifiers', 'alt+F4')
    assert audience.wait(timeout=5) == 0
    rows = (out / 'audience.log').read_text().splitlines()
    frames = [(int(p[1]), int(p[4])) for r in rows if len(p := r.split()) >= 5 and p[2] == 'present_call']
    op = (out / 'operator.log').read_text().splitlines()
    intervals = []
    begin = None
    for r in op:
        p = r.split()
        if len(p) >= 4 and p[2] == 'stall_begin':
            begin = (int(p[1]), int(p[3]))
        if len(p) >= 4 and p[2] == 'stall_end':
            end = int(p[1])
            inside = [t for t, _ in frames if begin[0] <= t <= end]
            assert len(inside) >= 2, (begin, end, inside)
            intervals.append(dict(start_us=begin[0], end_us=end, requested_ms=begin[1], present_calls=len(inside), max_gap_ms=max(b-a for a,b in zip(inside, inside[1:]))/1000))
    assert len(intervals) == 4
    assert sum(t > exit_us for t,_ in frames) > 10
    assert sum(t > killed_us for t,_ in frames) > 10
    gaps = sorted((b[1]-a[1])/1000 for a,b in zip(frames,frames[1:]))
    summary = dict(stalls=intervals, operator_exit_observed_us=exit_us, operator_kill_observed_us=killed_us, calls_after_exit=sum(t > exit_us for t,_ in frames), calls_after_kill=sum(t > killed_us for t,_ in frames), frames=len(frames), gap_p50_ms=gaps[len(gaps)//2], gap_p95_ms=gaps[int(len(gaps)*.95)], gap_max_ms=max(gaps))
    (out / 'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
    print(json.dumps(summary, indent=2))
finally:
    for p in children:
        if p.poll() is None:
            p.terminate()
            try:
                p.wait(timeout=3)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait(timeout=3)
    for f in files:
        f.close()
