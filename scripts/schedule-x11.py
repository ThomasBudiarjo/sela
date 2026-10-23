#!/usr/bin/env python3
"""Linux/X11 native check: the operator Schedule driven only by the keyboard.

Port of scripts/schedule-windows.py on the shared harness
(scripts/native_x11.py, docs/e2e-linux.md).

Usage: python3 scripts/schedule-x11.py [--display :100 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR] [--build]

Seeds a new library with two original fixture songs and launches the operator
with a private profile. By keyboard only, while checking the operator keeps
the input focus: Add to Schedule (twice, a duplicate) and once more for the
second song; Down/Up select items; Ctrl+Del removes one; Up moves an item;
Ctrl+S opens the Save dialog, where a path without extension is typed; Sela
adds .selaschedule. The saved file is read back as JSON (title, item order,
song ids). With Live output on, Page Down sends the selected item and Down
then Page Down the next one, each changing the captured audience output;
Ctrl+Del on the live item leaves the audience frame unchanged. With unsaved
changes, a window-manager close and Ctrl+Q both keep the window open behind a
guard; Cancel keeps it, Don't Save quits. A second launch opens the saved file
with Ctrl+O and the Open dialog; removing one of its items makes Ctrl+Q ask
again, which shows the reopened schedule had items. Mouse drag and drop is
covered by GPUI tests only.

X11 differences: GPUI opens file dialogs through xdg-desktop-portal on Linux,
so the dialogs are the GTK file chooser served by the run's private D-Bus
session (xdg-desktop-portal-gtk), driven by keyboard like the Windows common
dialog. WM_CLOSE is the window manager's close (Openbox Alt+F4, which sends
WM_DELETE_WINDOW). On the default xvfb display the audience is the 624x351
development window beside the operator.
"""

from __future__ import annotations

import importlib.util
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx


