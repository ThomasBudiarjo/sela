"""Windows native check: schedule preflight report and the Go Live warning (M1-11b).

Usage: python scripts/preflight-windows.py [--binary sela.exe] [--seed seed_library.exe]
       [--monitor secondary] [--out DIR]

Seeds a library whose saved schedule pins an image that is gone from the
managed folder (`seed_library --vanished-schedule`) and drives the operator by
keyboard. Ctrl+O opens the schedule: the file report must appear on its own
(operator capture differs from the one after Enter dismisses it, which is the
default Cancel). With Live output on, Page Down on the item whose image is
missing opens the Go Live warning; Enter (Cancel) leaves the audience output
exactly as it was, and choosing Go Live anyway shows the slide. The Locate…
file chooser is a native shell dialog and is covered by GPUI tests only, as
are the 24 MP downscale and Cancel import flows (images:: unit tests too).
Requires exclusive use of the keyboard and an unobstructed audience monitor.
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
_spec = importlib.util.spec_from_file_location(
    "live_output_windows", ROOT / "scripts" / "live-output-windows.py"
)
lo = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(lo)


def keys(hwnd: int, *sequence: str) -> None:
    for key in sequence:
        nw.press(hwnd, key)
        time.sleep(0.12)


def operator_digest(hwnd: int) -> str:
    return lo.digest(hwnd)


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
    parser.add_argument("--monitor", default="secondary")
    parser.add_argument(
        "--out",
        type=pathlib.Path,
        default=ROOT / ".amp/in/artifacts/preflight-windows",
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(f"FAIL: missing {path}")
            return 1
    summary: dict = {"binary": str(args.binary.resolve()), "monitor": args.monitor}
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-preflight-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library), "--vanished-schedule"], check=True)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    spawned: set[int] = set()
    app = nw.App(args.binary, ["--operator-library", str(library)])

    def save(hwnd: int, name: str) -> None:
        width, height, pixels = nw.capture_client(hwnd)
        nw.write_png(args.out / f"{name}.png", width, height, pixels)

    try:
        operator = app.window()
        nw.activate(operator)
        time.sleep(1.0)
        save(operator, "operator-start")
        keys(operator, "ctrl+o")
        time.sleep(0.8)
        keys(operator, "shift+tab", "enter")
        time.sleep(2.0)
        save(operator, "operator-report")
        with_report = operator_digest(operator)
        keys(operator, "enter")  # the default, Cancel, closes the report
        time.sleep(0.6)
        save(operator, "operator-report-closed")
        if operator_digest(operator) == with_report:
            raise nw.Failure("the report did not appear/close on schedule open")
        summary["report_shown_and_dismissed"] = True

        keys(operator, "alt+l", "up", "enter")
        found = lo.audience(app.process.pid, 15)
        if not found:
            raise nw.Failure(f"no audience window within 15s: {app.log()}")
        pid, screen = found
        spawned.add(pid)
        time.sleep(1.5)
        blank = lo.digest(screen)

        keys(operator, "down", "pagedown")
        time.sleep(1.5)
        save(operator, "operator-go-live-warning")
        warned = operator_digest(operator)
        if lo.digest(screen) != blank:
            raise nw.Failure("Go Live changed the audience before the warning was answered")
        keys(operator, "enter")  # Cancel
        time.sleep(1.0)
        save(operator, "operator-go-live-cancelled")
        if operator_digest(operator) == warned:
            raise nw.Failure("the Go Live warning did not close on Cancel")
        if lo.digest(screen) != blank:
            raise nw.Failure("Cancel changed the audience output")
        summary["cancel_kept_output"] = True

        keys(operator, "pagedown")
        time.sleep(1.5)
        keys(operator, "shift+tab", "enter")  # Go Live anyway
        sent = time.monotonic()
        while lo.digest(screen) == blank:
            if time.monotonic() - sent > 4.0:
                raise nw.Failure("Go Live anyway did not show the slide")
            time.sleep(0.05)
        width, height, pixels = nw.capture_client(screen)
        nw.write_png(args.out / "audience-live-anyway.png", width, height, pixels)
        summary["live_anyway_shown"] = True

        keys(operator, "ctrl+q")
        summary["exit"] = app.wait_exit()
        summary["status"] = "PASS"
        print(
            "PASS: opening a schedule with a missing image shows the report; Go Live "
            "warns, Cancel keeps the audience output, Go Live anyway shows the slide"
        )
        return 0
    except (nw.Failure, AssertionError) as error:
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        if app.hwnd and nw.user32.IsWindow(app.hwnd):
            save(app.hwnd, "failure-operator")
        return 1
    finally:
        app.close()
        for pid in spawned:
            if lo.alive(pid):
                lo.kill(pid)
        summary.setdefault("status", "FAIL")
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
