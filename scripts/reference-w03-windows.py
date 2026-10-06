"""Observe EasyWorship 8.0.49 Go Live / Black / Clear / Logo behavior (W02/W03).

Usage (PowerShell, repository root, EasyWorship already open and maximized with
one song previewed from the Songs tab; another resource tab replaces Preview
with that tab's selection. Keyboard and mouse must stay idle while it runs):

    python scripts/reference-w03-windows.py probe NAME      # capture only
    python scripts/reference-w03-windows.py press INPUT...  # e.g. black ctrl+b
    python scripts/reference-w03-windows.py run [--out DIR]  # full matrix

Inputs are toolbar buttons (golive, logo, black, clear, live), Preview/Live
slide rows (preview1..preview4, live1..live4, with a `dbl:` prefix for a
double-click), Live Output arrows (livenext, liveprev) or key chords
(ctrl+b, ctrl+l, ctrl+c, pagedown, right, left, home, end). Only the EasyWorship
PID's main window receives input, after it is verified foreground. Output is
captured from the screen rectangle of its Live Output window, so the evidence
is what the audience monitor shows. Every step appends one JSONL record with
the W03 fields this harness can observe; interpretation stays in the ledger.
"""

from __future__ import annotations

import argparse
import ctypes
import ctypes.wintypes as w
import hashlib
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_win as nw
import reference_win as rw

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUT = ROOT / ".amp/in/artifacts/reference/ew8-w03"
# Window-relative physical pixels of the maximized main window on the 2560x1440
# primary at 120 DPI (window rect -9,-9,2569,1389), measured from captures.
POINTS = {
    "golive": (2146, 107),
    "logo": (2311, 107),
    "black": (2382, 107),
    "clear": (2451, 107),
    "live": (2533, 107),
    "livenext": (2511, 967),
    "liveprev": (2471, 967),
}
PREVIEW_ROWS = [(1259, 227), (1259, 300), (1259, 341), (1259, 403)]
LIVE_ROWS = [(2291, 227), (2291, 300), (2291, 341), (2291, 403)]
KEYS = {
    "pagedown": [0x22],
    "right": [0x27],
    "left": [0x25],
    "home": [0x24],
    "end": [0x23],
    "ctrl+b": [0x11, ord("B")],
    "ctrl+l": [0x11, ord("L")],
    "ctrl+c": [0x11, ord("C")],
}
SETTLE = 1.2


def main_window() -> int:
    pids = rw.pids("EasyWorship.exe")
    for item in rw.windows(pids):
        if item["class"] == "FMTFormEZWorshipMain":
            return item["hwnd"]
    raise nw.Failure("EasyWorship main window not found")


def output_rect() -> tuple[int, int, int, int]:
    for item in rw.windows(rw.pids("EasyWorship.exe")):
        if item["class"] == "FMTForm_LiveOutput":
            return item["rect"]
    raise nw.Failure("EasyWorship Live Output window not found")


def grab(rect: tuple[int, int, int, int]) -> tuple[int, int, bytes]:
    left, top, right, bottom = rect
    width, height = right - left, bottom - top
    screen = nw.user32.GetDC(None)
    memory = nw.gdi32.CreateCompatibleDC(screen)
    bitmap = nw.gdi32.CreateCompatibleBitmap(screen, width, height)
    previous = nw.gdi32.SelectObject(memory, bitmap)
    try:
        nw.gdi32.BitBlt(
            memory, 0, 0, width, height, screen, left, top, 0x00CC0020 | 0x40000000
        )
        header = nw.BITMAPINFOHEADER(
            biSize=ctypes.sizeof(nw.BITMAPINFOHEADER),
            biWidth=width,
            biHeight=-height,
            biPlanes=1,
            biBitCount=32,
        )
        pixels = ctypes.create_string_buffer(width * height * 4)
        info = ctypes.create_string_buffer(bytes(header) + b"\0" * 16)
        nw.gdi32.GetDIBits(memory, bitmap, 0, height, pixels, info, 0)
        return width, height, pixels.raw
    finally:
        nw.gdi32.SelectObject(memory, previous)
        nw.gdi32.DeleteObject(bitmap)
        nw.gdi32.DeleteDC(memory)
        nw.user32.ReleaseDC(None, screen)