def _live_output():
    """live-output-x11.py, which owns the Tab count to the first song."""
    path = Path(__file__).resolve().parent / "live-output-x11.py"
    spec = importlib.util.spec_from_file_location("live_output_x11", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


TABS_TO_FIRST_SONG = _live_output().TABS_TO_FIRST_SONG
NAME = "sunday"


def saved_file(path: Path) -> tuple[str, list[bytes]] | None:
    """Title and pinned song ids of a saved .selaschedule document."""
    document = nx.read_schedule(path)
    if document is None:
        return None
    if document["format"] != "sela-schedule" or document["version"] != 4:
        raise nx.Failure(f"unexpected schedule document header in {path}")
    return document["title"], [bytes.fromhex(i["song_id"]) for i in document["items"]]


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    data = session.temp("sela-schedule-")
    library = nx.seed_library(data / "library.sqlite", "--second-song", seed=args.seed)
    first, second = nx.catalog_song_ids(library)
    app = session.app(["--operator-library", library], name="operator")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)  # the storage worker loads the catalog off the UI thread

    def keys(*sequence: str) -> None:
        nx.press(operator, *sequence)

    # Song 0 -> Add (twice: a duplicate) -> song 1 -> Add.
    keys(*["tab"] * TABS_TO_FIRST_SONG, "enter")
    keys(*["shift+tab"] * 4, "enter", "enter")
    # Only the selected song row is a tab stop: Down moves to song 1.
    keys(*["tab"] * 4, "down")
    keys(*["shift+tab"] * 4, "enter")
    time.sleep(0.3)
    session.save(operator, "operator-three-items")

    # Down x3 -> song 1 item; Up -> the duplicate; Ctrl+Del removes it.
    keys("down", "down", "down", "up", "ctrl+delete")
    # Down Down -> song 1 item; schedule Up moves it first.
    keys("down", "down", "tab", "enter")
    time.sleep(0.3)
    session.save(operator, "operator-reordered")

    keys("ctrl+s")
    time.sleep(0.8)
    schedule_file = data / f"{NAME}.selaschedule"
    session.save(nx.capture_stable(app.file_dialog()), "save-dialog")
    app.drive_file_dialog(data / NAME, owner=operator)
    stored = nx.wait_until(
        lambda: saved_file(schedule_file), 5, "Ctrl+S to save the schedule file"
    )
    summary["saved"] = [stored[0], len(stored[1])]
    if stored != (NAME, [second, first]):
        raise nx.Failure(f"unexpected saved schedule {summary['saved']}")
    if sorted(p.name for p in data.glob("*.selaschedule*")) != [schedule_file.name]:
        raise nx.Failure("the saved file name was doubled or a temp file remains")
    time.sleep(0.3)
    session.save(operator, "operator-saved")

    # Focus returned to schedule Up: back to Live output and turn it on.
    keys(*["shift+tab"] * 11, "enter")
    found = app.audience(15)
    if not found:
        raise nx.Failure(f"no audience window within 15s: {app.log()}")
    pid, screen = found
    time.sleep(1.5)
    if nx.focused() != operator:
        raise nx.Failure(
            f"the audience output took the focus; focus is {nx.describe_window(nx.focused())}"
        )
    blank = nx.digest(screen)

    # The selected item is the moved song 1 entry, now first.
    keys("pagedown")
    image, summary["first_item_live_observed_ms"] = nx.wait_for_change(
        screen, blank, 3.0, "first_item_live"
    )
    one = image.digest()
    keys("down")
    # Watched for a bounded second, so a late audience update still fails.
    deadline = time.monotonic() + 1.0
    while time.monotonic() < deadline:
        if nx.digest(screen) != one:
            raise nx.Failure("Down changed the audience output; it only previews")
        time.sleep(0.1)
    keys("pagedown")
    image, summary["second_item_live_observed_ms"] = nx.wait_for_change(
        screen, one, 3.0, "second_item_live"
    )
    two = image.digest()
    if image.ink_box() is None:
        raise nx.Failure("the second item shows no text")
    session.save(image, "audience-second-item")

    keys("ctrl+delete")
    time.sleep(1.0)
    if nx.digest(screen) != two:
        raise nx.Failure("removing the live item changed the audience output")
    session.save(operator, "operator-live-item-removed")
    summary["live_item_removal_kept_output"] = True

    # Unsaved changes: closing asks first.
    nx.wm_close(operator)
    time.sleep(1.5)
    if not app.running() or not nx.window_exists(operator):
        raise nx.Failure(
            "a window-manager close closed a schedule with unsaved changes"
        )
    session.save(operator, "operator-close-guard")
    keys("enter")  # Cancel
    keys("ctrl+q")
    time.sleep(1.0)
    if not app.running():
        raise nx.Failure("Ctrl+Q quit with unsaved schedule changes")
    keys("shift+tab", "enter")  # Don't Save
    summary["guarded_exit"] = app.wait_exit()
    if summary["guarded_exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['guarded_exit']}")
    if not nx.wait_dead(pid, 5):
        raise nx.Failure("quitting left the audience output running")
    session.close_app(app)

    app = session.app(["--operator-library", library], name="reopened")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)
    nx.press(operator, "ctrl+o")
    time.sleep(0.8)
    session.save(nx.capture_stable(app.file_dialog()), "open-dialog")
    app.drive_file_dialog(schedule_file, owner=operator)
    time.sleep(0.8)
    session.save(operator, "operator-reopened")
    summary["reopened_title"] = nx.window_name(operator)
    # Removing a reopened item makes it unsaved, so Ctrl+Q asks first.
    nx.press(operator, "down", "ctrl+delete", "ctrl+q")
    time.sleep(1.0)
    if not app.running():
        raise nx.Failure("the reopened schedule had no item to remove")
    session.save(operator, "operator-reopened-guard")
    nx.press(operator, "shift+tab", "enter")  # Don't Save
    summary["reopened_exit"] = app.wait_exit()
    if summary["reopened_exit"] != 0:
        raise nx.Failure(f"reopened operator exit status {summary['reopened_exit']}")
    return (
        "keyboard Add/duplicate/Down/Up/Ctrl+Del/move and Ctrl+S save the intended pinned items through "
        "the portal Save dialog; Page Down sends schedule items; removing the live item keeps the output; "
        "WM close and Ctrl+Q are guarded; Ctrl+O reopens the saved schedule through the portal Open dialog"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "schedule-x11"))
