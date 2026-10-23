#!/usr/bin/env python3
"""Linux/X11 native check: the Song Editor's Words list, preview and save.

Port of scripts/song-editor-windows.py on the shared harness
(scripts/native_x11.py, docs/e2e-linux.md).

Usage: python3 scripts/song-editor-x11.py [--display :100 | --kind xvfb|dual]
       [--binary target/debug/sela] [--out DIR] [--build]

Launches `sela --library` on a new library with a private profile. By keyboard
(and clicks into the toolbar Title, the Slides tab, thumbnails and the Format
toggle), while checking the editor keeps the input focus: the window opens as
"Song Editor - Untitled" with the caret in slide 1's label; Enter moves from
the label to the lyrics; Enter inside the lyrics is a line break; Ctrl+Enter
starts an unlabeled slide 2; Up moves to its label, where "Chorus" is typed.
The captured preview must show rendered text (near-white pixels on the black
slide). Home, Ctrl+Enter then Backspace split slide 2 at its start and join it
back. The title is typed, Ctrl+S saves and the stored revision is decoded from
SQLite (labels and lyrics per slide); the window title follows the song
title. Clicking the Slides tab must show rendered thumbnails; clicking slide 1
selects it, the + appends an unlabeled "Slide 3" and Ctrl+Z removes it again.
The toolbar Format toggle docks the Format pane; by Tab, Enter and a typed
#FF0000 the text Color of slide 1 turns red in the preview, Tab Space turns
Bold on, Ctrl+S stores exactly that format for slide 1 only. Ctrl+A on slide
1's thumbnail selects the whole song; the same Color path then stores green
for both slides. Slide backgrounds (M1-05h3), by keyboard in the Slide tab:
Fill Color Fill #FFAA00 turns slide 1's preview orange; Media Fill, Select
Media... (Right from Videos to Images) and Enter pick a seeded 4:1 profile image (Zoom covers the preview),
Aspect Ratio Maintain letterboxes it in black; Edit Slide Layouts sets the
master to #2040A0, which slide 2's thumbnail then shows; each step is saved
and decoded from SQLite. A double click on the preview returns to Words, where
typing over Ctrl+A leaves one unlabeled slide with slide 1's format and
background. Unchanged after saving that, Ctrl+Q closes without a guard.

X11 differences: Xvfb runs at scale 1.0 (96 dpi), so the Windows logical
coordinates are used unscaled; xdotool types ASCII through keysyms instead
of KEYEVENTF_UNICODE (all typed text here is ASCII); the double click is one
xdotool click with --repeat 2 instead of two separate SendInput clicks.
"""

from __future__ import annotations

import hashlib
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

SKY_RGB = (30, 110, 210)


def preview_ink(image: nx.Image) -> tuple[int, int]:
    """(black, near-white) samples in the right half, where the slide preview sits."""
    w, h = image.width, image.height
    region = (w // 2, h // 5, w - 8, h * 4 // 5)
    black = image.count(
        lambda p: p[0] < 12 and p[1] < 12 and p[2] < 12, step=3, region=region
    )
    white = image.count(
        lambda p: p[0] > 235 and p[1] > 235 and p[2] > 235, step=3, region=region
    )
    return black, white


def thumbnail_ink(image: nx.Image, scale: float) -> tuple[int, int]:
    """(black, white) samples in the Slides tab's thumbnail column. White is
    strict because the pane itself is #fbfbfa."""
    region = (
        int(40 * scale),
        int(130 * scale),
        int(240 * scale),
        min(image.height, int(450 * scale)),
    )
    black = image.count(lambda p: p[0] < 12 and p[1] < 12 and p[2] < 12, region=region)
    white = image.count(
        lambda p: p[0] > 253 and p[1] > 253 and p[2] > 253, region=region
    )
    return black, white


def format_button(image: nx.Image, scale: float) -> tuple[int, int] | None:
    """Center of the toolbar's Format toggle: the only dark (enabled) label
    between the insert groups and Library/Inspector; Animate and
    Presentation beside it are grey."""
    region = (
        int(320 * scale),
        int(24 * scale),
        image.width - int(200 * scale),
        int(46 * scale),
    )
    points = [(x, y) for x, y, p in image.samples(1, region) if max(p) < 0x70]
    if not points:
        return None
    return sum(x for x, _ in points) // len(points), sum(y for _, y in points) // len(
        points
    )


def preview_region(
    image: nx.Image, scale: float, thumbnails: bool = False
) -> tuple[int, int, int, int]:
    """Between the Slides-tab pane and the Format pane, or the thumbnail column."""
    left, right = (
        (int(40 * scale), int(240 * scale))
        if thumbnails
        else (int(270 * scale), image.width - int(290 * scale))
    )
    return left, int(110 * scale), right, image.height - int(80 * scale)


def red_ink(image: nx.Image, scale: float) -> int:
    return image.count(
        lambda p: p[0] > 200 and p[1] < 70 and p[2] < 70,
        region=preview_region(image, scale),
    )