def print_window(hwnd: int) -> tuple[int, int, bytes]:
    """Window-only pixels (PrintWindow), unaffected by overlapping applications."""
    left, top, right, bottom = window_rect(hwnd)
    width, height = right - left, bottom - top
    hdc = nw.user32.GetWindowDC(hwnd)
    memory = nw.gdi32.CreateCompatibleDC(hdc)
    bitmap = nw.gdi32.CreateCompatibleBitmap(hdc, width, height)
    previous = nw.gdi32.SelectObject(memory, bitmap)
    try:
        if not nw.user32.PrintWindow(w.HWND(hwnd), memory, rw.PW_RENDERFULLCONTENT):
            raise nw.Failure(f"PrintWindow failed for {hwnd:#x}")
        header = nw.BITMAPINFOHEADER(
            biSize=ctypes.sizeof(nw.BITMAPINFOHEADER),
            biWidth=width,
            biHeight=-height,
            biPlanes=1,
            biBitCount=32,
        )
        pixels = ctypes.create_string_buffer(width * height * 4)
        info = ctypes.create_string_buffer(bytes(header) + b"\0" * 16)
        nw.gdi32.GetDIBits(memory, bitmap, 0, height, pixels, info, 0)
        return width, height, pixels.raw
    finally:
        nw.gdi32.SelectObject(memory, previous)
        nw.gdi32.DeleteObject(bitmap)
        nw.gdi32.DeleteDC(memory)
        nw.user32.ReleaseDC(hwnd, hdc)


def window_rect(hwnd: int) -> tuple[int, int, int, int]:
    rect = w.RECT()
    nw.user32.GetWindowRect(hwnd, ctypes.byref(rect))
    return rect.left, rect.top, rect.right, rect.bottom


def shrink(width: int, height: int, bgra: bytes, factor: int) -> tuple[int, int, bytes]:
    out_w, out_h = width // factor, height // factor
    rows = bytearray()
    stride = width * 4
    for y in range(out_h):
        row = bgra[y * factor * stride : (y * factor + 1) * stride]
        rows += b"".join(row[x * factor * 4 : x * factor * 4 + 4] for x in range(out_w))
    return out_w, out_h, bytes(rows)


def crop(
    width: int, bgra: bytes, box: tuple[int, int, int, int]
) -> tuple[int, int, bytes]:
    left, top, right, bottom = box
    stride = width * 4
    rows = b"".join(
        bgra[y * stride + left * 4 : y * stride + right * 4] for y in range(top, bottom)
    )
    return right - left, bottom - top, rows


def body(width: int, height: int, bgra: bytes) -> bytes:
    """Output rows above the Windows taskbar, which stays on top of the output."""
    return bgra[: width * 4 * int(height * 0.92)]


# Toolbar crop x of each toggle's button padding (left and right of the icon).
BUTTONS = {
    "logo": (183, 239),
    "black": (254, 310),
    "clear": (323, 379),
    "live": (403, 463),
}


def indicators(width: int, bgra: bytes) -> dict:
    """Highlighted toggles: EasyWorship fills an active toggle's button blue."""
    found = {}
    for name, columns in BUTTONS.items():
        blue = total = 0
        for x in columns:
            for y in range(8, 72, 2):
                i = (y * width + x) * 4
                b, r = bgra[i], bgra[i + 2]
                total += 1
                blue += b > 110 and b > r + 40
        found[name] = blue / total > 0.5
    return found


def stats(width: int, height: int, bgra: bytes) -> dict:
    """Coarse audience description: mean color and share of bright/colored pixels."""
    total = bright = colored = 0
    sums = [0, 0, 0]
    for y in range(0, int(height * 0.92), 8):
        row = y * width * 4
        for x in range(0, width, 8):
            i = row + x * 4
            b, g, r = bgra[i], bgra[i + 1], bgra[i + 2]
            total += 1
            sums[0] += r
            sums[1] += g
            sums[2] += b
            bright += min(r, g, b) > 160
            colored += max(r, g, b) - min(r, g, b) > 40
    return {
        "mean_rgb": [round(s / total, 1) for s in sums],
        "bright_share": round(bright / total, 4),
        "colored_share": round(colored / total, 4),
        "sha256_16": hashlib.sha256(body(width, height, bgra)).hexdigest()[:16],
    }


