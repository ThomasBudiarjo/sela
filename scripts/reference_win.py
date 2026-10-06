"""Window-scoped Win32 helpers for observing an installed reference application.

Captures use PrintWindow on one HWND, so overlapping windows of other
applications never enter the evidence. Input is sent only after the target
window is foreground. Coordinates are physical client pixels of that window.
"""

from __future__ import annotations

import ctypes
import ctypes.wintypes as w
import subprocess
import sys
import time
from pathlib import Path

import native_win as nw

user32 = nw.user32
gdi32 = nw.gdi32
user32.PrintWindow.argtypes = [w.HWND, w.HDC, w.UINT]
user32.GetWindowDC.argtypes = [w.HWND]
user32.GetWindowDC.restype = w.HDC
user32.IsIconic.argtypes = [w.HWND]
user32.ShowWindow.argtypes = [w.HWND, ctypes.c_int]
user32.GetClassNameW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]

PW_RENDERFULLCONTENT = 2
SW_RESTORE = 9


def pids(image: str) -> set[int]:
    out = subprocess.run(
        ["tasklist", "/FI", f"IMAGENAME eq {image}", "/FO", "CSV", "/NH"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    found = set()
    for line in out.splitlines():
        parts = [p.strip('"') for p in line.split('","')]
        if len(parts) > 1 and parts[1].isdigit():
            found.add(int(parts[1]))
    return found


def windows(pid_set: set[int]) -> list[dict]:
    found: list[dict] = []

    @nw.WNDENUMPROC
    def visit(hwnd, _):
        pid = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if pid.value in pid_set and user32.IsWindowVisible(hwnd):
            rect = w.RECT()
            user32.GetWindowRect(hwnd, ctypes.byref(rect))
            cls = ctypes.create_unicode_buffer(256)
            user32.GetClassNameW(hwnd, cls, 256)
            found.append(
                {
                    "hwnd": int(hwnd),
                    "title": nw.title(hwnd),
                    "class": cls.value,
                    "minimized": bool(user32.IsIconic(hwnd)),
                    "rect": (rect.left, rect.top, rect.right, rect.bottom),
                    "dpi": user32.GetDpiForWindow(hwnd),
                }
            )
        return True

    user32.EnumWindows(visit, 0)
    return found


def restore(hwnd: int) -> None:
    if user32.IsIconic(hwnd):
        user32.ShowWindow(w.HWND(hwnd), SW_RESTORE)
        time.sleep(1.0)
    nw.activate(hwnd)


def capture(hwnd: int, path: Path) -> tuple[int, int]:
    rect = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(rect))
    width, height = rect.right - rect.left, rect.bottom - rect.top
    hdc = user32.GetWindowDC(hwnd)
    mdc = gdi32.CreateCompatibleDC(hdc)
    bmp = gdi32.CreateCompatibleBitmap(hdc, width, height)
    gdi32.SelectObject(mdc, bmp)
    try:
        if not user32.PrintWindow(w.HWND(hwnd), mdc, PW_RENDERFULLCONTENT):
            raise nw.Failure(f"PrintWindow failed for {hwnd:#x}")
        info = nw.BITMAPINFOHEADER()
        info.biSize = ctypes.sizeof(info)
        info.biWidth = width
        info.biHeight = -height
        info.biPlanes = 1
        info.biBitCount = 32
        buf = ctypes.create_string_buffer(width * height * 4)
        gdi32.GetDIBits(mdc, bmp, 0, height, buf, ctypes.byref(info), 0)
    finally:
        gdi32.DeleteObject(bmp)
        gdi32.DeleteDC(mdc)
        user32.ReleaseDC(hwnd, hdc)
    path.parent.mkdir(parents=True, exist_ok=True)
    nw.write_png(path, width, height, buf.raw)
    return width, height


def _screen(hwnd: int, x: int, y: int) -> tuple[int, int]:
    rect = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(rect))
    return rect.left + x, rect.top + y


def click(hwnd: int, x: int, y: int, double: bool = False, right: bool = False) -> None:
    """Click at window-relative physical coordinates (window rect, not client)."""
    nw.activate(hwnd)
    sx, sy = _screen(hwnd, x, y)
    user32.SetCursorPos(sx, sy)
    time.sleep(0.05)
    down, up = (0x0008, 0x0010) if right else (0x0002, 0x0004)
    for _ in range(2 if double else 1):
        user32.mouse_event(down, 0, 0, 0, 0)
        user32.mouse_event(up, 0, 0, 0, 0)
        time.sleep(0.05)


def type_text(hwnd: int, text: str) -> None:
    nw.activate(hwnd)
    for ch in text:
        if ch == "\n":
            nw._send(
                [
                    nw._key_input(nw.VK["enter"], False),
                    nw._key_input(nw.VK["enter"], True),
                ]
            )
            time.sleep(0.02)
            continue
        inputs = []
        for up in (False, True):
            item = nw.INPUT(type=1)
            item.u.ki = nw.KEYBDINPUT(
                wScan=ord(ch), dwFlags=0x0004 | (0x0002 if up else 0)
            )
            inputs.append(item)
        nw._send(inputs)
        time.sleep(0.01)


def key(hwnd: int, vk: int) -> None:
    nw.activate(hwnd)
    nw._send([nw._key_input(vk, False), nw._key_input(vk, True)])


if __name__ == "__main__":
    for item in windows(pids(sys.argv[1] if len(sys.argv) > 1 else "EasyWorship.exe")):
        print(item)
