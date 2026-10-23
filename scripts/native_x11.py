#!/usr/bin/env python3
"""Shared Linux/X11 harness for native Sela end-to-end checks.

Standard library only; drives the real GPUI operator and the real audience
renderer through external X11 tools (xdotool, xwininfo, xprop, ImageMagick
`import`), a private D-Bus session (dbus-daemon, gdbus) and, when installed,
AT-SPI (control positions by AccessKit author id) and xdg-desktop-portal-gtk
(the file dialogs GPUI opens on Linux). See docs/e2e-linux.md.

Every helper is scoped to processes this harness launched: windows are found
by PID (never by a broad title search), input is only sent after verifying
that the target window owns the X input focus, and cleanup kills every process
that carries this run's private environment tag, then deletes the profile.

Command line (not needed by feature scripts, which import this module):

    python3 scripts/native_x11.py doctor            # which capabilities work here
    python3 scripts/native_x11.py serve --display :100 [--kind xvfb|dual]
    python3 scripts/native_x11.py openbox-config PATH [--kind xvfb|dual]
"""

from __future__ import annotations

import argparse
import concurrent.futures
import ctypes
import hashlib
import json
import os
import re
import shutil
import signal
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time
import traceback
import uuid
import zlib
from collections.abc import Callable, Iterable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Self, TypeVar

ROOT = Path(__file__).resolve().parents[1]
OPERATOR_TITLE = "Sela — Technical preview"
AUDIENCE_TITLE = "Sela audience output"
STAGE_TITLE = "Sela stage output"
EDITOR_TITLE_PREFIX = "Song Editor - "
# GPUI (zed a846890) renders through wgpu's Vulkan backend on Linux, so the
# operator needs a Vulkan driver: pin Mesa's software rasterizer (lavapipe).
# VK_DRIVER_FILES=/dev/null, the earlier bootstrap workaround, now makes the
# operator fail with "Failed to create surface for any enabled backend".
LAVAPIPE_ICD = Path("/usr/share/vulkan/icd.d/lvp_icd.json")
TAG = "SELA_E2E_RUN"
EXIT_PASS, EXIT_FAIL, EXIT_ENVIRONMENT = 0, 1, 2
# xvfb kind: the audience runs in its development `window` placement; the
# harness window manager keeps it beside the operator instead of over it.
# 624x351 (16:9) plus its 1 px Openbox border fits right of the 1280 px
# operator (also bordered) on a 1920 px screen without overlapping it.
XVFB_AUDIENCE_SIZE = (624, 351)
# xvfb kind: the stage (Foldback) output in its `window` placement
# (`SELA_STAGE_MONITOR=window`) goes below the audience, same size, unfocused.
XVFB_STAGE_SIZE = (624, 351)
XVFB_STAGE_Y = 420
REQUIRED_TOOLS = ("xdotool", "xwininfo", "xprop", "import", "dbus-daemon", "gdbus")

T = TypeVar("T")


class Failure(Exception):
    """The application did not behave as asserted (exit status 1)."""


class EnvironmentProblem(Exception):
    """The harness could not set up the run (exit status 2): a missing tool,
    binary or display. Never reported as a pass."""


def log(message: str) -> None:
    print(f"[native_x11] {message}", flush=True)


# --------------------------------------------------------------------------
# Processes


def _become_subreaper() -> None:
    """Orphaned descendants (the audience child after an operator crash, D-Bus
    activated services) are re-parented to this process, so they can be reaped
    instead of lingering as zombies of an unrelated init."""
    try:
        ctypes.CDLL(None, use_errno=True).prctl(
            36, 1, 0, 0, 0
        )  # PR_SET_CHILD_SUBREAPER
    except (OSError, AttributeError):
        pass


_become_subreaper()


