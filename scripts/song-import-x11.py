#!/usr/bin/env python3
"""Linux/X11 native check: File > Import > Song Folder... end to end (M1-14,
M4-07 OpenSong/ChordPro).

Built on the shared harness (scripts/native_x11.py, docs/e2e-linux.md); the
Windows counterpart is scripts/song-import-windows.py.

Usage: python3 scripts/song-import-x11.py [--display :112 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR] [--build]

Seeds a new library with the original fixture songs, copies the synthetic
batch in tests/fixtures/import/batch (OpenLyrics, extensionless OpenSong,
ChordPro, a malformed XML, a ChordPro file without words and a text note)
into a temporary folder and launches the operator with a private profile.
By keyboard: Alt+F, Down x7, Enter opens File > Import > Song Folder...; the
portal folder chooser gets the folder. The staged preview must list five
files (three songs, two failures) with the summary "Ready: 3 ·
Not importable: 2 · Duplicates: 0", a malformed-file error and the
OpenLyrics attribute warnings, while the library is unchanged. Shift+Tab,
Enter presses Import (3); the library must then hold exactly three more
songs, decoded from SQLite with their titles and words (accented text kept
byte for byte). Typing "mañana" into the Songs search box must leave the one
OpenLyrics song; clicking it previews its four slides, a click selects
slide 2, Live output on and Go Live must show text on the audience output.
Importing the same folder again must mark the three songs as duplicates
with Skip chosen and Import (0). Ctrl+Q and Alt+F4 (WM_DELETE_WINDOW) with that preview open
must keep the operator and the preview and show the quit note
(PP-IMPF-09); Esc then cancels and the library stays the same. Ctrl+Q
then quits with exit status 0.

X11 differences: the chooser is the GTK folder chooser of
xdg-desktop-portal-gtk (Windows: the common folder dialog), driven by
typing the path into its location entry. "ñ" is typed through a spare
keycode mapped to ntilde (xmodmap) before the operator starts, because
GPUI does not see xdotool's on-the-fly mapping; Windows types it with
KEYEVENTF_UNICODE. On the default xvfb
display the audience is the 624x351 development window.
"""

from __future__ import annotations

import shutil
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

ROOT = Path(__file__).resolve().parent.parent
BATCH = ROOT / "tests" / "fixtures" / "import" / "batch"
FOLDER_ITEM_DOWNS = 7  # New Song -> ... -> Import > Song Folder...
IMPORTED = {"Canción del Alba", "Lantern Hymn", "Sungai Kehidupan"}
# A spare keycode that carries ntilde while the operator runs: GPUI reads
# the X keymap when it starts and ignores the mapping xdotool would add on
# the fly for a character outside the US layout.
SPARE_KEYCODE = 93
QUIT_NOTE = "Finish or cancel the import before quitting."


def titles(library: Path) -> list[str]:
    return sorted(fields[0] for fields in nx.song_payloads(library))


def lyrics_of(library: Path, title: str) -> list[str]:
    """Label/lyrics pairs of the head revision of the song titled `title`."""
    for fields in nx.song_payloads(library):
        if fields[0] == title:
            return fields[4:]
    raise nx.Failure(f"{title!r} is not in the library")


def drive_folder_dialog(app: nx.App, folder: Path, owner: int) -> None:
    """Types `folder` into the GTK folder chooser and accepts it. The chooser
    opens on Recent, where Enter in the location entry does nothing, so a
    click on the sidebar's Home row (client 57,59) first moves it to a real
    folder; Enter then selects the typed folder (checked 2026-10-22 with
    xdg-desktop-portal-gtk)."""
    dialog = app.file_dialog()
    time.sleep(0.6)
    nx.activate(dialog)
    nx.click(dialog, 57, 59)
    time.sleep(0.8)
    nx.press(dialog, "ctrl+l")
    time.sleep(0.3)
    nx.press(dialog, "ctrl+a", "backspace")
    nx.ensure_focus(dialog, "typing the folder path")
    nx.tool(
        "xdotool", "type", "--clearmodifiers", "--delay", "0", f"{folder}/", timeout=20
    )
    time.sleep(0.6)
    nx.press(dialog, "enter")
    nx.wait_until(
        lambda: not nx.viewable(dialog),
        12,
        "the portal folder chooser to close",
    )
    time.sleep(0.4)
    nx.activate(owner)


