"""Windows native check: Options dialog and settings store (M1-16 settings slice).

Usage: python scripts/options-windows.py [--binary sela.exe]
       [--seed seed_library.exe] [--out DIR] [--target \\\\.\\DISPLAY6]

Uses a private profile and no SELA_AUDIENCE_MONITOR override. Run 1: Live
output on (audience on the default non-primary monitor), Options via the
keyboard, pick --target, OK; the audience moves there and settings.txt names
it. Run 2 (restart): Live on again opens the audience on --target; Options then
Cancel leaves settings.txt byte-identical. Run 3: a settings file naming a
missing monitor shows the note, opens the audience on a non-primary monitor
and is not rewritten. Run 4: a truncated file starts with defaults and keeps a
backup. Needs three monitors (primary + two others) and exclusive keyboard
and mouse; never run with EasyWorship open. Captures and summary.json go to
--out; a sampled token table (pane header/border/selected row/focus) is
written for VAL-SHELL-007.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
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
_a11y_spec = importlib.util.spec_from_file_location(
    "a11y_windows", ROOT / "scripts" / "a11y-windows.py"
)
a11y = importlib.util.module_from_spec(_a11y_spec)
_a11y_spec.loader.exec_module(a11y)

BORDER = (0xDC, 0xDE, 0xDC)
CHROME = (0xF4, 0xF4, 0xF3)
SELECTED_ROW = (0xE3, 0xE7, 0xF3)
FOCUS_BG = (0xDC, 0xE3, 0xFA)


def monitors(binary: pathlib.Path) -> list[dict]:
    out = subprocess.run(
        [str(binary), "--list-monitors"], capture_output=True, text=True, check=True
    ).stdout
    rows = []
    for line in out.splitlines():
        name, width, height, x, y, primary = line.split("\t")
        rows.append(
            {
                "name": name,
                "width": int(width),
                "height": int(height),
                "x": int(x),
                "y": int(y),
                "primary": primary == "1",
            }
        )
    return rows


def rect(hwnd: int) -> tuple[int, int]:
    box = nw.w.RECT()
    nw.user32.GetWindowRect(hwnd, ctypes.byref(box))
    return box.left, box.top


def on_monitor(hwnd: int, monitor: dict) -> bool:
    return rect(hwnd) == (monitor["x"], monitor["y"])


def sha(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Session:
    def __init__(self, binary: pathlib.Path, library: pathlib.Path, out: pathlib.Path):
        os.environ.pop("SELA_AUDIENCE_MONITOR", None)
        self.out = out
        self.app = nw.App(binary, ["--operator-library", str(library)])
        self.hwnd = self.app.window()
        nw.activate(self.hwnd)
        time.sleep(1.2)

    def keys(self, *sequence: str) -> None:
        for key in sequence:
            nw.press(self.hwnd, key)
            time.sleep(0.15)

    def save(self, name: str) -> bytes:
        width, height, pixels = nw.capture_client(self.hwnd)
        nw.write_png(self.out / f"{name}.png", width, height, pixels)
        return pixels

    def live_on(self) -> int:
        a11y.show_live_output(self.hwnd)
        found = lo.audience(self.app.process.pid, 15)
        if not found:
            raise nw.Failure("no audience window after Live output on")
        time.sleep(0.8)
        return found[1]

    def open_options(self, from_live: bool) -> None:
        # The Options toolbar button moved to Edit > Options... (M1-02); the
        # Alt+E mnemonic is the EW8-OBS-010 route and works from any focus.
        del from_live
        self.keys("alt+e", "enter")
        time.sleep(0.5)

    def quit(self) -> None:
        self.keys("ctrl+q")
        self.app.wait_exit()

    def close(self) -> None:
        self.app.close()


def find(width: int, height: int, pixels: bytes, color: tuple[int, int, int]) -> int:
    return lo.hits(width, height, pixels, color, tolerance=2)


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
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/options-windows"
    )
    parser.add_argument("--target", default=r"\\.\DISPLAY6")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(
                f"FAIL: missing {path} (cargo build && cargo build --example seed_library)"
            )
            return 1
    summary: dict = {"status": "FAIL", "target": args.target}
    summary_path = args.out / "summary.json"
    sessions: list[Session] = []
    try:
        screens = monitors(args.binary)
        summary["monitors"] = screens
        target = next(m for m in screens if m["name"] == args.target)
        secondary = next(m for m in screens if not m["primary"])
        other = next(m for m in screens if not m["primary"] and m is not target)
        data = pathlib.Path(tempfile.mkdtemp(prefix="sela-options-"))
        library = data / "library.sqlite"
        subprocess.run([str(args.seed), str(library)], check=True)
        settings = data / "settings.txt"

        # Run 1: pick the target monitor while Live output is on.
        run = Session(args.binary, library, args.out)
        sessions.append(run)
        audience = run.live_on()
        summary["default_audience_on_secondary"] = on_monitor(audience, secondary)
        if not summary["default_audience_on_secondary"]:
            raise nw.Failure("default audience is not on the first non-primary monitor")
        run.open_options(from_live=True)
        pixels = run.save("01-options-open")
        width, height = nw.client_size(run.hwnd)
        summary["tokens_present"] = {
            "border": find(width, height, pixels, BORDER),
            "chrome": find(width, height, pixels, CHROME),
            "selected_row": find(width, height, pixels, SELECTED_ROW),
            "focus": find(width, height, pixels, FOCUS_BG),
        }
        rows_after = len(screens) - 1 - screens.index(target)
        # Focus starts on Cancel: OK, then rows from the last upwards.
        run.keys(*["shift+tab"] * (2 + rows_after), "space")
        run.save("02-options-target-picked")
        run.keys(*["tab"] * (rows_after + 1), "enter")  # to OK, confirm
        deadline = time.monotonic() + 15
        moved = False
        while time.monotonic() < deadline and not moved:
            found = lo.audience(run.app.process.pid, 0.5)
            moved = bool(found) and on_monitor(found[1], target)
            time.sleep(0.2)
        summary["audience_moved_to_target"] = moved
        if not moved:
            raise nw.Failure("audience did not move to the chosen monitor")
        time.sleep(0.5)
        text = settings.read_text()
        summary["settings_after_ok"] = text
        if f"output_monitor={args.target}" not in text:
            raise nw.Failure("settings.txt does not name the chosen monitor")
        run.save("03-after-ok")
        run.quit()

        # Run 2: restart keeps the choice; Cancel leaves the file untouched.
        before = sha(settings)
        run = Session(args.binary, library, args.out)
        sessions.append(run)
        audience = run.live_on()
        summary["restart_audience_on_target"] = on_monitor(audience, target)
        if not summary["restart_audience_on_target"]:
            raise nw.Failure("after restart the audience is not on the saved monitor")
        run.open_options(from_live=True)
        run.save("04-options-after-restart")
        run.keys("enter")  # Cancel has focus
        time.sleep(0.5)
        summary["cancel_keeps_file"] = sha(settings) == before
        if not summary["cancel_keeps_file"]:
            raise nw.Failure("Cancel changed settings.txt")
        run.quit()

        # Run 3: saved monitor that does not exist.
        settings.write_text("sela-settings 1\noutput_monitor=\\\\.\\DISPLAY99\n")
        before = sha(settings)
        run = Session(args.binary, library, args.out)
        sessions.append(run)
        time.sleep(1.5)
        run.save("05-missing-monitor-note")
        audience = run.live_on()
        summary["missing_audience_non_primary"] = not on_monitor(
            audience, next(m for m in screens if m["primary"])
        ) and on_monitor(audience, secondary)
        if not summary["missing_audience_non_primary"]:
            raise nw.Failure(
                "fallback audience is not on the default non-primary monitor"
            )
        summary["missing_file_unchanged"] = sha(settings) == before
        if not summary["missing_file_unchanged"]:
            raise nw.Failure("startup rewrote the settings file")
        run.quit()

        # Run 4: truncated file.
        settings.write_text("sela-settings 1\noutp")
        run = Session(args.binary, library, args.out)
        sessions.append(run)
        time.sleep(1.5)
        run.save("06-corrupt-note")
        backup = data / "settings.txt.bad"
        summary["corrupt_backup_kept"] = backup.exists()
        if not backup.exists():
            raise nw.Failure("corrupt settings were not kept as a backup")
        run.quit()
        summary["other_monitor"] = other["name"]
        summary["status"] = "PASS"
        return 0
    except Exception as error:  # noqa: BLE001
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        for session in sessions:
            session.close()
        summary_path.write_text(json.dumps(summary, indent=2))
        print(
            json.dumps({k: v for k, v in summary.items() if k != "monitors"}, indent=2)
        )


if __name__ == "__main__":
    sys.exit(main())
