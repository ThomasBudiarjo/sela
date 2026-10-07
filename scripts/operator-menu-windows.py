"""Windows native check: the operator Songs row context menu (M1-05i).

Usage: python scripts/operator-menu-windows.py [--binary sela.exe]
       [--seed seed_library.exe] [--out DIR]

Seeds a new library with two original fixture songs and launches the operator
with a private profile (Live output stays off). With the first Songs row
focused and selected (Space): Shift+F10 opens the menu and Escape closes it, leaving the operator
exactly as before; Shift+F10, Down, Enter (Edit Song…) opens the Song Editor
on that song, checked by its window title; a real right-click on the focused
row opens the menu at the pointer and a left click on its second item (Edit
Song…) opens the same song again. The song is added to the schedule; the Menu
key, Down, Down, Enter (Delete) asks for confirmation, Enter keeps the song
(SQLite still lists it); the same keys, Shift+Tab to Delete and Enter delete it
(SQLite marks it deleted). Ctrl+S then saves the schedule, whose saved item is
still the deleted song's pinned revision, and Ctrl+Q exits cleanly. Requires
exclusive use of the keyboard and mouse while it runs. Captures are written to
--out for review.
"""

from __future__ import annotations

import argparse
import ctypes
import importlib.util
import json
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
SONGS = ("Signal Hymn", "Quiet Canticle")
EDITOR_PREFIX = "Song Editor - "
TITLE = "afterdelete"
# Operator keyboard-focus background (0xdce3fa) as RGB, painted on the focused
# Songs row; the selected-row color (0xe3e7f3) differs by a few levels.
FOCUS_RGB = (0xDC, 0xE3, 0xFA)
# Songs menu geometry in logical pixels: 4 px top padding, 26 px items.
MENU_PAD, MENU_ITEM = 4, 26

nw.VK.update({"escape": 0x1B, "f10": 0x79, "apps": 0x5D})


def keys(hwnd: int, *sequence: str) -> None:
    for key in sequence:
        nw.press(hwnd, key)
        time.sleep(0.15)


def right_click(hwnd: int, x: int, y: int) -> None:
    """Right click at physical client coordinates of the foreground window."""
    if nw.user32.GetForegroundWindow() != hwnd:
        raise nw.Failure(f"window {hwnd:#x} lost the foreground before a right click")
    point = nw.w.POINT(x, y)
    nw.user32.ClientToScreen(hwnd, ctypes.byref(point))
    nw.user32.SetCursorPos(point.x, point.y)
    time.sleep(0.05)
    down, up = nw.INPUT(type=0), nw.INPUT(type=0)
    down.u.mi = nw.MOUSEINPUT(dwFlags=0x0008)
    up.u.mi = nw.MOUSEINPUT(dwFlags=0x0010)
    nw._send([down, up])


def song_ids(library: pathlib.Path) -> list[bytes]:
    """Library rows in operator catalog order (stable song ID order)."""
    with sqlite3.connect(library.as_uri() + "?mode=ro", uri=True) as db:
        return [row[0] for row in db.execute("SELECT id FROM songs ORDER BY id")]


def deleted(library: pathlib.Path, song: bytes) -> bool:
    with sqlite3.connect(library.as_uri() + "?mode=ro", uri=True) as db:
        (flag,) = db.execute("SELECT deleted FROM songs WHERE id=?", (song,)).fetchone()
        return flag == 1


def saved_items(library: pathlib.Path) -> list[tuple[str, list[tuple[bytes, int]]]]:
    with sqlite3.connect(library.as_uri() + "?mode=ro", uri=True) as db:
        rows = db.execute(
            "SELECT s.id, s.head, r.title FROM schedules s "
            "JOIN schedule_revisions r ON r.id = s.id AND r.revision = s.head"
        ).fetchall()
        return [
            (
                title,
                db.execute(
                    "SELECT song, song_revision FROM items "
                    "WHERE id=? AND revision=? ORDER BY position",
                    (sid, head),
                ).fetchall(),
            )
            for sid, head, title in rows
        ]


def changed_pixels(a: bytes, b: bytes) -> int:
    if len(a) != len(b):
        return max(len(a), len(b)) // 4
    return sum(1 for i in range(0, len(a), 4) if a[i : i + 3] != b[i : i + 3])


