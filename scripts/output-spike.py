#!/usr/bin/env python3
"""Bounded native spike supervisor; no service or display changes."""

import itertools
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time

out = pathlib.Path(
    sys.argv[1] if len(sys.argv) > 1 else ".amp/in/artifacts/output-spike"
)
out.mkdir(parents=True, exist_ok=True)
root = pathlib.Path(__file__).resolve().parent.parent
binary = (
    pathlib.Path(os.environ.get("CARGO_TARGET_DIR", root / "target"))
    / "debug/examples/output_spike"
)
if not os.environ.get("DISPLAY"):
    sys.exit("Set DISPLAY to an existing X11 session with a window manager.")
if not binary.is_file():
    sys.exit(f"Build the output_spike example first: {binary}")
scratch = tempfile.TemporaryDirectory(prefix="sela-output-")
env = dict(os.environ, XDG_RUNTIME_DIR=scratch.name)
children = []


def cmd(*args):
    return subprocess.check_output(args, env=env, timeout=5, text=True).strip()


def window(process):
    for _ in range(50):
        if process.poll() is not None:
            raise RuntimeError(f"process {process.pid} exited before window discovery")
        try:
            return cmd(
                "xdotool", "search", "--onlyvisible", "--pid", str(process.pid)
            ).splitlines()[0]
        except subprocess.CalledProcessError:
            time.sleep(0.1)
    raise RuntimeError(f"window missing for process {process.pid}")


def key(target, stroke):
    if cmd("xdotool", "getwindowfocus") != target:
        raise RuntimeError(
            "focus moved outside the test window; do not run concurrent input tests"
        )
    cmd("xdotool", "key", "--clearmodifiers", stroke)


def capture(name):
    # Capture root for compatibility, retain only interiors of owned windows.
    image = pathlib.Path(scratch.name) / "root.png"
    subprocess.run(
        ["import", "-window", "root", str(image)], env=env, check=True, timeout=5
    )
    crops = []
    for process in children:
        if process.poll() is not None:
            continue
        target = window(process)
        geometry = dict(
            line.split("=", 1)
            for line in cmd(
                "xdotool", "getwindowgeometry", "--shell", target
            ).splitlines()
        )
        crop = pathlib.Path(scratch.name) / f"{process.pid}.png"
        bounds = "{WIDTH}x{HEIGHT}+{X}+{Y}".format(**geometry)
        subprocess.run(
            ["convert", str(image), "-crop", bounds, "+repage", str(crop)],
            check=True,
            timeout=5,
        )
        crops.append(str(crop))
    if not crops:
        raise RuntimeError("no owned windows to capture")
    subprocess.run(
        ["convert", *crops, "+append", str(out / (name + ".png"))],
        check=True,
        timeout=5,
    )


def launch(name, *args):
    with open(out / (name + ".log"), "w") as f:
        p = subprocess.Popen([binary, *args], env=env, stdout=f, stderr=f)
    children.append(p)
    return p


