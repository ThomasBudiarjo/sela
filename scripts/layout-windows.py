"""Windows native check: layout persistence (M1-02 remainder).

Usage: python scripts/layout-windows.py [--binary sela.exe] [--seed seed_library.exe] [--out DIR]

Private profile. Run 1 drags the Schedule/Preview splitter and the
top/bottom (Resources) splitter and moves/resizes the window, then quits
normally (settings.txt gains pane_ratios/window). Run 2 restarts on the same
profile: the splitter columns/rows seen in the capture and the window
rectangle match run 1 within 2 px (VAL-SHELL-008). Run 3 edits settings.txt so
the saved window lies far outside every monitor: the window opens inside the
primary monitor at least at the minimum size (VAL-SHELL-009). Run 4 stores a
malformed layout: Sela starts with the default splitters and the next normal
close rewrites valid values (VAL-SHELL-010, with cargo tests). Run 5 moves the
window wholly onto the spare monitor and quits: the restart opens it on that
monitor at the same rectangle (SC-LAYOUT-03). Needs exclusive keyboard and
mouse; never run with EasyWorship open.
"""

from __future__ import annotations

import argparse
import ctypes
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parents[1]
BORDER = (0xDC, 0xDE, 0xDC)
MENU_BAR_H = 28
TOP_BAR_H = 40
MIN_WINDOW = (720, 440)
TOLERANCE = 2


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


def window_rect(hwnd: int) -> list[int]:
    box = nw.w.RECT()
    nw.user32.GetWindowRect(hwnd, ctypes.byref(box))
    return [box.left, box.top, box.right, box.bottom]


def border_positions(
    width: int, height: int, pixels: bytes, fixed: int, vertical: bool
) -> list[int]:
    """Coordinates of 1px BORDER lines along a row (vertical lines) or column."""
    found = []
    span = width if vertical else height
    for i in range(span):
        x, y = (i, fixed) if vertical else (fixed, i)
        o = (y * width + x) * 4
        b, g, r = pixels[o], pixels[o + 1], pixels[o + 2]
        if (r, g, b) == BORDER:
            found.append(i)
    return found


def close_enough(a: list[int], b: list[int]) -> bool:
    return len(a) == len(b) and all(abs(x - y) <= TOLERANCE for x, y in zip(a, b))


class Session:
    def __init__(self, binary: pathlib.Path, library: pathlib.Path, out: pathlib.Path):
        os.environ.pop("SELA_AUDIENCE_MONITOR", None)
        self.out = out
        self.app = nw.App(binary, ["--operator-library", str(library)])
        self.hwnd = self.app.window()
        nw.activate(self.hwnd)
        time.sleep(1.2)

    def capture(self, name: str) -> tuple[int, int, bytes]:
        width, height, pixels = nw.capture_client(self.hwnd)
        nw.write_png(self.out / f"{name}.png", width, height, pixels)
        return width, height, pixels

    def quit(self) -> None:
        nw.press(self.hwnd, "ctrl+q")
        self.app.wait_exit()

    def close(self) -> None:
        self.app.close()


def scan(session: Session, name: str) -> dict:
    width, height, pixels = session.capture(name)
    scale = nw.dpi(session.hwnd) / 96
    row = round((MENU_BAR_H + TOP_BAR_H + 40) * scale)
    # A column inside the Preview pane: only horizontal borders cross it.
    column = round(width * 0.5)
    return {
        "client": [width, height],
        "columns": border_positions(width, height, pixels, row, True),
        "rows": border_positions(width, height, pixels, column, False),
    }


def drag_splitters(session: Session) -> None:
    width, height = nw.client_size(session.hwnd)
    scale = nw.dpi(session.hwnd) / 96
    lw, lh = width / scale, height / scale
    available = lw - 12
    left = available * 0.24
    upper = (lh - 76 - MENU_BAR_H) * 0.62
    x = round((left + 3) * scale)
    y = round((MENU_BAR_H + TOP_BAR_H + upper + 3) * scale)
    mid = round((MENU_BAR_H + TOP_BAR_H + upper / 2) * scale)
    nw.mouse_move(session.hwnd, x, mid)
    time.sleep(0.2)
    nw.mouse_down(session.hwnd, x, mid)
    nw.drag_to(session.hwnd, x + round(90 * scale), mid)
    nw.mouse_up()
    time.sleep(0.3)
    nw.mouse_move(session.hwnd, round(width * 0.5), y)
    time.sleep(0.2)
    nw.mouse_down(session.hwnd, round(width * 0.5), y)
    nw.drag_to(session.hwnd, round(width * 0.5), y - round(70 * scale))
    nw.mouse_up()
    time.sleep(0.3)
    nw.mouse_move(session.hwnd, round(width * 0.5), round(80 * scale))
    time.sleep(0.2)


def place(session: Session, x: int, y: int, width: int, height: int) -> None:
    nw.user32.SetWindowPos(session.hwnd, 0, x, y, width, height, 0x0004 | 0x0010)
    time.sleep(0.8)


def read_layout(path: pathlib.Path) -> dict:
    text = path.read_text()
    ratios = re.search(r"pane_ratios=(\d+),(\d+),(\d+)", text)
    window = re.search(r"window=(-?\d+),(-?\d+),(\d+),(\d+)", text)
    return {
        "text": text,
        "ratios": [int(v) for v in ratios.groups()] if ratios else None,
        "window": [int(v) for v in window.groups()] if window else None,
    }


