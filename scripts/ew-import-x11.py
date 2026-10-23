#!/usr/bin/env python3
"""Linux/X11 native check: File > Import > EasyWorship Library... end to end
(M4-07 EasyWorship 6-8 child).

Built on the shared harness (scripts/native_x11.py, docs/e2e-linux.md); the
Windows counterpart is scripts/ew-import-windows.py.

Usage: python3 scripts/ew-import-x11.py [--display :121 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR] [--build]

Writes a synthetic EasyWorship library (original text; the schema is the
UNOBSERVED best guess in docs/import.md, "EasyWorship library") to
<tmp>/EasyWorship/Default/v6.1/Databases/Data: Songs.db with five song rows
and SongWords.db with words for four of them (EW8 and legacy RTF, a \\uN
escape; one song has no word row and one repeats another's song_uid), and
records the SHA-256, size and mtime of every file under <tmp>/EasyWorship.
Launches the operator on a seeded library. By keyboard: Alt+F, Down x8,
Enter opens File > Import > EasyWorship Library...; the portal folder
chooser gets the EasyWorship *profile* folder (the data folder is found
below it). The staged preview must list the five rows in title order with
"Ready: 4 · Not importable: 1 · Duplicates: 1" and Import (3) while the
library is unchanged. Shift+Tab, Enter presses Import (3); the library must
then hold exactly the three new songs with their EasyWorship metadata
(author, copyright, administrator, CCLI number) and words decoded from
SQLite. Importing the same library again must offer Import (0) with four
duplicates; Esc cancels and the library stays the same. Every file under
<tmp>/EasyWorship must keep its bytes, size and mtime, and no file may be
added there. Ctrl+Q then quits with exit status 0.

X11 differences: the chooser is the GTK folder chooser of
xdg-desktop-portal-gtk (Windows: the common folder dialog), driven like
song-import-x11.py by typing the path into its location entry.
"""

from __future__ import annotations

import importlib.util
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ew_library_fixture as fx
import native_x11 as nx

HERE = Path(__file__).resolve().parent
EW_ITEM_DOWNS = 8  # New Song -> ... -> Import > EasyWorship Library...


def song_import_module():
    """scripts/song-import-x11.py, for its GTK folder chooser driver."""
    spec = importlib.util.spec_from_file_location(
        "song_import_x11", HERE / "song-import-x11.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def titles(library: Path) -> list[str]:
    return sorted(fields[0] for fields in nx.song_payloads(library))


def preview_ready(app: nx.App, operator: int, writes: int):
    controls = app.controls(operator)
    confirm = controls.get("import-confirm")
    if confirm is None or confirm.name != f"Import ({writes})":
        return None
    if "import-status" in controls:  # still reading or checking
        return None
    return controls


def open_library_import(app: nx.App, operator: int, folder: Path, chooser) -> None:
    nx.mouse_move(operator, 1270, 790)
    nx.press(operator, "alt+f", *["down"] * EW_ITEM_DOWNS, "enter")
    time.sleep(0.6)
    chooser.drive_folder_dialog(app, folder, operator)


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    chooser = song_import_module()
    data_root = session.temp("sela-ew-import-")
    library = nx.seed_library(data_root / "library.sqlite", seed=args.seed)
    ew_root = data_root / "EasyWorship"
    profile = ew_root / "Default"
    fx.write_ew_library(profile / "v6.1" / "Databases" / "Data")
    sources = fx.fingerprint(ew_root)
    summary["source_files"] = [(path, digest) for path, digest, _, _ in sources]
    before = titles(library)
    summary["library_before"] = before

    app = session.app(["--operator-library", library], name="operator")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)

    open_library_import(app, operator, profile, chooser)
    controls = nx.wait_until(
        lambda: preview_ready(app, operator, 3), 30, "the staged EasyWorship preview"
    )
    session.save(operator, "operator-ew-preview")
    summary["summary"] = controls["import-summary"].name
    if controls["import-summary"].name != fx.SUMMARY:
        raise nx.Failure(f"unexpected summary {controls['import-summary'].name!r}")
    rows = [
        controls[f"import-row-{i}"].name
        for i in range(len(fx.ROWS))
        if f"import-row-{i}" in controls
    ]
    summary["rows"] = rows
    if rows != fx.ROWS:
        raise nx.Failure(f"unexpected preview rows {rows}")
    if "import-choice-4" in controls or "import-choice-0" not in controls:
        raise nx.Failure("choices must exist for songs and not for the failed row")
    if titles(library) != before:
        raise nx.Failure("the preview saved songs before Import was pressed")

    nx.press(operator, "shift+tab", "enter")
    after = nx.wait_until(
        lambda: (t := titles(library)) and len(t) == len(before) + 3 and t,
        15,
        "the import commit",
    )
    summary["library_after"] = after
    if set(after) - set(before) != fx.IMPORTED:
        raise nx.Failure(
            f"unexpected imported titles {sorted(set(after) - set(before))}"
        )
    harbor = fx.library_song(library, "Harbor Morning")
    kasih = fx.library_song(library, "Kasih Setia")
    if harbor is None or kasih is None:
        raise nx.Failure("an imported song is missing from the library")
    fields, (admin, number) = harbor
    summary["harbor"] = {
        "fields": fields,
        "administrator": admin,
        "song_number": number,
    }
    problems = fx.check_harbor(fields, admin, number)
    if kasih[0][4:6] != fx.KASIH_SECTION:
        problems.append(f"Kasih Setia stored as {kasih[0][4:6]}")
    if problems:
        raise nx.Failure("; ".join(problems))
    time.sleep(0.8)
    session.save(operator, "operator-ew-imported")
    if "import-dialog" in app.controls(operator):
        raise nx.Failure("the preview stayed open after the import")

    nx.activate(operator)
    open_library_import(app, operator, profile, chooser)
    controls = nx.wait_until(
        lambda: preview_ready(app, operator, 0), 30, "the duplicate preview"
    )
    session.save(operator, "operator-ew-duplicates")
    summary["duplicate_summary"] = controls["import-summary"].name
    if not controls["import-summary"].name.endswith(fx.REIMPORT_DUPLICATES):
        raise nx.Failure(f"duplicates not found: {controls['import-summary'].name!r}")
    for row in (0, 1, 2, 3):
        if f"import-choice-{row}-skip" not in controls:
            raise nx.Failure(f"row {row} offers no Skip choice")
    nx.press(operator, "escape")
    time.sleep(0.8)
    if "import-dialog" in app.controls(operator):
        raise nx.Failure("Esc did not cancel the import")
    if titles(library) != after:
        raise nx.Failure("cancelling the duplicate import changed the library")

    unchanged = fx.fingerprint(ew_root)
    if unchanged != sources:
        raise nx.Failure(f"the EasyWorship files changed: {sources} -> {unchanged}")
    summary["sources_unchanged"] = True

    nx.press(operator, "ctrl+q")
    summary["exit"] = app.wait_exit()
    if summary["exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['exit']}")
    return (
        "File > Import > EasyWorship Library... by keyboard finds the data folder under the "
        "profile, stages 4 songs in title order with a missing-words row and a song_uid "
        "duplicate, Import commits the 3 songs with author/copyright/administrator/CCLI and "
        "decoded words, a second import marks 4 duplicates as Skip, Esc cancels, and every "
        "EasyWorship file keeps its SHA-256, size and mtime"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "ew-import-x11"))
