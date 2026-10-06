"""Win32 helpers for native Sela checks. Standard library only (ctypes).

Every helper is scoped to a launched PID or HWND; nothing searches by title or
sends input without first verifying that the target window owns the foreground.
"""

from __future__ import annotations

import ctypes
import os
import struct
import subprocess
import tempfile
import time
import zlib
from ctypes import wintypes as w
from pathlib import Path

user32 = ctypes.WinDLL("user32", use_last_error=True)
gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

# Physical-pixel coordinates for every call below.
user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))

user32.GetForegroundWindow.restype = w.HWND
user32.GetDC.restype = w.HDC
user32.GetDC.argtypes = [w.HWND]
user32.ReleaseDC.argtypes = [w.HWND, w.HDC]
user32.GetDpiForWindow.argtypes = [w.HWND]
user32.GetDpiForWindow.restype = w.UINT
user32.IsWindow.argtypes = [w.HWND]
user32.IsWindowVisible.argtypes = [w.HWND]
user32.SetForegroundWindow.argtypes = [w.HWND]
user32.GetWindow.argtypes = [w.HWND, w.UINT]
user32.GetWindow.restype = w.HWND
user32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
user32.GetWindowThreadProcessId.restype = w.DWORD
user32.AttachThreadInput.argtypes = [w.DWORD, w.DWORD, w.BOOL]
user32.BringWindowToTop.argtypes = [w.HWND]
user32.GetClientRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.ClientToScreen.argtypes = [w.HWND, ctypes.POINTER(w.POINT)]
user32.SetWindowPos.argtypes = [w.HWND, w.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, w.UINT]
user32.GetWindowTextW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]
user32.MonitorFromWindow.argtypes = [w.HWND, w.DWORD]
user32.MonitorFromWindow.restype = w.HMONITOR
gdi32.CreateCompatibleDC.argtypes = [w.HDC]
gdi32.CreateCompatibleDC.restype = w.HDC
gdi32.CreateCompatibleBitmap.argtypes = [w.HDC, ctypes.c_int, ctypes.c_int]
gdi32.CreateCompatibleBitmap.restype = w.HBITMAP
gdi32.SelectObject.argtypes = [w.HDC, w.HGDIOBJ]
gdi32.SelectObject.restype = w.HGDIOBJ
gdi32.BitBlt.argtypes = [w.HDC, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, w.HDC, ctypes.c_int, ctypes.c_int, w.DWORD]
gdi32.GetDIBits.argtypes = [w.HDC, w.HBITMAP, w.UINT, w.UINT, ctypes.c_void_p, ctypes.c_void_p, w.UINT]
gdi32.DeleteObject.argtypes = [w.HGDIOBJ]
gdi32.DeleteDC.argtypes = [w.HDC]

WNDENUMPROC = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
user32.EnumWindows.argtypes = [WNDENUMPROC, w.LPARAM]

VK = {"ctrl": 0x11, "alt": 0x12, "shift": 0x10, "tab": 0x09, "enter": 0x0D, "space": 0x20, "f4": 0x73}
KEYEVENTF_KEYUP = 0x0002
SWP_NOMOVE, SWP_NOZORDER, SWP_NOACTIVATE = 0x0002, 0x0004, 0x0010


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [("dx", w.LONG), ("dy", w.LONG), ("mouseData", w.DWORD), ("dwFlags", w.DWORD),
                ("time", w.DWORD), ("dwExtraInfo", ctypes.c_size_t)]


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [("wVk", w.WORD), ("wScan", w.WORD), ("dwFlags", w.DWORD), ("time", w.DWORD),
                ("dwExtraInfo", ctypes.c_size_t)]


