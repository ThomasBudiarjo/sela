#!/usr/bin/env python3
"""Linux/X11 native E2E for video backgrounds and Live playback (M2-02,
M2-03, M2-05 basic; docs/media.md "Video backgrounds and playback").

Usage: python3 scripts/video-x11.py [--display :123 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR]
       [--build]

Needs the pinned LGPL FFmpeg in the development folder (`scripts/fetch-ffmpeg`).
The script makes an original 8 s clip with that FFmpeg (a green box moving
left to right over navy at 60 px/s, VP9 in WebM) and seeds "Motion Hymn" with the clip
as its song master (Verse 1, Chorus, Verse 2) plus a Bridge slide whose own
background names a missing video, and "Still Hymn" on a navy color. Checks,
in order:

1. Media tab › Videos lists "sela-motion" (named without its extension,
   EW8-OBS-039) and its row thumbnail, made off the UI thread, shows the
   clip's navy and green.
2. Go Live on Motion Hymn: the audience shows the video under the white
   lyrics; two captures 0.8 s apart differ in the video strip above the text
   while the text stays (text over a playing video). The Live pane shows the
   transport row (`video-transport`), its renderer-reported position reads
   "Playing", and an FFmpeg decoder child runs for it.
3. Next (Chorus): the lyrics change and the video keeps running without a
   restart (the reported position keeps rising past the 8 s clip and wraps:
   Repeating loops, SRC-10).
4. Pause holds the picture (two captures 1 s apart are identical, position
   "Paused"); Play continues it; Restart goes back to the first frames
   (the box is at the left edge again).
5. Black (Ctrl+B) shows plain black; Ctrl+B again shows the video still
   running underneath (UNOBSERVED best guess for EW8-OBS-006): the box moved
   on while Black was up.
6. Clear (Ctrl+C) shows the video without text; Ctrl+C again restores it.
7. Next (Bridge, missing video): the slide shows black behind its text with
   the warning "Background video ‘sela-missing’ is missing; showing black";
   the transport row goes away.
8. Live output off: the audience process and every FFmpeg child it started
   end (no `ffmpeg` left among the run's processes).

X11 differences: on the default xvfb display the audience is the 624x351
development window beside the operator (`--kind dual` for the
monitor-covering window); software Vulkan (lavapipe) renders, so frame
pacing and decode speed are not representative; no audio device exists, and
no EasyWorship reference runs here (the transport labels, the Videos
category placement and mask continuation are best guesses).
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

REPO = Path(__file__).resolve().parent.parent
FFMPEG = REPO / ".amp/in/ffmpeg/n8.1.3-lgpl/bin/ffmpeg"
NAVY = (0x20, 0x30, 0x60)
GREEN = (0, 255, 0)


def make_clip(out: Path) -> Path:
    if not FFMPEG.is_file():
        raise nx.EnvironmentProblem(f"missing {FFMPEG}; run scripts/fetch-ffmpeg first")
    graph = (
        "color=c=0x203060:size=640x360:rate=30:d=8[bg];"
        "color=c=0x00ff00:size=80x80:rate=30[box];"
        "[bg][box]overlay=x='t*60':y=30:shortest=1"
    )
    done = subprocess.run(
        [
            str(FFMPEG),
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-filter_complex",
            graph,
            "-c:v",
            "libvpx-vp9",
            "-deadline",
            "realtime",
            "-cpu-used",
            "8",
            "-b:v",
            "300k",
            str(out),
        ],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    if done.returncode != 0:
        raise nx.EnvironmentProblem(f"making the clip failed: {done.stderr.strip()}")
    return out


def box_x(shot: nx.Image) -> int | None:
    """Left edge of the green box in the video strip (top quarter), or None."""
    height = shot.height // 4
    for x in range(0, shot.width, 2):
        for y in range(4, height, 4):
            if nx.near(shot.pixel(x, y), GREEN, 60):
                return x
    return None


def strip(shot: nx.Image) -> nx.Image:
    """The video strip above the lyrics (top quarter of the audience)."""
    return shot.crop(0, 0, shot.width, shot.height // 4)


def ffmpeg_pids(app: nx.App) -> list[int]:
    found = []
    for pid in nx.tagged_pids(app.tag):
        try:
            name = Path(f"/proc/{pid}/comm").read_text().strip()
        except OSError:
            continue
        if name == "ffmpeg":
            found.append(pid)
    return found


def cpu_seconds(pid: int) -> float:
    """User plus system CPU time of a process, from /proc (0 when gone)."""
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    except OSError:
        return 0.0
    return (int(fields[11]) + int(fields[12])) / os.sysconf("SC_CLK_TCK")


def rss_mib(pid: int) -> float:
    try:
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) / 1024
    except OSError:
        pass
    return 0.0


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    data = session.temp("sela-video-")
    clip = make_clip(data / "motion.webm")
    library = nx.seed_library(
        data / "profile" / "library.sqlite",
        "--video-song",
        str(clip),
        seed=args.seed,
    )
    app = session.app(["--operator-library", library], name="operator")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.5)

    def control(aid: str, timeout: float = 10) -> nx.Control:
        return nx.wait_until(
            lambda: app.controls(operator).get(aid), timeout, f"control {aid}"
        )

    def press_control(aid: str) -> None:
        nx.click(operator, *control(aid).center)

    def status() -> str:
        return app.controls(operator)["Live-status"].name

    def position() -> str:
        return app.controls(operator)["video-position"].name

    def wait_position(text: str, what: str, timeout: float = 10) -> str:
        def probe():
            found = app.controls(operator).get("video-position")
            return found.name if found and text in found.name else None

        return nx.wait_until(probe, timeout, what)

    # 1. Media › Videos lists the clip with a thumbnail.
    press_control("Media")
    press_control("media-category-videos")
    row = control("video-0")
    if row.name != "sela-motion":
        raise nx.Failure(f"the Videos list names {row.name!r}, not 'sela-motion'")

    def thumbnail():
        shot = nx.capture(operator)
        # The picture is the row's left 80x45 after its 12 px padding.
        thumb = shot.crop(row.x + 12, row.y + 4, 80, 45)
        navy = thumb.count(NAVY, 24)
        green = thumb.count(GREEN, 60)
        return (navy, green) if navy > 20 and green > 2 else None

    summary["thumbnail_navy_green"] = nx.wait_until(
        thumbnail, 20, "the Videos row thumbnail"
    )
    session.save(operator, "01-videos-list")

    # 2. Schedule Motion Hymn, Live output on, Go Live.
    press_control("Songs")
    for _ in range(2):
        songs = app.controls(operator)
        hymn = next(
            (
                c
                for aid, c in songs.items()
                if aid.startswith("song-") and "Motion" in c.name
            ),
            None,
        )
        if hymn:
            break
        time.sleep(1)
    if hymn is None:
        raise nx.Failure("Motion Hymn is not in the Songs list")
    nx.click(operator, *hymn.center)
    time.sleep(0.4)
    press_control("add-to-schedule")
    control("schedule-item-0")
    press_control("live-output")
    found = app.audience(20)
    if not found:
        raise nx.Failure(f"no audience window within 20s: {app.log()}")
    audience_pid, screen = found
    time.sleep(1.5)
    nx.activate(operator)
    blank = nx.digest(screen)
    press_control("schedule-item-0")
    time.sleep(0.8)
    started = time.monotonic()
    nx.press(operator, "pagedown")
    _, ms = nx.wait_for_change(screen, blank, 10.0, "the video cue going live")
    summary["go_live_first_change_ms"] = ms
    control("video-transport")
    summary["position_after_go_live"] = wait_position("Playing", "the Playing report")
    session.save(operator, "02-operator-transport")
    summary["ffmpeg_playing"] = len(ffmpeg_pids(app))
    if not ffmpeg_pids(app):
        raise nx.Failure("no FFmpeg decoder runs for the live video")
    # This VM only (software Vulkan, debug build): CPU share of the audience
    # and its decoder over 2 s of playback, and the audience's RSS.
    watched = [audience_pid, *ffmpeg_pids(app)]
    before = {pid: cpu_seconds(pid) for pid in watched}
    time.sleep(2.0)
    summary["playback_cpu_percent"] = {
        ("audience" if pid == audience_pid else f"ffmpeg-{pid}"): round(
            (cpu_seconds(pid) - before[pid]) / 2.0 * 100, 1
        )
        for pid in watched
    }
    summary["audience_rss_mib"] = round(rss_mib(audience_pid), 1)
    first = nx.capture(screen)
    session.save(first, "02-verse-video")
    if first.ink_box(threshold=235) is None:
        raise nx.Failure("no lyrics over the video")
    if first.count(NAVY, 24) < 1000:
        raise nx.Failure("the audience does not show the navy video")
    time.sleep(0.8)
    second = nx.capture(screen)
    session.save(second, "02b-verse-video-later")
    moved = strip(first).difference(strip(second), tolerance=24)
    summary["video_strip_change"] = moved
    if moved <= 0.001 or box_x(first) == box_x(second):
        raise nx.Failure(
            f"the video did not move under the text (strip change {moved}, "
            f"box {box_x(first)} -> {box_x(second)})"
        )
    if second.ink_box(threshold=235) is None:
        raise nx.Failure("the lyrics left while the video played")

    # 3. Next: new text, same running video (loops past 4 s).
    nx.press(operator, "right")
    nx.wait_until(lambda: "Chorus" in status(), 10, "Chorus in the Live status")
    time.sleep(0.5)
    chorus = nx.capture(screen)
    session.save(chorus, "03-chorus-video")
    if chorus.ink_box(threshold=235) is None:
        raise nx.Failure("no Chorus text over the video")
    remaining = 8.6 - (time.monotonic() - started)
    if remaining > 0:
        time.sleep(remaining)
    summary["position_after_loop"] = position()
    if "Playing" not in position():
        raise nx.Failure(f"the video stopped at the loop point: {position()!r}")
    wrapped = position().split("·")[-1].strip().split("/")[0].strip()
    if wrapped not in ("0:00", "0:01", "0:02", "0:03", "0:04"):
        raise nx.Failure(f"the reported position did not wrap: {position()!r}")

    # 4. Pause, Play, Restart.
    press_control("video-play-pause")
    wait_position("Paused", "the Paused report")
    time.sleep(0.3)
    held = nx.capture(screen)
    time.sleep(1.0)
    if nx.capture(screen).digest() != held.digest():
        raise nx.Failure("the picture kept changing while paused")
    session.save(held, "04-paused")
    press_control("video-play-pause")
    wait_position("Playing", "Playing again")
    time.sleep(0.6)
    if nx.capture(screen).digest() == held.digest():
        raise nx.Failure("Play did not continue the video")
    # Let the box travel right, then Restart sends it back to the left.
    time.sleep(1.5)
    press_control("video-restart")
    wait_position("0:00", "the restart report", timeout=10)
    restarted = nx.capture(screen)
    summary["box_after_restart"] = box_x(restarted)
    if box_x(restarted) is None or box_x(restarted) > 90:
        raise nx.Failure(f"Restart did not start over (box at {box_x(restarted)})")
    session.save(restarted, "04b-restarted")

    # 5. Black over the video, the video running underneath.
    before_black = box_x(nx.capture(screen))
    nx.press(operator, "ctrl+b")
    nx.wait_until(lambda: "Mask: Black" in status(), 10, "Black in the Live status")
    time.sleep(0.5)
    black = nx.capture(screen)
    session.save(black, "05-black")
    if black.brightest() > 16:
        raise nx.Failure("Black does not black out the video")
    time.sleep(1.0)
    nx.press(operator, "ctrl+b")
    nx.wait_until(lambda: "Mask:" not in status(), 10, "Black off")
    time.sleep(0.5)
    after_black = nx.capture(screen)
    session.save(after_black, "05b-after-black")
    summary["box_before_after_black"] = [before_black, box_x(after_black)]
    if after_black.ink_box(threshold=235) is None:
        raise nx.Failure("the lyrics did not come back after Black")
    if before_black is None or (box_x(after_black) or 0) < before_black + 60:
        raise nx.Failure("the video stood still under Black")

    # 6. Clear: the video without text, then the text again.
    nx.press(operator, "ctrl+c")
    nx.wait_until(lambda: "Mask: Clear" in status(), 10, "Clear in the Live status")
    time.sleep(0.5)
    clear = nx.capture(screen)
    session.save(clear, "06-clear")
    if clear.ink_box(threshold=235) is not None:
        raise nx.Failure("Clear left text over the video")
    if clear.count(NAVY, 24) < 1000:
        raise nx.Failure("Clear did not keep the video")
    nx.press(operator, "ctrl+c")
    nx.wait_until(lambda: "Mask:" not in status(), 10, "Clear off")

    # 7. The Bridge's video is missing: black with the warning.
    nx.press(operator, "right")
    nx.wait_until(lambda: "Chorus" not in status(), 10, "Verse 2")
    nx.press(operator, "right")
    warning = "Background video ‘sela-missing’ is missing; showing black"
    nx.wait_until(lambda: warning in status() or None, 15, "the missing-video warning")
    nx.wait_until(lambda: "Bridge" in status(), 10, "Bridge in the Live status")
    time.sleep(0.6)
    bridge = nx.capture(screen)
    session.save(bridge, "07-bridge-missing")
    if bridge.ink_box(threshold=235) is None:
        raise nx.Failure("the Bridge text is not shown")
    if bridge.count(NAVY, 24) > 200:
        raise nx.Failure("the missing video did not substitute black")
    nx.wait_until(
        lambda: "video-transport" not in app.controls(operator),
        5,
        "the transport row to go away",
    )
    summary["ffmpeg_while_live"] = len(ffmpeg_pids(app))

    # 8. Live output off ends the audience and its FFmpeg children.
    press_control("live-output")
    nx.wait_until(lambda: app.audience(0) is None, 10, "the audience to close")
    nx.wait_until(lambda: not ffmpeg_pids(app), 10, "every FFmpeg child to end")
    session.save(operator, "08-off")
    return (
        "video background plays under the lyrics, loops, pauses, plays, restarts, "
        "runs on under Black and Clear, substitutes black for a missing file, "
        "and its FFmpeg children end with Live output"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "video-x11"))
