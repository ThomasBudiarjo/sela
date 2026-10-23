"""Native check: Songs Sort by keys, slide-label search and the Options › Song
Copyright Info group (songs-browser stream; Windows counterpart of
scripts/songs-browser-x11.py).

Through UI Automation and real input, on a library seeded with
`seed_library --sort-songs` (five original songs whose Title, Author,
Copyright, Administrator and Song Number orders all differ; one has the slide
label "Etsaer", another the title "Etsaer Praise"):

1. Title order on start.
2. Sort by > Author, Copyright and Administrator through the row context
   menu (mouse): the expected order, songs without the key last, and the
   choice ticked (UIA Toggle On) when the menu is reopened (EW8-OBS-064).
3. Sort by > Song Number and Descending by keyboard only (Shift+F10 on the
   focused row, Down x3, Right, Down x4, Enter; Descending is Down x7):
   numeric order, then reversed with songs without a number still last.
4. settings.txt holds `song_sort=song_number` and `song_sort_descending=1`;
   a restart lists in that order again.
5. Typing "etsa" in the Songs search box lists the title match first and the
   label-only match second; nothing goes Live.
6. Edit > Options > Song tab: tick "Show on first displayed slide only", OK
   (settings.txt `copyright_first_slide=1`). With Live output on the
   secondary monitor, "Echo Hymn" (copyright text) goes Live: slide 1 shows
   the copyright strip at the bottom of the audience frame, slide 2 does not.
   Then, with slide 2 live, Options › Song: untick "Show on first displayed
   slide only" and type "CCLI b." into License Info one virtual-key press
   per character (Shift+C, Shift+C, Shift+L, Shift+I, Space, B, VK_OEM_PERIOD;
   B and "." are the plain Black keys), OK: settings.txt has the change, and
   neither the typing nor OK changes the frame that is already live
   (settings apply to the next cue, PP-SB-07). Virtual-key presses, not
   nw.type_text: KEYEVENTF_UNICODE input arrives as VK_PACKET, which GPUI's
   Windows backend (zed a846890, crates/gpui_windows/src/events.rs
   `VK_PACKET => None`) never turns into a KeyDown, so it would skip the
   keymap and a leaked Black binding could not fail the check. A wrongful
   re-cue on OK is visible here: slide 2 would gain the strip once
   first-slide-only is unticked.

Takes over the real keyboard and mouse; never run with EasyWorship open.

Run: python scripts/songs-browser-windows.py [--out DIR] [--binary PATH]
     [--seed PATH] [--monitor secondary]

NOT RUN yet (written 2026-10-22 without a Windows host); see
docs/parity-pending.md, stream songs-browser.
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

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import native_win as nw


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


a11y = _load("a11y_windows", "a11y-windows.py")
rb = _load("resource_browser_windows", "resource-browser-windows.py")
sm = _load("song_metadata_windows", "song-metadata-windows.py")

TITLE = ["Alpha Hymn", "Bravo Hymn", "Delta Hymn", "Echo Hymn", "Etsaer Praise"]
AUTHOR = ["Echo Hymn", "Delta Hymn", "Bravo Hymn", "Etsaer Praise", "Alpha Hymn"]
COPYRIGHT = ["Echo Hymn", "Bravo Hymn", "Delta Hymn", "Etsaer Praise", "Alpha Hymn"]
ADMINISTRATOR = ["Echo Hymn", "Etsaer Praise", "Delta Hymn", "Alpha Hymn", "Bravo Hymn"]
NUMBER = ["Echo Hymn", "Bravo Hymn", "Etsaer Praise", "Delta Hymn", "Alpha Hymn"]
NUMBER_DESC = ["Delta Hymn", "Etsaer Praise", "Bravo Hymn", "Echo Hymn", "Alpha Hymn"]
LABEL_HITS = ["Etsaer Praise", "Echo Hymn"]
DOWNS_TO_SORT_BY = 3
DOWNS_TO_SONG_NUMBER = 4
DOWNS_TO_DESCENDING = 7
# VK_OEM_PERIOD (US layout); native_win.VK has no "." entry.
nw.VK.setdefault("period", 0xBE)


def press_keys(session, text: str) -> None:
    """Type text as one virtual-key press per character so GPUI's keymap sees
    every keystroke (WM_KEYDOWN, then WM_CHAR inserts the text). Covers the
    letters, digits, space and "." this script needs; uppercase uses Shift."""
    for char in text:
        if char == " ":
            key = "space"
        elif char == ".":
            key = "period"
        elif char.isupper():
            key = f"shift+{char.lower()}"
        elif char.isalnum():
            key = char
        else:
            raise ValueError(f"press_keys has no key for {char!r}")
        session.press(key, pause=0.1)


def browser_run(args, summary: dict, check) -> None:
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="sela-songs-browser-"))
    library = scratch / "library.sqlite"
    settings = scratch / "settings.txt"
    subprocess.run([str(args.seed), str(library), "--sort-songs"], check=True)
    session = rb.Session(args, summary, check, scratch, library)
    try:
        session.wait("Title order on start", lambda d: rb.titles(d) == TITLE)

        def choose(aid: str) -> None:
            session.click_aid("song-0", right=True)
            time.sleep(0.2)
            session.click_aid("song-menu-sort")
            time.sleep(0.2)
            session.click_aid(aid)

        for aid, want in (
            ("song-sort-author", AUTHOR),
            ("song-sort-copyright", COPYRIGHT),
            ("song-sort-administrator", ADMINISTRATOR),
        ):
            choose(aid)
            doc = session.wait(
                f"{aid} orders the rows", lambda d, w=want: rb.titles(d) == w
            )
            summary[aid] = rb.titles(doc)
            session.save(f"operator-{aid}")
            session.click_aid("song-0", right=True)
            session.click_aid("song-menu-sort")
            time.sleep(0.3)
            tick = a11y.nodes_by_id(session.doc()).get(aid, {})
            check.expect(
                tick.get("toggle") == "On", f"{aid} is ticked: {tick.get('toggle')}"
            )
            session.press("escape")

        session.click_aid("song-0")
        session.press("shift+f10", pause=0.4)
        for _ in range(DOWNS_TO_SORT_BY):
            session.press("down")
        session.press("right")
        for _ in range(DOWNS_TO_SONG_NUMBER):
            session.press("down")
        session.press("enter", pause=0.6)
        session.wait("keyboard Sort by > Song Number", lambda d: rb.titles(d) == NUMBER)
        session.press("shift+f10", pause=0.4)
        for _ in range(DOWNS_TO_SORT_BY):
            session.press("down")
        session.press("right")
        for _ in range(DOWNS_TO_DESCENDING):
            session.press("down")
        session.press("enter", pause=0.6)
        session.wait("keyboard Descending", lambda d: rb.titles(d) == NUMBER_DESC)
        time.sleep(0.5)
        text = settings.read_text() if settings.exists() else ""
        check.expect(
            "song_sort=song_number\nsong_sort_descending=1\n" in text,
            f"the order is saved: {text.splitlines()}",
        )

        session.click_aid("song-search")
        nw.type_text(session.hwnd, "etsa")
        doc = session.wait("label search 'etsa'", lambda d: rb.titles(d) == LABEL_HITS)
        session.save("operator-label-search")
        check.expect(
            "live-slide-0" not in a11y.nodes_by_id(doc),
            "typing never sends anything Live",
        )
        session.press("escape")
        # Alt+mnemonics never fire from a text field (`!SelaTextInput`).
        session.click_aid("song-0")

        session.press("alt+e")
        session.press("enter", pause=0.8)
        session.click_aid("options-tab-song")
        found = a11y.nodes_by_id(session.doc())
        for aid in (
            "options-copyright-display",
            "options-copyright-first-slide",
            "options-license-info",
        ):
            check.expect(aid in found, f"the Song tab shows {aid}")
        check.expect(
            found.get("options-copyright-display", {}).get("toggle") == "On",
            "Display Copyright Information starts on (EW default)",
        )
        session.click_aid("options-copyright-first-slide")
        session.save("options-song-tab")
        session.click_aid("dialog-confirm")
        time.sleep(0.6)
        check.expect(
            "copyright_first_slide=1\n" in settings.read_text(),
            "OK saves copyright_first_slide=1",
        )

        a11y.show_live_output(session.hwnd)
        audience = sm.Audience(session)
        audience.settled()
        session.click_aid("song-search")
        nw.type_text(session.hwnd, "echo")
        session.wait("Echo Hymn listed", lambda d: rb.titles(d) == ["Echo Hymn"])
        session.press("escape")
        session.click_aid("song-0")
        time.sleep(0.8)  # the song loads into Preview off the UI thread
        session.click_aid("go-live")
        time.sleep(1.2)
        audience.settled()
        width, height, pixels = audience.frame()
        first = sm.strip_ink(pixels, width, height, 0.86, 0.99)
        audience.save(args.out, "audience-slide-1")
        session.click_aid("live-slide-1")
        time.sleep(1.0)
        audience.settled()
        width, height, pixels = audience.frame()
        second = sm.strip_ink(pixels, width, height, 0.86, 0.99)
        audience.save(args.out, "audience-slide-2")
        summary["strip_ink"] = {"slide_1": first, "slide_2": second}
        check.expect(first > 200, f"slide 1 draws the strip ({first})")
        check.expect(
            second < 20, f"slide 2 draws no strip with first-slide-only ({second})"
        )
        # A real change: apply_options returns early when nothing changed.
        changes = audience.changes(
            1.5,
            lambda: (
                session.press("alt+e"),
                session.press("enter", pause=0.6),
                session.click_aid("options-tab-song"),
                session.click_aid("options-copyright-first-slide"),
                session.click_aid("options-license-info"),
                press_keys(session, "CCLI b."),
                session.click_aid("dialog-confirm"),
            ),
        )
        text = settings.read_text()
        check.expect(
            "copyright_first_slide=1\n" not in text
            and "license_info=CCLI b.\n" in text,
            f"OK saved the changed options: {text.splitlines()}",
        )
        check.expect(
            changes == 0,
            f"typing B/. in License Info and OK leave the live frame alone ({changes})",
        )
        session.quit()

        session.app = nw.App(
            args.binary, ["--operator-library", str(library)], scratch=scratch
        )
        session.hwnd = session.app.window()
        nw.activate(session.hwnd)
        time.sleep(1.6)
        session.wait(
            "the stored order after a restart", lambda d: rb.titles(d) == NUMBER_DESC
        )
        session.quit()
    finally:
        session.app.close()


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
        default=ROOT / ".amp/in/artifacts/songs-browser-windows",
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
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    try:
        browser_run(args, summary, check)
        summary["status"] = "FAIL" if check.failures else "PASS"
        return 0 if summary["status"] == "PASS" else 1
    except Exception as error:  # noqa: BLE001
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))
        print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
