"""Windows native check: UI language switch (i18n, M1-16 language slice).

Usage: python scripts/i18n-windows.py [--binary sela.exe]
       [--seed seed_library.exe] [--out DIR]

Uses a private profile and SELA_AUDIENCE_MONITOR=window. Run 1 (English):
select the first song, turn Live output on, Go Live; record the audience
pixels and pid. Switch Options > General > Language to Bahasa Indonesia by
keyboard and confirm: the operator chrome changes without restart, the
settings file gains language=id, and the audience window is the same process
with byte-identical pixels (no cue re-sent). Run 2: the restart opens in
Indonesian and Options shows Bahasa Indonesia selected. Switch back to
English; run 3 opens in English. Captures and summary.json go to --out.
Takes over the real keyboard; never run with EasyWorship open.
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
_spec = importlib.util.spec_from_file_location(
    "live_output_windows", ROOT / "scripts" / "live-output-windows.py"
)
lo = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(lo)

TABS_TO_FIRST_SONG = 26
SHIFT_TABS_SONG_TO_LIVE_OUTPUT = 16
TABS_LIVE_OUTPUT_TO_OPTIONS = 10
TABS_START_TO_OPTIONS = 21


def monitors(binary: pathlib.Path) -> list[str]:
    out = subprocess.run(
        [str(binary), "--list-monitors"], capture_output=True, text=True, check=True
    ).stdout
    return [line.split("\t")[0] for line in out.splitlines() if line.strip()]


class Session:
    def __init__(
        self, binary: pathlib.Path, library: pathlib.Path, out: pathlib.Path, monitor: str
    ):
        os.environ["SELA_AUDIENCE_MONITOR"] = monitor
        self.out = out
        self.app = nw.App(binary, ["--operator-library", str(library)])
        self.hwnd = self.app.window()
        nw.activate(self.hwnd)
        time.sleep(1.2)

    def keys(self, *sequence: str) -> None:
        for key in sequence:
            nw.press(self.hwnd, key)
            time.sleep(0.15)

    def save(self, name: str) -> None:
        width, height, pixels = nw.capture_client(self.hwnd)
        nw.write_png(self.out / f"{name}.png", width, height, pixels)

    def open_options(self, from_start: bool) -> None:
        if from_start:
            self.keys(*["tab"] * TABS_START_TO_OPTIONS, "enter")
        else:
            # Live output was toggled from here and focus never moved since.
            self.keys(*["tab"] * TABS_LIVE_OUTPUT_TO_OPTIONS, "enter")
        time.sleep(0.6)

    def pick_language(self, rows: int, indonesian: bool) -> None:
        # Focus starts on Cancel; rows are OK, monitors from the last, then
        # the language rows (Indonesian, English) walking backwards.
        steps = 2 + rows + (0 if indonesian else 1)
        self.keys(*["shift+tab"] * steps, "space")
        time.sleep(0.2)
        self.keys(*["tab"] * (rows + 1 + (0 if indonesian else 1)), "enter")
        time.sleep(0.6)

    def quit(self) -> None:
        self.keys("ctrl+q")
        self.app.wait_exit()

    def close(self) -> None:
        self.app.close()


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
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/i18n-windows"
    )
    parser.add_argument("--monitor", default="secondary")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(
                f"FAIL: missing {path} (cargo build && cargo build --example seed_library)"
            )
            return 1
    summary: dict = {"status": "FAIL"}
    summary_path = args.out / "summary.json"
    sessions: list[Session] = []
    try:
        rows = len(monitors(args.binary)) + 1  # the Default row plus monitors
        data = pathlib.Path(tempfile.mkdtemp(prefix="sela-i18n-"))
        library = data / "library.sqlite"
        subprocess.run([str(args.seed), str(library)], check=True)
        settings = data / "settings.txt"

        # Run 1: go live in English, then switch to Indonesian without a restart.
        run = Session(args.binary, library, args.out, args.monitor)
        sessions.append(run)
        run.save("01-english-operator")
        run.keys(*["tab"] * TABS_TO_FIRST_SONG, "enter", "tab", "enter")
        time.sleep(0.4)
        run.keys(*["shift+tab"] * SHIFT_TABS_SONG_TO_LIVE_OUTPUT, "enter")
        found = lo.audience(run.app.process.pid, 15)
        if not found:
            raise nw.Failure("no audience window after Live output on")
        audience_pid, audience_hwnd = found
        time.sleep(1.0)
        run.keys("pagedown")
        time.sleep(1.0)
        before = lo.digest(audience_hwnd)
        width, height, pixels = nw.capture_client(audience_hwnd)
        nw.write_png(args.out / "02-audience-before.png", width, height, pixels)
        if lo.ink_box(width, height, pixels) is None:
            raise nw.Failure("Go Live showed nothing on the audience output")
        run.save("03-english-live")

        run.open_options(from_start=False)
        run.save("04-options-english")
        run.pick_language(rows, indonesian=True)
        run.save("05-indonesian-operator")
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if settings.exists() and "language=id" in settings.read_text():
                break
            time.sleep(0.2)
        if not settings.exists():
            raise nw.Failure("settings.txt was never written")
        text = settings.read_text()
        summary["settings_after_switch"] = text
        if "language=id" not in text:
            raise nw.Failure("settings.txt does not record language=id")
        after = lo.digest(audience_hwnd)
        width, height, pixels = nw.capture_client(audience_hwnd)
        nw.write_png(args.out / "06-audience-after.png", width, height, pixels)
        summary["audience_same_process"] = bool(
            lo.alive(audience_pid) and lo.audience(run.app.process.pid, 1)
        )
        summary["audience_pixels_unchanged"] = before == after
        if not summary["audience_pixels_unchanged"]:
            raise nw.Failure("the language switch changed the audience output")
        run.quit()

        # Run 2: the restart opens in Indonesian.
        run = Session(args.binary, library, args.out, args.monitor)
        sessions.append(run)
        run.save("07-restart-indonesian")
        run.open_options(from_start=True)
        run.save("08-options-indonesian")
        run.pick_language(rows, indonesian=False)
        run.save("09-english-operator")
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if settings.exists() and "language=" not in settings.read_text():
                break
            time.sleep(0.2)
        summary["settings_after_switch_back"] = settings.read_text()
        if "language=" in summary["settings_after_switch_back"]:
            raise nw.Failure("switching back to English did not clear language=id")
        run.quit()

        # Run 3: the restart opens in English again.
        run = Session(args.binary, library, args.out, args.monitor)
        sessions.append(run)
        run.save("10-restart-english")
        run.quit()
        summary["status"] = "PASS"
        return 0
    except Exception as error:  # noqa: BLE001
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        for session in sessions:
            session.close()
        summary_path.write_text(json.dumps(summary, indent=2))
        print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
