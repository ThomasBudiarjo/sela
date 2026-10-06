"""Windows native check: the Song Editor's Words list, preview and save.

Usage: python scripts/song-editor-windows.py [--binary sela.exe] [--out DIR]

Launches `sela --library` on a new library with a private profile. By keyboard
(and one click into the toolbar Title), while asserting the editor keeps the
foreground: the window opens as "Song Editor - Untitled" with the caret in
slide 1's label; Enter moves from the label to the lyrics; Enter inside the
lyrics is a line break; Ctrl+Enter starts an unlabeled slide 2; Up moves to its
label, where "Chorus" is typed. The captured preview must show rendered text
(near-white pixels on the black slide). Home, Ctrl+Enter then Backspace split
slide 2 at its start and join it back. The title is typed, Ctrl+S saves and the
stored revision is decoded from SQLite (labels and lyrics per slide); the
window title follows the song title. Unchanged after saving, Ctrl+Q closes
without a guard. Requires exclusive use of the keyboard and mouse while it
runs.
"""

from __future__ import annotations

import argparse
import pathlib
import sqlite3
import struct
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parents[1]


def payloads(library: pathlib.Path) -> list[list[str]]:
    with sqlite3.connect(library.as_uri() + "?mode=ro", uri=True) as db:
        rows = db.execute(
            "SELECT r.payload FROM songs s JOIN song_revisions r "
            "ON r.id=s.id AND r.revision=s.head WHERE s.deleted=0"
        ).fetchall()
    result = []
    for (data,) in rows:
        fields = []
        while data:
            length = struct.unpack("<I", data[:4])[0]
            fields.append(data[4 : 4 + length].decode("utf-8"))
            data = data[4 + length :]
        result.append(fields)
    return result


def preview_ink(width: int, height: int, bgra: bytes) -> tuple[int, int]:
    """(black, near-white) samples in the right half, where the slide preview sits."""
    black = white = 0
    for y in range(height // 5, height * 4 // 5, 3):
        row = y * width * 4
        for x in range(width // 2, width - 8, 3):
            b, g, r = bgra[row + 4 * x : row + 4 * x + 3]
            if r < 12 and g < 12 and b < 12:
                black += 1
            elif r > 235 and g > 235 and b > 235:
                white += 1
    return black, white


def keys(hwnd: int, *sequence: str) -> None:
    for key in sequence:
        nw.press(hwnd, key)
        time.sleep(0.15)


def write(hwnd: int, text: str) -> None:
    nw.type_text(hwnd, text)
    time.sleep(0.25)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--binary", type=pathlib.Path, default=ROOT / "target/debug/sela.exe"
    )
    parser.add_argument(
        "--out",
        type=pathlib.Path,
        default=ROOT / ".amp/in/artifacts/song-editor-windows",
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="sela-native-"))
    library = scratch / "library.sqlite"
    app = nw.App(args.binary, ["--library", str(library)], scratch=scratch)
    try:
        hwnd = app.window()
        time.sleep(1.5)
        assert nw.title(hwnd) == "Song Editor - Untitled", nw.title(hwnd)
        nw.activate(hwnd)
        scale = nw.dpi(hwnd) / 96

        def capture(name: str) -> tuple[int, int, bytes]:
            time.sleep(0.8)
            shot = nw.capture_client(hwnd)
            nw.write_png(args.out / f"{name}.png", *shot)
            return shot

        capture("00-new-song")
        write(hwnd, "Verse 1")
        keys(hwnd, "enter")
        write(hwnd, "Amazing grace")
        keys(hwnd, "enter")
        write(hwnd, "how sweet the sound")
        keys(hwnd, "ctrl+enter")
        write(hwnd, "That saved a wretch")
        keys(hwnd, "up")
        write(hwnd, "Chorus")
        keys(hwnd, "down")
        width, height, bgra = capture("01-words")
        black, white = preview_ink(width, height, bgra)
        print(f"preview samples: black={black} white={white}")
        assert black > 2000 and white > 40, "preview shows no rendered slide text"

        # Split slide 2 at its start (an empty slide 2, the text in slide 3),
        # then Backspace at slide 3's start joins it back.
        keys(hwnd, "home", "ctrl+enter")
        capture("02-split")
        keys(hwnd, "backspace")
        capture("03-joined")

        nw.click(hwnd, int(80 * scale), int(20 * scale))
        time.sleep(0.3)
        write(hwnd, "Native Hymn")
        keys(hwnd, "ctrl+s")
        time.sleep(1.5)
        saved = payloads(library)
        expected = [
            "Native Hymn",
            "",
            "",
            "",
            "Verse 1",
            "Amazing grace\nhow sweet the sound",
            "Chorus",
            "That saved a wretch",
        ]
        assert saved == [expected], saved
        assert nw.title(hwnd) == "Song Editor - Native Hymn", nw.title(hwnd)
        capture("04-saved")
        keys(hwnd, "ctrl+q")
        code = app.wait_exit()
        assert code == 0, code
        assert payloads(library) == [expected]
        print(f"ok: {args.out}")
        return 0
    finally:
        app.close()


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (nw.Failure, AssertionError) as error:
        print(f"FAIL: {error}")
        sys.exit(1)
