#!/usr/bin/env python3
"""Linux/X11 native check: FFmpeg located at startup, with and without it.

Feature media-core (M2-01/M2-02; VAL-MEDIA-001/002/003) on the shared
harness (scripts/native_x11.py, docs/e2e-linux.md).

Usage: python3 scripts/media-backend-x11.py [--display :100 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR]
       [--build]

Prerequisite: scripts/fetch-ffmpeg (the pinned LGPL 8.1 development copy in
.amp/in/ffmpeg/n8.1.3-lgpl); without it the first run fails, which is the
point: a passing run proves FFmpeg was present.

Three operator launches on the same seeded library with a private profile:

1. "with": SELA_FFMPEG_DIR empty, so the debug build's development copy is
   used. The log gets "media: FFmpeg n8.1... accepted from the development
   copy" and no `media-unavailable` control appears in the AT-SPI tree.
2. "gpl": SELA_FFMPEG_DIR=/usr/bin (Ubuntu's FFmpeg, a GPL build) and
   SELA_FFMPEG_NO_DEV_COPY=1. The log names the refusal (--enable-gpl), the
   status bar shows the `media-unavailable` status "Video and audio
   unavailable: the FFmpeg found is a GPL build" (amber pixels checked), and
   the operator still works: Live output on, a song previewed by click,
   Go Live shows its text on the audience, Ctrl+B turns the audience black.
3. "missing": SELA_FFMPEG_DIR empty and SELA_FFMPEG_NO_DEV_COPY=1. The log
   says unavailable (Missing) and the warning reads "... FFmpeg was not
   found".

The warning must appear within 20 s of the window (the probe bound) while
the window already responds; Ctrl+Q then exits cleanly each time.

X11 differences: none in the checked behavior. The audience is the xvfb
development window; Windows additionally has to show no console window for
the probe children (CREATE_NO_WINDOW), checked by media-backend-windows.py.
"""

from __future__ import annotations

import re
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

GPL_DIR = "/usr/bin"
WARNING_ID = "media-unavailable"
GPL_TEXT = "Video and audio unavailable: the FFmpeg found is a GPL build"
MISSING_TEXT = "Video and audio unavailable: FFmpeg was not found"
# `SENDING` (amber) of the warning text and dot.
AMBER_RGB = (0xD9, 0x9A, 0x2B)
LOG_TIMEOUT = 20.0


def media_line(app: nx.App) -> str:
    """The operator's single `sela: media:` log line, once it is written."""

    def probe():
        for line in app.log(200_000).splitlines():
            if line.startswith("sela: media: "):
                return line
        return None

    return nx.wait_until(probe, LOG_TIMEOUT, "the media log line")


def launch(
    session: nx.Session, library: Path, name: str, env: dict
) -> tuple[nx.App, int]:
    app = session.app(["--operator-library", library], name=name, env=env)
    operator = app.window()
    if nx.window_name(operator) != nx.OPERATOR_TITLE:
        raise nx.Failure(f"{name}: unexpected window {nx.window_name(operator)!r}")
    nx.activate(operator)
    return app, operator


def warning(app: nx.App, operator: int) -> nx.Control | None:
    return app.controls(operator).get(WARNING_ID)


def wait_warning(app: nx.App, operator: int, name: str) -> nx.Control:
    return nx.wait_until(
        lambda: warning(app, operator),
        LOG_TIMEOUT,
        f"{name}: the {WARNING_ID} status",
        interval=0.5,
    )


def amber_in(image: nx.Image, control: nx.Control) -> int:
    region = (
        control.x,
        control.y,
        control.x + control.width,
        control.y + control.height,
    )
    return image.count(AMBER_RGB, tolerance=40, region=region)


def quit_cleanly(session: nx.Session, app: nx.App, operator: int, name: str) -> None:
    nx.press(operator, "ctrl+q")
    code = app.wait_exit(10)
    if code != 0:
        raise nx.Failure(f"{name}: exit status {code}")
    session.close_app(app)


def with_ffmpeg(session: nx.Session, library: Path) -> str:
    summary = session.summary
    app, operator = launch(session, library, "with", {"SELA_FFMPEG_DIR": ""})
    line = media_line(app)
    summary["with_log"] = line
    match = re.search(r"FFmpeg (n8\.1\S*) accepted from the development copy", line)
    if not match:
        raise nx.Failure(f"with: the development copy was not accepted: {line}")
    time.sleep(1.0)
    if warning(app, operator) is not None:
        raise nx.Failure(
            "with: the media-unavailable warning shows although FFmpeg was found"
        )
    session.save(nx.capture_stable(operator), "with-operator")
    quit_cleanly(session, app, operator, "with")
    return match.group(1)