def tool(
    *args: str, timeout: float = 10, check: bool = True, env: dict | None = None
) -> str:
    """Run an external tool with a hard timeout; returns stripped stdout."""
    try:
        done = subprocess.run(
            [str(a) for a in args],
            capture_output=True,
            text=True,
            timeout=timeout,
            env=env,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise Failure(
            f"{args[0]} timed out after {timeout}s: {' '.join(map(str, args))}"
        ) from error
    except FileNotFoundError as error:
        raise EnvironmentProblem(f"missing tool {args[0]!r}") from error
    if check and done.returncode != 0:
        raise Failure(
            f"{' '.join(map(str, args))} failed ({done.returncode}): {done.stderr.strip()[-400:]}"
        )
    return done.stdout.strip()


def missing_tools(names: Iterable[str] = REQUIRED_TOOLS) -> list[str]:
    return [name for name in names if shutil.which(name) is None]


def _environ(pid: int) -> bytes:
    try:
        return Path(f"/proc/{pid}/environ").read_bytes()
    except OSError:
        return b""


def tagged_pids(tag: str) -> list[int]:
    """Live processes whose environment carries `SELA_E2E_RUN=<tag>`; every
    process a harness object starts (and everything they spawn) inherits it."""
    needle = f"{TAG}={tag}".encode() + b"\0"
    found = []
    for entry in Path("/proc").iterdir():
        if (
            entry.name.isdigit()
            and int(entry.name) != os.getpid()
            and needle in _environ(int(entry.name))
            and alive(int(entry.name))
        ):
            found.append(int(entry.name))
    return found


def _stat(pid: int) -> list[str] | None:
    try:
        text = Path(f"/proc/{pid}/stat").read_text()
    except OSError:
        return None
    return text[text.rindex(")") + 2 :].split()


def alive(pid: int) -> bool:
    """True while the process exists and is not a zombie."""
    fields = _stat(pid)
    return fields is not None and fields[0] not in ("Z", "X")


def children(pid: int) -> list[int]:
    found = []
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit():
            fields = _stat(int(entry.name))
            if fields and int(fields[1]) == pid and fields[0] not in ("Z", "X"):
                found.append(int(entry.name))
    return found


def wait_dead(pid: int, timeout: float) -> bool:
    deadline = time.monotonic() + timeout
    while alive(pid):
        if time.monotonic() >= deadline:
            return False
        time.sleep(0.05)
    return True


# PIDs of processes this harness started through subprocess.Popen: their exit
# status belongs to Popen.wait, so reap() must never collect them.
_POPEN_PIDS: set[int] = set()


def _popen(command: list[str], **kwargs) -> subprocess.Popen:
    process = subprocess.Popen(command, **kwargs)
    _POPEN_PIDS.add(process.pid)
    return process


def reap() -> None:
    """Collect exit statuses of orphans re-parented to this subreaper (never
    a process whose status a Popen object still owns)."""
    me = os.getpid()
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit() or int(entry.name) in _POPEN_PIDS:
            continue
        fields = _stat(int(entry.name))
        if fields and fields[0] == "Z" and int(fields[1]) == me:
            with _ignore(ChildProcessError):
                os.waitpid(int(entry.name), os.WNOHANG)


def kill_tagged(tag: str, grace: float = 2.0) -> list[int]:
    """TERM, then KILL, every process carrying `tag`; returns the PIDs found."""
    victims = tagged_pids(tag)
    for pid in victims:
        with _ignore(ProcessLookupError, PermissionError):
            os.kill(pid, signal.SIGTERM)
    deadline = time.monotonic() + grace
    while any(alive(p) for p in victims) and time.monotonic() < deadline:
        time.sleep(0.05)
    for pid in tagged_pids(tag):
        with _ignore(ProcessLookupError, PermissionError):
            os.kill(pid, signal.SIGKILL)
    deadline = time.monotonic() + grace
    while tagged_pids(tag) and time.monotonic() < deadline:
        time.sleep(0.05)
    return victims


class _ignore:
    def __init__(self, *errors: type[BaseException]):
        self.errors = errors

    def __enter__(self) -> None:
        return None

    def __exit__(self, kind, value, trace) -> bool:
        return kind is not None and issubclass(kind, self.errors)


def wait_until(
    probe: Callable[[], T], timeout: float, what: str, interval: float = 0.05
) -> T:
    """Poll `probe` until it returns a truthy value (returned) or fail after
    `timeout` seconds. Every wait in a feature script must be bounded."""
    deadline = time.monotonic() + timeout
    while True:
        value = probe()
        if value:
            return value
        if time.monotonic() >= deadline:
            raise Failure(f"timed out after {timeout}s waiting for {what}")
        time.sleep(interval)


# --------------------------------------------------------------------------
# Display


@dataclass
class Monitor:
    name: str
    width: int
    height: int
    x: int
    y: int
    primary: bool


def _openbox_rc(kind: str, screen: tuple[int, int]) -> str:
    """The system Openbox rc.xml (keeps Alt+F4 -> Close) plus harness rules.

    xvfb kind only: the operator opens at the top-left and the development
    `window` audience at the top-right, unfocused, so the two never overlap
    on one 1920x1080 screen and captures of either stay unobstructed. The
    dual kind adds no rules: the audience's real placement and focus behavior
    are under test there.
    """
    base = Path("/etc/xdg/openbox/rc.xml")
    text = (
        base.read_text()
        if base.is_file()
        else "<openbox_config>\n<applications>\n</applications>\n</openbox_config>\n"
    )
    rules = ""
    if kind == "xvfb":
        width, height = XVFB_AUDIENCE_SIZE
        rules = f"""
  <application title="{OPERATOR_TITLE}" type="normal">
    <position force="yes"><x>0</x><y>0</y></position>
  </application>
  <application title="{AUDIENCE_TITLE}">
    <position force="yes"><x>{screen[0] - width - 2}</x><y>0</y></position>
    <size><width>{width}</width><height>{height}</height></size>
    <focus>no</focus>
  </application>
  <application title="{STAGE_TITLE}">
    <position force="yes"><x>{screen[0] - XVFB_STAGE_SIZE[0] - 2}</x><y>{XVFB_STAGE_Y}</y></position>
    <size><width>{XVFB_STAGE_SIZE[0]}</width><height>{XVFB_STAGE_SIZE[1]}</height></size>
    <focus>no</focus>
  </application>
"""
    return text.replace("<applications>", "<applications>" + rules, 1)


_DUMMY_CONF = """Section "ServerFlags"
    Option "AutoAddDevices" "false"
    Option "AutoEnableDevices" "false"
    Option "DontVTSwitch" "true"
EndSection
Section "Device"
    Identifier "dummy"
    Driver "dummy"
    VideoRam 256000
EndSection
Section "Monitor"
    Identifier "monitor"
    HorizSync 5.0 - 1000.0
    VertRefresh 5.0 - 200.0
    Modeline "1920x1080" 148.50 1920 2008 2052 2200 1080 1084 1089 1125 +hsync +vsync
EndSection
Section "Screen"
    Identifier "screen"
    Device "dummy"
    Monitor "monitor"
    DefaultDepth 24
    SubSection "Display"
        Depth 24
        Virtual 3840 1080
    EndSubSection
EndSection
"""


class Display:
    """An X display: private (started and owned by this object) or attached.

    Kinds: `xvfb` — one 1920x1080 Xvfb screen; Xvfb has a single RandR CRTC,
    so winit sees one monitor and the audience uses its `window` placement.
    `dual` — Xorg with the dummy driver and two 1920x1080 CRTCs (DUMMY0
    primary, DUMMY1 to its right): the audience opens on the real
    `secondary` placement, borderless and covering DUMMY1.
    `triple` — as `dual` plus DUMMY2 to the right of DUMMY1, so the stage
    (Foldback) output can take its production placement on a third monitor.
    """

    def __init__(
        self, name: str, kind: str, owned: bool, tag: str, scratch: Path | None
    ):
        self.name, self.kind, self.owned, self.tag, self.scratch = (
            name,
            kind,
            owned,
            tag,
            scratch,
        )
        self.monitors: list[Monitor] = []
        self._previous = os.environ.get("DISPLAY")
        os.environ["DISPLAY"] = name

    @property
    def audience_monitor(self) -> str:
        """`SELA_AUDIENCE_MONITOR` for apps on this display."""
        return "secondary" if len(self.monitors) >= 2 else "window"

    @property
    def size(self) -> tuple[int, int]:
        out = tool("xdpyinfo", check=False) if shutil.which("xdpyinfo") else ""
        match = re.search(r"dimensions:\s+(\d+)x(\d+)", out)
        if match:
            return int(match[1]), int(match[2])
        right = max((m.x + m.width for m in self.monitors), default=1920)
        bottom = max((m.y + m.height for m in self.monitors), default=1080)
        return right, bottom

    def describe(self) -> dict:
        return {
            "display": self.name,
            "kind": self.kind,
            "owned": self.owned,
            "audience_monitor": self.audience_monitor,
            "monitors": [m.__dict__ for m in self.monitors],
        }

    # -- construction

    @classmethod
    def attach(cls, name: str, binary: Path | None = None) -> Display:
        """Use an existing display; nothing about it is changed or stopped."""
        display = cls(name, "attached", False, "", None)
        try:
            tool("xprop", "-root", "_NET_SUPPORTING_WM_CHECK", timeout=5)
        except Failure as error:
            display.restore()
            raise EnvironmentProblem(f"cannot use display {name}: {error}") from error
        display.monitors = list_monitors(binary)
        display.kind = "dual" if len(display.monitors) >= 2 else "xvfb"
        if "window id" not in tool(
            "xprop", "-root", "_NET_SUPPORTING_WM_CHECK", check=False
        ):
            log(f"warning: no EWMH window manager on {name}; focus checks will fail")
        return display

    @classmethod
    def start(
        cls,
        kind: str = "xvfb",
        width: int = 1920,
        height: int = 1080,
        binary: Path | None = None,
    ) -> Display:
        """Start a private X server (number chosen by the server) and Openbox."""
        needed = ["openbox"] + (["Xvfb"] if kind == "xvfb" else ["Xorg", "xrandr"])
        absent = missing_tools(needed)
        if (
            kind in ("dual", "triple")
            and not Path("/usr/lib/xorg/modules/drivers/dummy_drv.so").exists()
        ):
            absent.append("xserver-xorg-video-dummy")
        if absent:
            raise EnvironmentProblem(f"display kind {kind!r} needs {', '.join(absent)}")
        tag = uuid.uuid4().hex
        scratch = Path(tempfile.mkdtemp(prefix="sela-display-"))
        env = dict(os.environ, **{TAG: tag})
        read, write = os.pipe()
        if kind == "xvfb":
            command = [
                "Xvfb",
                "-displayfd",
                str(write),
                "-screen",
                "0",
                f"{width}x{height}x24",
                "-nolisten",
                "tcp",
                "-noreset",
            ]
        elif kind in ("dual", "triple"):
            conf = scratch / "dummy.conf"
            conf.write_text(
                _DUMMY_CONF.replace("Virtual 3840 1080", "Virtual 5760 1080")
                if kind == "triple"
                else _DUMMY_CONF
            )
            command = [
                "Xorg",
                "-displayfd",
                str(write),
                "-config",
                str(conf),
                "-noreset",
                "-nolisten",
                "tcp",
                "-logfile",
                str(scratch / "xorg.log"),
            ]
        else:
            raise EnvironmentProblem(f"unknown display kind {kind!r}")
        with (scratch / "server.log").open("wb") as server_log:
            server = _popen(
                command,
                pass_fds=(write,),
                stdout=server_log,
                stderr=subprocess.STDOUT,
                env=env,
                start_new_session=True,
            )
        os.close(write)
        number = b""
        deadline = time.monotonic() + 15
        os.set_blocking(read, False)
        while not number.endswith(b"\n"):
            if server.poll() is not None or time.monotonic() > deadline:
                os.close(read)
                kill_tagged(tag)
                text = (scratch / "server.log").read_text(errors="replace")[-1500:]
                shutil.rmtree(scratch, ignore_errors=True)
                raise EnvironmentProblem(f"{command[0]} did not start: {text}")
            with _ignore(BlockingIOError):
                number += os.read(read, 16)
            time.sleep(0.05)
        os.close(read)
        display = cls(f":{number.decode().strip()}", kind, True, tag, scratch)
        try:
            if kind == "triple":
                tool("xrandr", "--addmode", "DUMMY2", "1920x1080")
                tool(
                    "xrandr",
                    "--output",
                    "DUMMY2",
                    "--mode",
                    "1920x1080",
                    "--pos",
                    "3840x0",
                )
            if kind in ("dual", "triple"):
                tool("xrandr", "--addmode", "DUMMY1", "1920x1080")
                tool(
                    "xrandr",
                    "--output",
                    "DUMMY0",
                    "--mode",
                    "1920x1080",
                    "--pos",
                    "0x0",
                    "--primary",
                    "--output",
                    "DUMMY1",
                    "--mode",
                    "1920x1080",
                    "--pos",
                    "1920x0",
                )
            screen = display.size
            rc = scratch / "openbox-rc.xml"
            rc.write_text(_openbox_rc(kind, screen))
            with (scratch / "openbox.log").open("wb") as wm_log:
                _popen(
                    ["openbox", "--config-file", str(rc)],
                    stdout=wm_log,
                    stderr=subprocess.STDOUT,
                    env=dict(env, DISPLAY=display.name),
                    start_new_session=True,
                )
            wait_until(
                lambda: (
                    "window id"
                    in tool("xprop", "-root", "_NET_SUPPORTING_WM_CHECK", check=False)
                ),
                10,
                "the Openbox window manager",
            )
            display.monitors = list_monitors(binary)
        except BaseException:
            display.close()
            raise
        log(
            f"started {kind} display {display.name} with {len(display.monitors)} monitor(s)"
        )
        return display

    def restore(self) -> None:
        if self._previous is None:
            os.environ.pop("DISPLAY", None)
        else:
            os.environ["DISPLAY"] = self._previous

    def close(self) -> None:
        if self.owned:
            kill_tagged(self.tag)
            reap()
            if self.scratch:
                shutil.rmtree(self.scratch, ignore_errors=True)
        self.restore()

    def __enter__(self) -> Self:
        return self

    def __exit__(self, *exc) -> None:
        self.close()


def list_monitors(binary: Path | None = None) -> list[Monitor]:
    """Monitors as the audience process (winit, RandR CRTCs) sees them, from
    `sela --list-monitors`; falls back to xrandr's active CRTCs."""
    lines: list[str] = []
    if binary and binary.is_file():
        lines = tool(
            str(binary), "--list-monitors", check=False, timeout=20
        ).splitlines()
    monitors = []
    for line in lines:
        parts = line.split("\t")
        if len(parts) == 6:
            name, width, height, x, y, primary = parts
            monitors.append(
                Monitor(name, int(width), int(height), int(x), int(y), primary == "1")
            )
    if monitors or not shutil.which("xrandr"):
        return monitors
    for match in re.finditer(
        r"^(\S+) connected (primary )?(\d+)x(\d+)\+(\d+)\+(\d+)",
        tool("xrandr", "--query", check=False),
        re.MULTILINE,
    ):
        monitors.append(
            Monitor(
                match[1],
                int(match[3]),
                int(match[4]),
                int(match[5]),
                int(match[6]),
                bool(match[2]),
            )
        )
    return monitors


# --------------------------------------------------------------------------
# Windows, focus and input (XTEST through xdotool)

KEYSYMS = {
    "enter": "Return",
    "return": "Return",
    "tab": "Tab",
    "pagedown": "Next",
    "pageup": "Prior",
    "delete": "Delete",
    "del": "Delete",
    "backspace": "BackSpace",
    "escape": "Escape",
    "esc": "Escape",
    "space": "space",
    "home": "Home",
    "end": "End",
    "up": "Up",
    "down": "Down",
    "left": "Left",
    "right": "Right",
    "insert": "Insert",
    ".": "period",
    ",": "comma",
    "/": "slash",
    "-": "minus",
    "=": "equal",
    "+": "plus",
    "#": "numbersign",
}
MODIFIERS = {
    "ctrl": "ctrl",
    "control": "ctrl",
    "alt": "alt",
    "shift": "shift",
    "super": "super",
}


def keysym_chord(keys: str) -> str:
    """'ctrl+shift+tab' / 'pagedown' (native_win names) -> an xdotool chord."""
    parts = keys.split("+") if keys != "+" else ["+"]
    out = []
    for part in parts:
        low = part.lower()
        if low in MODIFIERS:
            out.append(MODIFIERS[low])
        elif low in KEYSYMS:
            out.append(KEYSYMS[low])
        elif re.fullmatch(r"f\d{1,2}", low):
            out.append(low.upper())
        else:
            out.append(part)
    return "+".join(out)


def visible_windows(pid: int) -> list[int]:
    """Visible top-level client windows whose _NET_WM_PID is `pid`."""
    found = tool(
        "xdotool", "search", "--onlyvisible", "--pid", str(pid), check=False, timeout=5
    )
    return [int(line) for line in found.splitlines() if line.strip().isdigit()]


def window_name(wid: int) -> str:
    return tool("xdotool", "getwindowname", str(wid), check=False, timeout=5)


def window_pid(wid: int) -> int | None:
    text = tool("xdotool", "getwindowpid", str(wid), check=False, timeout=5)
    return int(text) if text.isdigit() else None


def window_exists(wid: int) -> bool:
    return (
        subprocess.run(
            ["xwininfo", "-id", str(wid)], capture_output=True, timeout=5, check=False
        ).returncode
        == 0
    )


def focused() -> int:
    text = tool("xdotool", "getwindowfocus", check=False, timeout=5)
    return int(text) if text.isdigit() else 0


def describe_window(wid: int) -> str:
    if not wid:
        return "no focused window"
    return f"window={wid} pid={window_pid(wid)} title={window_name(wid)!r}"


@dataclass
class Geometry:
    x: int
    y: int
    width: int
    height: int
    viewable: bool = True


def geometry(wid: int) -> Geometry:
    """Absolute client-area geometry (xdotool's reparented values are offset
    by the frame; xwininfo's absolute upper-left is the client origin)."""
    info = tool("xwininfo", "-id", str(wid), timeout=5)
    values = {}
    for key, label in (
        ("x", "Absolute upper-left X:"),
        ("y", "Absolute upper-left Y:"),
        ("width", "Width:"),
        ("height", "Height:"),
    ):
        match = re.search(re.escape(label) + r"\s*(-?\d+)", info)
        if not match:
            raise Failure(f"xwininfo has no {label!r} for {wid}")
        values[key] = int(match[1])
    return Geometry(**values, viewable="IsViewable" in info)


def viewable(wid: int) -> bool:
    """`wid` exists and is mapped, from one xwininfo query: a window can
    vanish between two queries (BadDrawable), which counts as not viewable."""
    try:
        return geometry(wid).viewable
    except Failure:
        return False


THIEVES: list[str] = []


def activate(wid: int, timeout: float = 3.0) -> None:
    """Give `wid` the input focus and verify it (bounded)."""
    deadline = time.monotonic() + timeout
    while True:
        tool("xdotool", "windowactivate", "--sync", str(wid), check=False, timeout=5)
        if focused() == wid:
            return
        tool("xdotool", "windowfocus", "--sync", str(wid), check=False, timeout=5)
        if focused() == wid:
            return
        if time.monotonic() >= deadline:
            raise Failure(
                f"window {wid} never received the input focus; focus is {describe_window(focused())}"
            )
        time.sleep(0.1)


def ensure_focus(wid: int, what: str, strict: bool = False) -> None:
    """Verify `wid` has the focus before an input step. A theft is recorded in
    THIEVES and printed; `strict` fails the run instead of re-activating."""
    current = focused()
    if current == wid:
        return
    thief = describe_window(current)
    THIEVES.append(f"before {what}: {thief}")
    log(f"focus thief before {what}: {thief}")
    if strict:
        raise Failure(f"window {wid} lost the focus before {what}; focus is {thief}")
    activate(wid)


def press(wid: int, *chords: str, strict: bool = False, pause: float = 0.12) -> None:
    """Send key chords (native_win names such as 'ctrl+q', 'pagedown', or
    xdotool keysyms) only while `wid` is verified to own the focus."""
    for keys in chords:
        ensure_focus(wid, repr(keys), strict)
        tool("xdotool", "key", "--clearmodifiers", keysym_chord(keys), timeout=5)
        time.sleep(pause)


def type_text(
    wid: int, text: str, strict: bool = False, delay_ms: int = 1, pause: float = 0.2
) -> None:
    ensure_focus(wid, "typing", strict)
    tool(
        "xdotool",
        "type",
        "--clearmodifiers",
        "--delay",
        str(delay_ms),
        text,
        timeout=30,
    )
    time.sleep(pause)


def _screen(wid: int, x: int, y: int) -> tuple[int, int]:
    g = geometry(wid)
    return g.x + x, g.y + y


def mouse_move(wid: int, x: int, y: int) -> None:
    """Move the pointer to window-relative client coordinates."""
    sx, sy = _screen(wid, x, y)
    # `mousemove --sync` waits for a motion event and hangs when the pointer
    # is already there, so verify the location by polling instead.
    tool("xdotool", "mousemove", str(sx), str(sy), timeout=5)
    wait_until(
        lambda: pointer() == (sx, sy), 2.0, f"the pointer at {sx},{sy}", interval=0.02
    )


def pointer() -> tuple[int, int]:
    """Absolute pointer position."""
    text = tool("xdotool", "getmouselocation", "--shell", timeout=5)
    values = dict(line.split("=", 1) for line in text.splitlines() if "=" in line)
    return int(values.get("X", -1)), int(values.get("Y", -1))


def click(
    wid: int,
    x: int,
    y: int,
    button: int = 1,
    repeat: int = 1,
    strict: bool = False,
    pause: float = 0.15,
) -> None:
    """Click at window-relative client coordinates of the focused window."""
    ensure_focus(wid, "a click", strict)
    mouse_move(wid, x, y)
    args = (
        ["xdotool", "click"]
        + (["--repeat", str(repeat), "--delay", "60"] if repeat > 1 else [])
        + [str(button)]
    )
    tool(*args, timeout=5)
    time.sleep(pause)


def double_click(wid: int, x: int, y: int, strict: bool = False) -> None:
    click(wid, x, y, repeat=2, strict=strict)


def right_click(wid: int, x: int, y: int, strict: bool = False) -> None:
    click(wid, x, y, button=3, strict=strict)


def click_with(wid: int, modifiers: str, x: int, y: int, strict: bool = False) -> None:
    """Click with e.g. 'ctrl' or 'shift' held."""
    ensure_focus(wid, "a modified click", strict)
    mods = keysym_chord(modifiers).split("+")
    mouse_move(wid, x, y)
    tool("xdotool", "keydown", *mods, timeout=5)
    try:
        tool("xdotool", "click", "1", timeout=5)
    finally:
        tool("xdotool", "keyup", *reversed(mods), timeout=5)
    time.sleep(0.15)


def mouse_down(wid: int, x: int, y: int, button: int = 1) -> None:
    mouse_move(wid, x, y)
    tool("xdotool", "mousedown", str(button), timeout=5)


def mouse_up(button: int = 1) -> None:
    tool("xdotool", "mouseup", str(button), timeout=5)


def drag(
    wid: int,
    start: tuple[int, int],
    end: tuple[int, int],
    steps: int = 12,
    pause: float = 0.02,
) -> None:
    """Press at `start`, move through intermediate points, release at `end`."""
    ensure_focus(wid, "a drag")
    mouse_down(wid, *start)
    try:
        for i in range(1, steps + 1):
            mouse_move(
                wid,
                start[0] + (end[0] - start[0]) * i // steps,
                start[1] + (end[1] - start[1]) * i // steps,
            )
            time.sleep(pause)
    finally:
        mouse_up()
    time.sleep(0.2)


def scroll(wid: int, x: int, y: int, clicks: int) -> None:
    """Wheel at (x, y): positive scrolls down, negative up."""
    mouse_move(wid, x, y)
    button = "5" if clicks > 0 else "4"
    tool(
        "xdotool",
        "click",
        "--repeat",
        str(abs(clicks)),
        "--delay",
        "60",
        button,
        timeout=15,
    )
    time.sleep(0.2)


def wm_close(wid: int, strict: bool = False) -> None:
    """Ask the window manager to close the window (Openbox Alt+F4 sends
    WM_DELETE_WINDOW, the X11 counterpart of WM_CLOSE)."""
    press(wid, "alt+F4", strict=strict)


def resize(wid: int, width: int, height: int, timeout: float = 5.0) -> tuple[int, int]:
    """Request a client size; returns the settled size (which the app's
    minimum may have clamped)."""
    tool(
        "xdotool",
        "windowsize",
        "--sync",
        str(wid),
        str(width),
        str(height),
        check=False,
        timeout=5,
    )
    last, stable = None, 0
    deadline = time.monotonic() + timeout
    while stable < 3 and time.monotonic() < deadline:
        time.sleep(0.1)
        g = geometry(wid)
        size = (g.width, g.height)
        stable = stable + 1 if size == last else 0
        last = size
    return last


def move(wid: int, x: int, y: int) -> None:
    tool(
        "xdotool",
        "windowmove",
        "--sync",
        str(wid),
        str(x),
        str(y),
        check=False,
        timeout=5,
    )
    time.sleep(0.2)


# --------------------------------------------------------------------------
# Captures


class Image:
    """8-bit RGB pixels, row-major, top-down."""

    def __init__(self, width: int, height: int, rgb: bytes):
        self.width, self.height, self.rgb = width, height, rgb

    @classmethod
    def from_ppm(cls, data: bytes) -> Image:
        tokens, offset = [], 0
        while len(tokens) < 4:
            match = re.compile(rb"\s*(#[^\n]*\n\s*)*(\S+)").match(data, offset)
            if not match:
                raise Failure("malformed PPM capture")
            tokens.append(match[2])
            offset = match.end()
        if tokens[0] != b"P6" or int(tokens[3]) != 255:
            raise Failure(f"unexpected capture format {tokens}")
        width, height = int(tokens[1]), int(tokens[2])
        pixels = data[offset + 1 : offset + 1 + width * height * 3]
        if len(pixels) != width * height * 3:
            raise Failure("truncated PPM capture")
        return cls(width, height, pixels)

    def crop(self, x: int, y: int, width: int, height: int) -> Image:
        x0, y0 = max(0, x), max(0, y)
        x1, y1 = min(self.width, x + width), min(self.height, y + height)
        if x1 <= x0 or y1 <= y0:
            raise Failure(f"crop {width}x{height}+{x}+{y} is off screen")
        stride = self.width * 3
        rows = [
            self.rgb[r * stride + x0 * 3 : r * stride + x1 * 3] for r in range(y0, y1)
        ]
        return Image(x1 - x0, y1 - y0, b"".join(rows))

    def pixel(self, x: int, y: int) -> tuple[int, int, int]:
        i = (y * self.width + x) * 3
        return self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]

    def center(self) -> tuple[int, int, int]:
        return self.pixel(self.width // 2, self.height // 2)

    def digest(self) -> str:
        return hashlib.sha256(self.rgb).hexdigest()

    def save(self, path: Path) -> Path:
        write_png(path, self.width, self.height, self.rgb)
        return path

    def _region(self, region):
        x0, y0, x1, y1 = region or (0, 0, self.width, self.height)
        return max(0, x0), max(0, y0), min(self.width, x1), min(self.height, y1)

    def samples(self, step: int = 2, region: tuple[int, int, int, int] | None = None):
        """(x, y, (r, g, b)) every `step` pixels inside region (x0, y0, x1, y1)."""
        x0, y0, x1, y1 = self._region(region)
        rgb, width = self.rgb, self.width
        for y in range(y0, y1, step):
            row = y * width * 3
            for x in range(x0, x1, step):
                i = row + x * 3
                yield x, y, (rgb[i], rgb[i + 1], rgb[i + 2])

    def count(
        self,
        match: tuple[int, int, int] | Callable,
        tolerance: int = 30,
        step: int = 2,
        region=None,
    ) -> int:
        test = _matcher(match, tolerance)
        return sum(1 for _, _, p in self.samples(step, region) if test(p))

    def box(
        self,
        match: tuple[int, int, int] | Callable,
        tolerance: int = 30,
        step: int = 2,
        region=None,
    ) -> tuple[int, int, int, int] | None:
        """Bounding box (left, top, right, bottom) of matching samples."""
        test = _matcher(match, tolerance)
        left = top = 1 << 30
        right = bottom = -1
        for x, y, p in self.samples(step, region):
            if test(p):
                left, top, right, bottom = (
                    min(left, x),
                    min(top, y),
                    max(right, x),
                    max(bottom, y),
                )
        return None if right < 0 else (left, top, right, bottom)

    def ink_box(self, threshold: int = 128, step: int = 2, region=None):
        """Box of near-white samples (slide text on a dark background)."""
        return self.box(
            lambda p: p[0] > threshold and p[1] > threshold and p[2] > threshold,
            step=step,
            region=region,
        )

    def brightest(self, step: int = 7) -> int:
        return max(max(p) for _, _, p in self.samples(step))

    def colors(self, step: int = 7, limit: int = 4096) -> int:
        seen = set()
        for _, _, p in self.samples(step):
            seen.add(p)
            if len(seen) >= limit:
                break
        return len(seen)

    def difference(self, other: Image, tolerance: int = 0, step: int = 2) -> float:
        """Fraction of sampled pixels that differ by more than `tolerance`."""
        if (self.width, self.height) != (other.width, other.height):
            return 1.0
        total = differ = 0
        for x, y, p in self.samples(step):
            total += 1
            q = other.pixel(x, y)
            if any(abs(a - b) > tolerance for a, b in zip(p, q)):
                differ += 1
        return differ / max(1, total)


def _matcher(match, tolerance):
    if callable(match):
        return match
    r, g, b = match
    return lambda p: (
        abs(p[0] - r) <= tolerance
        and abs(p[1] - g) <= tolerance
        and abs(p[2] - b) <= tolerance
    )


def near(a: Iterable[int], b: Iterable[int], tolerance: int = 12) -> bool:
    return all(abs(x - y) <= tolerance for x, y in zip(a, b))


def capture_root() -> Image:
    """The whole screen. Direct-window `import` hangs or captures black on
    GL/Vulkan windows under Xvfb, so captures are root grabs cropped to a
    window; never keep or share a root capture."""
    try:
        data = subprocess.run(
            ["import", "-window", "root", "-depth", "8", "ppm:-"],
            capture_output=True,
            timeout=15,
            check=True,
        ).stdout
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        raise Failure(f"root capture failed: {error}") from error
    return Image.from_ppm(data)


def capture(wid: int) -> Image:
    """The window's client area as it is on screen (whatever is on top)."""
    g = geometry(wid)
    return capture_root().crop(g.x, g.y, g.width, g.height)


def digest(wid: int) -> str:
    return capture(wid).digest()


def capture_stable(wid: int, timeout: float = 3.0, interval: float = 0.1) -> Image:
    """Capture once two consecutive captures agree (a settled frame)."""
    previous = capture(wid)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        time.sleep(interval)
        current = capture(wid)
        if current.rgb == previous.rgb:
            return current
        previous = current
    return previous


def wait_for_change(
    wid: int, before: str, timeout: float, what: str
) -> tuple[Image, int]:
    """Wait until the window's capture digest differs from `before`; returns
    the settled capture and the milliseconds until the first change."""
    sent = time.monotonic()
    while True:
        shot = capture(wid)
        if shot.digest() != before:
            break
        if time.monotonic() - sent > timeout:
            raise Failure(
                f"{what}: the captured output did not change within {timeout}s"
            )
        time.sleep(0.02)
    elapsed = round((time.monotonic() - sent) * 1000)
    time.sleep(0.3)
    return capture_stable(wid), elapsed


def write_png(
    path: Path, width: int, height: int, rgb: bytes, alpha: bool = False
) -> None:
    """Minimal PNG writer (RGB, or RGBA when `alpha`)."""
    channels = 4 if alpha else 3
    stride = width * channels
    raw = b"".join(b"\0" + rgb[y * stride : (y + 1) * stride] for y in range(height))

    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data))
        )

    header = struct.pack(">IIBBBBB", width, height, 8, 6 if alpha else 2, 0, 0, 0)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(raw, 6))
        + chunk(b"IEND", b"")
    )


