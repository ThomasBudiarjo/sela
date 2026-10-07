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
window title follows the song title. Clicking the Slides tab must show
rendered thumbnails (black slides with white text in the narrowed left pane);
clicking slide 1 selects it, the + appends an unlabeled "Slide 3" and Ctrl+Z
removes it again. The toolbar Format toggle docks the Format pane; by Tab,
Enter and a typed #FF0000 the text Color of slide 1 turns red in the preview,
Tab Space turns Bold on, Ctrl+S stores exactly that format for slide 1 only.
Ctrl+A on slide 1's thumbnail selects the whole song; the same Color path
then stores green for both slides (Ctrl+A turns the pane to its Slide tab, so
the path first selects the Text tab). Slide backgrounds (M1-05h3), by
keyboard in the Slide tab: Fill ▾ Color Fill with #FFAA00 turns slide 1's
preview orange; Media Fill, Select Media… and Enter pick a seeded 4:1 profile
image (Zoom covers the preview), Aspect Ratio ▾ Maintain letterboxes it in
black; Edit Slide Layouts sets the master to #2040A0, which slide 2's
thumbnail then shows; each step is saved and decoded from SQLite. A double
click on the preview returns to Words, where typing over Ctrl+A leaves one
unlabeled slide with slide 1's format and background. Unchanged after saving
that, Ctrl+Q closes without a guard.
Requires exclusive use of the keyboard and mouse while it runs.
"""

from __future__ import annotations

import argparse
import hashlib
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


def thumbnail_ink(
    width: int, height: int, bgra: bytes, scale: float
) -> tuple[int, int]:
    """(black, white) samples in the Slides tab's thumbnail column. White is
    strict because the pane itself is #fbfbfa."""
    black = white = 0
    for y in range(int(130 * scale), min(height, int(450 * scale)), 2):
        row = y * width * 4
        for x in range(int(40 * scale), int(240 * scale), 2):
            b, g, r = bgra[row + 4 * x : row + 4 * x + 3]
            if r < 12 and g < 12 and b < 12:
                black += 1
            elif r > 253 and g > 253 and b > 253:
                white += 1
    return black, white


def format_button(
    width: int, height: int, bgra: bytes, scale: float
) -> tuple[int, int] | None:
    """Center of the toolbar's Format toggle: the only dark (enabled) label
    between the insert groups and Library/Inspector; Animate and
    Presentation beside it are grey."""
    xs: list[int] = []
    ys: list[int] = []
    for y in range(int(24 * scale), int(46 * scale)):
        row = y * width * 4
        for x in range(int(320 * scale), width - int(200 * scale)):
            b, g, r = bgra[row + 4 * x : row + 4 * x + 3]
            if r < 0x70 and g < 0x70 and b < 0x70:
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    return sum(xs) // len(xs), sum(ys) // len(ys)


def red_ink(width: int, height: int, bgra: bytes, scale: float) -> int:
    """Red samples between the Slides-tab pane and the Format pane."""
    red = 0
    for y in range(int(110 * scale), height - int(80 * scale), 2):
        row = y * width * 4
        for x in range(int(270 * scale), width - int(290 * scale), 2):
            b, g, r = bgra[row + 4 * x : row + 4 * x + 3]
            if r > 200 and g < 70 and b < 70:
                red += 1
    return red


def ink(
    width: int,
    height: int,
    bgra: bytes,
    scale: float,
    rgb: tuple[int, int, int],
    thumbnails: bool = False,
) -> int:
    """Samples within 8 of `rgb`: in the preview between the Slides-tab pane
    and the Format pane, or in the thumbnail column."""
    left, right = (
        (int(40 * scale), int(240 * scale))
        if thumbnails
        else (int(270 * scale), width - int(290 * scale))
    )
    count = 0
    for y in range(int(110 * scale), height - int(80 * scale), 2):
        row = y * width * 4
        for x in range(left, right, 2):
            b, g, r = bgra[row + 4 * x : row + 4 * x + 3]
            if abs(r - rgb[0]) <= 8 and abs(g - rgb[1]) <= 8 and abs(b - rgb[2]) <= 8:
                count += 1
    return count


def backgrounds(library: pathlib.Path) -> tuple[list[tuple[int, bytes]], list[bytes]]:
    """(slide backgrounds by position, song master) of the head revision."""
    with sqlite3.connect(library.as_uri() + "?mode=ro", uri=True) as db:
        slides = db.execute(
            "SELECT b.position, b.background FROM section_backgrounds b JOIN songs s "
            "ON b.song=s.id AND b.revision=s.head ORDER BY b.position"
        ).fetchall()
        master = db.execute(
            "SELECT b.background FROM song_backgrounds b JOIN songs s "
            "ON b.song=s.id AND b.revision=s.head"
        ).fetchall()
    return slides, [row[0] for row in master]


