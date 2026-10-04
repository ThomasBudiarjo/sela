#!/usr/bin/env python3
"""Bounded PID-scoped native shell interaction/capture; existing X11 only."""

import os
import subprocess
import tempfile
import time
from pathlib import Path


def run(*args):
    return subprocess.check_output(args, text=True, timeout=10).strip()


def main():
    artifacts = Path(".amp/in/artifacts/operator-shell")
    artifacts.mkdir(parents=True, exist_ok=True)
    binary = (
        Path(os.environ.get("CARGO_TARGET_DIR", "target")).resolve()
        / "debug/sela"
    )
    with tempfile.TemporaryDirectory(prefix="sela-shell-") as scratch:
        env = dict(os.environ, XDG_RUNTIME_DIR=scratch)
        with open(Path(scratch) / "app.log", "w") as log:
            app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=log)
            try:
                window = ""
                for _ in range(100):
                    assert app.poll() is None, "application exited during startup"
                    found = subprocess.run(
                        ["xdotool", "search", "--onlyvisible", "--pid", str(app.pid)],
                        capture_output=True,
                        text=True,
                        timeout=5,
                        check=False,
                    )
                    if found.stdout.strip():
                        window = found.stdout.splitlines()[0]
                        break
                    time.sleep(0.1)
                assert window, "startup timed out"

                def focus():
                    run("xdotool", "windowactivate", "--sync", window)
                    assert run("xdotool", "getwindowfocus") == window

                def geometry():
                    g = dict(
                        line.split("=")
                        for line in run(
                            "xdotool", "getwindowgeometry", "--shell", window
                        ).splitlines()
                    )
                    # xdotool's reparented coordinates include decoration offsets.
                    info = run("xwininfo", "-id", window)
                    for line in info.splitlines():
                        if "Absolute upper-left X:" in line:
                            g["X"] = line.split(":")[-1].strip()
                        if "Absolute upper-left Y:" in line:
                            g["Y"] = line.split(":")[-1].strip()
                    return g

                def resize(w, h):
                    focus()
                    run("xdotool", "windowsize", window, str(w), str(h))
                    for _ in range(50):
                        g = geometry()
                        if int(g["WIDTH"]) == w and int(g["HEIGHT"]) == h:
                            time.sleep(0.3)
                            return
                        time.sleep(0.1)
                    raise AssertionError("resize timed out")

                def pointer(x, y):
                    focus()
                    g = geometry()
                    run(
                        "xdotool",
                        "mousemove",
                        str(int(g["X"]) + x),
                        str(int(g["Y"]) + y),
                    )

                def click(x, y):
                    pointer(x, y)
                    run("xdotool", "click", "1")
                    time.sleep(0.2)

                def capture(name):
                    g = geometry()
                    root = str(Path(scratch) / "root.png")
                    run("import", "-window", "root", root)
                    run(
                        "convert",
                        root,
                        "-crop",
                        f"{g['WIDTH']}x{g['HEIGHT']}+{g['X']}+{g['Y']}",
                        "+repage",
                        str(artifacts / f"{name}.png"),
                    )

                resize(1280, 800)
                capture("normal")
                resize(720, 440)
                capture("compact")
                click(160, 318)
                capture("scriptures")
                resize(1280, 800)
                click(160, 510)
                capture("collapsed")
                click(330, 20)  # Reset layout, original top-bar control.
                pointer(307, 180)
                run("xdotool", "mousedown", "1")
                pointer(50, 180)
                pointer(0, 180)
                run("xdotool", "mouseup", "1")
                time.sleep(0.3)
                capture("drag-minimum")
                focus()
                run("xdotool", "key", "--clearmodifiers", "ctrl+q")
                assert app.wait(timeout=10) == 0
                print(
                    "PASS: native size/focus/exit assertions; tab/collapse/reset/drag "
                    "input captured for required visual state inspection"
                )
            finally:
                if app.poll() is None:
                    app.terminate()
                    try:
                        app.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        app.kill()
                        app.wait(timeout=5)


if __name__ == "__main__":
    main()