class _INPUTUNION(ctypes.Union):
    _fields_ = [("mi", MOUSEINPUT), ("ki", KEYBDINPUT)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", w.DWORD), ("u", _INPUTUNION)]


user32.SendInput.argtypes = [w.UINT, ctypes.POINTER(INPUT), ctypes.c_int]


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [("biSize", w.DWORD), ("biWidth", w.LONG), ("biHeight", w.LONG), ("biPlanes", w.WORD),
                ("biBitCount", w.WORD), ("biCompression", w.DWORD), ("biSizeImage", w.DWORD),
                ("biXPelsPerMeter", w.LONG), ("biYPelsPerMeter", w.LONG), ("biClrUsed", w.DWORD),
                ("biClrImportant", w.DWORD)]


class Failure(Exception):
    pass


def _send(inputs: list[INPUT]) -> None:
    array = (INPUT * len(inputs))(*inputs)
    if user32.SendInput(len(inputs), array, ctypes.sizeof(INPUT)) != len(inputs):
        raise Failure(f"SendInput failed: {ctypes.get_last_error()}")


def _key_input(vk: int, up: bool) -> INPUT:
    item = INPUT(type=1)
    item.u.ki = KEYBDINPUT(wVk=vk, dwFlags=KEYEVENTF_KEYUP if up else 0)
    return item


def _vk(name: str) -> int:
    name = name.lower()
    if name in VK:
        return VK[name]
    if len(name) == 1 and name.isalnum():
        return ord(name.upper())
    raise ValueError(name)


def top_windows(pid: int) -> list[int]:
    found: list[int] = []

    def visit(hwnd, _):
        owner = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(hwnd) and not user32.GetWindow(hwnd, 4):
            # winit keeps a visible, unowned 0x0 helper window; it is never a target.
            rect = w.RECT()
            if user32.GetClientRect(hwnd, ctypes.byref(rect)) and rect.right > rect.left and rect.bottom > rect.top:
                found.append(int(hwnd))
        return True

    user32.EnumWindows(WNDENUMPROC(visit), 0)
    return found


def title(hwnd: int) -> str:
    buffer = ctypes.create_unicode_buffer(512)
    user32.GetWindowTextW(hwnd, buffer, 512)
    return buffer.value


def activate(hwnd: int) -> None:
    for attempt in range(30):
        if user32.GetForegroundWindow() == hwnd:
            return
        if attempt < 10:
            # A zero-motion injected event makes this process the last input source,
            # which Windows requires before honoring SetForegroundWindow.
            _send([INPUT(type=0)])
            user32.SetForegroundWindow(hwnd)
        else:
            # Fallback: share the current foreground thread's input state briefly.
            foreground = user32.GetForegroundWindow()
            theirs = user32.GetWindowThreadProcessId(foreground, None) if foreground else 0
            ours = kernel32.GetCurrentThreadId()
            attached = bool(theirs) and theirs != ours and user32.AttachThreadInput(ours, theirs, True)
            try:
                user32.BringWindowToTop(hwnd)
                user32.SetForegroundWindow(hwnd)
            finally:
                if attached:
                    user32.AttachThreadInput(ours, theirs, False)
        time.sleep(0.1)
    raise Failure(f"window {hwnd:#x} never became foreground")


def chord(hwnd: int, keys: str) -> None:
    """Send e.g. 'ctrl+q' only while the PID window is verified foreground."""
    activate(hwnd)
    if user32.GetForegroundWindow() != hwnd:
        raise Failure("focus changed before input")
    codes = [_vk(k) for k in keys.split("+")]
    _send([_key_input(c, False) for c in codes] + [_key_input(c, True) for c in reversed(codes)])


def dpi(hwnd: int) -> int:
    return int(user32.GetDpiForWindow(hwnd))


def client_size(hwnd: int) -> tuple[int, int]:
    rect = w.RECT()
    if not user32.GetClientRect(hwnd, ctypes.byref(rect)):
        raise Failure("GetClientRect failed")
    return rect.right - rect.left, rect.bottom - rect.top


def resize_client(hwnd: int, width: int, height: int) -> tuple[int, int]:
    """Request a physical client size; returns the settled client size."""
    outer, inner = w.RECT(), w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(outer))
    user32.GetClientRect(hwnd, ctypes.byref(inner))
    extra_w = (outer.right - outer.left) - (inner.right - inner.left)
    extra_h = (outer.bottom - outer.top) - (inner.bottom - inner.top)
    user32.SetWindowPos(hwnd, None, 0, 0, width + extra_w, height + extra_h,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE)
    last, stable = None, 0
    for _ in range(50):
        time.sleep(0.1)
        size = client_size(hwnd)
        stable = stable + 1 if size == last else 0
        last = size
        if stable >= 3:
            break
    return last


