"""Windows native check: video backgrounds and Live playback (M2-02, M2-03,
M2-05 basic).

Windows counterpart of scripts/video-x11.py, same checks in the same order
(see that docstring): an original 8 s clip (a green box moving right at
60 px/s over navy, VP9 in WebM, with a 440 Hz Opus tone) made with the
pinned FFmpeg; "Motion Hymn"
over it as the song master plus a Bridge naming a missing video; Media ›
Videos lists "sela-motion" with a thumbnail; Go Live plays the video under
the lyrics (two captures 0.8 s apart differ above the text, two ffmpeg.exe
children of the audience run, picture and sound, the transport row reports
"Playing" and "Sound on" when a default output device exists, M2-04); Next keeps
the video running and its reported position wraps after 8 s; Pause holds the
picture, Play continues, Restart starts over; Black blacks out while the
video runs on underneath; Clear shows the video without text; the Bridge
substitutes black with the missing-video warning and no transport; Live
output off ends the audience and its ffmpeg.exe children (Job Object).

Usage: python scripts/video-windows.py [--binary target\\debug\\sela.exe]
       [--seed target\\debug\\examples\\seed_library.exe] [--out DIR]

Prerequisites: Windows 11, scripts\\fetch-ffmpeg.ps1 (the pinned LGPL win64
build in .amp\\in\\ffmpeg\\n8.1.3-lgpl), `cargo build --locked` and `cargo
build --locked --examples`, a secondary monitor for the audience
(SELA_AUDIENCE_MONITOR=secondary), the keyboard and mouse idle for the whole
run. Takes over the real keyboard; never run with EasyWorship open. Also
listen (speakers on, PP-VP-13): the tone plays from Go Live, stops while
paused, starts over on Restart and keeps playing under Black and Clear;
the script cannot hear it, so note what you heard in the work log.
Status: NOT RUN (no Windows host when written, 2026-10-23).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parents[1]
FFMPEG = ROOT / ".amp" / "in" / "ffmpeg" / "n8.1.3-lgpl" / "bin" / "ffmpeg.exe"
NAVY = (0x20, 0x30, 0x60)
GREEN = (0, 255, 0)
WARNING = "Background video ‘sela-missing’ is missing; showing black"


def load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lo = load("live_output_windows", "live-output-windows.py")
a11y = load("a11y_windows", "a11y-windows.py")


def wait_for(timeout: float, what: str, probe):
    deadline = time.monotonic() + timeout
    while True:
        value = probe()
        if value:
            return value
        if time.monotonic() >= deadline:
            raise nw.Failure(f"timed out waiting for {what}")
        time.sleep(0.2)


def make_clip(out: pathlib.Path) -> pathlib.Path:
    if not FFMPEG.is_file():
        raise nw.Failure(f"missing {FFMPEG}; run scripts\\fetch-ffmpeg.ps1 first")
    graph = (
        "color=c=0x203060:size=640x360:rate=30:d=8[bg];"
        "color=c=0x00ff00:size=80x80:rate=30[box];"
        "[bg][box]overlay=x='t*60':y=30:shortest=1[v];"
        "sine=frequency=440:sample_rate=48000:duration=8[a]"
    )
    subprocess.run(
        [
            str(FFMPEG),
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-filter_complex",
            graph,
            "-map",
            "[v]",
            "-map",
            "[a]",
            "-c:a",
            "libopus",
            "-b:a",
            "64k",
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
        check=True,
        timeout=60,
    )
    return out


def pixel(width: int, bgra: bytes, x: int, y: int) -> tuple[int, int, int]:
    i = (y * width + x) * 4
    return bgra[i + 2], bgra[i + 1], bgra[i]


def box_x(width: int, height: int, bgra: bytes) -> int | None:
    """Left edge of the green box in the top quarter, or None."""
    for x in range(0, width, 2):
        for y in range(4, height // 4, 4):
            if lo.near(pixel(width, bgra, x, y), GREEN, 60):
                return x
    return None


def count(width: int, height: int, bgra: bytes, rgb: tuple[int, int, int]) -> int:
    return sum(
        1
        for y in range(0, height, 4)
        for x in range(0, width, 4)
        if lo.near(pixel(width, bgra, x, y), rgb, 24)
    )


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
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/video-windows"
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    summary: dict = {"status": "FAIL"}
    app = None
    try:
        data = pathlib.Path(tempfile.mkdtemp(prefix="sela-video-"))
        clip = make_clip(data / "motion.webm")
        library = data / "profile" / "library.sqlite"
        library.parent.mkdir(parents=True)
        subprocess.run(
            [str(args.seed), str(library), "--video-song", str(clip)], check=True
        )
        os.environ["SELA_AUDIENCE_MONITOR"] = "secondary"
        app = nw.App(args.binary, ["--operator-library", str(library)])
        operator = app.window()
        nw.activate(operator)
        time.sleep(1.5)

        def nodes() -> dict[str, dict]:
            return a11y.nodes_by_id(a11y.uia(operator))

        def press_control(aid: str) -> None:
            a11y.click_node(operator, wait_for(10, aid, lambda: nodes().get(aid)))

        def position(needle: str, what: str) -> str:
            def probe():
                node = nodes().get("video-position")
                return node and needle in node.get("name", "") and node["name"]

            return wait_for(10, what, probe)

        def shot(hwnd: int, name: str) -> tuple[int, int, bytes]:
            width, height, pixels = nw.capture_client(hwnd)
            nw.write_png(args.out / f"{name}.png", width, height, pixels)
            return width, height, pixels

        # 1. Media › Videos lists the clip with a thumbnail.
        press_control("Media")
        press_control("media-category-videos")
        row = wait_for(10, "video-0", lambda: nodes().get("video-0"))
        if row.get("name") != "sela-motion":
            raise nw.Failure(f"the Videos list names {row.get('name')!r}")
        time.sleep(3)
        shot(operator, "01-videos-list")

        # 2. Schedule Motion Hymn, Live output on, Go Live.
        press_control("Songs")
        hymn = wait_for(
            10,
            "Motion Hymn in Songs",
            lambda: next(
                (
                    aid
                    for aid, node in nodes().items()
                    if aid.startswith("song-") and "Motion" in node.get("name", "")
                ),
                None,
            ),
        )
        press_control(hymn)
        press_control("add-to-schedule")
        a11y.show_live_output(operator)
        found = lo.audience(app.process.pid, 20)
        if not found:
            raise nw.Failure("no audience window after Live output on")
        audience_pid, screen = found
        a11y.wait_live_status(operator, "×", 30, "audience output never became ready")
        nw.activate(operator)
        press_control("schedule-item-0")
        before = lo.digest(screen)
        started = time.monotonic()
        nw.press(operator, "pagedown")
        wait_for(10, "the video cue going live", lambda: lo.digest(screen) != before)
        summary["position_after_go_live"] = position("Playing", "the Playing report")
        # "Sound on", "No sound output" (no device) or "Sound failed".
        summary["sound_note"] = position("ound", "the sound note")
        if "Sound failed" in summary["sound_note"]:
            raise nw.Failure(f"the audio failed: {summary['sound_note']!r}")
        if len(lo.children(audience_pid)) < 2:
            raise nw.Failure("picture and sound ffmpeg.exe children expected")
        first = shot(screen, "02-verse-video")
        if lo.ink_box(*first) is None:
            raise nw.Failure("no lyrics over the video")
        if count(*first, NAVY) < 100:
            raise nw.Failure("the audience does not show the navy video")
        time.sleep(0.8)
        second = shot(screen, "02b-verse-video-later")
        if box_x(*first) == box_x(*second):
            raise nw.Failure("the video did not move under the text")

        # 3. Next keeps the video; it loops past 8 s.
        nw.press(operator, "right")
        a11y.wait_live_status(operator, "Chorus", 10, "Chorus in the Live status")
        remaining = 8.6 - (time.monotonic() - started)
        if remaining > 0:
            time.sleep(remaining)
        summary["position_after_loop"] = position("Playing", "playing past the loop")

        # 4. Pause, Play, Restart.
        press_control("video-play-pause")
        position("Paused", "the Paused report")
        time.sleep(0.3)
        held = lo.digest(screen)
        time.sleep(1.0)
        if lo.digest(screen) != held:
            raise nw.Failure("the picture kept changing while paused")
        press_control("video-play-pause")
        position("Playing", "Playing again")
        time.sleep(1.5)
        press_control("video-restart")
        position("0:00", "the restart report")
        restarted = shot(screen, "04-restarted")
        summary["box_after_restart"] = box_x(*restarted)
        if box_x(*restarted) is None or box_x(*restarted) > 90:
            raise nw.Failure("Restart did not start over")

        # 5. Black over the video, running underneath.
        before_black = box_x(*nw.capture_client(screen))
        nw.press(operator, "ctrl+b")
        a11y.wait_live_status(operator, "Mask: Black", 10, "Black")
        time.sleep(0.5)
        if lo.brightest(nw.capture_client(screen)[2]) > 16:
            raise nw.Failure("Black does not black out the video")
        time.sleep(1.0)
        nw.press(operator, "ctrl+b")
        time.sleep(1.0)
        after_black = shot(screen, "05-after-black")
        summary["box_before_after_black"] = [before_black, box_x(*after_black)]
        if before_black is None or (box_x(*after_black) or 0) < before_black + 60:
            raise nw.Failure("the video stood still under Black")

        # 6. Clear: video without text.
        nw.press(operator, "ctrl+c")
        a11y.wait_live_status(operator, "Mask: Clear", 10, "Clear")
        time.sleep(0.5)
        clear = shot(screen, "06-clear")
        if lo.ink_box(*clear) is not None:
            raise nw.Failure("Clear left text over the video")
        nw.press(operator, "ctrl+c")

        # 7. The Bridge's missing video substitutes black with the warning.
        nw.press(operator, "right")
        a11y.wait_live_status(operator, "Verse 2", 10, "Verse 2")
        nw.press(operator, "right")
        a11y.wait_live_status(operator, WARNING, 15, "the missing-video warning")
        time.sleep(0.6)
        bridge = shot(screen, "07-bridge-missing")
        if lo.ink_box(*bridge) is None or count(*bridge, NAVY) > 50:
            raise nw.Failure("the Bridge does not show text on black")
        wait_for(
            5, "the transport row to go away", lambda: "video-transport" not in nodes()
        )

        # 8. Live output off ends the audience and its decoder children.
        children = lo.children(audience_pid)
        press_control("live-output")
        if not lo.wait_dead(audience_pid, 10):
            raise nw.Failure("Live output off left the audience running")
        for pid in children:
            if not lo.wait_dead(pid, 10):
                raise nw.Failure(f"decoder child {pid} outlived the audience")
        summary["status"] = "PASS"
    except (nw.Failure, subprocess.SubprocessError) as failure:
        summary["error"] = str(failure)
        print(f"FAIL: {failure}")
    finally:
        if app is not None:
            app.close()
        (args.out / "summary.json").write_text(
            json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8"
        )
        print(json.dumps(summary, indent=2, ensure_ascii=False))
    return 0 if summary["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