try:
    audience = launch("audience", "--audience")
    aw = window(audience)
    operator = launch("operator")
    ow = window(operator)
    cmd("xdotool", "windowactivate", "--sync", ow)
    time.sleep(1)
    capture("baseline")
    key(ow, "ctrl+p")
    key(ow, "ctrl+p")  # Must reject, not enqueue a second preparation.
    time.sleep(0.2)
    capture("preparing")
    key(ow, "ctrl+1")  # Operator must handle this while preparation is pending.
    time.sleep(2.2)
    capture("prepared")
    for stroke, delay in [("ctrl+1", 0.4), ("ctrl+2", 0.8), ("ctrl+3", 2.4)]:
        key(ow, stroke)
        if stroke == "ctrl+3":
            time.sleep(0.25)
            capture("stall-a")
            time.sleep(0.5)
            capture("stall-b")
            # Queue an action behind the stall to distinguish control responsiveness.
            key(ow, "ctrl+1")
        time.sleep(delay)
    cmd("xdotool", "windowsize", aw, "600", "320")
    time.sleep(0.5)
    key(ow, "ctrl+q")
    assert operator.wait(timeout=5) == 0
    exit_us = time.time_ns() // 1000
    time.sleep(1)
    capture("after-exit-a")
    time.sleep(0.5)
    capture("after-exit-b")
    # A second operator is forcibly terminated; parent supervisor remains owner.
    operator = launch("operator-killed")
    window(operator)
    time.sleep(0.5)
    operator.kill()
    operator.wait(timeout=5)
    killed_us = time.time_ns() // 1000
    time.sleep(1)
    capture("after-kill")
    cmd("xdotool", "windowactivate", "--sync", aw)
    key(aw, "alt+F4")
    assert audience.wait(timeout=5) == 0
    rows = (out / "audience.log").read_text().splitlines()
    frames = [
        (int(p[1]), int(p[4]))
        for r in rows
        if len(p := r.split()) >= 5 and p[2] == "present_call"
    ]
    op = (out / "operator.log").read_text().splitlines()
    intervals = []
    begin = None
    for r in op:
        p = r.split()
        if len(p) >= 4 and p[2] == "stall_begin":
            begin = (int(p[1]), int(p[3]))
        if len(p) >= 4 and p[2] == "stall_end":
            end = int(p[1])
            assert begin is not None and int(p[3]) == begin[1]
            assert end - begin[0] >= begin[1] * 1000
            inside = [t for t, _ in frames if begin[0] <= t <= end]
            assert len(inside) >= 2, (begin, end, inside)
            edges = [begin[0], *inside, end]
            gap = max(b - a for a, b in itertools.pairwise(edges)) / 1000
            # Detect long freezes, including start/end gaps. Not a product frame SLO.
            if begin[1] == 2000:
                assert gap < 500, f"audience froze during UI stall: {gap}ms"
            intervals.append(
                {
                    "start_us": begin[0],
                    "end_us": end,
                    "requested_ms": begin[1],
                    "present_calls": len(inside),
                    "max_gap_ms": gap,
                }
            )
            begin = None
    assert [i["requested_ms"] for i in intervals] == [100, 100, 500, 2000, 100]
    assert begin is None
    preparation = {}
    for row in op:
        parts = row.split()
        if len(parts) == 3 and parts[2].startswith("preparation_"):
            assert parts[2] not in preparation, "duplicate preparation event"
            preparation[parts[2]] = int(parts[1])
    assert set(preparation) == {
        "preparation_begin",
        "preparation_end",
        "preparation_busy",
        "preparation_observed",
    }
    start = preparation["preparation_begin"]
    end = preparation["preparation_end"]
    assert end - start >= 2_000_000
    assert start <= intervals[0]["start_us"] < intervals[0]["end_us"] < end
    assert preparation["preparation_busy"] < end <= preparation["preparation_observed"]
    progress = [start, *(t for t, _ in frames if start <= t <= end), end]
    assert len(progress) > 12
    prep_gap = max(b - a for a, b in itertools.pairwise(progress)) / 1000
    assert prep_gap < 500, "audience froze during preparation"
    assert sum(t > exit_us for t, _ in frames) > 10
    assert sum(t > killed_us for t, _ in frames) > 10
    gaps = sorted((b[1] - a[1]) / 1000 for a, b in itertools.pairwise(frames))
    summary = {
        "stalls": intervals,
        "preparation": preparation,
        "preparation_max_gap_ms": prep_gap,
        "operator_exit_observed_us": exit_us,
        "operator_kill_observed_us": killed_us,
        "calls_after_exit": sum(t > exit_us for t, _ in frames),
        "calls_after_kill": sum(t > killed_us for t, _ in frames),
        "frames": len(frames),
        "gap_p50_ms": gaps[len(gaps) // 2],
        "gap_p95_ms": gaps[int(len(gaps) * 0.95)],
        "gap_max_ms": max(gaps),
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
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
    scratch.cleanup()