def gpl_refused(session: nx.Session, library: Path) -> None:
    summary = session.summary
    if (
        not (Path(GPL_DIR) / "ffmpeg").is_file()
        or not (Path(GPL_DIR) / "ffprobe").is_file()
    ):
        raise nx.EnvironmentProblem(
            f"no GPL FFmpeg pair in {GPL_DIR} (apt install ffmpeg)"
        )
    started = time.monotonic()
    app, operator = launch(
        session,
        library,
        "gpl",
        {"SELA_FFMPEG_DIR": GPL_DIR, "SELA_FFMPEG_NO_DEV_COPY": "1"},
    )
    line = media_line(app)
    summary["gpl_log"] = line
    if "unavailable (Gpl)" not in line or "--enable-gpl" not in line:
        raise nx.Failure(f"gpl: the log does not name the GPL refusal: {line}")
    control = wait_warning(app, operator, "gpl")
    summary["gpl_warning_seconds"] = round(time.monotonic() - started, 3)
    summary["gpl_warning_name"] = control.name
    if control.name != GPL_TEXT:
        raise nx.Failure(f"gpl: warning reads {control.name!r}, expected {GPL_TEXT!r}")
    time.sleep(0.3)
    image = session.save(nx.capture_stable(operator), "gpl-operator-warning")
    amber = amber_in(image, control)
    summary["gpl_warning_amber_samples"] = amber
    if amber < 20:
        raise nx.Failure(f"gpl: the warning is not drawn in amber ({amber} samples)")

    # The show still runs without FFmpeg: Live output, Go Live, Black.
    controls = app.controls(operator)
    nx.click(operator, *controls["live-output"].center)
    found = app.audience(15)
    if not found:
        raise nx.Failure(f"gpl: no audience window: {app.log()}")
    _, screen = found
    time.sleep(1.5)
    nx.activate(operator)
    blank = nx.digest(screen)
    nx.click(operator, *app.control("song-0", operator).center)
    time.sleep(0.5)
    # A focused song row pages its list with Page Down, so use Go Live.
    nx.click(operator, *controls["go-live"].center)
    image, elapsed = nx.wait_for_change(screen, blank, 5.0, "gpl go_live")
    session.save(image, "gpl-audience-go-live")
    summary["gpl_go_live_ms"] = elapsed
    if image.ink_box() is None:
        raise nx.Failure("gpl: Go Live showed no text on the audience output")
    slide = image.digest()
    nx.press(operator, "ctrl+b")
    image, _ = nx.wait_for_change(screen, slide, 3.0, "gpl black")
    session.save(image, "gpl-audience-black")
    if image.brightest() > 16:
        raise nx.Failure("gpl: Ctrl+B did not turn the audience black")
    if warning(app, operator) is None:
        raise nx.Failure("gpl: the warning disappeared")
    session.save(nx.capture_stable(operator), "gpl-operator-live")
    quit_cleanly(session, app, operator, "gpl")


def missing(session: nx.Session, library: Path) -> None:
    summary = session.summary
    app, operator = launch(
        session,
        library,
        "missing",
        {"SELA_FFMPEG_DIR": "", "SELA_FFMPEG_NO_DEV_COPY": "1"},
    )
    line = media_line(app)
    summary["missing_log"] = line
    if "unavailable (Missing)" not in line:
        raise nx.Failure(f"missing: unexpected log line: {line}")
    control = wait_warning(app, operator, "missing")
    summary["missing_warning_name"] = control.name
    if control.name != MISSING_TEXT:
        raise nx.Failure(f"missing: warning reads {control.name!r}")
    session.save(nx.capture_stable(operator), "missing-operator-warning")
    quit_cleanly(session, app, operator, "missing")


def body(session: nx.Session) -> str:
    data = session.temp("sela-media-")
    library = nx.seed_library(data / "library.sqlite", seed=session.args.seed)
    version = with_ffmpeg(session, library)
    session.summary["accepted_version"] = version
    gpl_refused(session, library)
    missing(session, library)
    return (
        f"FFmpeg {version} accepted without a warning; the GPL system build and a "
        "missing FFmpeg each show the media-unavailable status while Live output, "
        "Go Live and Black keep working"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "media-backend-x11"))