def ink(
    image: nx.Image, scale: float, rgb: tuple[int, int, int], thumbnails: bool = False
) -> int:
    """Samples within 8 of `rgb` in the preview or the thumbnail column."""
    return image.count(
        rgb, tolerance=8, region=preview_region(image, scale, thumbnails)
    )


def body(session: nx.Session) -> str:
    summary = session.summary
    scratch = session.temp("sela-editor-")
    library = scratch / "library.sqlite"
    # A 4:1 profile image for the Slide tab's Select Media...
    sky = nx.solid_png(
        scratch / "Resources" / "Images" / "Native Sky.png", 400, 100, SKY_RGB
    )
    sky_hash = hashlib.sha256(sky.read_bytes()).digest()
    app = session.app(["--library", library], name="editor", scratch=scratch)
    hwnd = app.window()
    time.sleep(1.5)
    title = nx.window_name(hwnd)
    assert title == "Song Editor - Untitled", title
    nx.activate(hwnd)
    scale = 1.0  # Xvfb: 96 dpi, GPUI scale factor 1 (see the operator log)
    summary["editor_client"] = [nx.geometry(hwnd).width, nx.geometry(hwnd).height]

    def capture(name: str) -> nx.Image:
        time.sleep(0.8)
        return session.save(nx.capture_stable(hwnd), name)

    def keys(*sequence: str) -> None:
        nx.press(hwnd, *sequence, pause=0.15)

    def write(text: str) -> None:
        nx.type_text(hwnd, text, pause=0.25)

    def click(x: float, y: float) -> None:
        nx.click(hwnd, int(x), int(y))

    capture("00-new-song")
    write("Verse 1")
    keys("enter")
    write("Amazing grace")
    keys("enter")
    write("how sweet the sound")
    keys("ctrl+enter")
    write("That saved a wretch")
    keys("up")
    write("Chorus")
    keys("down")
    image = capture("01-words")
    black, white = preview_ink(image)
    summary["preview_samples"] = [black, white]
    assert black > 2000 and white > 40, (
        f"preview shows no rendered slide text ({black}, {white})"
    )

    # Split slide 2 at its start (an empty slide 2, the text in slide 3), then
    # Backspace at slide 3's start joins it back.
    keys("home", "ctrl+enter")
    capture("02-split")
    keys("backspace")
    capture("03-joined")

    click(80 * scale, 20 * scale)
    time.sleep(0.3)
    write("Native Hymn")
    keys("ctrl+s")
    time.sleep(1.5)
    saved = nx.song_payloads(library)
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
    assert nx.window_name(hwnd) == "Song Editor - Native Hymn", nx.window_name(hwnd)
    capture("04-saved")

    # Slides tab: rendered thumbnails in the narrowed left pane.
    click(89 * scale, 113 * scale)
    time.sleep(1.0)
    image = capture("05-slides")
    black, white = thumbnail_ink(image, scale)
    summary["thumbnail_samples"] = [black, white]
    assert black > 500 and white > 10, f"no rendered thumbnails ({black}, {white})"
    click(140 * scale, 210 * scale)
    capture("06-slide-1-selected")
    # + appends an unlabeled slide ("Slide 3" caption); undo restores.
    click(20 * scale, image.height - 63 * scale)
    capture("07-unlabeled-slide")
    keys("ctrl+z")

    # Format pane (M1-05g3a): the toolbar toggle docks it at the right. From
    # slide 1's thumbnail: Tab past slide 2 and the Slide and Text tabs
    # reaches the Font trigger, four more the text Color trigger; Enter opens
    # it with the hex field focused. Red, then Tab Space toggles Bold.
    image = capture("08-before-format")
    button = format_button(image, scale)
    assert button, "no Format toggle in the toolbar"
    click(*button)
    time.sleep(0.5)
    click(140 * scale, 210 * scale)
    capture("09-format-pane")
    keys(*["tab"] * 8, "enter")
    capture("10-color-popover")
    keys("ctrl+a")
    write("#FF0000")
    keys("enter", "tab", "space")
    image = capture("11-red-bold")
    red = red_ink(image, scale)
    summary["red_samples"] = red
    assert red > 200, f"preview did not turn red ({red})"
    keys("ctrl+s")
    time.sleep(1.5)
    assert nx.song_payloads(library) == [expected], nx.song_payloads(library)
    # Codec 1, mask bold (bit 1) + color (bit 5), then bold, RGB.
    stored = nx.section_formats(library)
    assert stored == [(0, bytes([1, 0x22, 0, 1, 0xFF, 0, 0]))], stored
    capture("12-format-saved")

    # Whole-song selection (M1-05g3b): Ctrl+A on slide 1's thumbnail, then
    # the same keyboard path makes every slide's text green.
    click(140 * scale, 210 * scale)
    keys("ctrl+a")
    capture("13-all-selected")
    # Ctrl+A shows the Slide tab (EW8-OBS-035): Enter on the Text tab, then
    # Font and four more to Color.
    keys(*["tab"] * 3, "enter", *["tab"] * 5, "enter", "ctrl+a")
    write("#00FF00")
    keys("enter", "ctrl+s")
    time.sleep(1.5)
    capture("14-all-green")
    assert nx.song_payloads(library) == [expected], nx.song_payloads(library)
    stored = nx.section_formats(library)
    assert stored == [
        (0, bytes([1, 0x22, 0, 1, 0, 0xFF, 0])),
        (1, bytes([1, 0x20, 0, 0, 0xFF, 0])),
    ], stored

    # Slide backgrounds (M1-05h3). From slide 1's thumbnail: slide 2, the
    # Slide tab (Enter), the Text tab, then Fill whose rows are Master, None,
    # Color Fill, Gradient Fill (unavailable), Media Fill.
    click(140 * scale, 210 * scale)
    keys("tab", "tab", "enter", "tab", "tab", "enter", "down", "down", "enter")
    keys("tab", "enter", "ctrl+a")
    write("#FFAA00")
    keys("enter")
    image = capture("16-color-fill")
    orange = ink(image, scale, (255, 170, 0))
    summary["orange_samples"] = orange
    assert orange > 2000, f"preview did not show the Color Fill ({orange})"
    keys("ctrl+s")
    time.sleep(1.5)
    # Codec 1, Color tag 1, RGB, aspect Zoom (2).
    slides, master = nx.backgrounds(library)
    assert slides == [(0, bytes([1, 1, 0xFF, 0xAA, 0, 2]))], slides
    assert master == [], master

    # Back to Fill, Down skips Gradient Fill to Media Fill; Select Media...
    # lists the profile image and Enter picks it (Zoom).
    # It opens on Videos (EW8-OBS-039, empty here); Right shows Images.
    keys("shift+tab", "enter", "down", "enter", "tab", "enter")
    time.sleep(1.0)
    keys("right")
    time.sleep(1.5)
    capture("17-select-media")
    keys("enter")
    time.sleep(1.0)
    image = capture("18-media-zoom")
    zoom_sky = ink(image, scale, SKY_RGB)
    zoom_black = ink(image, scale, (0, 0, 0))
    summary["zoom_samples"] = [zoom_sky, zoom_black]
    assert zoom_sky > 2000, f"preview did not show the image ({zoom_sky})"
    # Aspect Ratio (Zoom checked): Up Up is Maintain.
    keys("tab", "enter", "up", "up", "enter")
    image = capture("19-media-maintain")
    bars = ink(image, scale, (0, 0, 0))
    summary["maintain_black_samples"] = bars
    assert bars > zoom_black + 2000, (
        f"Maintain shows no black bars ({bars} vs {zoom_black})"
    )
    keys("ctrl+s")
    time.sleep(1.5)
    name = b"Native Sky.png"
    picture = bytes([1, 3, len(name)]) + name + sky_hash + bytes([0])
    slides, master = nx.backgrounds(library)
    assert slides == [(0, picture)], slides

    # Edit Slide Layouts focuses Fill for the master (None, Color Fill, ...);
    # slide 2 has no background, so its thumbnail follows.
    keys("tab", "enter")
    time.sleep(0.5)
    keys("enter", "down", "enter", "tab", "enter", "ctrl+a")
    write("#2040A0")
    keys("enter")
    image = capture("20-layouts-master")
    navy = ink(image, scale, (0x20, 0x40, 0xA0))
    summary["master_preview_samples"] = navy
    assert navy > 2000, f"the Layouts preview did not show the master ({navy})"
    click(89 * scale, 113 * scale)
    time.sleep(1.5)
    image = capture("21-master-on-slide-2")
    navy = ink(image, scale, (0x20, 0x40, 0xA0), thumbnails=True)
    summary["master_thumbnail_samples"] = navy
    assert navy > 300, f"slide 2's thumbnail does not follow the master ({navy})"
    keys("ctrl+s")
    time.sleep(1.5)
    slides, master = nx.backgrounds(library)
    assert slides == [(0, picture)], slides
    assert master == [bytes([1, 1, 0x20, 0x40, 0xA0, 2])], master

    # Double click on the preview returns to Words with the caret in slide 1;
    # typing over Ctrl+A leaves one unlabeled slide.
    left, _, right, _ = preview_region(image, scale)
    nx.double_click(hwnd, (left + right) // 2, image.height // 2)
    time.sleep(0.5)
    keys("ctrl+a")
    write("Hallelujah")
    capture("15-typed-over")
    keys("ctrl+s")
    time.sleep(1.5)
    replaced = ["Native Hymn", "", "", "", "", "Hallelujah"]
    assert nx.song_payloads(library) == [replaced], nx.song_payloads(library)
    assert nx.section_formats(library) == [stored[0]], nx.section_formats(library)
    assert nx.backgrounds(library) == (slides, master), nx.backgrounds(library)
    keys("ctrl+q")
    code = app.wait_exit()
    assert code == 0, code
    assert nx.song_payloads(library) == [replaced]
    return (
        "Words entry/split/join, save and window title, rendered Slides thumbnails, + and undo, Format pane "
        "red/bold and whole-song green, Color/Media/Maintain slide backgrounds and the layout master, double "
        "click back to Words and Ctrl+Q without a guard, each decoded from SQLite"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "song-editor-x11", seed=False))