def inside(rect: list[int], monitor: dict) -> bool:
    return (
        rect[0] >= monitor["x"]
        and rect[1] >= monitor["y"]
        and rect[2] <= monitor["x"] + monitor["width"]
        and rect[3] <= monitor["y"] + monitor["height"]
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=pathlib.Path, default=ROOT / "target/debug/sela.exe")
    parser.add_argument(
        "--seed", type=pathlib.Path, default=ROOT / "target/debug/examples/seed_library.exe"
    )
    parser.add_argument(
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/layout-windows"
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(f"FAIL: missing {path}")
            return 1
    summary: dict = {"status": "FAIL"}
    sessions: list[Session] = []
    try:
        screens = monitors(args.binary)
        primary = next(m for m in screens if m["primary"])
        summary["primary"] = primary
        data = pathlib.Path(tempfile.mkdtemp(prefix="sela-layout-"))
        library = data / "library.sqlite"
        subprocess.run([str(args.seed), str(library)], check=True)
        settings = data / "settings.txt"

        # Run 1: drag both splitters, move and resize the window, quit.
        run = Session(args.binary, library, args.out)
        sessions.append(run)
        place(run, primary["x"] + 120, primary["y"] + 90, 1420, 900)
        default_scan = scan(run, "01-default")
        drag_splitters(run)
        before = scan(run, "02-dragged")
        summary["default_scan"] = default_scan
        summary["before"] = before
        if close_enough(before["columns"], default_scan["columns"]) or close_enough(
            before["rows"], default_scan["rows"]
        ):
            raise nw.Failure("a splitter drag did not move its splitter")
        rect_before = window_rect(run.hwnd)
        run.quit()
        saved = read_layout(settings)
        summary["settings_after_quit"] = saved["text"]
        if not saved["ratios"] or not saved["window"]:
            raise nw.Failure("settings.txt has no pane_ratios/window after a normal quit")

        # Run 2: restart restores splitters and window.
        run = Session(args.binary, library, args.out)
        sessions.append(run)
        after = scan(run, "03-restarted")
        rect_after = window_rect(run.hwnd)
        summary["after"] = after
        summary["window_before"] = rect_before
        summary["window_after"] = rect_after
        if not close_enough(before["columns"], after["columns"]):
            raise nw.Failure(f"columns {before['columns']} != {after['columns']}")
        if not close_enough(before["rows"], after["rows"]):
            raise nw.Failure(f"rows {before['rows']} != {after['rows']}")
        if not close_enough(rect_before, rect_after):
            raise nw.Failure(f"window {rect_before} != {rect_after}")
        run.quit()

        # Run 3: a saved window far outside every monitor is moved on screen.
        text = settings.read_text()
        text = re.sub(r"window=.*", "window=40000,30000,1280,800", text)
        settings.write_text(text)
        run = Session(args.binary, library, args.out)
        sessions.append(run)
        rect = window_rect(run.hwnd)
        run.capture("04-offscreen-corrected")
        summary["offscreen_window"] = rect
        scale = nw.dpi(run.hwnd) / 96
        if not inside(rect, primary):
            raise nw.Failure(f"window {rect} is not inside the primary monitor")
        if (rect[2] - rect[0]) < MIN_WINDOW[0] * scale - 20 or (rect[3] - rect[1]) < MIN_WINDOW[1] * scale - 20:
            raise nw.Failure(f"window {rect} is smaller than the minimum size")
        run.quit()

        # Run 4: a malformed layout starts with defaults and is overwritten.
        settings.write_text("sela-settings 1\npane_ratios={bad json\nwindow=1,2\n")
        run = Session(args.binary, library, args.out)
        sessions.append(run)
        corrupt_scan = scan(run, "06-corrupt-layout")
        summary["corrupt_scan"] = corrupt_scan
        run.quit()
        rewritten = read_layout(settings)
        summary["settings_after_corrupt_quit"] = rewritten["text"]
        if rewritten["ratios"] != [2400, 6200, 6200] or "bad json" in rewritten["text"]:
            raise nw.Failure("corrupt layout was not replaced by the default on close")

        spare = nw.spare_monitor(screens)
        summary["spare"] = spare
        run = Session(args.binary, library, args.out)
        sessions.append(run)
        place(run, spare["x"] + 80, spare["y"] + 60, 1200, 760)
        rect_before = window_rect(run.hwnd)
        run.capture("07-on-spare")
        run.quit()
        summary["settings_after_spare_quit"] = read_layout(settings)["text"]
        run = Session(args.binary, library, args.out)
        sessions.append(run)
        rect_after = window_rect(run.hwnd)
        run.capture("08-spare-restarted")
        summary["spare_window_before"] = rect_before
        summary["spare_window_after"] = rect_after
        if not inside(rect_before, spare):
            raise nw.Failure(f"setup: window {rect_before} is not on {spare['name']}")
        if not inside(rect_after, spare):
            raise nw.Failure(f"window {rect_after} did not reopen on {spare['name']}")
        if not close_enough(rect_before, rect_after):
            raise nw.Failure(f"window on the spare monitor {rect_before} != {rect_after}")
        run.quit()
        summary["status"] = "PASS"
    except nw.Failure as error:
        summary["error"] = str(error)
    finally:
        for session in sessions:
            session.close()
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))
    print(summary["status"], summary.get("error", ""))
    return 0 if summary["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
