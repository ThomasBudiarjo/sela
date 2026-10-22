"""Native check: File > Import > Song Folder... on the real Windows desktop
(M1-14 song import, M4-07 OpenSong/ChordPro).

Windows counterpart of scripts/song-import-x11.py, through UI Automation
(scripts/uia-names-windows.ps1) and screen captures of the real operator:

- seeds a new library and copies the synthetic batch in
  tests/fixtures/import/batch (OpenLyrics, extensionless OpenSong, ChordPro,
  a malformed XML, a ChordPro file without words, a text note) into a
  temporary folder whose name has a space;
- Alt+F, Down x7, Enter opens Import > Song Folder...; the common folder
  dialog gets the folder path; the staged preview must list five files
  (three songs, two failures) with the summary
  "Ready: 3 · Not importable: 2 · Duplicates: 0" while the library is
  unchanged;
- Shift+Tab, Enter presses Import (3): the library must hold exactly three
  more songs, read back from SQLite with their accented words intact;
- typing "mañana" (KEYEVENTF_UNICODE) into the Songs search box leaves the
  one OpenLyrics song; clicking it previews four slides, slide 2 is
  selected, Live > Show Live Output and Go Live put text on the audience;
- the same folder again shows three duplicates with Skip chosen and
  Import (0); Ctrl+Q and WM_CLOSE with that preview open keep the
  operator and the preview and show the quit note (PP-IMPF-09); Esc
  cancels and the library is unchanged; Ctrl+Q exits 0.

Takes over the real keyboard and mouse; never run with EasyWorship open.
NOT RUN yet: written on Linux without a Windows host (import-framework
stream, 2026-10-22); see docs/parity-pending.md.

Run: python scripts/song-import-windows.py [--out DIR] [--monitor secondary]
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import shutil
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import native_win as nw


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lo = _load("live_output_windows", "live-output-windows.py")
a11y = _load("a11y_windows", "a11y-windows.py")

BATCH = ROOT / "tests" / "fixtures" / "import" / "batch"
FOLDER_ITEM_DOWNS = 7  # New Song -> ... -> Import > Song Folder...
QUIT_NOTE = "Finish or cancel the import before quitting."
WM_CLOSE = 0x0010
IMPORTED = {"Canción del Alba", "Lantern Hymn", "Sungai Kehidupan"}
ROWS = [
    "Sungai Kehidupan",
    "broken.xml",
    "Canción del Alba",
    "empty.cho",
    "Lantern Hymn",
]


def payloads(library: pathlib.Path) -> list[list[str]]:
    """Head revision payload fields of every live song, read-only."""
    uri = library.resolve().as_uri() + "?mode=ro"
    with sqlite3.connect(uri, uri=True, timeout=2) as db:
        rows = db.execute(
            "SELECT r.payload FROM songs s JOIN song_revisions r "
            "ON r.id=s.id AND r.revision=s.head WHERE s.deleted=0 ORDER BY s.id"
        ).fetchall()
    out = []
    for (data,) in rows:
        fields = []
        while data:
            length = struct.unpack("<I", data[:4])[0]
            fields.append(data[4 : 4 + length].decode("utf-8"))
            data = data[4 + length :]
        out.append(fields)
    return out


def titles(library: pathlib.Path) -> list[str]:
    return sorted(fields[0] for fields in payloads(library))


def drive_folder_dialog(pid: int, owner: int, folder: pathlib.Path) -> None:
    """Types the folder into the common folder dialog's Folder box. The
    first Enter may only enter the folder, so Enter repeats (bounded) until
    the dialog closes."""
    dialog = nw.file_dialog(pid, owner)
    time.sleep(0.6)
    nw.activate(dialog)
    nw.type_text(dialog, str(folder))
    time.sleep(0.3)
    deadline = time.monotonic() + 12
    while nw.user32.IsWindow(dialog) and nw.user32.IsWindowVisible(dialog):
        if time.monotonic() > deadline:
            raise nw.Failure("the folder dialog did not close")
        nw.activate(dialog)
        nw.press(dialog, "enter")
        time.sleep(1.5)
    time.sleep(0.4)
    nw.activate(owner)


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
        default=ROOT / ".amp/in/artifacts/song-import-windows",
    )
    parser.add_argument("--monitor", default="secondary")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(f"FAIL: missing {path}")
            return 1
    summary: dict = {"status": "FAIL", "binary": str(args.binary.resolve())}
    check = a11y.Check(summary)
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-import-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library)], check=True)
    folder = data / "Lagu Impor"
    shutil.copytree(BATCH, folder)
    before = titles(library)
    summary["library_before"] = before
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    app = nw.App(args.binary, ["--operator-library", str(library)])
    hwnd = 0
    audience_hwnd = 0
    try:
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(1.2)

        def save(name: str) -> None:
            width, height, pixels = nw.capture_client(hwnd)
            nw.write_png(args.out / f"{name}.png", width, height, pixels)

        def press(keys: str) -> None:
            nw.press(hwnd, keys)
            time.sleep(0.12)

        def open_folder_import() -> None:
            a11y.park_cursor(hwnd)
            press("alt+f")
            for _ in range(FOLDER_ITEM_DOWNS):
                press("down")
            press("enter")
            time.sleep(0.8)
            drive_folder_dialog(app.process.pid, hwnd, folder)

        def preview(writes: int, timeout: float = 20.0) -> dict:
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                found = a11y.nodes_by_id(a11y.uia(hwnd))
                confirm = found.get("import-confirm")
                if (
                    confirm
                    and confirm["name"] == f"Import ({writes})"
                    and "import-status" not in found
                ):
                    return found
                time.sleep(0.3)
            raise nw.Failure(
                f"the staged preview with Import ({writes}) did not appear"
            )

        open_folder_import()
        found = preview(3)
        save("01-import-preview")
        summary["summary"] = found["import-summary"]["name"]
        check.expect(
            found["import-summary"]["name"]
            == "Ready: 3 · Not importable: 2 · Duplicates: 0",
            f"preview summary {found['import-summary']['name']!r}",
        )
        rows = [
            found[f"import-row-{i}"]["name"]
            for i in range(5)
            if f"import-row-{i}" in found
        ]
        check.expect(rows == ROWS, f"preview rows {rows}")
        check.expect(
            "import-choice-1" not in found and "import-choice-0" in found,
            "choices for songs only, none for a failed file",
        )
        check.expect(titles(library) == before, "nothing is saved while staged")

        press("shift+tab")
        press("enter")
        deadline = time.monotonic() + 10
        after = titles(library)
        while len(after) != len(before) + 3 and time.monotonic() < deadline:
            time.sleep(0.3)
            after = titles(library)
        summary["library_after"] = after
        check.expect(set(after) - set(before) == IMPORTED, f"imported titles {after}")
        cancion = next((f for f in payloads(library) if f[0] == "Canción del Alba"), [])
        check.expect(
            cancion[4:6]
            == ["Verse 1", "Despierta el día sobre el mar\nla mañana canta sin cesar"],
            "accented words stored exactly",
        )
        time.sleep(0.8)
        save("02-imported")

        found = a11y.nodes_by_id(a11y.uia(hwnd))
        a11y.click_node(hwnd, found["song-search"])
        nw.type_text(hwnd, "mañana")
        hit = None
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            found = a11y.nodes_by_id(a11y.uia(hwnd))
            first = found.get("song-0")
            if first and first["name"] == "Canción del Alba" and "song-1" not in found:
                hit = first
                break
            time.sleep(0.3)
        check.expect(hit is not None, "'mañana' finds only Canción del Alba")
        save("03-search-accented")
        if hit is None:
            raise nw.Failure("accented search failed")
        a11y.click_node(hwnd, hit)
        found = a11y.nodes_by_id(a11y.uia(hwnd))
        deadline = time.monotonic() + 10
        while "preview-slide-3" not in found and time.monotonic() < deadline:
            time.sleep(0.3)
            found = a11y.nodes_by_id(a11y.uia(hwnd))
        check.expect("preview-slide-3" in found, "four preview slides")
        a11y.click_node(hwnd, found["preview-slide-1"])
        save("04-preview-slide-2")

        a11y.show_live_output(hwnd)
        seen = lo.audience(app.process.pid, 15)
        if not seen:
            raise nw.Failure("no audience window after Live output on")
        _, audience_hwnd = seen
        time.sleep(1.0)
        nw.activate(hwnd)
        found = a11y.nodes_by_id(a11y.uia(hwnd))
        a11y.click_node(hwnd, found["go-live"])
        live = a11y.wait_for(5.0, lambda: a11y.audience_ink(audience_hwnd))
        check.expect(live, "the imported slide shows text on the audience")
        width, height, pixels = nw.capture_client(audience_hwnd)
        nw.write_png(args.out / "05-audience-imported-slide.png", width, height, pixels)
        a11y.show_live_output(hwnd)
        time.sleep(0.8)
        nw.activate(hwnd)

        open_folder_import()
        found = preview(0)
        save("06-import-duplicates")
        summary["duplicate_summary"] = found["import-summary"]["name"]
        check.expect(
            found["import-summary"]["name"].endswith("Duplicates: 3"),
            f"three duplicates: {found['import-summary']['name']!r}",
        )
        check.expect(
            all(f"import-choice-{row}-skip" in found for row in (0, 2, 4))
            and "import-bulk" in found,
            "duplicate rows offer Skip and the bulk choice exists",
        )
        # Ctrl+Q and WM_CLOSE with the preview open keep the operator and the
        # preview and say why (PP-IMPF-09).
        press("ctrl+q")
        note = None
        deadline = time.monotonic() + 5.0
        while note is None and time.monotonic() < deadline:
            note = a11y.nodes_by_id(a11y.uia(hwnd)).get("import-quit-note")
            time.sleep(0.2)
        summary["quit_note"] = note["name"] if note else None
        check.expect(
            bool(note) and note["name"] == QUIT_NOTE,
            f"Ctrl+Q with the preview open shows the quit note: {summary['quit_note']!r}",
        )
        nw.user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
        time.sleep(1.0)
        found = a11y.nodes_by_id(a11y.uia(hwnd))
        check.expect(
            app.process.poll() is None
            and "import-dialog" in found
            and "import-confirm" in found,
            "WM_CLOSE with the preview open keeps the operator and the preview",
        )
        save("07-import-quit-refused")
        press("escape")
        time.sleep(0.8)
        check.expect(
            "import-dialog" not in a11y.nodes_by_id(a11y.uia(hwnd)),
            "Esc cancels the import",
        )
        check.expect(titles(library) == after, "cancelling changed nothing")

        press("escape")
        press("ctrl+q")
        summary["exit"] = app.wait_exit()
        hwnd = 0
        check.expect(summary["exit"] == 0, f"operator exit status {summary['exit']}")
        summary["status"] = "FAIL" if check.failures else "PASS"
        return 0 if summary["status"] == "PASS" else 1
    except Exception as error:  # noqa: BLE001
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        if audience_hwnd:
            for pid in lo.children(app.process.pid):
                lo.kill(pid)
        app.close()
        (args.out / "summary.json").write_text(
            json.dumps(summary, indent=2, ensure_ascii=False)
        )
        print(json.dumps(summary, indent=2, ensure_ascii=False))
        shutil.rmtree(data, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
