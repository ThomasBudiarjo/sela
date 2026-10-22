"""Windows native check: FFmpeg located at startup, with and without it.

Feature media-core (M2-01/M2-02; VAL-MEDIA-001/002/003/030/031). Windows
counterpart of scripts/media-backend-x11.py.

Usage: python scripts/media-backend-windows.py [--binary target\\debug\\sela.exe]
       [--seed target\\debug\\examples\\seed_library.exe] [--out DIR]
       [--gpl-dir DIR] [--skip-cargo]

Prerequisites: scripts\\fetch-ffmpeg.ps1 (the pinned LGPL win64 build in
.amp\\in\\ffmpeg\\n8.1.3-lgpl), `cargo build --locked` and `cargo build
--locked --examples`, the owner's keyboard and mouse idle, no other Sela.
Uses a private profile and SELA_AUDIENCE_MONITOR=window.

1. "with": SELA_FFMPEG_DIR empty, so the debug build's development copy is
   used: the log gets "media: FFmpeg n8.1... accepted from the development
   copy", the UIA tree has no `media-unavailable` node, and no console
   window (ConsoleWindowClass) appears while the probe children run
   (CREATE_NO_WINDOW, VAL-MEDIA-031).
2. "gpl" (only with --gpl-dir, a folder holding a GPL ffmpeg.exe and
   ffprobe.exe such as a gyan.dev "full" build): SELA_FFMPEG_DIR=<dir> and
   SELA_FFMPEG_NO_DEV_COPY=1. The log names --enable-gpl, the UIA status
   `media-unavailable` reads "Video and audio unavailable: the FFmpeg found
   is a GPL build", and the show still runs: Live output on, the first song
   previewed, Go Live, Ctrl+B with the Black button's UIA toggle On.
3. "missing": SELA_FFMPEG_DIR empty and SELA_FFMPEG_NO_DEV_COPY=1: the
   status reads "Video and audio unavailable: FFmpeg was not found", and
   Live output, Go Live and Black work as in run 2.
4. Unless --skip-cargo: `cargo test --locked --test media_decode` with
   SELA_REQUIRE_FFMPEG=1, which decodes generated clips with the real build
   and checks that dropping a decoder and an outside kill end ffmpeg.exe
   (the Job Object with KILL_ON_JOB_CLOSE, VAL-MEDIA-030).

Captures, logs and summary.json go to --out. Takes over the real keyboard;
never run with EasyWorship open.
"""

from __future__ import annotations

import argparse
import ctypes
import importlib.util
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
_a11y_spec = importlib.util.spec_from_file_location(
    "a11y_windows", ROOT / "scripts" / "a11y-windows.py"
)
a11y = importlib.util.module_from_spec(_a11y_spec)
_a11y_spec.loader.exec_module(a11y)

USER32 = ctypes.WinDLL("user32", use_last_error=True)
DEV_COPY = ROOT / ".amp" / "in" / "ffmpeg" / "n8.1.3-lgpl"
WIN64_SHA256 = "3e47bda1607740550141e37c0e49d1e5182b34699f15adfd137ee266d346811a"
WARNING_ID = "media-unavailable"
GPL_TEXT = "Video and audio unavailable: the FFmpeg found is a GPL build"
MISSING_TEXT = "Video and audio unavailable: FFmpeg was not found"
LOG_TIMEOUT = 20.0


def console_windows() -> set[int]:
    """Visible top-level console windows on the desktop."""
    found: set[int] = set()
    buffer = ctypes.create_unicode_buffer(64)

    @ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
    def visit(hwnd, _):
        if USER32.IsWindowVisible(hwnd):
            USER32.GetClassNameW(hwnd, buffer, 64)
            if buffer.value == "ConsoleWindowClass":
                found.add(int(hwnd))
        return True

    USER32.EnumWindows(visit, 0)
    return found


class Check:
    def __init__(self, out: pathlib.Path):
        self.out = out
        self.summary: dict = {"checks": [], "failures": []}

    def expect(self, condition: bool, what: str) -> None:
        self.summary["checks"].append({"check": what, "ok": bool(condition)})
        if not condition:
            self.summary["failures"].append(what)
            print(f"FAILED: {what}")

    def write(self) -> None:
        (self.out / "summary.json").write_text(
            json.dumps(self.summary, indent=2), encoding="utf-8"
        )


def media_line(app: nw.App) -> str:
    deadline = time.monotonic() + LOG_TIMEOUT
    while time.monotonic() < deadline:
        for line in app.log().splitlines():
            if line.startswith("sela: media: "):
                return line
        time.sleep(0.2)
    raise nw.Failure(f"no media log line within {LOG_TIMEOUT}s: {app.log()}")


def warning_node(hwnd: int) -> dict | None:
    return a11y.nodes_by_id(a11y.uia(hwnd)).get(WARNING_ID)


def launch(binary: pathlib.Path, library: pathlib.Path, env: dict) -> nw.App:
    for key in ("SELA_FFMPEG_DIR", "SELA_FFMPEG_NO_DEV_COPY"):
        os.environ.pop(key, None)
    os.environ.update(env)
    os.environ["SELA_AUDIENCE_MONITOR"] = "window"
    return nw.App(binary, ["--operator-library", str(library)])


def save(hwnd: int, out: pathlib.Path, name: str) -> None:
    width, height, bgra = nw.capture_client(hwnd)
    nw.write_png(out / f"{name}.png", width, height, bgra)


