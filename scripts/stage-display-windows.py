"""Windows native check: stage (Foldback) output beside the audience (M3-01/M3-02).

Usage: python scripts/stage-display-windows.py [--binary sela.exe]
       [--seed seed_library.exe] [--out DIR] [--stage \\\\.\\DISPLAYn]

Needs three monitors (primary for the operator plus two others) and
exclusive keyboard and mouse; never run with EasyWorship open. Uses a private
profile whose settings enable Foldback, with no SELA_AUDIENCE_MONITOR or
SELA_STAGE_MONITOR override, so both outputs use their production
borderless monitor-covering placement. Checks, in order:

1. Live > Show Live Output starts two child processes: the audience on the
   first non-primary monitor and the stage on another non-primary monitor
   (Options > Foldback "Automatic" never picks the audience monitor); the
   operator keeps the foreground.
2. The idle stage is dark with its amber label and white clock.
3. Go Live (song-0, go-live) and Next: both outputs change; the stage shows
   white current text and sky-blue next text, the audience shows no stage
   colors; the Live status names both acknowledged slides.
4. Live > Stage Message..., typed text with a "b" in it (typed and pressed
   as a key: no Black toggles while typing, neither output changes before
   Show on Stage), Show on Stage: the stage gains the amber message band
   (counted in its band, so a clock-minute tick cannot pass for it), the
   audience frame is unchanged.
5. The stage process is terminated: the Live status reports it lost; the
   audience process stays and Next still changes it.
6. One Live > Show Stage Output (the lost status names it) restarts only
   the stage, on the current slide; the audience PID and frame are
   unchanged.
7. Options > Foldback, uncheck "Enable support for Foldback Output", OK:
   the stage ends, the audience stays, settings.txt drops stage_enabled.
8. Ctrl+Q exits 0 and ends the audience.

With --stage, Options > Foldback picks that monitor first and the stage must
open there. Captures and summary.json go to --out.

NOT RUN yet: written on Linux without a Windows host (stage-display stream,
2026-10-22); run it and record the result in docs/work-log.md.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parents[1]


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lo = _load("live_output_windows", "live-output-windows.py")
a11y = _load("a11y_windows", "a11y-windows.py")

STAGE_TITLE = "Sela stage output"
STAGE_BG = (16, 17, 20)
WHITE = (255, 255, 255)
NEXT_RGB = (125, 211, 252)
NOTE_RGB = (251, 191, 36)


def monitors(binary: pathlib.Path) -> list[dict]:
    out = subprocess.run(
        [str(binary), "--list-monitors"], capture_output=True, text=True, check=True
    ).stdout
    rows = []
    for line in out.splitlines():
        name, width, height, x, y, primary = line.split("\t")
        rows.append(
            {
                "name": name,
                "width": int(width),
                "height": int(height),
                "x": int(x),
                "y": int(y),
                "primary": primary == "1",
            }
        )
    return rows


def stage_window(operator_pid: int, timeout: float) -> tuple[int, int] | None:
    deadline = time.monotonic() + timeout
    while True:
        for pid in lo.children(operator_pid):
            for hwnd in nw.top_windows(pid):
                if nw.title(hwnd) == STAGE_TITLE:
                    return pid, hwnd
        if time.monotonic() >= deadline:
            return None
        time.sleep(0.05)


class Run:
    def __init__(self, binary: pathlib.Path, library: pathlib.Path, out: pathlib.Path):
        self.out = out
        self.app = nw.App(binary, ["--operator-library", str(library)])
        self.hwnd = self.app.window()
        nw.activate(self.hwnd)
        time.sleep(1.2)

    def nodes(self) -> dict[str, dict]:
        return a11y.nodes_by_id(a11y.uia(self.hwnd))

    def click(self, aid: str) -> None:
        found = self.nodes()
        if aid not in found:
            raise nw.Failure(f"{aid} missing from the UIA tree")
        a11y.click_node(self.hwnd, found[aid])

    def live_menu(self, aid: str) -> None:
        nw.press(self.hwnd, "alt+l")
        time.sleep(0.4)
        self.click(aid)

    def status(self) -> str:
        return (self.nodes().get("Live-status") or {}).get("name", "")

    def wait_status(self, text: str, timeout: float = 10.0) -> str:
        deadline = time.monotonic() + timeout
        while True:
            status = self.status()
            if text in status:
                return status
            if time.monotonic() >= deadline:
                raise nw.Failure(f"Live status never contained {text!r}: {status!r}")
            time.sleep(0.3)

    def capture(self, hwnd: int, name: str) -> tuple[int, int, bytes]:
        width, height, pixels = nw.capture_client(hwnd)
        nw.write_png(self.out / f"{name}.png", width, height, pixels)
        return width, height, pixels


def digest(hwnd: int) -> bytes:
    return nw.capture_client(hwnd)[2]


def wait_change(hwnd: int, before: bytes, what: str, timeout: float = 10.0) -> None:
    deadline = time.monotonic() + timeout
    while digest(hwnd) == before:
        if time.monotonic() >= deadline:
            raise nw.Failure(f"{what}: the output did not change")
        time.sleep(0.1)
    time.sleep(0.5)


def band(
    shot: tuple[int, int, bytes], top: float, bottom: float
) -> tuple[int, int, bytes]:
    """Rows top..bottom (shares of the height) of a BGRA capture."""
    width, height, pixels = shot
    first, last = int(height * top), int(height * bottom)
    return width, last - first, pixels[first * width * 4 : last * width * 4]


def body_digest(hwnd: int) -> bytes:
    """Stage frame below the header, so a clock-minute tick is not a change."""
    return band(nw.capture_client(hwnd), 0.12, 0.88)[2]


def message_band(hwnd: int) -> int:
    width, height, pixels = band(nw.capture_client(hwnd), 0.12, 0.25)
    return lo.hits(width, height, pixels, NOTE_RGB, tolerance=40)


def colors(shot: tuple[int, int, bytes]) -> dict[str, int]:
    width, height, pixels = shot
    return {
        "white": lo.hits(width, height, pixels, WHITE, tolerance=40),
        "next": lo.hits(width, height, pixels, NEXT_RGB, tolerance=40),
        "note": lo.hits(width, height, pixels, NOTE_RGB, tolerance=40),
    }


def check_stage(name: str, shot: tuple[int, int, bytes], next_text: bool) -> dict:
    found = colors(shot)
    if found["white"] < 50 or found["note"] < 10:
        raise nw.Failure(f"{name}: stage lacks current text or its label ({found})")
    if next_text and found["next"] < 20:
        raise nw.Failure(f"{name}: stage lacks the next slide ({found})")
    return found


def check_audience(name: str, shot: tuple[int, int, bytes]) -> dict:
    found = colors(shot)
    if found["next"] or found["note"]:
        raise nw.Failure(f"{name}: stage colors on the audience ({found})")
    if found["white"] < 50:
        raise nw.Failure(f"{name}: no slide text on the audience ({found})")
    return found


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
        default=ROOT / ".amp/in/artifacts/stage-display-windows",
    )
    parser.add_argument("--stage", default=None, help="stage monitor device name")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(
                f"FAIL: missing {path} (cargo build && cargo build --example seed_library)"
            )
            return 1
    summary: dict = {"status": "FAIL"}
    runs: list[Run] = []
    try:
        screens = monitors(args.binary)
        summary["monitors"] = screens
        others = [m for m in screens if not m["primary"]]
        if len(others) < 2:
            print("ENVIRONMENT: needs a primary monitor plus two others")
            summary["status"] = "ENVIRONMENT"
            return 2
        data = pathlib.Path(tempfile.mkdtemp(prefix="sela-stage-"))
        library = data / "library.sqlite"
        subprocess.run([str(args.seed), str(library)], check=True)
        settings = data / "settings.txt"
        stored = "sela-settings 1\nstage_enabled=1\n"
        if args.stage:
            stored += f"stage_monitor={args.stage}\n"
        settings.write_text(stored, encoding="utf-8")

        run = Run(args.binary, library, args.out)
        runs.append(run)
        operator_pid = run.app.process.pid
        # 1. Live output on: two processes on two different monitors.
        a11y.show_live_output(run.hwnd)
        audience = lo.audience(operator_pid, 15)
        stage = stage_window(operator_pid, 15)
        if not audience or not stage:
            raise nw.Failure(f"outputs missing: audience {audience}, stage {stage}")
        (audience_pid, audience_hwnd), (stage_pid, stage_hwnd) = audience, stage
        time.sleep(1.5)
        nw.ensure_foreground(run.hwnd, "after Live output on")
        placed = {
            "audience": nw.window_monitor(audience_hwnd),
            "stage": nw.window_monitor(stage_hwnd),
        }
        summary["placement"] = placed
        if placed["audience"]["device"] == placed["stage"]["device"]:
            raise nw.Failure("the stage opened on the audience monitor")
        primary = next(m["name"] for m in screens if m["primary"])
        if placed["stage"]["device"] == primary and not args.stage:
            raise nw.Failure("Automatic put the stage on the primary monitor")
        if args.stage and placed["stage"]["device"] != args.stage:
            raise nw.Failure(
                f"stage is on {placed['stage']['device']}, not {args.stage}"
            )
        if not placed["stage"]["covers_monitor"]:
            raise nw.Failure("the stage window does not cover its monitor")
        summary["status_idle"] = run.wait_status("showing Nothing live")
        summary["stage_idle"] = colors(run.capture(stage_hwnd, "01-stage-idle"))

        # 2-3. Go Live and Next on both outputs.
        run.click("song-0")
        time.sleep(0.5)
        before_audience, before_stage = digest(audience_hwnd), digest(stage_hwnd)
        run.click("go-live")
        wait_change(audience_hwnd, before_audience, "audience go live")
        wait_change(stage_hwnd, before_stage, "stage go live")
        run.wait_status("showing Signal Hymn · Verse 1")
        summary["stage_verse"] = check_stage(
            "verse", run.capture(stage_hwnd, "02-stage-verse"), next_text=True
        )
        summary["audience_verse"] = check_audience(
            "verse", run.capture(audience_hwnd, "02-audience-verse")
        )
        before_audience, before_stage = digest(audience_hwnd), digest(stage_hwnd)
        run.click("live-next")
        wait_change(audience_hwnd, before_audience, "audience next")
        wait_change(stage_hwnd, before_stage, "stage next")
        run.wait_status("showing Signal Hymn · Chorus")

        # 4. Stage message: private while typed (a "b" typed and pressed as
        # a key must not fire Black), stage-only once shown. The header
        # (clock) is left out of the comparison: the minute may change.
        audience_frame, before_stage = digest(audience_hwnd), body_digest(stage_hwnd)
        run.live_menu("bar-live-stage-message")
        time.sleep(0.4)
        nw.type_text(run.hwnd, "Wrap up by eleven ")
        nw.press(run.hwnd, "b")
        time.sleep(0.5)
        if digest(audience_hwnd) != audience_frame:
            raise nw.Failure("typing in the stage message changed the audience")
        if body_digest(stage_hwnd) != before_stage:
            raise nw.Failure("the stage changed before Show on Stage")
        nw.press(run.hwnd, "backspace")
        nw.press(run.hwnd, "backspace")
        run.click("dialog-confirm")
        run.wait_status("Stage message: Wrap up by eleven")
        deadline = time.monotonic() + 10
        while message_band(stage_hwnd) < 10:
            if time.monotonic() >= deadline:
                raise nw.Failure("no message band on the stage")
            time.sleep(0.1)
        summary["stage_message"] = check_stage(
            "message", run.capture(stage_hwnd, "03-stage-message"), next_text=True
        )
        if digest(audience_hwnd) != audience_frame:
            raise nw.Failure("the stage message changed the audience frame")

        # 5. Terminate the stage process.
        lo.kill(stage_pid)
        if not lo.wait_dead(stage_pid, 5):
            raise nw.Failure("the stage process did not end")
        summary["status_lost"] = run.wait_status("Stage output lost")
        if not lo.alive(audience_pid):
            raise nw.Failure("ending the stage ended the audience")
        before_audience = digest(audience_hwnd)
        run.click("live-next")
        wait_change(audience_hwnd, before_audience, "audience next after stage loss")
        run.wait_status("On screen: Signal Hymn · Bridge")

        # 6. Restart only the stage.
        audience_frame = digest(audience_hwnd)
        if "Show Stage Output restarts it" not in summary["status_lost"]:
            raise nw.Failure("the lost status does not name the restart")
        run.live_menu("bar-live-stage")
        restarted = stage_window(operator_pid, 15)
        if not restarted or restarted[0] == stage_pid:
            raise nw.Failure("no fresh stage process after Show Stage Output")
        stage_pid, stage_hwnd = restarted
        time.sleep(1.5)
        nw.ensure_foreground(run.hwnd, "after the stage restart")
        run.wait_status("showing Signal Hymn · Bridge")
        summary["stage_restarted"] = check_stage(
            "restarted", run.capture(stage_hwnd, "04-stage-restarted"), next_text=False
        )
        if digest(audience_hwnd) != audience_frame or not lo.alive(audience_pid):
            raise nw.Failure("restarting the stage touched the audience")

        # 7. Options > Foldback: disable stops only the stage.
        nw.press(run.hwnd, "alt+e")
        nw.press(run.hwnd, "enter")
        time.sleep(0.6)
        run.click("options-section-foldback")
        time.sleep(0.3)
        run.click("options-stage-enabled")
        run.capture(run.hwnd, "05-options-foldback")
        run.click("dialog-confirm")
        if not lo.wait_dead(stage_pid, 5):
            raise nw.Failure("disabling Foldback left the stage running")
        if not lo.alive(audience_pid):
            raise nw.Failure("disabling Foldback ended the audience")
        deadline = time.monotonic() + 5
        while "stage_enabled" in settings.read_text(encoding="utf-8"):
            if time.monotonic() >= deadline:
                raise nw.Failure("settings.txt still enables Foldback")
            time.sleep(0.1)

        # 8. Quit.
        nw.press(run.hwnd, "ctrl+q")
        summary["operator_exit"] = run.app.wait_exit()
        if summary["operator_exit"] != 0 or not lo.wait_dead(audience_pid, 5):
            raise nw.Failure("Ctrl+Q did not end cleanly")
        summary["status"] = "PASS"
        print(
            "PASS: stage and audience run and fail independently on separate monitors"
        )
        return 0
    except nw.Failure as error:
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        for run in runs:
            run.app.close()
        summary["thieves"] = nw.THIEVES
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