def solid_png(path: Path, width: int, height: int, rgb: tuple[int, int, int]) -> Path:
    """An original one-color fixture image."""
    write_png(path, width, height, bytes(rgb) * (width * height))
    return path


# --------------------------------------------------------------------------
# Library and schedule files (read-only; never through the application)


def sqlite_ro(path: Path) -> sqlite3.Connection:
    return sqlite3.connect(
        Path(path).resolve().as_uri() + "?mode=ro", uri=True, timeout=2
    )


def _fields(data: bytes) -> list[str]:
    fields = []
    while data:
        length = struct.unpack("<I", data[:4])[0]
        fields.append(data[4 : 4 + length].decode("utf-8"))
        data = data[4 + length :]
    return fields


def song_payloads(library: Path) -> list[list[str]]:
    """Head revision payload fields of every live song (title, metadata,
    then label/lyrics per section)."""
    if not Path(library).exists():
        return []
    with sqlite_ro(library) as db:
        rows = db.execute(
            "SELECT r.payload FROM songs s JOIN song_revisions r "
            "ON r.id=s.id AND r.revision=s.head WHERE s.deleted=0 ORDER BY s.id"
        ).fetchall()
    return [_fields(data) for (data,) in rows]


def section_ids(library: Path) -> list[bytes]:
    with sqlite_ro(library) as db:
        return [
            row[0]
            for row in db.execute(
                "SELECT i.section FROM songs s JOIN section_ids i ON i.song=s.id AND i.revision=s.head "
                "WHERE s.deleted=0 ORDER BY s.id, i.position"
            )
        ]


