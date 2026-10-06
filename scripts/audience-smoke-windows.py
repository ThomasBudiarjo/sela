#!/usr/bin/env python3
"""Windows smoke for the application audience mode, `sela --audience`.

Spawns the audience on a chosen monitor (default: first non-primary), checks the
Ready and surface-extent reports, applies one centered text cue sized to the
reported surface, then closes the controller pipes and verifies the applied
scene is retained until the window is closed. Captures only the audience
window's own client area. Physical scanout is not measured.
"""

from __future__ import annotations

import argparse
import ctypes
import ctypes.wintypes
import json
import os
import pathlib
import queue
import secrets
import struct
import subprocess
import sys
import threading
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parent.parent
WM_CLOSE = 0x0010
MONITOR_DEFAULTTONEAREST = 2


class MONITORINFO(ctypes.Structure):
    _fields_ = [
        ("cbSize", ctypes.c_ulong),
        ("rcMonitor", ctypes.wintypes.RECT),
        ("rcWork", ctypes.wintypes.RECT),
        ("dwFlags", ctypes.c_ulong),
    ]


def window_and_monitor_rects(hwnd: int) -> tuple[tuple[int, ...], tuple[int, ...]]:
    rect = ctypes.wintypes.RECT()
    nw.user32.GetWindowRect(hwnd, ctypes.byref(rect))
    info = MONITORINFO(cbSize=ctypes.sizeof(MONITORINFO))
    ctypes.windll.user32.GetMonitorInfoW(
        nw.user32.MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), ctypes.byref(info)
    )
    m = info.rcMonitor
    return (rect.left, rect.top, rect.right, rect.bottom), (
        m.left,
        m.top,
        m.right,
        m.bottom,
    )


class Child:
    def __init__(self, argv: list[str], log: pathlib.Path):
        self.log = log.open("wb")
        self.proc = subprocess.Popen(
            argv,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self.log,
            bufsize=0,
        )
        self.queue: queue.Queue[bytes] = queue.Queue()
        self.buffer = bytearray()
        threading.Thread(target=self._pump, daemon=True).start()

    def _pump(self) -> None:
        while chunk := self.proc.stdout.read(65536):
            self.queue.put(chunk)
        self.queue.put(b"")

    def read(self, size: int, timeout: float) -> bytes:
        deadline = time.monotonic() + timeout
        while len(self.buffer) < size:
            try:
                chunk = self.queue.get(timeout=max(0.0, deadline - time.monotonic()))
            except queue.Empty:
                raise TimeoutError("audience frame missing; output unknown") from None
            if not chunk:
                raise RuntimeError("audience disconnected")
            self.buffer.extend(chunk)
        data = bytes(self.buffer[:size])
        del self.buffer[:size]
        return data

    def frame(self, timeout: float = 3.0) -> tuple[int, bytes]:
        magic, version, kind, size = struct.unpack("<4sBBH", self.read(8, timeout))
        assert (magic, version) == (b"SCUE", 1), (magic, version)
        return kind, self.read(size, timeout)

    def close(self) -> None:
        if self.proc.poll() is None:
            self.proc.kill()
            self.proc.wait(5)
        self.log.close()


def text_cue(
    epoch: int, seq: int, extent: tuple[int, int], text: str, size: int
) -> bytes:
    font = (ROOT / "tests/fixtures/DejaVuSans.ttf").read_bytes()
    content = text.encode()
    version = (991).to_bytes(16, "little") + struct.pack("<Q", 1)
    body = (
        epoch.to_bytes(16, "little")
        + struct.pack("<QBI", seq, 0, 3000)
        + (77).to_bytes(16, "little")
        + struct.pack("<QII", seq, *extent)
        + b"\x00"
        + bytes([0, 0, 0, 255])
        + b"\x01"
        + version
        + struct.pack("<HI", size, len(content))
        + content
        + struct.pack("<I", len(font))
        + font
    )
    return struct.pack("<4sBBHI", b"SCUE", 2, 1, 0, len(body)) + body


def ink_box(width: int, height: int, bgra: bytes) -> tuple[int, int, int, int] | None:
    left, top, right, bottom = width, height, -1, -1
    for y in range(0, height, 2):
        row = y * width * 4
        for x in range(0, width, 2):
            i = row + x * 4
            if bgra[i] > 128 and bgra[i + 1] > 128 and bgra[i + 2] > 128:
                left, top, right, bottom = (
                    min(left, x),
                    min(top, y),
                    max(right, x),
                    max(bottom, y),
                )
    return None if right < 0 else (left, top, right, bottom)


