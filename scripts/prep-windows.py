"""Windows native check: Black stays prompt while a heavy cue prepares (VAL-PREP-003).

Usage: python scripts/prep-windows.py [--binary sela.exe] [--seed seed_library.exe]
       [--monitor name:\\\\.\\DISPLAY1] [--out DIR]

Seeds a 100-slide song (28-line slides), turns Live output and Normalize text
size on (every cue then fits all 100 slides), then presses Go Live and
immediately Ctrl+B. Asserts the audience goes black and the operator's Black
button lights within 250 ms of the Black key, that the operator window is not
hung, and that the slide appears once Black is toggled off. Needs exclusive
use of the keyboard and an unobstructed audience monitor. Reuses the helpers
of live-output-windows.py.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parents[1]
BOUND_MS = 250
# `secondary` means "first non-primary monitor in winit's order", which moved to
# DISPLAY6 when the primary display changed; qualification names the panel.
DEFAULT_MONITOR = r"name:\\.\DISPLAY1"

spec = importlib.util.spec_from_file_location(
    "live_output", ROOT / "scripts" / "live-output-windows.py"
)
lo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lo)


def black_lit(hwnd: int) -> bool:
    """Early-exit scan for the acknowledged-mask fill; fast enough to time."""
    width, height, bgra = nw.capture_client(hwnd)
    scale = nw.dpi(hwnd) / 96
    found = 0
    for y in range(round(28 * scale), min(height, round(68 * scale)), 3):
        row = y * width * 4
        for x in range(0, width, 3):
            i = row + x * 4
            if lo.near(tuple(bgra[i : i + 3]), lo.MASK_ON_BGR, 3):
                found += 1
                if found >= 20:
                    return True
    return False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary", type=pathlib.Path, default=ROOT / "target/debug/sela.exe"
    )
    parser.add_argument(
        "--seed",
        type=pathlib.Path,
        default=ROOT / "target/debug/examples/seed_library.exe",
    )
    parser.add_argument("--monitor", default=DEFAULT_MONITOR)
    parser.add_argument(
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/prep-windows"
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    summary: dict = {"status": "FAIL", "bound_ms": BOUND_MS}
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-prep-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library), "--heavy-song"], check=True)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    app = nw.App(args.binary, ["--operator-library", str(library)])
    exit_code = 1
    try:
        operator = app.window()
        run = lo.Run(operator, args.out, summary)
        nw.activate(operator)
        time.sleep(1.0)
        run.keys(*["tab"] * lo.TABS_TO_FIRST_SONG, "enter", "tab", "enter")
        time.sleep(1.5)  # faces resolve off the UI thread
        run.keys(*["shift+tab"] * 15, "enter")  # Live output on
        found = lo.audience(app.process.pid, 15)
        if not found:
            raise nw.Failure("no audience window")
        _, screen = found
        resolved = nw.window_monitor(screen)
        summary["requested_monitor"] = args.monitor
        summary["audience_monitor"] = resolved
        if args.monitor.startswith("name:"):
            wanted = args.monitor[len("name:") :]
            if resolved["device"] != wanted or not resolved["covers_monitor"]:
                raise nw.Failure(
                    f"audience is on {resolved['device']} {resolved['window']}, "
                    f"requested {wanted}"
                )
        time.sleep(1.5)
        # Live output -> Go Live, Previous, Next, Logo, Black, Clear, Normalize.
        run.keys(*["tab"] * 7, "enter")
        time.sleep(0.3)
        run.save(operator, "operator-before")

        nw.press(operator, "pagedown")  # Go Live: normalized fit of 100 slides
        sent = time.monotonic()
        pressed_wall = time.time()
        nw.press(operator, "ctrl+b")
        pressed = time.monotonic()
        audience_polls = []
        while True:
            began = time.monotonic()
            level = lo.brightest(nw.capture_client(screen)[2])
            audience_polls.append((round((began - pressed) * 1000), round((time.monotonic() - began) * 1000), level))
            if level <= 16:
                break
            if time.monotonic() - pressed > 3:
                raise nw.Failure("the audience did not go black within 3 s")
            time.sleep(0.01)
        summary["audience_polls"] = audience_polls
        black_ms = round((time.monotonic() - pressed) * 1000)
        summary["key_wall_us"] = round(pressed_wall * 1e6)
        summary["black_seen_wall_us"] = round(time.time() * 1e6)
        polls = []
        while True:
            began = time.monotonic()
            lit = black_lit(operator)
            polls.append((round((began - pressed) * 1000), round((time.monotonic() - began) * 1000), lit))
            if lit:
                break
            if time.monotonic() - pressed > 3:
                raise nw.Failure("the Black button never lit")
            time.sleep(0.01)
        summary["polls"] = polls
        summary["lit_seen_wall_us"] = round(time.time() * 1e6)
        lit_ms = round((time.monotonic() - pressed) * 1000)
        summary.update(black_observed_ms=black_ms, black_lit_ms=lit_ms)
        run.save(operator, "operator-black")
        hung = bool(nw.user32.IsHungAppWindow(operator))
        summary["operator_hung"] = hung
        if hung:
            raise nw.Failure("the operator window is not responding")
        if max(black_ms, lit_ms) > BOUND_MS:
            raise nw.Failure(f"Black took {max(black_ms, lit_ms)} ms (> {BOUND_MS})")
        summary["go_live_to_black_key_ms"] = round((pressed - sent) * 1000)

        run.keys("ctrl+b")  # unmask: the prepared slide must now be on screen
        deadline = time.monotonic() + 10
        while True:
            width, height, pixels = nw.capture_client(screen)
            if lo.ink_box(width, height, pixels) is not None:
                break
            if time.monotonic() > deadline:
                raise nw.Failure("the heavy slide never appeared after unmasking")
            time.sleep(0.05)
        run.save(screen, "audience-slide")
        run.keys("ctrl+q")
        summary["operator_exit"] = app.wait_exit()
        summary["status"] = "PASS"
        exit_code = 0
    except nw.Failure as failure:
        summary["failure"] = str(failure)
        print(f"FAIL: {failure}")
    finally:
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))
        print(json.dumps(summary, indent=2))
        app.close()
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
