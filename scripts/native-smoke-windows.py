"""Windows native smoke for the GPUI operator window (Win32 counterpart of native-smoke.sh).

Usage: python scripts/native-smoke-windows.py [path\\to\\sela.exe] [artifact-directory]

Launches the real binary twice with a private LOCALAPPDATA/APPDATA/TEMP, verifies
PID-scoped foreground before each injected key, checks survival after an unbound
key, DPI-scaled resize and the 720x440 logical minimum, then requires clean exit
and window removal for Ctrl+Q and Alt+F4. Exclusive keyboard focus is required
while it runs; concurrent focus changes invalidate the run.
"""

from __future__ import annotations

import json
import shutil
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_win as nw


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    binary = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "target" / "debug" / "sela.exe"
    artifacts = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    if not binary.is_file():
        print(f"FAIL: missing binary {binary}")
        return 1
    summary: dict = {"binary": str(binary.resolve())}
    app = nw.App(binary)
    scratch = app.scratch
    try:
        hwnd = app.window()
        started = time.monotonic()
        summary["title"] = nw.title(hwnd)
        scale = nw.dpi(hwnd) / 96
        summary["dpi"] = nw.dpi(hwnd)
        nw.activate(hwnd)
        nw.chord(hwnd, "ctrl+j")
        time.sleep(0.3)
        if app.process.poll() is not None:
            raise nw.Failure("Ctrl+J closed the app")
        target = (round(720 * scale), round(440 * scale))
        settled = nw.resize_client(hwnd, *target)
        summary["resize_request"], summary["resize_client"] = target, settled
        if abs(settled[0] - target[0]) > 2 or abs(settled[1] - target[1]) > 2:
            raise nw.Failure(f"resize settled at {settled}, wanted {target}")
        below = nw.resize_client(hwnd, round(500 * scale), round(300 * scale))
        summary["below_minimum_client"] = below
        if below[0] < target[0] - 2 or below[1] < target[1] - 2:
            raise nw.Failure(f"window shrank below 720x440 logical minimum: {below}")
        nw.resize_client(hwnd, *target)
        nw.activate(hwnd)
        time.sleep(0.5)
        width, height, pixels = nw.capture_client(hwnd)
        summary["capture"] = [width, height]
        summary["sampled_colors"] = nw.color_count(pixels)
        if summary["sampled_colors"] < 3:
            raise nw.Failure("capture looks blank (fewer than 3 sampled colors)")
        if artifacts:
            artifacts.mkdir(parents=True, exist_ok=True)
            nw.write_png(artifacts / "m0-01-windows-small.png", width, height, pixels)
        nw.chord(hwnd, "ctrl+q")
        code = app.wait_exit()
        summary["ctrl_q_exit"] = code
        summary["ctrl_q_seconds_since_window"] = round(time.monotonic() - started, 3)
        if code != 0:
            raise nw.Failure(f"Ctrl+Q exit status {code}")
        print("PASS: PID-scoped focus, unbound key, DPI resize/minimum, Ctrl+Q, clean exit/window removal")
        app.close()
        first_log = app.log_path

        app = nw.App(binary, scratch=scratch)
        hwnd = app.window()
        nw.chord(hwnd, "alt+f4")
        code = app.wait_exit()
        summary["alt_f4_exit"] = code
        if code != 0:
            raise nw.Failure(f"Alt+F4 exit status {code}")
        print("PASS: Alt+F4 close, clean exit/window removal")
        if artifacts:
            shutil.copy(first_log, artifacts / "m0-01-windows.log")
            (artifacts / "m0-01-windows-summary.json").write_text(json.dumps(summary, indent=2))
        print(json.dumps(summary))
        return 0
    except nw.Failure as error:
        print(f"FAIL: {error}\n{app.log()}")
        return 1
    finally:
        app.close()
        shutil.rmtree(scratch, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
