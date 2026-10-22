"""Windows native check: Live navigation keys (M1-10 remainder).

Usage: python scripts/navigation-windows.py [--binary sela.exe]
       [--seed seed_library.exe] [--out DIR]

Private profile, SELA_AUDIENCE_MONITOR=secondary. Adds the first seeded song to
the schedule, turns Live output on, Go Live, then drives the keyboard (as a
presenter remote would) and reads the operator's own Live status through UIA:
Right/Left/Home/End move slides, Left at the first slide stays, a rapid burst
of Right ends on the last requested slide, V,1,Enter jumps to Verse 1,
B (presenter remote) blanks the audience output and toggles back, and killing
the audience process shows the separate Disconnected indicator. Captures and
summary.json go to --out. Takes over the real keyboard; never run with
EasyWorship open.
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


def load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lo = load("live_output_windows", "live-output-windows.py")
a11y = load("a11y_windows", "a11y-windows.py")


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
    parser.add_argument(
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/navigation-windows"
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    summary: dict = {"status": "FAIL", "steps": {}}
    summary_path = args.out / "summary.json"
    app = None
    try:
        data = pathlib.Path(tempfile.mkdtemp(prefix="sela-nav-"))
        library = data / "library.sqlite"
        subprocess.run([str(args.seed), str(library)], check=True)
        os.environ["SELA_AUDIENCE_MONITOR"] = "secondary"
        app = nw.App(args.binary, ["--operator-library", str(library)])
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(1.2)

        def status() -> str:
            return a11y.live_status(hwnd)

        def step(name: str, keys: list[str], needle: str, unless: str = "") -> None:
            for key in keys:
                nw.press(hwnd, key)
                time.sleep(0.12)
            summary["steps"][name] = a11y.wait_live_status(
                hwnd, needle, 10, name, unless=unless
            )

        def shot(name: str) -> None:
            width, height, pixels = nw.capture_client(hwnd)
            nw.write_png(args.out / f"{name}.png", width, height, pixels)

        found = a11y.nodes_by_id(a11y.uia(hwnd))
        a11y.click_node(hwnd, found["song-0"])
        a11y.click_node(hwnd, found["add-to-schedule"])
        time.sleep(0.4)
        a11y.show_live_output(hwnd)
        audience = lo.audience(app.process.pid, 15)
        if not audience:
            raise nw.Failure("no audience window after Live output on")
        audience_pid, audience_hwnd = audience
        a11y.wait_live_status(hwnd, "×", 30, "audience output never became ready")

        step("go-live", ["pagedown"], "On screen:", unless="nothing confirmed")
        step("verse-1", ["home"], "Verse 1")
        step("right", ["right"], "Chorus")
        step("end", ["end"], "Bridge")
        last = summary["steps"]["end"]
        step("right-at-last-stays", ["right"], "Bridge")
        if summary["steps"]["right-at-last-stays"] != last:
            raise nw.Failure("Right past the last slide moved off the last slide")
        step("home-again", ["home"], "Verse 1")
        step("left-at-first-stays", ["left"], "Verse 1")
        step("rapid-right", ["right"] * 3, "Bridge")
        step("label-jump", ["v", "1", "enter"], "Verse 1")
        shot("01-label-jump")

        if not a11y.wait_for(5.0, lambda: a11y.audience_ink(audience_hwnd)):
            raise nw.Failure("no ink on the audience output before Black")
        nw.press(hwnd, "b")
        if not a11y.wait_for(5.0, lambda: not a11y.audience_ink(audience_hwnd)):
            w_, h_, px_ = nw.capture_client(audience_hwnd)
            nw.write_png(args.out / "05-audience-after-b.png", w_, h_, px_)
            summary["audience_brightest"] = lo.brightest(px_)
            shot("04-black-failed")
            raise nw.Failure(f"B did not blank the audience output: {status()!r}")
        nw.press(hwnd, "b")
        if not a11y.wait_for(5.0, lambda: a11y.audience_ink(audience_hwnd)):
            raise nw.Failure("B again did not restore the audience output")
        summary["steps"]["black-toggle"] = "blank then restored"
        shot("02-after-black-toggle")

        lo.kill(audience_pid)
        summary["steps"]["disconnected"] = a11y.wait_live_status(
            hwnd, "Disconnected", 15, "audience loss"
        )
        shot("03-disconnected")
        summary["status"] = "PASS"
    except nw.Failure as failure:
        summary["error"] = str(failure)
        print(f"FAIL: {failure}")
    finally:
        if app is not None:
            app.close()
        summary_path.write_text(
            json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8"
        )
        print(json.dumps(summary, indent=2, ensure_ascii=False))
    return 0 if summary["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