def section_formats(library: Path) -> list[tuple[int, bytes]]:
    with sqlite_ro(library) as db:
        return db.execute(
            "SELECT f.position, f.format FROM section_formats f JOIN songs s "
            "ON f.song=s.id AND f.revision=s.head ORDER BY f.position"
        ).fetchall()


def backgrounds(library: Path) -> tuple[list[tuple[int, bytes]], list[bytes]]:
    """(slide backgrounds by position, song master backgrounds) of the head revision."""
    with sqlite_ro(library) as db:
        slides = db.execute(
            "SELECT b.position, b.background FROM section_backgrounds b JOIN songs s "
            "ON b.song=s.id AND b.revision=s.head ORDER BY b.position"
        ).fetchall()
        master = db.execute(
            "SELECT b.background FROM song_backgrounds b JOIN songs s "
            "ON b.song=s.id AND b.revision=s.head"
        ).fetchall()
    return slides, [row[0] for row in master]


def catalog_song_ids(library: Path) -> list[bytes]:
    """Song ids in the operator catalog's order (title, ignoring case)."""
    with sqlite_ro(library) as db:
        return [
            row[0]
            for row in db.execute(
                "SELECT m.song FROM search_rows m JOIN song_search f ON f.rowid=m.row "
                "ORDER BY f.title COLLATE NOCASE"
            )
        ]