def capture_client(hwnd: int) -> tuple[int, int, bytes]:
    """Screen BitBlt of the visible client area; returns top-down BGRA."""
    width, height = client_size(hwnd)
    origin = w.POINT(0, 0)
    user32.ClientToScreen(hwnd, ctypes.byref(origin))
    screen = user32.GetDC(None)
    memory = gdi32.CreateCompatibleDC(screen)
    bitmap = gdi32.CreateCompatibleBitmap(screen, width, height)
    previous = gdi32.SelectObject(memory, bitmap)
    try:
        if not gdi32.BitBlt(memory, 0, 0, width, height, screen, origin.x, origin.y, 0x00CC0020 | 0x40000000):
            raise Failure("BitBlt failed")
        header = BITMAPINFOHEADER(biSize=ctypes.sizeof(BITMAPINFOHEADER), biWidth=width, biHeight=-height,
                                  biPlanes=1, biBitCount=32, biCompression=0)
        pixels = ctypes.create_string_buffer(width * height * 4)
        info = ctypes.create_string_buffer(bytes(header) + b"\0" * 16)
        if gdi32.GetDIBits(memory, bitmap, 0, height, pixels, info, 0) != height:
            raise Failure("GetDIBits failed")
        return width, height, pixels.raw
    finally:
        gdi32.SelectObject(memory, previous)
        gdi32.DeleteObject(bitmap)
        gdi32.DeleteDC(memory)
        user32.ReleaseDC(None, screen)


def bgra_to_rgb(bgra: bytes) -> bytes:
    rgb = bytearray(len(bgra) // 4 * 3)
    rgb[0::3], rgb[1::3], rgb[2::3] = bgra[2::4], bgra[1::4], bgra[0::4]
    return bytes(rgb)


def write_png(path: Path, width: int, height: int, bgra: bytes) -> None:
    rows = bytearray()
    stride = width * 4
    for y in range(height):
        row = bgra[y * stride:(y + 1) * stride]
        rgb = bytearray(width * 3)
        rgb[0::3], rgb[1::3], rgb[2::3] = row[2::4], row[1::4], row[0::4]
        rows += b"\0" + rgb

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(bytes(rows), 6)) + chunk(b"IEND", b""))


def color_count(bgra: bytes, limit: int = 4096) -> int:
    seen = set()
    for i in range(0, len(bgra), 4 * 7):
        seen.add(bgra[i:i + 3])
        if len(seen) >= limit:
            break
    return len(seen)


class App:
    """One launched Sela process with a private profile; always reaped on exit."""

    def __init__(self, binary: Path, args: list[str] | None = None, scratch: Path | None = None):
        self.owned = scratch is None
        self.scratch = Path(tempfile.mkdtemp(prefix="sela-native-")) if scratch is None else scratch
        for name in ("local", "roaming", "temp"):
            (self.scratch / name).mkdir(exist_ok=True)
        env = dict(os.environ)
        env.update(LOCALAPPDATA=str(self.scratch / "local"), APPDATA=str(self.scratch / "roaming"),
                   TEMP=str(self.scratch / "temp"), TMP=str(self.scratch / "temp"),
                   RUST_LOG=os.environ.get("RUST_LOG", "info"))
        self.log_path = self.scratch / f"app-{time.time_ns()}.log"
        self._log = self.log_path.open("wb")
        self.process = subprocess.Popen([str(binary), *(args or [])], env=env, stdout=self._log,
                                        stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
        self.hwnd = 0

    def window(self, timeout: float = 15.0) -> int:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise Failure(f"exited early ({self.process.returncode}): {self.log()}")
            found = top_windows(self.process.pid)
            if found:
                self.hwnd = found[0]
                return self.hwnd
            time.sleep(0.1)
        raise Failure("window startup timed out")

    def wait_exit(self, timeout: float = 10.0) -> int:
        try:
            code = self.process.wait(timeout)
        except subprocess.TimeoutExpired as error:
            raise Failure("process exit timed out") from error
        if self.hwnd and user32.IsWindow(self.hwnd):
            raise Failure("window survived process exit")
        return code

    def log(self) -> str:
        self._log.flush()
        return self.log_path.read_text(errors="replace")[-4000:]

    def close(self) -> None:
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(2)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        self._log.close()