def settle(rect: tuple[int, int, int, int], timeout: float = 4.0):
    """Capture until three consecutive identical frames; returns (seconds, frame, stable)."""
    start = time.monotonic()
    last, same = None, 0
    while True:
        frame = grab(rect)
        digest = hashlib.sha256(body(*frame)).digest()
        same = same + 1 if digest == last else 0
        last = digest
        elapsed = time.monotonic() - start
        if same >= 2 or elapsed > timeout:
            return round(elapsed, 2), frame, same >= 2
        time.sleep(0.1)


def paste(
    canvas: bytearray, width: int, image: tuple[int, int, bytes], x: int, y: int
) -> None:
    w_, h_, pixels = image
    for row in range(h_):
        start = ((y + row) * width + x) * 4
        canvas[start : start + w_ * 4] = pixels[row * w_ * 4 : (row + 1) * w_ * 4]


class Observer:
    ROW = 220

    def __init__(self, out: Path, run: str):
        self.out, self.run = out, run
        self.out.mkdir(parents=True, exist_ok=True)
        self.hwnd = main_window()
        self.output = output_rect()
        self.records = (self.out / f"{run}.jsonl").open("a", encoding="utf-8")
        self.step = 0
        self.sheet: list[tuple] = []
        self.sheet_case = None

    def send(self, item: str) -> None:
        double = item.startswith("dbl:")
        name = item.removeprefix("dbl:")
        if name in KEYS:
            nw.activate(self.hwnd)
            if nw.user32.GetForegroundWindow() != self.hwnd:
                raise nw.Failure("EasyWorship lost the foreground before a key")
            codes = KEYS[name]
            nw._send(
                [nw._key_input(c, False) for c in codes]
                + [nw._key_input(c, True) for c in reversed(codes)]
            )
            return
        if name in POINTS:
            point = POINTS[name]
        elif name.startswith("preview"):
            point = PREVIEW_ROWS[int(name[7:]) - 1]
        elif name.startswith("live"):
            point = LIVE_ROWS[int(name[4:]) - 1]
        else:
            raise ValueError(item)
        rw.click(self.hwnd, *point, double=double)
        # Park the cursor off the output so hover never enters the evidence.
        left, top, _, _ = window_rect(self.hwnd)
        nw.user32.SetCursorPos(left + 1300, top + 1000)

    def flush_sheet(self) -> None:
        """One contact sheet per case: toolbar | Live pane | audience, one row per step."""
        if not self.sheet:
            return
        width = 469 + 140 + 320 + 20
        canvas = bytearray(width * self.ROW * len(self.sheet) * 4)
        for index, (toolbar, live, output) in enumerate(self.sheet):
            y = index * self.ROW
            paste(canvas, width, toolbar, 0, y)
            paste(canvas, width, live, 479, y)
            paste(canvas, width, output, 629, y)
        nw.write_png(
            self.out / f"{self.run}-{self.sheet_case}-sheet.png",
            width,
            self.ROW * len(self.sheet),
            bytes(canvas),
        )
        self.sheet = []

    def capture(self, case: str, label: str) -> dict:
        if case != self.sheet_case:
            self.flush_sheet()
            self.sheet_case = case
        self.step += 1
        name = f"{case}-{self.step:03d}-{label}".replace("+", "_").replace(":", "_")
        seconds, (width, height, pixels), stable = settle(self.output)
        nw.write_png(self.out / f"{name}-output.png", *shrink(width, height, pixels, 4))
        main = print_window(self.hwnd)
        toolbar = crop(main[0], main[2], (2100, 60, 2569, 140))
        live = shrink(*crop(main[0], main[2], (2010, 140, 2569, 1010)), 4)
        self.sheet.append((toolbar, live, shrink(width, height, pixels, 8)))
        return {
            "indicators": indicators(toolbar[0], toolbar[2]),
            "output": stats(width, height, pixels),
            "settle_seconds": seconds,
            "stable": stable,
            "evidence": name,
        }

    def record(self, case: str, prestate: str, inputs: list[str]) -> dict:
        sent = []
        for item in inputs:
            self.send(item)
            sent.append(item)
            if item != inputs[-1]:
                time.sleep(SETTLE)
        row = {
            "run": self.run,
            "case": case,
            "prestate": prestate,
            "input": sent,
            "time": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        }
        row.update(self.capture(case, "-".join(sent) or "state"))
        self.records.write(json.dumps(row) + "\n")
        self.records.flush()
        print(json.dumps(row))
        return row