def read_schedule(path: Path) -> dict | None:
    """A saved .selaschedule document, or None while it does not exist."""
    path = Path(path)
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


# --------------------------------------------------------------------------
# Binaries


def default_binary() -> Path:
    if os.environ.get("SELA_BINARY"):
        return Path(os.environ["SELA_BINARY"]).resolve()
    return (
        Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
        / "debug"
        / "sela"
    )


def default_seed() -> Path:
    return default_binary().parent / "examples" / "seed_library"


def build(examples: Iterable[str] = ("seed_library",)) -> None:
    """cargo build the debug `sela` binary and examples (-j 3, --locked)."""
    command = ["cargo", "build", "--locked", "-j", "3", "--bin", "sela"]
    for example in examples:
        command += ["--example", example]
    log(" ".join(command))
    try:
        done = subprocess.run(command, cwd=ROOT, timeout=3600, check=False)
    except subprocess.TimeoutExpired as error:
        raise EnvironmentProblem(f"cargo build timed out after {error.timeout}s")
    if done.returncode != 0:
        raise EnvironmentProblem("cargo build failed")


def seed_library(path: Path, *flags: str, seed: Path | None = None) -> Path:
    """A new library of original fixture songs (examples/seed_library)."""
    seed = seed or default_seed()
    if not seed.is_file():
        raise EnvironmentProblem(
            f"missing {seed} (cargo build --example seed_library, or pass --build)"
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        done = subprocess.run(
            [str(seed), str(path), *flags],
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise EnvironmentProblem(f"seed_library timed out after {error.timeout}s")
    if done.returncode != 0:
        raise EnvironmentProblem(f"seed_library failed: {done.stderr.strip()}")
    return path


# --------------------------------------------------------------------------
# AT-SPI (AccessKit) control lookup through gdbus


def _gv_strings(text: str) -> list[str]:
    """String literals of a GVariant text dump, in order."""
    out, i = [], 0
    while i < len(text):
        quote = text[i]
        if quote in "'\"":
            j, chars = i + 1, []
            while j < len(text) and text[j] != quote:
                if text[j] == "\\" and j + 1 < len(text):
                    j += 1
                chars.append(text[j])
                j += 1
            out.append("".join(chars))
            i = j + 1
        else:
            i += 1
    return out


@dataclass
class Control:
    id: str
    name: str
    role: str
    x: int
    y: int
    width: int
    height: int

    @property
    def center(self) -> tuple[int, int]:
        return self.x + self.width // 2, self.y + self.height // 2


# AT-SPI role numbers 0-99 (at-spi2-core AtspiRole, also atspi-common's
# Role), named the way atspi_role_get_name spells them.
_ATSPI_ROLES = [
    "invalid",
    "accelerator label",
    "alert",
    "animation",
    "arrow",
    "calendar",
    "canvas",
    "check box",
    "check menu item",
    "color chooser",
    "column header",
    "combo box",
    "date editor",
    "desktop icon",
    "desktop frame",
    "dial",
    "dialog",
    "directory pane",
    "drawing area",
    "file chooser",
    "filler",
    "focus traversable",
    "font chooser",
    "frame",
    "glass pane",
    "html container",
    "icon",
    "image",
    "internal frame",
    "label",
    "layered pane",
    "list",
    "list item",
    "menu",
    "menu bar",
    "menu item",
    "option pane",
    "page tab",
    "page tab list",
    "panel",
    "password text",
    "popup menu",
    "progress bar",
    "push button",
    "radio button",
    "radio menu item",
    "root pane",
    "row header",
    "scroll bar",
    "scroll pane",
    "separator",
    "slider",
    "spin button",
    "split pane",
    "status bar",
    "table",
    "table cell",
    "table column header",
    "table row header",
    "tearoff menu item",
    "terminal",
    "text",
    "toggle button",
    "tool bar",
    "tool tip",
    "tree",
    "tree table",
    "unknown",
    "viewport",
    "window",
    "extended",
    "header",
    "footer",
    "paragraph",
    "ruler",
    "application",
    "autocomplete",
    "editbar",
    "embedded",
    "entry",
    "chart",
    "caption",
    "document frame",
    "heading",
    "page",
    "section",
    "redundant object",
    "form",
    "link",
    "input method window",
    "table row",
    "tree item",
    "document spreadsheet",
    "document presentation",
    "document text",
    "document web",
    "document email",
    "comment",
    "list box",
    "grouping",
]


def _atspi_role_name(number: int) -> str:
    return _ATSPI_ROLES[number] if 0 <= number < len(_ATSPI_ROLES) else str(number)


@dataclass
class _Atspi:
    address: str
    calls: int = 0

    def call(self, dest: str, path: str, method: str, *args: str) -> str:
        self.calls += 1
        return tool(
            "gdbus",
            "call",
            "--address",
            self.address,
            "--dest",
            dest,
            "--object-path",
            path,
            "--method",
            method,
            *args,
            check=False,
            timeout=5,
        )

    def children(self, dest: str, path: str) -> list[tuple[str, str]]:
        strings = _gv_strings(
            self.call(dest, path, "org.a11y.atspi.Accessible.GetChildren")
        )
        return list(zip(strings[0::2], strings[1::2]))

    def node(self, dest: str, path: str) -> dict:
        props = self.call(
            dest,
            path,
            "org.freedesktop.DBus.Properties.GetAll",
            "org.a11y.atspi.Accessible",
        )
        values = {}
        for key in ("Name", "AccessibleId"):
            match = re.search(
                rf"'{key}': <('(?:[^'\\]|\\.)*'|\"(?:[^\"\\]|\\.)*\")>", props
            )
            values[key] = _gv_strings(match[1])[0] if match else ""
        role = _gv_strings(
            self.call(dest, path, "org.a11y.atspi.Accessible.GetRoleName")
        )
        if not role or not role[0]:
            # AccessKit's AT-SPI adapter (accesskit_unix 0.22) answers
            # GetRoleName with an empty string; GetRole has the number.
            number = re.search(
                r"(?:uint32 )?(\d+),?\)",
                self.call(dest, path, "org.a11y.atspi.Accessible.GetRole"),
            )
            role = [_atspi_role_name(int(number[1]))] if number else []
        extents = re.findall(
            r"-?\d+", self.call(dest, path, "org.a11y.atspi.Component.GetExtents", "1")
        )
        return {
            "name": values["Name"],
            "id": values["AccessibleId"],
            "role": role[0] if role else "",
            "extents": [int(v) for v in extents[:4]] if len(extents) >= 4 else None,
        }


# --------------------------------------------------------------------------
# Application


class App:
    """One launched Sela process with a private profile and D-Bus session.

    The profile lives in a new temporary directory: HOME, XDG_RUNTIME_DIR
    (mode 700), XDG_CONFIG_HOME, XDG_DATA_HOME (the default library is
    XDG_DATA_HOME/sela/library.sqlite) and XDG_CACHE_HOME. With
    `--operator-library PATH` or `--library PATH` the Sela profile (settings,
    Resources, logo.txt) is PATH's folder instead. A private dbus-daemon
    serves the xdg-desktop-portal (GTK file dialogs) and AT-SPI (control
    positions); nothing touches the user's session bus.
    """

    def __init__(
        self,
        binary: Path,
        args: Iterable[str] = (),
        *,
        display: Display,
        name: str = "app",
        scratch: Path | None = None,
        env: dict | None = None,
        a11y: bool = True,
        portal: bool = True,
        audience_monitor: str | None = None,
        audience_backend: str | None = None,
    ):
        if not Path(binary).is_file():
            raise EnvironmentProblem(
                f"missing binary {binary} (cargo build, or pass --build)"
            )
        self.binary, self.name, self.display = Path(binary), name, display
        self.tag = uuid.uuid4().hex
        self.owned = scratch is None
        self.scratch = (
            Path(tempfile.mkdtemp(prefix=f"sela-e2e-{name}-"))
            if scratch is None
            else Path(scratch)
        )
        dirs = {
            key: self.scratch / sub
            for key, sub in (
                ("HOME", "home"),
                ("XDG_RUNTIME_DIR", "runtime"),
                ("XDG_CONFIG_HOME", "config"),
                ("XDG_DATA_HOME", "data"),
                ("XDG_CACHE_HOME", "cache"),
            )
        }
        for path in dirs.values():
            path.mkdir(parents=True, exist_ok=True)
        dirs["XDG_RUNTIME_DIR"].chmod(0o700)
        portals = dirs["XDG_CONFIG_HOME"] / "xdg-desktop-portal"
        portals.mkdir(exist_ok=True)
        (portals / "portals.conf").write_text("[preferred]\ndefault=gtk\n")
        self.env = {
            k: v
            for k, v in os.environ.items()
            if not k.startswith(("DBUS_", "XDG_", "WAYLAND"))
        }
        self.env.update({k: str(v) for k, v in dirs.items()})
        self.env.update(
            {
                TAG: self.tag,
                "DISPLAY": display.name,
                "RUST_LOG": os.environ.get("RUST_LOG", "info"),
                "XDG_CURRENT_DESKTOP": "sela-e2e",
                "GTK_USE_PORTAL": "0",
                "NO_AT_BRIDGE": "1",
                "SELA_AUDIENCE_MONITOR": audience_monitor or display.audience_monitor,
            }
        )
        if LAVAPIPE_ICD.is_file() and "SELA_E2E_KEEP_VULKAN" not in os.environ:
            self.env["VK_DRIVER_FILES"] = str(LAVAPIPE_ICD)
        if audience_backend:
            self.env["SELA_AUDIENCE_BACKEND"] = audience_backend
        self.env.update(env or {})
        self.wid = 0
        self.process: subprocess.Popen | None = None
        self.bus: subprocess.Popen | None = None
        self._atspi: _Atspi | None = None
        self.log_path = self.scratch / f"{name}-{time.time_ns()}.log"
        self._log = self.log_path.open("wb")
        try:
            self._start_bus(a11y, portal)
            self.process = _popen(
                [str(self.binary), *map(str, args)],
                env=self.env,
                stdout=self._log,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
        except BaseException:
            self.close()
            raise
        self.pid = self.process.pid

    def _start_bus(self, a11y: bool, portal: bool) -> None:
        if shutil.which("dbus-daemon") is None:
            raise EnvironmentProblem("missing dbus-daemon (package dbus-daemon)")
        self.bus = _popen(
            ["dbus-daemon", "--session", "--nofork", "--print-address=1"],
            env=self.env,
            stdout=subprocess.PIPE,
            stderr=self._log,
            start_new_session=True,
        )
        os.set_blocking(self.bus.stdout.fileno(), False)
        address = b""
        deadline = time.monotonic() + 10
        while not address.endswith(b"\n"):
            if self.bus.poll() is not None or time.monotonic() > deadline:
                raise EnvironmentProblem("the private dbus-daemon did not start")
            with _ignore(BlockingIOError, TypeError):
                address += self.bus.stdout.read() or b""
            time.sleep(0.02)
        self.env["DBUS_SESSION_BUS_ADDRESS"] = address.decode().strip()
        if not portal:
            # Without a portal GPUI's file prompts fail with a visible error.
            self.env["XDG_DESKTOP_PORTAL_DIR"] = str(self.scratch / "no-portals")
        if a11y and Path("/usr/libexec/at-spi-bus-launcher").exists():
            tool(
                "gdbus",
                "call",
                "--session",
                "--dest",
                "org.a11y.Bus",
                "--object-path",
                "/org/a11y/bus",
                "--method",
                "org.freedesktop.DBus.Properties.Set",
                "org.a11y.Status",
                "IsEnabled",
                "<true>",
                env=self.env,
                check=False,
                timeout=10,
            )

    # -- process state

    def running(self) -> bool:
        return self.process is not None and self.process.poll() is None

    def log(self, tail: int = 4000) -> str:
        self._log.flush()
        return self.log_path.read_text(errors="replace")[-tail:]

    def wait_exit(self, timeout: float = 10.0) -> int:
        """Exit status; fails if the process or its windows outlive `timeout`."""
        try:
            code = self.process.wait(timeout)
        except subprocess.TimeoutExpired as error:
            raise Failure(
                f"{self.name}: process exit timed out after {timeout}s"
            ) from error
        if self.wid and viewable(self.wid):
            raise Failure(f"{self.name}: window {self.wid} survived process exit")
        return code

    def descendants(self) -> list[int]:
        found, queue = [], [self.pid]
        while queue:
            for child in children(queue.pop()):
                found.append(child)
                queue.append(child)
        return found

    # -- windows

    def windows(self) -> list[int]:
        return visible_windows(self.pid) if self.running() else []

    def all_windows(self) -> list[int]:
        """Visible windows of this process and its descendants (the audience)."""
        if not self.running():
            return []
        return [
            w for pid in (self.pid, *self.descendants()) for w in visible_windows(pid)
        ]

    def window(self, timeout: float = 30.0, title: str | None = None) -> int:
        """The first visible window of this PID (optionally with `title`);
        becomes `self.wid`, the default target of controls()."""

        def probe():
            if not self.running():
                raise Failure(
                    f"{self.name} exited early ({self.process.returncode}): {self.log()}"
                )
            for wid in self.windows():
                if title is None or window_name(wid) == title:
                    return wid
            return None

        self.wid = wait_until(
            probe,
            timeout,
            f"a visible {self.name} window" + (f" {title!r}" if title else ""),
            interval=0.1,
        )
        return self.wid

    def new_window(self, known: Iterable[int], timeout: float = 10.0) -> int:
        """A visible window of this PID that is not in `known`."""
        known = set(known)
        return wait_until(
            lambda: next((w for w in self.windows() if w not in known), None),
            timeout,
            f"a new {self.name} window",
            interval=0.1,
        )

    def audience(self, timeout: float) -> tuple[int, int] | None:
        """(pid, window) of the audience child process, or None after `timeout`."""
        deadline = time.monotonic() + timeout
        while True:
            for pid in children(self.pid):
                for wid in visible_windows(pid):
                    if window_name(wid) == AUDIENCE_TITLE:
                        return pid, wid
            if time.monotonic() >= deadline:
                return None
            time.sleep(0.05)

    def stage(self, timeout: float) -> tuple[int, int] | None:
        """(pid, window) of the stage (`sela --stage`) child, or None after
        `timeout`."""
        deadline = time.monotonic() + timeout
        while True:
            for pid in children(self.pid):
                for wid in visible_windows(pid):
                    if window_name(wid) == STAGE_TITLE:
                        return pid, wid
            if time.monotonic() >= deadline:
                return None
            time.sleep(0.05)

    # -- AT-SPI controls

    def _a11y(self) -> _Atspi:
        if self._atspi is None:
            if not Path("/usr/libexec/at-spi-bus-launcher").exists():
                raise EnvironmentProblem(
                    "control lookup needs at-spi2-core (AT-SPI bus launcher)"
                )
            reply = tool(
                "gdbus",
                "call",
                "--session",
                "--dest",
                "org.a11y.Bus",
                "--object-path",
                "/org/a11y/bus",
                "--method",
                "org.a11y.Bus.GetAddress",
                env=self.env,
                timeout=10,
            )
            self._atspi = _Atspi(_gv_strings(reply)[0])
        return self._atspi

    def _app_root(self, atspi: _Atspi) -> tuple[str, str] | None:
        """This app's AT-SPI bus and root, or None while it has not
        registered yet (a polled probe keeps waiting)."""
        for bus, path in atspi.children(
            "org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root"
        ):
            pid = re.findall(
                r"\d+",
                atspi.call(
                    "org.freedesktop.DBus",
                    "/org/freedesktop/DBus",
                    "org.freedesktop.DBus.GetConnectionUnixProcessID",
                    bus,
                ),
            )
            if pid and int(pid[-1]) == self.pid:
                return bus, path
        return None

    def controls(
        self, wid: int | None = None, timeout: float = 10.0
    ) -> dict[str, Control]:
        """Controls with an AccessKit author id in window `wid` (default
        self.wid), keyed by id, in window-relative client coordinates.
        Coordinates are only valid for the layout at the time of the call."""
        wid = wid or self.wid
        atspi = self._a11y()

        def collect():
            registered = self._app_root(atspi)
            if registered is None:
                return None
            bus, root = registered
            # Read the title on every attempt: it can change while we wait
            # (the UI language or the schedule name arrives after the first
            # frame), and a stale title never matches the AT-SPI frame.
            title = window_name(wid)
            frames = [p for _, p in atspi.children(bus, root)]
            with concurrent.futures.ThreadPoolExecutor(8) as pool:
                for frame in frames:
                    if pool.submit(atspi.node, bus, frame).result()["name"] != title:
                        continue
                    found: dict[str, Control] = {}
                    level = [frame]
                    while level:
                        kids = [
                            k
                            for ks in pool.map(lambda p: atspi.children(bus, p), level)
                            for _, k in ks
                        ]
                        for node in pool.map(lambda p: atspi.node(bus, p), kids):
                            if (
                                node["id"]
                                and node["extents"]
                                and node["extents"][2] > 0
                            ):
                                found[node["id"]] = Control(
                                    node["id"],
                                    node["name"],
                                    node["role"],
                                    *node["extents"],
                                )
                        level = kids
                    return found
            return None

        return wait_until(
            collect,
            timeout,
            f"the AT-SPI tree of {window_name(wid)!r}",
            interval=0.3,
        )

    def control(self, aid: str, wid: int | None = None) -> Control:
        found = self.controls(wid)
        if aid not in found:
            raise Failure(
                f"control {aid!r} missing from the AT-SPI tree ({sorted(found)[:40]})"
            )
        return found[aid]

    # -- file dialogs (xdg-desktop-portal-gtk)

    def file_dialog(self, timeout: float = 10.0) -> int:
        """The visible portal file chooser opened for this app (GTK, run by
        this app's private D-Bus session, so it carries the same tag)."""

        def probe():
            for pid in tagged_pids(self.tag):
                if pid != self.pid and pid not in self.descendants():
                    for wid in visible_windows(pid):
                        return wid
            return None

        return wait_until(
            probe,
            timeout,
            "the portal file dialog (xdg-desktop-portal-gtk)",
            interval=0.1,
        )

    def drive_file_dialog(
        self, path: str | Path, cancel: bool = False, owner: int | None = None
    ) -> None:
        """Enter `path` into the GTK Save/Open dialog and confirm (or Escape).

        The location entry is opened with Ctrl+L and the path typed in one
        burst: GTK's inline completion rewrites a path typed slowly."""
        dialog = self.file_dialog()
        time.sleep(0.6)
        activate(dialog)
        if cancel:
            press(dialog, "escape")
        else:
            press(dialog, "ctrl+l")
            time.sleep(0.3)
            press(dialog, "ctrl+a", "backspace")
            ensure_focus(dialog, "typing a path")
            tool(
                "xdotool",
                "type",
                "--clearmodifiers",
                "--delay",
                "0",
                str(path),
                timeout=20,
            )
            time.sleep(0.4)
            press(dialog, "enter")
        wait_until(
            lambda: not viewable(dialog),
            12,
            "the portal file dialog to close",
        )
        time.sleep(0.4)
        if owner:
            activate(owner)

    # -- cleanup

    def close(self) -> None:
        """Kill the process tree (operator, audience, D-Bus services) and,
        when owned, delete the profile."""
        kill_tagged(self.tag)
        for proc in (self.process, self.bus):
            if proc is not None and proc.poll() is None:
                proc.kill()
            if proc is not None:
                with _ignore(subprocess.TimeoutExpired):
                    proc.wait(5)
        reap()
        with _ignore(Exception):
            self._log.close()
        if self.owned:
            shutil.rmtree(self.scratch, ignore_errors=True)


# --------------------------------------------------------------------------
# Script scaffolding


@dataclass
class Session:
    """What a feature script's body receives."""

    args: argparse.Namespace
    display: Display
    out: Path
    summary: dict = field(default_factory=dict)
    apps: list[App] = field(default_factory=list)
    temps: list[Path] = field(default_factory=list)

    def app(self, args: Iterable[str] = (), **kwargs) -> App:
        app = App(self.args.binary, args, display=self.display, **kwargs)
        self.apps.append(app)
        return app

    def temp(self, prefix: str = "sela-e2e-data-") -> Path:
        path = Path(tempfile.mkdtemp(prefix=prefix))
        self.temps.append(path)
        return path

    def save(self, what: int | Image, name: str) -> Image:
        """Save a window capture (or an Image) as <out>/<name>.png."""
        image = what if isinstance(what, Image) else capture(what)
        image.save(self.out / f"{name}.png")
        return image

    def close_app(self, app: App) -> None:
        self._keep_log(app)
        app.close()

    def _keep_log(self, app: App) -> None:
        with _ignore(OSError):
            shutil.copyfile(app.log_path, self.out / f"{app.name}.log")


def arguments(
    description: str, out_name: str, seed: bool = True
) -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=description, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--binary",
        type=Path,
        default=default_binary(),
        help="sela binary (default: target/debug)",
    )
    if seed:
        parser.add_argument(
            "--seed",
            type=Path,
            default=default_seed(),
            help="examples/seed_library binary",
        )
    parser.add_argument(
        "--display",
        help="attach to this existing display (e.g. :100) instead of a private one",
    )
    parser.add_argument(
        "--kind",
        choices=("xvfb", "dual", "triple"),
        default="xvfb",
        help="private display kind (ignored with --display)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=ROOT / ".amp/in/artifacts" / out_name,
        help="artifact folder",
    )
    parser.add_argument(
        "--build", action="store_true", help="cargo build sela and seed_library first"
    )
    return parser


def main(
    body: Callable[[Session], str],
    description: str,
    out_name: str,
    seed: bool = True,
    argv: list[str] | None = None,
    options: Callable[[argparse.ArgumentParser], None] | None = None,
) -> int:
    """Run a feature script: parse arguments, set up the display, run `body`
    (which returns the PASS message), and always clean up.

    Exit status: 0 pass, 1 behavior failure, 2 environment/setup problem,
    130 interrupted. The last line printed is `PASS: ...` or `FAIL: ...`
    (or `ENVIRONMENT: ...`); <out>/summary.json records the details."""
    parser = arguments(description, out_name, seed)
    if options:
        options(parser)
    args = parser.parse_args(argv)
    args.binary = Path(args.binary).resolve()
    if seed:
        args.seed = Path(args.seed).resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    # Stale captures from an earlier run must not pass for this run's evidence.
    for stale in (
        *args.out.glob("*.png"),
        *args.out.glob("*.log"),
        args.out / "summary.json",
    ):
        stale.unlink(missing_ok=True)
    summary: dict = {
        "script": out_name,
        "binary": str(args.binary),
        "started": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
    }
    session: Session | None = None
    display: Display | None = None
    status = EXIT_FAIL

    def interrupted(signum, _frame):
        raise KeyboardInterrupt(f"signal {signum}")

    signal.signal(signal.SIGTERM, interrupted)
    try:
        absent = missing_tools()
        if absent:
            raise EnvironmentProblem(f"missing tools: {', '.join(absent)}")
        if args.build:
            build()
        for path in (args.binary, *([args.seed] if seed else [])):
            if not path.is_file():
                raise EnvironmentProblem(
                    f"missing {path} (cargo build && cargo build --example seed_library)"
                )
        summary["binary_sha256"] = hashlib.sha256(args.binary.read_bytes()).hexdigest()
        with _ignore(Exception):
            summary["revision"] = tool(
                "git", "-C", str(ROOT), "rev-parse", "HEAD", check=False
            )
        display = (
            Display.attach(args.display, args.binary)
            if args.display
            else Display.start(args.kind, binary=args.binary)
        )
        summary["display"] = display.describe()
        session = Session(args, display, args.out, summary)
        message = body(session)
        summary["status"] = "PASS"
        print(f"PASS: {message}")
        status = EXIT_PASS
    except Failure as error:
        summary["status"], summary["error"] = "FAIL", str(error)
        print(f"FAIL: {error}")
        status = EXIT_FAIL
    except AssertionError as error:
        summary["status"], summary["error"] = "FAIL", f"assertion: {error}"
        summary["traceback"] = traceback.format_exc()[-2000:]
        print(f"FAIL: assertion: {error}\n{summary['traceback']}")
        status = EXIT_FAIL
    except EnvironmentProblem as error:
        summary["status"], summary["error"] = "ENVIRONMENT", str(error)
        print(f"ENVIRONMENT: {error}")
        status = EXIT_ENVIRONMENT
    except KeyboardInterrupt:
        summary["status"] = "INTERRUPTED"
        print("FAIL: interrupted")
        status = 130
    finally:
        if session is not None:
            if status != EXIT_PASS:
                for app in session.apps:
                    for index, wid in enumerate(app.all_windows()):
                        with _ignore(Exception):
                            capture(wid).save(
                                args.out / f"failure-{app.name}-{index}.png"
                            )
            for app in session.apps:
                session._keep_log(app)
                app.close()
            for path in session.temps:
                shutil.rmtree(path, ignore_errors=True)
        if display is not None:
            display.close()
        summary["focus_thieves"] = THIEVES
        summary["finished"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
        (args.out / "summary.json").write_text(
            json.dumps(summary, indent=2, default=str)
        )
    return status


# --------------------------------------------------------------------------
# Command line


def _doctor(binary: Path) -> int:
    print(f"binary: {binary} ({'present' if binary.is_file() else 'MISSING'})")
    for name in REQUIRED_TOOLS + ("Xvfb", "openbox", "Xorg", "xrandr", "xdpyinfo"):
        print(f"{name:14} {shutil.which(name) or 'missing'}")
    checks = {
        "lavapipe ICD": LAVAPIPE_ICD.is_file(),
        "AT-SPI (controls)": Path("/usr/libexec/at-spi-bus-launcher").exists(),
        "portal GTK (file dialogs)": Path(
            "/usr/libexec/xdg-desktop-portal-gtk"
        ).exists(),
        "dummy driver (dual)": Path(
            "/usr/lib/xorg/modules/drivers/dummy_drv.so"
        ).exists(),
    }
    for name, ok in checks.items():
        print(f"{name:26} {'yes' if ok else 'no'}")
    return 0 if not missing_tools() and binary.is_file() else 2


def _cli(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="command", required=True)
    doctor = sub.add_parser(
        "doctor", help="report which harness capabilities are available"
    )
    doctor.add_argument("--binary", type=Path, default=default_binary())
    serve = sub.add_parser(
        "serve", help="start a display + Openbox and keep it until interrupted"
    )
    serve.add_argument("--kind", choices=("xvfb", "dual", "triple"), default="xvfb")
    serve.add_argument("--display", help="display number to use (xvfb only), e.g. :100")
    config = sub.add_parser("openbox-config", help="write the harness Openbox rc.xml")
    config.add_argument("path", type=Path)
    config.add_argument("--kind", choices=("xvfb", "dual", "triple"), default="xvfb")
    config.add_argument("--screen", default="1920x1080")
    args = parser.parse_args(argv)
    if args.command == "doctor":
        return _doctor(args.binary)
    if args.command == "openbox-config":
        width, height = (int(v) for v in args.screen.split("x"))
        args.path.write_text(_openbox_rc(args.kind, (width, height)))
        return 0
    if args.display:
        if args.kind != "xvfb":
            raise SystemExit("--display is only supported with --kind xvfb")
        tag = uuid.uuid4().hex
        env = dict(os.environ, **{TAG: tag})
        scratch = Path(tempfile.mkdtemp(prefix="sela-display-"))
        _popen(
            [
                "Xvfb",
                args.display,
                "-screen",
                "0",
                "1920x1080x24",
                "-nolisten",
                "tcp",
                "-noreset",
            ],
            env=env,
            start_new_session=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        display = Display(args.display, "xvfb", True, tag, scratch)
        wait_until(
            lambda: tool("xdpyinfo", check=False, timeout=5),
            15,
            f"Xvfb {args.display}",
            0.2,
        )
        rc = scratch / "openbox-rc.xml"
        rc.write_text(_openbox_rc("xvfb", display.size))
        _popen(
            ["openbox", "--config-file", str(rc)],
            env=dict(env, DISPLAY=args.display),
            start_new_session=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        wait_until(
            lambda: (
                "window id"
                in tool("xprop", "-root", "_NET_SUPPORTING_WM_CHECK", check=False)
            ),
            10,
            "Openbox",
        )
    else:
        display = Display.start(args.kind, binary=default_binary())
    print(
        f"DISPLAY={display.name} kind={display.kind} (Ctrl+C or SIGTERM stops it)",
        flush=True,
    )
    signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
    try:
        while True:
            time.sleep(3600)
    except KeyboardInterrupt:
        pass
    finally:
        display.close()
    return 0


if __name__ == "__main__":
    sys.exit(_cli(sys.argv[1:]))