def wait_editor(pid: int, operator: int, timeout: float = 15.0) -> tuple[int, str]:
    """The editor window and the song title it loaded."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        for hwnd in nw.top_windows(pid):
            name = nw.title(hwnd)
            if hwnd != operator and name.startswith(EDITOR_PREFIX):
                song = name[len(EDITOR_PREFIX) :]
                if song != "Untitled":
                    return hwnd, song
        time.sleep(0.1)
    raise nw.Failure("Edit Song… opened no editor with a loaded song")


def close_editor(hwnd: int) -> None:
    nw.user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
    deadline = time.monotonic() + 5
    while nw.user32.IsWindow(hwnd):
        if time.monotonic() > deadline:
            raise nw.Failure("the unchanged editor did not close")
        time.sleep(0.05)


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
        default=ROOT / ".amp/in/artifacts/operator-menu-windows",
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(
                f"FAIL: missing {path} (cargo build && cargo build --example seed_library)"
            )
            return 1
    summary: dict = {"binary": str(args.binary.resolve())}
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-menu-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library), "--second-song"], check=True)
    first, _ = song_ids(library)
    app = nw.App(args.binary, ["--operator-library", str(library)])

    def save(hwnd: int, name: str) -> bytes:
        width, height, pixels = nw.capture_client(hwnd)
        nw.write_png(args.out / f"{name}.png", width, height, pixels)
        return pixels

    try:
        operator = app.window()
        nw.activate(operator)
        time.sleep(1.0)  # storage worker loads the catalog off the UI thread
        keys(operator, *["tab"] * lo.TABS_TO_FIRST_SONG)
        # Opening the menu selects its row, so select it first: the baseline
        # then already shows the song in Preview and only the menu differs.
        keys(operator, "space")
        time.sleep(0.8)
        before = save(operator, "00-row-focused")

        # Shift+F10 opens the menu; Escape closes it and nothing else changed.
        keys(operator, "shift+f10")
        time.sleep(0.3)
        opened = save(operator, "01-menu-keyboard")
        summary["menu_pixels"] = changed_pixels(before, opened)
        if summary["menu_pixels"] < 500:
            raise nw.Failure(
                "Shift+F10 did not open the Songs menu (F10 system-menu handling?)"
            )
        keys(operator, "escape")
        time.sleep(0.3)
        closed = save(operator, "02-menu-escaped")
        summary["escape_residue_pixels"] = changed_pixels(before, closed)
        if summary["escape_residue_pixels"] > 0:
            raise nw.Failure("Escape left the operator different from before the menu")

        # Keyboard Edit Song…: the editor opens on the focused row's song.
        keys(operator, "shift+f10", "down", "enter")
        editor, song = wait_editor(app.process.pid, operator)
        if song not in SONGS:
            raise nw.Failure(f"editor opened an unexpected song {song!r}")
        summary["first_row"] = song
        time.sleep(0.8)
        save(editor, "03-editor-keyboard")
        close_editor(editor)
        nw.activate(operator)
        time.sleep(0.5)

        # Pointer: right-click the focused row, then click Edit Song….
        width, height, pixels = nw.capture_client(operator)
        box = lo.color_box(width, height, pixels, FOCUS_RGB, tolerance=3)
        if box is None:
            raise nw.Failure("no focused Songs row after the editor closed")
        x, y = (box[0] + box[2]) // 2, (box[1] + box[3]) // 2
        right_click(operator, x, y)
        time.sleep(0.4)
        save(operator, "04-menu-pointer")
        scale = nw.dpi(operator) / 96
        nw.click(
            operator,
            x + round(60 * scale),
            y + round((MENU_PAD + MENU_ITEM + MENU_ITEM / 2) * scale),
        )
        editor, again = wait_editor(app.process.pid, operator)
        if again != song:
            raise nw.Failure(f"right-click Edit Song… opened {again!r}, not {song!r}")
        time.sleep(0.8)
        save(editor, "05-editor-pointer")
        close_editor(editor)
        nw.activate(operator)
        time.sleep(0.5)

        # Schedule the song, then back to its row (Add to Schedule is four
        # stops before the first row).
        keys(operator, *["shift+tab"] * 4, "enter", *["tab"] * 4)
        time.sleep(0.3)

        # Delete asks first; Enter on the default Keep leaves the song.
        keys(operator, "apps", "down", "down", "enter")
        time.sleep(0.4)
        save(operator, "06-delete-confirm")
        keys(operator, "enter")
        time.sleep(1.0)
        if deleted(library, first):
            raise nw.Failure("Keep deleted the song")
        summary["keep_kept_song"] = True

        # Delete → Shift+Tab to Delete → Enter deletes it in SQLite.
        keys(operator, "apps", "down", "down", "enter", "shift+tab", "enter")
        deadline = time.monotonic() + 5
        while not deleted(library, first):
            if time.monotonic() > deadline:
                raise nw.Failure("confirmed Delete did not delete the song")
            time.sleep(0.05)
        time.sleep(0.8)
        save(operator, "07-deleted")
        summary["deleted"] = True

        # The schedule still pins the deleted song's revision.
        keys(operator, "ctrl+s")
        time.sleep(0.4)
        keys(operator, *TITLE, "enter")
        deadline = time.monotonic() + 5
        while not saved_items(library):
            if time.monotonic() > deadline:
                raise nw.Failure("Ctrl+S did not save the schedule")
            time.sleep(0.05)
        stored = saved_items(library)
        summary["saved"] = [(title, len(items)) for title, items in stored]
        if [(title, [s for s, _ in items]) for title, items in stored] != [
            (TITLE, [first])
        ]:
            raise nw.Failure(f"unexpected saved schedule {summary['saved']}")
        save(operator, "08-schedule-saved")

        keys(operator, "ctrl+q")
        summary["exit"] = app.wait_exit()
        if summary["exit"] != 0:
            raise nw.Failure(f"operator exit status {summary['exit']}")
        summary["status"] = "PASS"
        print(
            "PASS: Shift+F10/Escape, keyboard and right-click Edit Song… open the "
            "row's song; Delete asks, Keep keeps, Delete deletes; the saved "
            "schedule still pins the deleted song; Ctrl+Q exits cleanly"
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
        summary.setdefault("status", "FAIL")
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