def formats(library: pathlib.Path) -> list[tuple[int, bytes]]:
    with sqlite3.connect(library.as_uri() + "?mode=ro", uri=True) as db:
        return db.execute(
            "SELECT f.position, f.format FROM section_formats f JOIN songs s "
            "ON f.song=s.id AND f.revision=s.head ORDER BY f.position"
        ).fetchall()


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
    # A 4:1 profile image for the Slide tab's Select Media…
    images = scratch / "Resources" / "Images"
    images.mkdir(parents=True)
    sky_rgb = (30, 110, 210)
    sky = images / "Native Sky.png"
    nw.write_png(
        sky, 400, 100, bytes([sky_rgb[2], sky_rgb[1], sky_rgb[0], 255]) * 40000
    )
    sky_hash = hashlib.sha256(sky.read_bytes()).digest()
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

        # Slides tab: rendered thumbnails in the narrowed left pane.
        nw.click(hwnd, int(89 * scale), int(113 * scale))
        time.sleep(1.0)
        width, height, bgra = capture("05-slides")
        black, white = thumbnail_ink(width, height, bgra, scale)
        print(f"thumbnail samples: black={black} white={white}")
        assert black > 500 and white > 10, "no rendered thumbnails"
        nw.click(hwnd, int(140 * scale), int(210 * scale))
        capture("06-slide-1-selected")
        # + appends an unlabeled slide ("Slide 3" caption); undo restores.
        nw.click(hwnd, int(20 * scale), height - int(63 * scale))
        capture("07-unlabeled-slide")
        keys(hwnd, "ctrl+z")

        # Format pane (M1-05g3a): the toolbar toggle docks it at the right.
        # Keyboard from slide 1's thumbnail: Tab past slide 2 and the Slide
        # and Text tabs reaches the Font trigger, four more the text Color
        # trigger; Enter opens it with the hex field focused. Red, then Tab
        # Space toggles Bold.
        width, height, bgra = capture("08-before-format")
        button = format_button(width, height, bgra, scale)
        assert button, "no Format toggle in the toolbar"
        nw.click(hwnd, *button)
        time.sleep(0.5)
        nw.click(hwnd, int(140 * scale), int(210 * scale))
        capture("09-format-pane")
        keys(hwnd, *["tab"] * 8, "enter")
        capture("10-color-popover")
        keys(hwnd, "ctrl+a")
        write(hwnd, "#FF0000")
        keys(hwnd, "enter", "tab", "space")
        width, height, bgra = capture("11-red-bold")
        red = red_ink(width, height, bgra, scale)
        print(f"red preview samples: {red}")
        assert red > 200, "preview did not turn red"
        keys(hwnd, "ctrl+s")
        time.sleep(1.5)
        assert payloads(library) == [expected], payloads(library)
        # Codec 1, mask bold (bit 1) + color (bit 5), then bold, RGB.
        stored = formats(library)
        assert stored == [(0, bytes([1, 0x22, 0, 1, 0xFF, 0, 0]))], stored
        capture("12-format-saved")

        # Whole-song selection (M1-05g3b): Ctrl+A on slide 1's thumbnail,
        # then the same keyboard path makes every slide's text green.
        nw.click(hwnd, int(140 * scale), int(210 * scale))
        keys(hwnd, "ctrl+a")
        capture("13-all-selected")
        # Ctrl+A shows the Slide tab (EW8-OBS-035): Enter on the Text tab,
        # then Font and four more to Color.
        keys(hwnd, *["tab"] * 3, "enter", *["tab"] * 5, "enter", "ctrl+a")
        write(hwnd, "#00FF00")
        keys(hwnd, "enter", "ctrl+s")
        time.sleep(1.5)
        capture("14-all-green")
        assert payloads(library) == [expected], payloads(library)
        stored = formats(library)
        assert stored == [
            (0, bytes([1, 0x22, 0, 1, 0, 0xFF, 0])),
            (1, bytes([1, 0x20, 0, 0, 0xFF, 0])),
        ], stored

        # Slide backgrounds (M1-05h3). From slide 1's thumbnail: slide 2, the
        # Slide tab (Enter), the Text tab, then Fill ▾ whose rows are Master,
        # None, Color Fill, Gradient Fill (unavailable), Media Fill.
        nw.click(hwnd, int(140 * scale), int(210 * scale))
        keys(
            hwnd, "tab", "tab", "enter", "tab", "tab", "enter", "down", "down", "enter"
        )
        keys(hwnd, "tab", "enter", "ctrl+a")
        write(hwnd, "#FFAA00")
        keys(hwnd, "enter")
        width, height, bgra = capture("16-color-fill")
        orange = ink(width, height, bgra, scale, (255, 170, 0))
        print(f"orange preview samples: {orange}")
        assert orange > 2000, "preview did not show the Color Fill"
        keys(hwnd, "ctrl+s")
        time.sleep(1.5)
        # Codec 1, Color tag 1, RGB, aspect Zoom (2).
        slides, master = backgrounds(library)
        assert slides == [(0, bytes([1, 1, 0xFF, 0xAA, 0, 2]))], slides
        assert master == [], master

        # Back to Fill ▾, Down skips Gradient Fill to Media Fill; Select
        # Media… lists the profile image and Enter picks it (Zoom).
        keys(hwnd, "shift+tab", "enter", "down", "enter", "tab", "enter")
        time.sleep(1.5)
        capture("17-select-media")
        keys(hwnd, "enter")
        time.sleep(1.0)
        width, height, bgra = capture("18-media-zoom")
        zoom_sky = ink(width, height, bgra, scale, sky_rgb)
        zoom_black = ink(width, height, bgra, scale, (0, 0, 0))
        print(f"zoom preview samples: sky={zoom_sky} black={zoom_black}")
        assert zoom_sky > 2000, "preview did not show the image"
        # Aspect Ratio ▾ (Zoom checked): Up Up is Maintain.
        keys(hwnd, "tab", "enter", "up", "up", "enter")
        width, height, bgra = capture("19-media-maintain")
        bars = ink(width, height, bgra, scale, (0, 0, 0))
        print(f"maintain preview samples: black={bars}")
        assert bars > zoom_black + 2000, "Maintain shows no black bars"
        keys(hwnd, "ctrl+s")
        time.sleep(1.5)
        name = b"Native Sky.png"
        image = bytes([1, 3, len(name)]) + name + sky_hash + bytes([0])
        slides, master = backgrounds(library)
        assert slides == [(0, image)], slides

        # Edit Slide Layouts focuses Fill ▾ for the master (None, Color
        # Fill, ...); slide 2 has no background, so its thumbnail follows.
        keys(hwnd, "tab", "enter")
        time.sleep(0.5)
        keys(hwnd, "enter", "down", "enter", "tab", "enter", "ctrl+a")
        write(hwnd, "#2040A0")
        keys(hwnd, "enter")
        width, height, bgra = capture("20-layouts-master")
        navy = ink(width, height, bgra, scale, (0x20, 0x40, 0xA0))
        print(f"master preview samples: {navy}")
        assert navy > 2000, "the Layouts preview did not show the master"
        nw.click(hwnd, int(89 * scale), int(113 * scale))
        time.sleep(1.5)
        width, height, bgra = capture("21-master-on-slide-2")
        navy = ink(width, height, bgra, scale, (0x20, 0x40, 0xA0), thumbnails=True)
        print(f"master thumbnail samples: {navy}")
        assert navy > 300, "slide 2's thumbnail does not follow the master"
        keys(hwnd, "ctrl+s")
        time.sleep(1.5)
        slides, master = backgrounds(library)
        assert slides == [(0, image)], slides
        assert master == [bytes([1, 1, 0x20, 0x40, 0xA0, 2])], master

        # Double click on the preview returns to Words with the caret in
        # slide 1; typing over Ctrl+A leaves one unlabeled slide.
        middle = (int(270 * scale) + width - int(290 * scale)) // 2
        nw.click(hwnd, middle, height // 2)
        nw.click(hwnd, middle, height // 2)
        time.sleep(0.5)
        keys(hwnd, "ctrl+a")
        write(hwnd, "Hallelujah")
        capture("15-typed-over")
        keys(hwnd, "ctrl+s")
        time.sleep(1.5)
        replaced = ["Native Hymn", "", "", "", "", "Hallelujah"]
        assert payloads(library) == [replaced], payloads(library)
        assert formats(library) == [stored[0]], formats(library)
        assert backgrounds(library) == (slides, master), backgrounds(library)
        keys(hwnd, "ctrl+q")
        code = app.wait_exit()
        assert code == 0, code
        assert payloads(library) == [replaced]
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