MASKS = ["black", "clear", "logo"]


def matrix(obs: Observer) -> None:
    def reset(case: str) -> None:
        # Masks are switched off by reading the toolbar indicators, never
        # assumed; the case's first row records the state this produced.
        for _ in range(4):
            main = print_window(obs.hwnd)
            toolbar = crop(main[0], main[2], (2100, 60, 2569, 140))
            state = indicators(toolbar[0], toolbar[2])
            active = [name for name in MASKS if state[name]]
            if not state["live"]:
                active.append("live")
            if not active:
                break
            for name in active:
                obs.send(name)
                time.sleep(SETTLE)
        else:
            raise nw.Failure("could not return to all masks off with Live on")
        obs.record(case, "masks off, live on", ["preview1", "golive"])

    case = "w02-selection"
    reset(case)
    obs.record(case, "A1 live", ["preview2"])
    obs.record(case, "preview 2 selected", ["preview3"])
    obs.record(case, "preview 3 selected", ["live3"])
    obs.record(case, "live 3 clicked", ["livenext"])
    obs.record(case, "after live next", ["livenext"])
    obs.record(case, "after second live next", ["liveprev"])
    obs.record(case, "after live previous", ["dbl:preview1"])

    for mask in MASKS:
        case = f"single-{mask}"
        reset(case)
        for _ in range(3):
            obs.record(case, "previous row", [mask])
    for first in MASKS:
        for second in MASKS:
            if first == second:
                continue
            case = f"pair-{first}-{second}"
            reset(case)
            for item in (first, second, second, first):
                obs.record(case, "previous row", [item])
    for order in (
        ("black", "clear", "logo"),
        ("black", "logo", "clear"),
        ("clear", "black", "logo"),
        ("clear", "logo", "black"),
        ("logo", "black", "clear"),
        ("logo", "clear", "black"),
    ):
        case = "triple-" + "-".join(order)
        reset(case)
        for item in (*order, *reversed(order)):
            obs.record(case, "previous row", [item])
    for mask in MASKS:
        case = f"masked-apply-golive-{mask}"
        reset(case)
        obs.record(case, "previous row", [mask])
        obs.record(case, "masked", ["preview2", "golive"])
        obs.record(case, "after go live", [mask])
        case = f"masked-apply-dblpreview-{mask}"
        reset(case)
        obs.record(case, "previous row", [mask])
        obs.record(case, "masked", ["dbl:preview3"])
        obs.record(case, "after double-click", [mask])
        case = f"masked-nav-{mask}"
        reset(case)
        obs.record(case, "previous row", [mask])
        obs.record(case, "masked", ["livenext"])
        obs.record(case, "masked next", ["liveprev"])
        obs.record(case, "masked previous", ["dbl:live2"])
        obs.record(case, "after live double-click", [mask])
        case = f"lifecycle-{mask}"
        reset(case)
        obs.record(case, "previous row", [mask])
        obs.record(case, "masked", ["live"])
        obs.record(case, "output off", ["live"])
        obs.record(case, "output on", [mask])
    for chord, mask in (("ctrl+b", "black"), ("ctrl+l", "logo"), ("ctrl+c", "clear")):
        case = f"shortcut-{mask}"
        reset(case)
        for _ in range(2):
            obs.record(case, "previous row", [chord])
    case = "shortcut-pagedown"
    reset(case)
    obs.record(case, "A1 live", ["preview3"])
    obs.record(case, "preview 3 selected", ["pagedown"])
    obs.record(case, "after page down", ["right"])
    obs.record(case, "after right", ["left"])
    obs.record(case, "after left", ["end"])
    obs.record(case, "after end", ["home"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["probe", "press", "seq", "run"])
    parser.add_argument("items", nargs="*")
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = parser.parse_args()
    run = time.strftime("RUN-W03-%Y%m%d-%H%M%S")
    obs = Observer(args.out, run if args.mode == "run" else "probe")
    if args.mode == "probe":
        obs.record(args.items[0] if args.items else "probe", "manual", [])
    elif args.mode == "press":
        obs.record("press", "manual", args.items)
    elif args.mode == "seq":
        for item in args.items:
            obs.record("seq", "previous row", [item])
    else:
        matrix(obs)
    obs.flush_sheet()
    return 0


if __name__ == "__main__":
    sys.exit(main())