def preview_ready(app: nx.App, operator: int, writes: int):
    """The staged preview in its Review phase: the Import button names how
    many songs it would write."""
    controls = app.controls(operator)
    confirm = controls.get("import-confirm")
    if confirm is None or confirm.name != f"Import ({writes})":
        return None
    if "import-status" in controls:  # still reading or checking
        return None
    return controls


def open_folder_import(app: nx.App, operator: int, folder: Path) -> None:
    # Park the pointer outside where the chooser opens: a GTK tooltip under
    # it would be the portal's first visible window.
    nx.mouse_move(operator, 1270, 790)
    nx.press(operator, "alt+f", *["down"] * FOLDER_ITEM_DOWNS, "enter")
    time.sleep(0.6)
    drive_folder_dialog(app, folder, operator)


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    data = session.temp("sela-import-")
    library = nx.seed_library(data / "library.sqlite", seed=args.seed)
    folder = data / "Lagu Impor"
    shutil.copytree(BATCH, folder)
    before = titles(library)
    summary["library_before"] = before
    # Restore the keycode's own mapping afterwards: with --display the
    # script may run on a real session.
    original = next(
        (
            " ".join(line.split())
            for line in nx.tool("xmodmap", "-pke").splitlines()
            if line.split()[:2] == ["keycode", str(SPARE_KEYCODE)]
        ),
        f"keycode {SPARE_KEYCODE} =",
    )
    summary["spare_keycode_before"] = original
    nx.tool("xmodmap", "-e", f"keycode {SPARE_KEYCODE} = ntilde Ntilde")
    try:
        return run(session, library, folder, before)
    finally:
        nx.tool("xmodmap", "-e", original, check=False)


