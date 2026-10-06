"""Windows native check: the operator Schedule driven only by the keyboard.

Usage: python scripts/schedule-windows.py [--binary sela.exe] [--seed seed_library.exe]
       [--monitor secondary] [--out DIR]

Seeds a new library with two original fixture songs and launches the operator
with a private profile. By keyboard only, while asserting the operator keeps
the foreground: Add to Schedule (twice, a duplicate) and once more for the
second song; Down/Up select items; Ctrl+Del removes one; Up moves an item;
Ctrl+S asks for a title, which is typed, and Enter saves. The saved schedule is
read back from SQLite (title, item order, pinned revisions). With Live output
on, Page Down sends the selected item and Down then Page Down the next one,
each changing the captured audience output; Ctrl+Del on the live item leaves
the audience frame unchanged. With unsaved changes, WM_CLOSE and Ctrl+Q both
keep the window open behind a guard; Cancel keeps it, Discard quits. A second
launch opens the saved schedule with Ctrl+O; removing one of its items makes
Ctrl+Q ask again, which shows the reopened schedule had items. Requires exclusive use of the
keyboard and an unobstructed audience monitor while it runs. Mouse drag and
drop is covered by GPUI tests only.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import sqlite3
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

WM_CLOSE = 0x0010
TITLE = "sunday"


def keys(hwnd: int, *sequence: str) -> None:
    for key in sequence:
        nw.press(hwnd, key)
        time.sleep(0.12)


def song_ids(library: pathlib.Path) -> list[bytes]:
    """Library rows in operator catalog order (stable song ID order)."""
    with sqlite3.connect(library.as_uri() + "?mode=ro", uri=True) as db:
        return [row[0] for row in db.execute("SELECT id FROM songs ORDER BY id")]


def saved_schedules(library: pathlib.Path) -> list[tuple[str, list[bytes]]]:
    with sqlite3.connect(library.as_uri() + "?mode=ro", uri=True) as db:
        rows = db.execute(
            "SELECT s.id, s.head, r.title FROM schedules s "
            "JOIN schedule_revisions r ON r.id = s.id AND r.revision = s.head"
        ).fetchall()
        return [
            (
                title,
                [
                    item[0]
                    for item in db.execute(
                        "SELECT song FROM items WHERE id=? AND revision=? ORDER BY position",
                        (sid, head),
                    )
                ],
            )
            for sid, head, title in rows
        ]


def wait_change(screen: int, before: str, name: str, summary: dict) -> str:
    sent = time.monotonic()
    while lo.digest(screen) == before:
        if time.monotonic() - sent > 3.0:
            raise nw.Failure(f"{name}: audience output did not change within 3s")
        time.sleep(0.02)
    summary[f"{name}_observed_ms"] = round((time.monotonic() - sent) * 1000)
    time.sleep(0.3)
    return lo.digest(screen)


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
        default=ROOT / ".amp/in/artifacts/schedule-windows",
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
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-schedule-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library), "--second-song"], check=True)
    first, second = song_ids(library)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    spawned: set[int] = set()
    app = nw.App(args.binary, ["--operator-library", str(library)])

    def save(hwnd: int, name: str) -> None:
        width, height, pixels = nw.capture_client(hwnd)
        nw.write_png(args.out / f"{name}.png", width, height, pixels)

    try:
        operator = app.window()
        nw.activate(operator)
        time.sleep(1.0)  # storage worker loads the catalog off the UI thread

        # Song 0 -> Add (twice: a duplicate) -> song 1 -> Add.
        keys(operator, *["tab"] * lo.TABS_TO_FIRST_SONG, "enter")
        keys(operator, *["shift+tab"] * 4, "enter", "enter")
        keys(operator, *["tab"] * 5, "enter")
        keys(operator, *["shift+tab"] * 5, "enter")
        time.sleep(0.3)
        save(operator, "operator-three-items")

        # Down x3 -> song 1 item; Up -> the duplicate; Ctrl+Del removes it.
        keys(operator, "down", "down", "down", "up", "ctrl+delete")
        # Down Down -> song 1 item; schedule Up moves it first.
        keys(operator, "down", "down", "tab", "enter")
        time.sleep(0.3)
        save(operator, "operator-reordered")

        keys(operator, "ctrl+s")
        time.sleep(0.3)
        save(operator, "operator-save-as")
        keys(operator, *TITLE, "enter")
        deadline = time.monotonic() + 5
        while not saved_schedules(library):
            if time.monotonic() > deadline:
                raise nw.Failure("Ctrl+S did not save the schedule")
            time.sleep(0.05)
        stored = saved_schedules(library)
        summary["saved"] = [(title, len(items)) for title, items in stored]
        if stored != [(TITLE, [second, first])]:
            raise nw.Failure(f"unexpected saved schedule {summary['saved']}")
        time.sleep(0.3)
        save(operator, "operator-saved")

        # Focus returned to schedule Up: back to Live output and turn it on.
        keys(operator, *["shift+tab"] * 11, "enter")
        found = lo.audience(app.process.pid, 15)
        if not found:
            raise nw.Failure(f"no audience window within 15s: {app.log()}")
        pid, screen = found
        spawned.add(pid)
        time.sleep(1.5)
        if nw.user32.GetForegroundWindow() != operator:
            raise nw.Failure("the audience output took the foreground")
        blank = lo.digest(screen)

        # The selected item is the moved song 1 entry, now first.
        keys(operator, "pagedown")
        one = wait_change(screen, blank, "first_item_live", summary)
        keys(operator, "down")
        time.sleep(0.5)
        if lo.digest(screen) != one:
            raise nw.Failure("Down changed the audience output; it only previews")
        keys(operator, "pagedown")
        two = wait_change(screen, one, "second_item_live", summary)
        width, height, pixels = nw.capture_client(screen)
        if lo.ink_box(width, height, pixels) is None:
            raise nw.Failure("the second item shows no text")
        nw.write_png(args.out / "audience-second-item.png", width, height, pixels)

        keys(operator, "ctrl+delete")
        time.sleep(1.0)
        if lo.digest(screen) != two:
            raise nw.Failure("removing the live item changed the audience output")
        save(operator, "operator-live-item-removed")
        summary["live_item_removal_kept_output"] = True

        # Unsaved changes: closing asks first.
        nw.user32.PostMessageW(operator, WM_CLOSE, 0, 0)
        time.sleep(1.5)
        if app.process.poll() is not None or not nw.user32.IsWindow(operator):
            raise nw.Failure("WM_CLOSE closed a schedule with unsaved changes")
        save(operator, "operator-close-guard")
        keys(operator, "enter")  # Cancel
        keys(operator, "ctrl+q")
        time.sleep(1.0)
        if app.process.poll() is not None:
            raise nw.Failure("Ctrl+Q quit with unsaved schedule changes")
        keys(operator, "shift+tab", "enter")  # Discard changes
        summary["guarded_exit"] = app.wait_exit()
        if summary["guarded_exit"] != 0:
            raise nw.Failure(f"operator exit status {summary['guarded_exit']}")
        if not lo.wait_dead(pid, 5):
            raise nw.Failure("quitting left the audience output running")

        app.close()
        app = nw.App(args.binary, ["--operator-library", str(library)])
        operator = app.window()
        nw.activate(operator)
        time.sleep(1.0)
        keys(operator, "ctrl+o")
        time.sleep(0.8)
        save(operator, "operator-open-list")
        keys(operator, "shift+tab", "enter")  # the only saved schedule
        time.sleep(0.8)
        save(operator, "operator-reopened")
        # Removing a reopened item makes it unsaved, so Ctrl+Q asks first.
        keys(operator, "down", "ctrl+delete", "ctrl+q")
        time.sleep(1.0)
        if app.process.poll() is not None:
            raise nw.Failure("the reopened schedule had no item to remove")
        save(operator, "operator-reopened-guard")
        keys(operator, "shift+tab", "enter")  # Discard changes
        summary["reopened_exit"] = app.wait_exit()
        if summary["reopened_exit"] != 0:
            raise nw.Failure(
                f"reopened operator exit status {summary['reopened_exit']}"
            )
        summary["status"] = "PASS"
        print(
            "PASS: keyboard Add/duplicate/Down/Up/Ctrl+Del/move and Ctrl+S save the "
            "intended pinned items; Page Down sends schedule items; removing the live "
            "item keeps the output; WM_CLOSE and Ctrl+Q are guarded; Ctrl+O reopens "
            "the saved schedule"
        )
        return 0
    except (nw.Failure, AssertionError) as error:
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        if app.hwnd and nw.user32.IsWindow(app.hwnd):
            save(app.hwnd, "failure-operator")
        return 1
    finally:
        app.close()
        for pid in spawned:
            if lo.alive(pid):
                lo.kill(pid)
        summary.setdefault("status", "FAIL")
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