def foreign_pixels(
    width: int, height: int, bgra: bytes, box: tuple[int, int, int, int]
) -> int:
    count = 0
    for y in range(0, height, 2):
        row = y * width * 4
        for x in range(0, width, 2):
            if box[0] - 4 <= x <= box[2] + 4 and box[1] - 4 <= y <= box[3] + 4:
                continue
            i = row + x * 4
            count += max(bgra[i], bgra[i + 1], bgra[i + 2]) > 16
    return count


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary", type=pathlib.Path, default=ROOT / "target/debug/sela.exe"
    )
    parser.add_argument("--monitor", default="secondary")
    parser.add_argument("--backend", default="dx12")
    parser.add_argument(
        "--out",
        type=pathlib.Path,
        default=ROOT / ".amp/in/artifacts/audience-smoke-windows",
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "summary.json").unlink(missing_ok=True)
    epoch = int.from_bytes(secrets.token_bytes(16), "little") | 1
    foreground_before = nw.user32.GetForegroundWindow()
    child = Child(
        [str(args.binary), "--audience", f"{epoch:x}", args.monitor, args.backend],
        args.out / "audience.log",
    )
    summary: dict = {
        "epoch_hex": f"{epoch:x}",
        "monitor": args.monitor,
        "backend": args.backend,
    }
    try:
        kind, body = child.frame(timeout=15)
        assert kind == 3 and int.from_bytes(body[:16], "little") == epoch, (
            "Ready first, same epoch"
        )
        summary["max_dimension"] = struct.unpack("<I", body[16:])[0]
        kind, body = child.frame()
        assert kind == 4 and int.from_bytes(body[:16], "little") == epoch, (
            "surface report follows Ready"
        )
        extent = struct.unpack("<II", body[16:24])
        summary["surface"] = extent
        hwnd = next(
            h
            for h in nw.top_windows(child.proc.pid)
            if nw.title(h) == "Sela audience output"
        )
        summary["client"] = nw.client_size(hwnd)
        summary["dpi"] = nw.dpi(hwnd)
        summary["foreground_is_audience"] = nw.user32.GetForegroundWindow() == hwnd
        summary["foreground_unchanged"] = (
            nw.user32.GetForegroundWindow() == foreground_before
        )
        assert not summary["foreground_is_audience"], (
            "audience must not take keyboard focus"
        )
        assert tuple(summary["client"]) == tuple(extent), (summary["client"], extent)
        summary["window_rect"], summary["monitor_rect"] = window_and_monitor_rects(hwnd)
        assert summary["window_rect"] == summary["monitor_rect"], (
            "audience must cover its whole monitor"
        )

        size = max(12, min(96, extent[1] // 10))
        child.proc.stdin.write(
            text_cue(epoch, 1, extent, "Sela audience\ncentered check", size)
        )
        outcomes = []
        summary["later_surface_reports"] = []
        while len(outcomes) < 2:
            kind, body = child.frame()
            if kind == 4:
                summary["later_surface_reports"].append(
                    struct.unpack("<II", body[16:24])
                )
                continue
            assert kind == 2 and struct.unpack("<Q", body[16:24])[0] == 1, (
                kind,
                body.hex(),
            )
            outcomes.append(body[24])
        assert outcomes == [0, 1], f"Accepted then Applied, got {outcomes}"
        assert not summary["later_surface_reports"], (
            "first surface report must be settled"
        )
        time.sleep(0.3)
        width, height, bgra = nw.capture_client(hwnd)
        nw.write_png(args.out / "applied.png", width, height, bgra)
        box = ink_box(width, height, bgra)
        assert box, "no white text visible"
        cx, cy = (box[0] + box[2]) / 2, (box[1] + box[3]) / 2
        summary["ink_box"] = box
        # The capture is of the composed screen, so a taskbar or another window
        # over the output shows up as non-black pixels outside the text.
        summary["foreign_pixels"] = foreign_pixels(width, height, bgra, box)
        assert summary["foreign_pixels"] == 0, "something is drawn over the output"
        assert (
            abs(cx - width / 2) < width * 0.05 and abs(cy - height / 2) < height * 0.08
        ), (cx, cy)

        child.proc.stdin.close()
        time.sleep(1.5)
        assert child.proc.poll() is None, (
            "audience must retain output after controller loss"
        )
        width, height, after = nw.capture_client(hwnd)
        nw.write_png(
            args.out / "retained-after-controller-loss.png", width, height, after
        )
        assert ink_box(width, height, after) == box, "retained scene changed"

        ctypes.windll.user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
        summary["exit_code"] = child.proc.wait(5)
        assert summary["exit_code"] == 0
        summary["status"] = "PASS"
        print(
            "PASS: audience Ready/surface report, centered Applied cue, retained after controller loss"
        )
        return 0
    finally:
        child.close()
        summary.setdefault("status", "FAIL")
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))
        if os.environ.get("SELA_SMOKE_VERBOSE"):
            print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
