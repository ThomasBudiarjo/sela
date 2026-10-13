"""Native check (VAL-SHELL-007): the main window, Options and the Song Editor
paint the same border, selected-row and focus tokens and the same pane header
height.

Captures the three windows at the default 1600x1000 client size with a selected
row and a focused control, samples RGB at fixed client pixels and measures each
pane header as the distance between its two horizontal border rows. Writes
summary.json with the sampled table. Every token must agree within 2 per
channel and the header heights must be equal.

Takes over the real keyboard and mouse; run one native script at a time.
"""

import argparse
import json
import pathlib
import subprocess
import sys
import tempfile
import time
from collections import Counter

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parents[1]
TOLERANCE = 2
BORDER = (0xDC, 0xDE, 0xDC)
SELECTED_ROW = (0xE3, 0xE7, 0xF3)
FOCUS_BG = (0xDC, 0xE3, 0xFA)
CLIENT = (1600, 1000)

# Search regions (client physical pixels at the default 1600x1000 client) in
# which each token's paint must appear; the first pixel within SEARCH of the
# token is sampled, and the sampled values must then agree within TOLERANCE.
SEARCH = 8
REGIONS = {
    "main": {"selected_row": (700, 700, 900, 750), "focus": (1050, 40, 1150, 60)},
    "options": {"selected_row": (450, 240, 550, 260), "focus": (1150, 765, 1250, 790)},
    "song_editor": {"selected_row": (8, 165, 44, 260), "focus": (8, 120, 70, 155)},
}
TOKENS = {"selected_row": SELECTED_ROW, "focus": FOCUS_BG}
HEADER_COLUMN = {"main": 600, "options": 600, "song_editor": 300}
HEADER_SCAN = {"main": (60, 140), "options": (160, 240), "song_editor": (100, 175)}


class Image:
    def __init__(self, hwnd: int, out: pathlib.Path, name: str):
        self.width, self.height, self.bgra = nw.capture_client(hwnd)
        nw.write_png(out / f"{name}.png", self.width, self.height, self.bgra)

    def pixel(self, x: int, y: int) -> list[int]:
        i = (y * self.width + x) * 4
        return [self.bgra[i + 2], self.bgra[i + 1], self.bgra[i]]


def border_rows(image: Image, x: int, lo: int, hi: int) -> list[int]:
    return [
        y
        for y in range(lo, hi)
        if all(abs(a - b) <= TOLERANCE for a, b in zip(image.pixel(x, y), BORDER))
    ]


def search(image: Image, region: tuple, token: tuple) -> tuple[list[int], list[int]]:
    """The most frequent color near the token in the region (rounded corners
    and anti-aliasing blend, flat fills dominate), and one pixel having it."""
    x0, y0, x1, y1 = region
    counts: Counter = Counter()
    first: dict = {}
    for y in range(y0, y1):
        for x in range(x0, x1):
            rgb = tuple(image.pixel(x, y))
            if all(abs(a - b) <= SEARCH for a, b in zip(rgb, token)):
                counts[rgb] += 1
                first.setdefault(rgb, [x, y])
    if not counts:
        raise nw.Failure(f"no paint near {token} in region {region}")
    rgb = counts.most_common(1)[0][0]
    return first[rgb], list(rgb)


def park(hwnd: int) -> None:
    point = nw.w.POINT(CLIENT[0] - 10, CLIENT[1] // 2)
    nw.user32.ClientToScreen(hwnd, nw.ctypes.byref(point))
    nw.user32.SetCursorPos(point.x, point.y)
    time.sleep(0.3)


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
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/chrome-windows"
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    summary: dict = {"status": "FAIL"}
    app = None
    try:
        data = pathlib.Path(tempfile.mkdtemp(prefix="sela-chrome-"))
        library = data / "library.sqlite"
        subprocess.run([str(args.seed), str(library)], check=True)
        app = nw.App(args.binary, ["--operator-library", str(library)])
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(1.5)
        if nw.client_size(hwnd) != CLIENT:
            nw.resize_client(hwnd, *CLIENT)
            time.sleep(0.8)
        shots: dict[str, Image] = {}

        nw.click(hwnd, 440, 725)  # first song row
        time.sleep(0.5)
        nw.click(hwnd, 1125, 59)  # Go Live takes focus
        time.sleep(0.8)
        park(hwnd)
        shots["main"] = Image(hwnd, args.out, "main")

        for key in ("alt+e", "enter"):
            nw.press(hwnd, key)
            time.sleep(0.5)
        time.sleep(0.5)
        park(hwnd)
        shots["options"] = Image(hwnd, args.out, "options")
        app.close()
        app = nw.App(args.binary, ["--library", str(data / "editor.sqlite")])
        editor = app.window()
        nw.activate(editor)
        time.sleep(1.5)
        if nw.client_size(editor) != CLIENT:
            nw.resize_client(editor, *CLIENT)
            time.sleep(0.8)
        nw.click(editor, 20, 220)  # section 1 number gutter
        time.sleep(0.5)
        nw.click(editor, 40, 132)  # Words tab takes focus
        time.sleep(0.5)
        park(editor)
        shots["song_editor"] = Image(editor, args.out, "song_editor")
        table = {}
        for name, image in shots.items():
            samples = {}
            for key, region in REGIONS[name].items():
                xy, rgb = search(image, region, TOKENS[key])
                samples[key] = {"xy": xy, "rgb": rgb}
            x = HEADER_COLUMN[name]
            lo, hi = HEADER_SCAN[name]
            rows = border_rows(image, x, lo, hi)
            samples["border"] = {
                "xy": [x, rows[0] if rows else None],
                "rgb": list(BORDER),
            }
            if rows:
                samples["border"]["rgb"] = image.pixel(x, rows[0])
            table[name] = {
                "size": [image.width, image.height],
                "samples": samples,
                "border_rows": rows,
                "header_px": rows[1] - rows[0] if len(rows) >= 2 else None,
            }
        deltas = {
            key: max(
                max(t["samples"][key]["rgb"][i] for t in table.values())
                - min(t["samples"][key]["rgb"][i] for t in table.values())
                for i in range(3)
            )
            for key in ("border", "selected_row", "focus")
        }
        heights = {name: t["header_px"] for name, t in table.items()}
        summary.update(
            table=table,
            max_channel_delta=deltas,
            header_heights=heights,
            colors_within_tolerance=all(d <= TOLERANCE for d in deltas.values()),
            same_header_height=len(set(heights.values())) == 1
            and None not in heights.values(),
        )
        if not (summary["colors_within_tolerance"] and summary["same_header_height"]):
            raise nw.Failure("chrome tokens or header heights differ across windows")
        summary["status"] = "PASS"
        return 0
    except Exception as error:  # noqa: BLE001
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        if app:
            app.close()
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))
        print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