def run(session: nx.Session, library: Path, folder: Path, before: list[str]) -> str:
    summary = session.summary
    app = session.app(["--operator-library", library], name="operator")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)

    open_folder_import(app, operator, folder)
    controls = nx.wait_until(
        lambda: preview_ready(app, operator, 3), 20, "the staged import preview"
    )
    session.save(operator, "operator-import-preview")
    summary["summary"] = controls["import-summary"].name
    if (
        controls["import-summary"].name
        != "Ready: 3 · Not importable: 2 · Duplicates: 0"
    ):
        raise nx.Failure(
            f"unexpected preview summary {controls['import-summary'].name!r}"
        )
    rows = [
        controls[f"import-row-{i}"].name
        for i in range(5)
        if f"import-row-{i}" in controls
    ]
    summary["rows"] = rows
    if rows != [
        "Sungai Kehidupan",
        "broken.xml",
        "Canción del Alba",
        "empty.cho",
        "Lantern Hymn",
    ]:
        raise nx.Failure(f"unexpected preview rows {rows}")
    if "import-choice-1" in controls or "import-choice-0" not in controls:
        raise nx.Failure("choices must exist for songs and not for failed files")
    if titles(library) != before:
        raise nx.Failure("the preview saved songs before Import was pressed")

    # Focus starts on Cancel: Shift+Tab, Enter is Import (3).
    nx.press(operator, "shift+tab", "enter")
    after = nx.wait_until(
        lambda: (t := titles(library)) and len(t) == len(before) + 3 and t,
        10,
        "the import commit",
    )
    summary["library_after"] = after
    if set(after) - set(before) != IMPORTED:
        raise nx.Failure(
            f"unexpected imported titles {sorted(set(after) - set(before))}"
        )
    cancion = lyrics_of(library, "Canción del Alba")
    summary["cancion_sections"] = cancion
    if cancion[:2] != [
        "Verse 1",
        "Despierta el día sobre el mar\nla mañana canta sin cesar",
    ]:
        raise nx.Failure(f"Canción del Alba stored as {cancion[:2]}")
    time.sleep(0.8)
    session.save(operator, "operator-imported")
    if "import-dialog" in app.controls(operator):
        raise nx.Failure("the preview stayed open after the import")

    # Accented search finds the imported OpenLyrics song only.
    search = app.control("song-search", operator)
    nx.click(operator, *search.center)
    time.sleep(0.3)
    nx.type_text(operator, "mañana")

    def one_hit():
        found = app.controls(operator)
        first = found.get("song-0")
        return (
            first
            if first and first.name == "Canción del Alba" and "song-1" not in found
            else None
        )

    hit = nx.wait_until(one_hit, 10, "the accented search hit")
    session.save(operator, "operator-search-accented")
    nx.click(operator, *hit.center)
    slide = nx.wait_until(
        lambda: app.controls(operator).get("preview-slide-3"), 10, "four preview slides"
    )
    del slide
    second = app.control("preview-slide-1", operator)
    nx.click(operator, *second.center)
    time.sleep(0.4)
    session.save(operator, "operator-preview-imported-slide")

    nx.click(operator, *app.control("live-output", operator).center)
    found = app.audience(15)
    if not found:
        raise nx.Failure(f"no audience window within 15s: {app.log()}")
    _, screen = found
    time.sleep(1.5)
    nx.activate(operator)
    blank = nx.digest(screen)
    nx.click(operator, *app.control("go-live", operator).center)
    image, summary["imported_slide_live_ms"] = nx.wait_for_change(
        screen, blank, 5.0, "imported slide live"
    )
    if image.ink_box() is None:
        raise nx.Failure("the imported slide shows no text on the audience output")
    session.save(image, "audience-imported-slide")
    nx.click(operator, *app.control("live-output", operator).center)
    time.sleep(0.8)

    # The same folder again: every song is a duplicate with Skip chosen.
    nx.activate(operator)
    open_folder_import(app, operator, folder)
    controls = nx.wait_until(
        lambda: preview_ready(app, operator, 0), 20, "the duplicate preview"
    )
    session.save(operator, "operator-import-duplicates")
    summary["duplicate_summary"] = controls["import-summary"].name
    if not controls["import-summary"].name.endswith("Duplicates: 3"):
        raise nx.Failure(f"duplicates not found: {controls['import-summary'].name!r}")
    for row in (0, 2, 4):
        chosen = controls.get(f"import-choice-{row}-skip")
        if chosen is None:
            raise nx.Failure(f"row {row} offers no Skip choice")
    if "import-bulk" not in controls:
        raise nx.Failure("no bulk choice for the duplicates")
    # Ctrl+Q and a window-manager close with the preview open must keep the
    # operator and the preview (PP-IMPF-09) and say why.
    nx.press(operator, "ctrl+q")
    note = nx.wait_until(
        lambda: app.controls(operator).get("import-quit-note"),
        5,
        "the quit note in the preview",
    )
    summary["quit_note"] = note.name
    if note.name != QUIT_NOTE:
        raise nx.Failure(f"unexpected quit note {note.name!r}")
    nx.wm_close(operator)
    time.sleep(1.0)
    if not app.running() or not nx.window_exists(operator):
        raise nx.Failure("closing the window with the preview open quit the operator")
    controls = app.controls(operator)
    if "import-dialog" not in controls or "import-confirm" not in controls:
        raise nx.Failure("quit or close replaced the open preview")
    session.save(operator, "operator-import-quit-refused")
    nx.press(operator, "escape")
    time.sleep(0.8)
    if "import-dialog" in app.controls(operator):
        raise nx.Failure("Esc did not cancel the import")
    if titles(library) != after:
        raise nx.Failure("cancelling the duplicate import changed the library")

    nx.press(operator, "escape")  # leave the search box
    nx.press(operator, "ctrl+q")
    summary["exit"] = app.wait_exit()
    if summary["exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['exit']}")
    return (
        "File > Import > Song Folder... by keyboard stages 3 songs and 2 failures with warnings, "
        "Import commits exactly the 3 songs with accented words intact, 'mañana' finds the "
        "OpenLyrics song, its slide 2 previews and goes live with text, a second import marks "
        "3 duplicates as Skip, Ctrl+Q and Alt+F4 are refused while that preview is open, "
        "and Esc cancels without changes"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "song-import-x11"))
