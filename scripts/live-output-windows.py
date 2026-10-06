"""Windows native check: operator Live output drives the real audience renderer.

Usage: python scripts/live-output-windows.py [--binary sela.exe] [--seed seed_library.exe]
       [--monitor secondary] [--out DIR]

Seeds a new library with original fixture songs, launches the operator with a
private profile, and drives it only by keyboard while asserting the operator
keeps the foreground (no re-activation between keys). Checks: no audience before
Live is on; the audience child appears on the requested monitor without taking
focus; a new session shows nothing until Go Live; Go Live, Next (stopping at the
last slide) and Previous change the captured audience output; Live off ends the
child; Ctrl+Q exits cleanly and ends the child. Requires exclusive use of the
keyboard and an unobstructed audience monitor while it runs.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time
from ctypes import wintypes as w

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIENCE_TITLE = "Sela audience output"
OPERATOR_TITLE = "Sela — Technical preview"
# Operator tab order: controls 0-9, Live output, Go Live, Previous, Next, songs, slides.
TABS_TO_FIRST_SONG = 15

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)


class PROCESSENTRY32W(ctypes.Structure):
    _fields_ = [
        ("dwSize", w.DWORD),
        ("cntUsage", w.DWORD),
        ("th32ProcessID", w.DWORD),
        ("th32DefaultHeapID", ctypes.c_size_t),
        ("th32ModuleID", w.DWORD),
        ("cntThreads", w.DWORD),
        ("th32ParentProcessID", w.DWORD),
        ("pcPriClassBase", w.LONG),
        ("dwFlags", w.DWORD),
        ("szExeFile", w.WCHAR * 260),
    ]


kernel32.CreateToolhelp32Snapshot.restype = w.HANDLE
kernel32.Process32FirstW.argtypes = [w.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
kernel32.Process32NextW.argtypes = [w.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
kernel32.OpenProcess.restype = w.HANDLE


def children(pid: int) -> list[int]:
    snapshot = kernel32.CreateToolhelp32Snapshot(0x2, 0)
    entry = PROCESSENTRY32W(dwSize=ctypes.sizeof(PROCESSENTRY32W))
    found = []
    try:
        ok = kernel32.Process32FirstW(snapshot, ctypes.byref(entry))
        while ok:
            if entry.th32ParentProcessID == pid:
                found.append(entry.th32ProcessID)
            ok = kernel32.Process32NextW(snapshot, ctypes.byref(entry))
    finally:
        kernel32.CloseHandle(snapshot)
    return found


def alive(pid: int) -> bool:
    handle = kernel32.OpenProcess(
        0x1000, False, pid
    )  # PROCESS_QUERY_LIMITED_INFORMATION
    if not handle:
        return False
    try:
        code = w.DWORD()
        kernel32.GetExitCodeProcess(handle, ctypes.byref(code))
        return code.value == 259  # STILL_ACTIVE
    finally:
        kernel32.CloseHandle(handle)


def kill(pid: int) -> None:
    handle = kernel32.OpenProcess(0x0001, False, pid)  # PROCESS_TERMINATE
    if handle:
        kernel32.TerminateProcess(handle, 1)
        kernel32.CloseHandle(handle)


def audience(operator_pid: int, timeout: float) -> tuple[int, int] | None:
    deadline = time.monotonic() + timeout
    while True:
        for pid in children(operator_pid):
            for hwnd in nw.top_windows(pid):
                if nw.title(hwnd) == AUDIENCE_TITLE:
                    return pid, hwnd
        if time.monotonic() >= deadline:
            return None
        time.sleep(0.05)


def wait_dead(pid: int, timeout: float) -> bool:
    deadline = time.monotonic() + timeout
    while alive(pid):
        if time.monotonic() >= deadline:
            return False
        time.sleep(0.05)
    return True


def digest(hwnd: int) -> str:
    return hashlib.sha256(nw.capture_client(hwnd)[2]).hexdigest()


def ink_box(width: int, height: int, bgra: bytes) -> tuple[int, int, int, int] | None:
    left, top, right, bottom = width, height, -1, -1
    for y in range(0, height, 2):
        row = y * width * 4
        for x in range(0, width, 2):
            i = row + x * 4
            if bgra[i] > 128 and bgra[i + 1] > 128 and bgra[i + 2] > 128:
                left, top = min(left, x), min(top, y)
                right, bottom = max(right, x), max(bottom, y)
    return None if right < 0 else (left, top, right, bottom)


class Run:
    def __init__(self, operator: int, out: pathlib.Path, summary: dict):
        self.operator, self.out, self.summary = operator, out, summary

    def keys(self, *sequence: str) -> None:
        for keys in sequence:
            nw.press(self.operator, keys)
            time.sleep(0.12)

    def save(self, hwnd: int, name: str) -> tuple[int, int, bytes]:
        width, height, pixels = nw.capture_client(hwnd)
        nw.write_png(self.out / f"{name}.png", width, height, pixels)
        return width, height, pixels

    def change(
        self, hwnd: int, before: str, keys: str, name: str, timeout: float = 3.0
    ) -> str:
        """Press keys and wait for the captured output to differ from `before`."""
        sent = time.monotonic()
        self.keys(keys)
        while digest(hwnd) == before:
            if time.monotonic() - sent > timeout:
                raise nw.Failure(
                    f"{name}: audience output did not change within {timeout}s"
                )
            time.sleep(0.02)
        self.summary[f"{name}_observed_ms"] = round((time.monotonic() - sent) * 1000)
        time.sleep(0.3)
        width, height, pixels = self.save(hwnd, name)
        box = ink_box(width, height, pixels)
        if box is None:
            raise nw.Failure(f"{name}: no text visible on the audience output")
        self.summary[f"{name}_ink_box"] = box
        return digest(hwnd)


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
        default=ROOT / ".amp/in/artifacts/live-output-windows",
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(
                f"FAIL: missing {path} (cargo build && cargo build --example seed_library)"
            )
            return 1
    summary: dict = {"binary": str(args.binary.resolve()), "monitor": args.monitor}
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-live-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library)], check=True)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    app = nw.App(args.binary, ["--operator-library", str(library)])
    spawned: set[int] = set()
    try:
        operator = app.window()
        summary["operator_title"] = nw.title(operator)
        if summary["operator_title"] != OPERATOR_TITLE:
            raise nw.Failure(
                f"unexpected operator window {summary['operator_title']!r}"
            )
        run = Run(operator, args.out, summary)
        nw.activate(operator)
        time.sleep(1.0)  # storage worker loads the catalog off the UI thread

        run.keys(*["tab"] * TABS_TO_FIRST_SONG, "enter", "tab", "enter")
        time.sleep(0.3)
        run.save(operator, "operator-preview")
        if audience(app.process.pid, 0.5):
            raise nw.Failure("an audience output exists before Live output is on")

        # Slide 0 -> song 0 -> Next -> Previous -> Go Live -> Live output.
        run.keys(*["shift+tab"] * 5, "enter")
        started = time.monotonic()
        found = audience(app.process.pid, 15)
        if not found:
            raise nw.Failure(f"no audience window within 15s: {app.log()}")
        pid, screen = found
        spawned.add(pid)
        summary["audience_window_seconds"] = round(time.monotonic() - started, 3)
        time.sleep(1.5)
        if nw.user32.GetForegroundWindow() != operator:
            raise nw.Failure(
                "the audience output took the foreground from the operator"
            )
        summary["audience_client"] = nw.client_size(screen)
        width, height, pixels = run.save(screen, "audience-new-session")
        if ink_box(width, height, pixels) is not None:
            raise nw.Failure("a new output session showed content before Go Live")
        run.save(operator, "operator-live-on")

        blank = digest(screen)
        run.keys("tab")  # Go Live
        first = run.change(screen, blank, "enter", "go_live")
        run.save(operator, "operator-go-live")
        run.keys("tab", "tab")  # Next
        second = run.change(screen, first, "enter", "next_1")
        third = run.change(screen, second, "enter", "next_2")
        run.keys("enter")
        time.sleep(1.0)
        if digest(screen) != third:
            raise nw.Failure("Next past the last slide changed the audience output")
        run.keys("shift+tab")  # Previous
        back = run.change(screen, third, "enter", "previous")
        if back != second:
            summary["previous_matches_second"] = False
            raise nw.Failure("Previous did not restore the second slide's output")
        run.save(operator, "operator-previous")

        run.keys("shift+tab", "shift+tab", "enter")  # Live output off
        if not wait_dead(pid, 5):
            raise nw.Failure("Live output off did not end the audience child")
        summary["live_off_ended_child"] = True
        run.save(operator, "operator-live-off")

        run.keys("enter")  # Live output on again: a new session, no replay.
        found = audience(app.process.pid, 15)
        if not found:
            raise nw.Failure("no audience window after turning Live output back on")
        pid, screen = found
        spawned.add(pid)
        time.sleep(1.5)
        width, height, pixels = run.save(screen, "audience-second-session")
        if ink_box(width, height, pixels) is not None:
            raise nw.Failure("a new output session replayed an earlier slide")

        run.keys("ctrl+q")
        summary["operator_exit"] = app.wait_exit()
        if summary["operator_exit"] != 0:
            raise nw.Failure(f"operator exit status {summary['operator_exit']}")
        summary["quit_ended_child"] = wait_dead(pid, 5)
        if not summary["quit_ended_child"]:
            raise nw.Failure("Ctrl+Q left the audience output running")
        summary["status"] = "PASS"
        print(
            "PASS: Live on spawns a non-activating audience, Go Live/Next/Previous change it, "
            "Next stops at the end, new sessions start empty, Live off and Ctrl+Q end it"
        )
        return 0
    except (nw.Failure, AssertionError) as error:
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        for name, hwnd in (
            ("operator", app.hwnd),
            ("audience", locals().get("screen")),
        ):
            if hwnd and nw.user32.IsWindow(hwnd):
                run.save(hwnd, f"failure-{name}")
        return 1
    finally:
        app.close()
        for pid in spawned:
            if alive(pid):
                kill(pid)
        summary.setdefault("status", "FAIL")
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))
        if os.environ.get("SELA_SMOKE_VERBOSE"):
            print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