def show_still_runs(check: Check, app: nw.App, hwnd: int, label: str) -> None:
    """Live output on, the first song previewed, Go Live, then Black."""
    found = a11y.nodes_by_id(a11y.uia(hwnd))
    a11y.click_node(hwnd, found["live-output"])
    time.sleep(2.0)
    nw.activate(hwnd)
    found = a11y.nodes_by_id(a11y.uia(hwnd))
    a11y.click_node(hwnd, found["song-0"])
    a11y.click_node(hwnd, found["go-live"])
    time.sleep(1.0)
    nw.press(hwnd, "ctrl+b")
    time.sleep(1.0)
    found = a11y.nodes_by_id(a11y.uia(hwnd))
    check.expect(found["live-output"].get("toggle") == "On", f"{label}: Live output on")
    check.expect(
        found["mask-black"].get("toggle") == "On", f"{label}: Black acknowledged"
    )
    save(hwnd, check.out, f"{label}-operator-live")


def quit_cleanly(check: Check, app: nw.App, hwnd: int, label: str) -> None:
    nw.press(hwnd, "ctrl+q")
    check.expect(app.wait_exit(10) == 0, f"{label}: Ctrl+Q exits cleanly")
    (check.out / f"{label}.log").write_text(app.log(), encoding="utf-8")
    app.close()


def run_with(check: Check, binary: pathlib.Path, library: pathlib.Path) -> None:
    before = console_windows()
    app = launch(binary, library, {"SELA_FFMPEG_DIR": ""})
    try:
        hwnd = app.window()
        nw.activate(hwnd)
        consoles: set[int] = set()
        deadline = time.monotonic() + 6.0
        while time.monotonic() < deadline:
            consoles |= console_windows() - before
            time.sleep(0.05)
        check.summary["with_console_windows"] = len(consoles)
        check.expect(not consoles, "with: no console window flashed during the probe")
        line = media_line(app)
        check.summary["with_log"] = line
        check.expect(
            "accepted from the development copy" in line and "FFmpeg n8.1" in line,
            "with: the pinned development copy is accepted and logged",
        )
        check.expect(warning_node(hwnd) is None, "with: no media-unavailable status")
        save(hwnd, check.out, "with-operator")
        quit_cleanly(check, app, hwnd, "with")
    finally:
        app.close()


def run_unavailable(
    check: Check,
    binary: pathlib.Path,
    library: pathlib.Path,
    label: str,
    env: dict,
    expected: str,
    log_marker: str,
) -> None:
    app = launch(binary, library, env)
    try:
        hwnd = app.window()
        nw.activate(hwnd)
        line = media_line(app)
        check.summary[f"{label}_log"] = line
        check.expect(
            log_marker in line, f"{label}: log names the reason ({log_marker})"
        )
        node = None
        deadline = time.monotonic() + LOG_TIMEOUT
        while node is None and time.monotonic() < deadline:
            node = warning_node(hwnd)
        check.expect(node is not None, f"{label}: media-unavailable status present")
        if node is not None:
            check.summary[f"{label}_warning"] = node["name"]
            check.expect(node["name"] == expected, f"{label}: warning text")
        save(hwnd, check.out, f"{label}-operator-warning")
        show_still_runs(check, app, hwnd, label)
        quit_cleanly(check, app, hwnd, label)
    finally:
        app.close()


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
        "--out",
        type=pathlib.Path,
        default=ROOT / ".amp/in/artifacts/media-backend-windows",
    )
    parser.add_argument("--gpl-dir", type=pathlib.Path)
    parser.add_argument("--skip-cargo", action="store_true")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    check = Check(args.out)
    try:
        pin = (DEV_COPY / "SELA-PIN.txt").read_text(encoding="utf-8")
        check.expect(
            f"sha256={WIN64_SHA256}" in pin,
            "the win64 development copy is the pinned build",
        )
        data = pathlib.Path(tempfile.mkdtemp(prefix="sela-media-"))
        library = data / "library.sqlite"
        subprocess.run([str(args.seed), str(library)], check=True)
        run_with(check, args.binary, library)
        if args.gpl_dir:
            run_unavailable(
                check,
                args.binary,
                library,
                "gpl",
                {"SELA_FFMPEG_DIR": str(args.gpl_dir), "SELA_FFMPEG_NO_DEV_COPY": "1"},
                GPL_TEXT,
                "--enable-gpl",
            )
        else:
            check.summary["gpl"] = "skipped: no --gpl-dir"
        run_unavailable(
            check,
            args.binary,
            library,
            "missing",
            {"SELA_FFMPEG_DIR": "", "SELA_FFMPEG_NO_DEV_COPY": "1"},
            MISSING_TEXT,
            "unavailable (Missing)",
        )
        if not args.skip_cargo:
            env = dict(os.environ, SELA_REQUIRE_FFMPEG="1")
            for key in ("SELA_FFMPEG_DIR", "SELA_FFMPEG_NO_DEV_COPY"):
                env.pop(key, None)
            cargo = subprocess.run(
                ["cargo", "test", "--locked", "--test", "media_decode"],
                cwd=ROOT,
                env=env,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                check=False,
            )
            (args.out / "media_decode.txt").write_text(
                cargo.stdout + cargo.stderr, encoding="utf-8"
            )
            check.expect(cargo.returncode == 0, "cargo test --test media_decode passes")
            check.summary["media_decode_tail"] = cargo.stdout.strip().splitlines()[-3:]
    except nw.Failure as error:
        check.expect(False, f"aborted: {error}")
    check.write()
    if check.summary["failures"]:
        print(f"FAIL: {len(check.summary['failures'])} check(s) failed; see {args.out}")
        return 1
    print(
        "PASS: FFmpeg accepted without a console window; missing (and GPL) FFmpeg show the status while the show runs"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
