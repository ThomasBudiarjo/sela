#!/usr/bin/env python3
"""Linux/X11 native check: Songs Sort by keys, slide-label search and the
Options › Song Copyright Info group (songs-browser stream).

Built on the shared harness (scripts/native_x11.py, docs/e2e-linux.md).

Usage: python3 scripts/songs-browser-x11.py [--display :114 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR] [--build]

Seeds a new library with five original songs (`seed_library --sort-songs`)
whose Title, Author, Copyright, Administrator and Song Number orders all
differ; one has the slide label "Etsaer" and another the title "Etsaer
Praise". Launches the operator with a private profile and checks, reading the
Songs rows through AT-SPI (AccessKit ids `song-0`..`song-4` and their names):

1. Title order on start.
2. Sort by Author, Copyright and Administrator chosen with the mouse (right
   click a row, Sort by, the choice): each gives the expected order with
   songs that lack the key last (EW8-OBS-064; empty-last is a best guess).
3. Sort by Song Number chosen by keyboard only (Shift+F10 on the focused
   row, Down to Sort by, Right, Down to Song Number, Enter): numeric order
   (10 < 0042 < 77 < 900), then Descending by keyboard.
4. settings.txt in the profile holds `song_sort=song_number` and
   `song_sort_descending=1`; a second launch lists in that order again.
5. Live output on and the first listed song Live, so a leaked show-control
   key would change the captured audience frame. Typing "etsa" in the Songs
   search box lists the title match first and the label-only match ("Echo
   Hymn", label "Etsaer") second (EW8-OBS-045/046, schema 7 label index);
   typing "bravo." (the plain Black keys B and ".") lists Bravo Hymn (title)
   then Delta Hymn (author "bravo writer"); the audience frame never changes.
6. Edit › Options › Song tab: "Show on first displayed slide only" is ticked
   with the mouse, "CCLI b." is typed into License Info with real keys, and
   OK writes `copyright_first_slide=1` and `license_info=CCLI b.`; neither
   the typing nor OK changes the live audience frame (settings apply to the
   next cue).

Captures: operator after each sort, the search result, the Options Song tab,
the live audience frame.

X11 differences: the audience runs in its xvfb development window (624x351);
the copyright strip itself needs a 360 px tall output, so its drawing is
covered by GPUI tests and the Windows script, and here the audience frame is
only compared for change. The Songs menu is driven by AT-SPI positions for
the mouse path; the keyboard path uses Shift+F10 like the Windows script.
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

TITLE = ["Alpha Hymn", "Bravo Hymn", "Delta Hymn", "Echo Hymn", "Etsaer Praise"]
AUTHOR = ["Echo Hymn", "Delta Hymn", "Bravo Hymn", "Etsaer Praise", "Alpha Hymn"]
COPYRIGHT = ["Echo Hymn", "Bravo Hymn", "Delta Hymn", "Etsaer Praise", "Alpha Hymn"]
ADMINISTRATOR = ["Echo Hymn", "Etsaer Praise", "Delta Hymn", "Alpha Hymn", "Bravo Hymn"]
NUMBER = ["Echo Hymn", "Bravo Hymn", "Etsaer Praise", "Delta Hymn", "Alpha Hymn"]
NUMBER_DESC = ["Delta Hymn", "Etsaer Praise", "Bravo Hymn", "Echo Hymn", "Alpha Hymn"]
LABEL_HITS = ["Etsaer Praise", "Echo Hymn"]
# Songs menu lines with an empty schedule: New Song, Edit Song, Delete,
# (Update items in Schedule, disabled and skipped), Sort by.
DOWNS_TO_SORT_BY = 3
# Sort by choices after Right (highlight on Title): Title, Author, Copyright,
# Administrator, Song Number, Date modified, Ascending, Descending.
DOWNS_TO_SONG_NUMBER = 4
DOWNS_TO_DESCENDING = 7


def rows(app: nx.App, window: int) -> list[str]:
    """Song row names in list order, from the AT-SPI tree."""
    found = app.controls(window)
    names = []
    for index in range(10):
        control = found.get(f"song-{index}")
        if control is None:
            break
        names.append(control.name)
    return names


def wait_rows(app: nx.App, window: int, want: list[str], what: str) -> list[str]:
    def probe():
        shown = rows(app, window)
        return shown if shown == want else None

    try:
        return nx.wait_until(probe, 10, what)
    except nx.Failure as error:
        raise nx.Failure(
            f"{what}: rows {rows(app, window)}, expected {want}"
        ) from error


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    data = session.temp("sela-songs-browser-")
    library = nx.seed_library(data / "library.sqlite", "--sort-songs", seed=args.seed)
    settings = data / "settings.txt"
    app = session.app(["--operator-library", library], name="operator")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)  # the storage worker loads the Songs list off the UI thread

    summary["title"] = wait_rows(app, operator, TITLE, "Title order on start")

    def choose(choice: str) -> None:
        nx.right_click(operator, *app.control("song-0", operator).center)
        time.sleep(0.4)
        nx.click(operator, *app.control("song-menu-sort", operator).center)
        time.sleep(0.4)
        nx.click(operator, *app.control(choice, operator).center)

    for choice, want in (
        ("song-sort-author", AUTHOR),
        ("song-sort-copyright", COPYRIGHT),
        ("song-sort-administrator", ADMINISTRATOR),
    ):
        choose(choice)
        summary[choice] = wait_rows(app, operator, want, choice)
        time.sleep(0.3)
        session.save(operator, f"operator-{choice}")

    # Keyboard only: the focused row's Songs menu, Sort by, Song Number.
    nx.click(operator, *app.control("song-0", operator).center)
    time.sleep(0.3)
    nx.press(operator, "shift+F10")
    time.sleep(0.4)
    nx.press(operator, *["down"] * DOWNS_TO_SORT_BY, "right")
    nx.press(operator, *["down"] * DOWNS_TO_SONG_NUMBER, "enter")
    summary["song-sort-song-number"] = wait_rows(
        app, operator, NUMBER, "keyboard Sort by Song Number"
    )
    session.save(operator, "operator-song-sort-song-number")
    nx.press(operator, "shift+F10")
    time.sleep(0.4)
    nx.press(operator, *["down"] * DOWNS_TO_SORT_BY, "right")
    nx.press(operator, *["down"] * DOWNS_TO_DESCENDING, "enter")
    summary["song-sort-descending"] = wait_rows(
        app, operator, NUMBER_DESC, "keyboard Descending"
    )
    stored = nx.wait_until(
        lambda: (
            settings.exists()
            and "song_sort=song_number\nsong_sort_descending=1\n"
            in settings.read_text()
            and settings.read_text()
        ),
        5,
        "the Sort by choice in settings.txt",
    )
    summary["settings_after_sort"] = stored.splitlines()

    # Live output on with the first listed song Live: a show-control key
    # leaking from a text field would change this frame.
    nx.click(operator, *app.control("song-0", operator).center)
    time.sleep(0.8)  # the song loads into Preview off the UI thread
    nx.click(operator, *app.control("live-output", operator).center)
    found = app.audience(15)
    if not found:
        raise nx.Failure(f"no audience window within 15s: {app.log()}")
    _, screen = found
    time.sleep(1.5)
    if nx.focused() != operator:
        nx.activate(operator)
    blank = nx.capture_stable(screen).digest()
    nx.click(operator, *app.control("go-live", operator).center)
    live, _ = nx.wait_for_change(screen, blank, 5, "Go Live")
    if live.ink_box() is None:
        raise nx.Failure("the live slide shows no text")
    live_frame = live.digest()
    session.save(live, "audience-live")

    def unchanged(what: str, seconds: float = 1.0) -> None:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if nx.digest(screen) != live_frame:
                session.save(nx.capture_stable(screen), "audience-changed")
                raise nx.Failure(f"{what} changed the live audience frame")
            time.sleep(0.1)

    # Slide label search: the title match first, the label-only match next.
    nx.click(operator, *app.control("song-search", operator).center)
    time.sleep(0.3)
    nx.type_text(operator, "etsa")
    summary["label_search"] = wait_rows(
        app, operator, LABEL_HITS, "label search 'etsa'"
    )
    time.sleep(0.3)
    session.save(operator, "operator-label-search")
    unchanged("typing 'etsa' in the search box")
    nx.press(operator, "escape")
    wait_rows(app, operator, NUMBER_DESC, "Esc clears the search")
    nx.type_text(operator, "bravo.")
    summary["black_keys_search"] = wait_rows(
        app, operator, ["Bravo Hymn", "Delta Hymn"], "search 'bravo.'"
    )
    unchanged("typing B and '.' in the search box")
    summary["search_left_live_frame"] = True

    # Options › Song: first displayed slide only.
    nx.press(operator, "escape")  # clear the search; focus stays in the box
    wait_rows(app, operator, NUMBER_DESC, "Esc restores the sorted list")
    # Alt+mnemonics never fire from a text field (`!SelaTextInput`).
    nx.click(operator, *app.control("song-0", operator).center)
    time.sleep(0.3)
    nx.press(operator, "alt+e", "enter")
    time.sleep(0.6)
    nx.click(operator, *app.control("options-tab-song", operator).center)
    time.sleep(0.4)
    controls = app.controls(operator)
    for needed in (
        "options-copyright-display",
        "options-copyright-first-slide",
        "options-license-info",
    ):
        if needed not in controls:
            raise nx.Failure(f"the Options Song tab lacks {needed}")
    nx.click(operator, *controls["options-copyright-first-slide"].center)
    time.sleep(0.3)
    nx.click(operator, *controls["options-license-info"].center)
    time.sleep(0.3)
    nx.type_text(operator, "CCLI b.")
    unchanged("typing B and '.' in License Info")
    session.save(nx.capture_stable(operator), "options-song-tab")
    nx.click(operator, *app.control("dialog-confirm", operator).center)
    stored = nx.wait_until(
        lambda: (
            "copyright_first_slide=1\n" in settings.read_text()
            and "license_info=CCLI b.\n" in settings.read_text()
            and settings.read_text()
        ),
        5,
        "copyright_first_slide=1 and license_info in settings.txt",
    )
    summary["settings_after_options"] = stored.splitlines()
    if "copyright_display=0" in stored:
        raise nx.Failure("Display Copyright Information was turned off")
    unchanged("Options OK with changed copyright settings", 1.5)
    summary["options_left_live_frame"] = True

    nx.press(operator, "ctrl+q")
    summary["exit"] = app.wait_exit()
    if summary["exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['exit']}")
    session.close_app(app)

    # A second launch keeps the stored order.
    app = session.app(["--operator-library", library], name="reopened")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)
    summary["reopened"] = wait_rows(
        app, operator, NUMBER_DESC, "stored order on restart"
    )
    session.save(operator, "operator-reopened")
    nx.press(operator, "ctrl+q")
    summary["reopened_exit"] = app.wait_exit()
    if summary["reopened_exit"] != 0:
        raise nx.Failure(f"reopened operator exit status {summary['reopened_exit']}")
    return (
        "Sort by Author/Copyright/Administrator (mouse) and Song Number + Descending (keyboard) "
        "order the Songs rows as expected and persist across a restart; 'etsa' finds the title "
        "match then the slide-label match; Options › Song first-slide-only and License Info are "
        "saved; typing in both fields and OK leave the live audience frame unchanged"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "songs-browser-x11"))
