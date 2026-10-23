#!/usr/bin/env python3
"""Linux/X11 native E2E scenario S01 (backlog "Repeatable end-to-end
scenarios"): fresh offline profile -> create a song -> schedule -> save ->
close/reopen -> Go Live with the real audience process.

Usage: python3 scripts/scenario-s01-x11.py [--display :111 | --kind xvfb|dual]
       [--binary target/debug/sela] [--out DIR] [--build]

Starts `sela` with no arguments in a new, empty private profile (HOME and
XDG_* folders of one temporary directory, nothing seeded), so the operator
creates its library at XDG_DATA_HOME/sela/library.sqlite. Checks, in order:

1. Fresh profile: the window title is "Sela — Technical preview", the library
   file is created and holds no song, Songs shows no row. Edit › Options
   (Alt+E, Enter) is an AT-SPI `dialog` named "Options"; Esc closes it.
2. Create a song: toolbar New ▾ › New Song opens "Song Editor - Untitled" in
   the same process. By keyboard: two slides ("Verse 1": two lines, "Chorus":
   one line) and the title typed into the toolbar Title box; in the Slides
   tab the Format pane's Slide › Fill sets a Color Fill #FFAA00 on slide 1
   (the layout/asset stand-in: themes, M1-08, do not exist yet). A window
   manager close then shows the close prompt as a `dialog` named "Save
   changes before closing?" (Yes saves, No discards); Esc cancels it and the
   editor stays. Ctrl+S saves. The stored revision is decoded from SQLite (labels, lyrics, slide
   background). The editor closes by the window manager without a prompt.
3. Schedule: the new song appears as `song-0` in Songs; Add to Schedule makes
   the window title "Untitled schedule* — Sela". Ctrl+Q opens the
   unsaved-changes dialog, which the AT-SPI tree exposes as a `dialog` named
   with its question (SC2-A11Y-01); Cancel (focused) keeps the window.
4. Save: Ctrl+S opens the portal Save dialog; "s01-service" is typed and Sela
   adds .selaschedule. The file is read back: one item pinning the stored
   song id and revision; the title becomes "s01-service.selaschedule — Sela".
   A splitter drag makes the pane layout non-default.
5. Close: Ctrl+Q quits without asking (exit status 0).
6. Reopen: a second launch with the same profile restores the pane layout
   (the same AT-SPI rectangles for the Schedule list, Preview and Live), and
   File › Open Recent lists the schedule; choosing it shows exactly one item
   under the saved, clean title.
7. Go Live: Live output on starts the real audience process; selecting the
   item changes nothing there; Page Down sends it. The audience frame turns
   into the orange slide with near-white text, and the operator's Live status
   (AT-SPI `Live-status`) names the acknowledged slide ("On screen: S01 Hymn ·
   Verse 1"). Right (Go Live handed the keys to the Live pane) shows the
   Chorus on black and the status follows. Ctrl+Q quits and the audience
   process ends.

X11 differences: the file dialog is the GTK portal chooser on the run's
private D-Bus session; Alt+F4 (Openbox) stands in for WM_CLOSE; on the default
xvfb display the audience is the 624x351 development window beside the
operator, not a monitor-covering window (use --kind dual for that). Fonts are
DejaVu (no Arial). This is not EasyWorship parity, hardware or timing
evidence.
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

TITLE = "S01 Hymn"
NAME = "s01-service"
ORANGE = (0xFF, 0xAA, 0x00)
IDLE_TITLE = "Sela — Technical preview"
UNSAVED_QUESTION = "The current Schedule is not saved. Would you like to save it now?"
# Yes saves and closes, No discards (EW8-OBS-012), so the editor asks to save.
CLOSE_QUESTION = "Save changes before closing?"
LYRICS = [
    "Verse 1",
    "Morning light on quiet water\nevery shore awake and singing",
    "Chorus",
    "Lift the morning song",
]


def songs(library: Path) -> list[tuple[str, int]]:
    """(id hex, head revision) of every live song."""
    if not library.exists():
        return []
    with nx.sqlite_ro(library) as db:
        return db.execute(
            "SELECT lower(hex(id)), head FROM songs WHERE deleted=0 ORDER BY id"
        ).fetchall()


def format_button(image: nx.Image) -> tuple[int, int] | None:
    """The editor toolbar's Format toggle: the only dark (enabled) label
    between the insert groups and Library/Inspector (song-editor-x11.py)."""
    region = (320, 24, image.width - 200, 46)
    points = [(x, y) for x, y, p in image.samples(1, region) if max(p) < 0x70]
    if not points:
        return None
    return (
        sum(x for x, _ in points) // len(points),
        sum(y for _, y in points) // len(points),
    )


def pane_rects(app: nx.App, wid: int) -> dict[str, list[int]]:
    """Preview and Live rectangles plus the Schedule list's x and width (its
    height shrinks while the pane shows a status line such as "Saved")."""
    found = app.controls(wid)
    rects = {
        aid: [found[aid].x, found[aid].y, found[aid].width, found[aid].height]
        for aid in ("Preview", "Live")
    }
    rects["schedule-list"] = [found["schedule-list"].x, found["schedule-list"].width]
    return rects


def closed(wid: int) -> bool:
    """The window is gone or unmapped (it may vanish between two queries)."""
    return not nx.viewable(wid)


def close_saved_editor(app: nx.App, editor: int) -> None:
    """Window-manager close of an editor whose save landed; a close prompt
    here would mean the editor still counted its document as unsaved."""
    time.sleep(1.0)  # the editor takes the save reply off its worker
    nx.wm_close(editor)

    def probe():
        if closed(editor):
            return True
        try:
            prompted = "close-prompt" in app.controls(editor, timeout=2)
        except nx.Failure:
            prompted = False
        if prompted:
            raise nx.Failure("closing the saved editor asked to save")
        return False

    nx.wait_until(probe, 10, "the saved editor to close without a prompt")


def live_status(app: nx.App, wid: int) -> str:
    return app.controls(wid)["Live-status"].name


def wait_status(app: nx.App, wid: int, text: str, what: str) -> str:
    def probe():
        status = live_status(app, wid)
        return status if text in status else None

    return nx.wait_until(probe, 5, what)


def create_song(
    session: nx.Session, app: nx.App, operator: int, library: Path
) -> None:
    """New Song in an editor window of the same process, saved with a slide
    background."""
    summary = session.summary
    nx.click(operator, *app.control("new-menu", operator).center)
    time.sleep(0.5)
    nx.click(operator, *app.control("new-song-menu", operator).center)
    editor = app.new_window([operator], 15)
    time.sleep(1.5)
    title = nx.window_name(editor)
    if title != "Song Editor - Untitled":
        raise nx.Failure(f"New Song opened {title!r}")
    nx.activate(editor)

    def keys(*sequence: str) -> None:
        nx.press(editor, *sequence, pause=0.15)

    def write(text: str) -> None:
        nx.type_text(editor, text, pause=0.25)

    def capture(name: str) -> nx.Image:
        time.sleep(0.8)
        return session.save(nx.capture_stable(editor), name)

    # EW8-OBS-023: the caret starts in slide 1's label.
    write(LYRICS[0])
    keys("enter")
    first, second = LYRICS[1].split("\n")
    write(first)
    keys("enter")
    write(second)
    keys("ctrl+enter")
    write(LYRICS[3])
    keys("up")
    write(LYRICS[2])
    keys("down")
    nx.click(editor, 80, 20)
    time.sleep(0.3)
    write(TITLE)
    capture("02-editor-words")

    # Slide 1 background: Slides tab, slide 1, Format pane, then Slide ›
    # Fill › Color Fill #FFAA00 (the keyboard path of song-editor-x11.py).
    nx.click(editor, 89, 113)
    time.sleep(1.0)
    nx.click(editor, 140, 210)
    image = capture("03-editor-slides")
    button = format_button(image)
    if not button:
        raise nx.Failure("no Format toggle in the editor toolbar")
    nx.click(editor, *button)
    time.sleep(0.5)
    nx.click(editor, 140, 210)
    keys("tab", "tab", "enter", "tab", "tab", "enter", "down", "down", "enter")
    keys("tab", "enter", "ctrl+a")
    write("#FFAA00")
    keys("enter")
    image = capture("04-editor-orange")
    orange = image.count(ORANGE, tolerance=8)
    summary["editor_orange_samples"] = orange
    if orange < 2000:
        raise nx.Failure(f"the editor preview shows no Color Fill ({orange})")
    # Closing with unsaved edits asks whether to save, as an announced dialog
    # (SC2-A11Y-01, SC2-I18N-02); Esc is Cancel and keeps the editor.
    nx.wm_close(editor)
    prompt = nx.wait_until(
        lambda: app.controls(editor).get("close-prompt"), 10, "the close prompt"
    )
    summary["close_prompt"] = [prompt.role, prompt.name]
    if (prompt.role, prompt.name) != ("dialog", CLOSE_QUESTION):
        raise nx.Failure(f"the close prompt is not announced: {summary['close_prompt']}")
    capture("04b-editor-close-prompt")
    keys("escape")

    def prompt_gone() -> bool:
        # Without the prompt the editor exposes no author ids at all, which
        # controls() reports as a timeout.
        try:
            return "close-prompt" not in app.controls(editor, timeout=2)
        except nx.Failure:
            return True

    nx.wait_until(prompt_gone, 5, "Esc to cancel the close prompt")
    if closed(editor):
        raise nx.Failure("Esc on the close prompt closed the editor")
    keys("ctrl+s")
    expected = [TITLE, "", "", "", *LYRICS]
    stored = nx.wait_until(
        lambda: [p for p in nx.song_payloads(library) if p[0] == TITLE],
        10,
        "Ctrl+S to store the new song",
    )
    if stored != [expected]:
        raise nx.Failure(f"unexpected stored song {stored}")
    slides, master = nx.backgrounds(library)
    # Codec 1, Color tag 1, RGB, aspect Zoom (2); slide 2 has none.
    if slides != [(0, bytes([1, 1, 0xFF, 0xAA, 0, 2]))] or master != []:
        raise nx.Failure(f"unexpected stored backgrounds {slides} {master}")
    summary["editor_title"] = nx.window_name(editor)
    capture("05-editor-saved")
    close_saved_editor(app, editor)
    nx.activate(operator)


def body(session: nx.Session) -> str:
    summary = session.summary
    profile = session.temp("sela-s01-profile-")
    library = profile / "data" / "sela" / "library.sqlite"

    # 1. Fresh profile: nothing seeded, the operator creates its library.
    app = session.app([], name="operator", scratch=profile)
    operator = app.window()
    nx.activate(operator)
    nx.wait_until(library.exists, 10, "the operator to create its library")
    time.sleep(1.0)
    summary["fresh_title"] = nx.window_name(operator)
    if summary["fresh_title"] != IDLE_TITLE:
        raise nx.Failure(f"fresh title {summary['fresh_title']!r}")
    if songs(library) or "song-0" in app.controls(operator):
        raise nx.Failure("a fresh profile already has songs")
    session.save(operator, "01-fresh")
    # Options (Edit › Options…, Alt+E) is an announced dialog (SC2-A11Y-01).
    nx.press(operator, "alt+e", "enter", pause=0.4)
    options = nx.wait_until(
        lambda: app.controls(operator).get("options-dialog"), 5, "the Options dialog"
    )
    summary["options_dialog"] = [options.role, options.name]
    if (options.role, options.name) != ("dialog", "Options"):
        raise nx.Failure(f"Options is not an announced dialog: {summary['options_dialog']}")
    nx.press(operator, "escape")
    nx.wait_until(
        lambda: "options-dialog" not in app.controls(operator), 5, "Esc to close Options"
    )

    # 2. Create a song.
    create_song(session, app, operator, library)
    [(song_id, revision)] = songs(library)
    summary["song"] = [song_id, revision]

    # 3. Schedule it; the unsaved prompt is an announced dialog.
    row = nx.wait_until(
        lambda: app.controls(operator).get("song-0"), 10, "the new song in Songs"
    )
    nx.click(operator, *row.center)
    time.sleep(0.5)
    nx.click(operator, *app.control("add-to-schedule", operator).center)
    nx.wait_until(
        lambda: nx.window_name(operator) == "Untitled schedule* — Sela",
        5,
        "the dirty untitled title",
    )
    nx.press(operator, "ctrl+q")
    time.sleep(0.8)
    dialog = app.control("schedule-dialog", operator)
    summary["unsaved_dialog"] = [dialog.role, dialog.name]
    if (dialog.role, dialog.name) != ("dialog", UNSAVED_QUESTION):
        raise nx.Failure(
            f"the unsaved prompt is not an announced dialog: {summary['unsaved_dialog']}"
        )
    session.save(operator, "06-unsaved-dialog")
    nx.press(operator, "enter")  # Cancel has the focus
    time.sleep(0.5)
    if not app.running() or "schedule-dialog" in app.controls(operator):
        raise nx.Failure("Cancel did not keep the window")

    # 4. Save through the portal dialog.
    nx.press(operator, "ctrl+s")
    time.sleep(0.8)
    session.save(nx.capture_stable(app.file_dialog()), "07-save-dialog")
    app.drive_file_dialog(profile / NAME, owner=operator)
    schedule_file = profile / f"{NAME}.selaschedule"
    document = nx.wait_until(
        lambda: nx.read_schedule(schedule_file), 10, "the saved schedule file"
    )
    items = [[i["song_id"], i["revision"]] for i in document["items"]]
    summary["saved"] = [document["title"], items]
    if document["title"] != NAME or items != [[song_id, revision]]:
        raise nx.Failure(f"unexpected saved schedule {summary['saved']}")
    nx.wait_until(
        lambda: nx.window_name(operator) == f"{NAME}.selaschedule — Sela",
        5,
        "the saved title",
    )
    # A non-default layout: drag the Schedule/Preview splitter right.
    before = pane_rects(app, operator)
    split_x = before["Preview"][0] - 3
    nx.drag(operator, (split_x, 300), (split_x + 80, 300))
    time.sleep(0.5)
    layout = pane_rects(app, operator)
    summary["layout"] = layout
    if layout == before:
        raise nx.Failure("the splitter drag did not change the layout")
    session.save(operator, "08-saved")

    # 5. Close.
    nx.press(operator, "ctrl+q")
    summary["first_exit"] = app.wait_exit()
    if summary["first_exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['first_exit']}")
    session.close_app(app)

    # 6. Reopen with the same profile.
    app = session.app([], name="reopened", scratch=profile)
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.5)
    reopened = pane_rects(app, operator)
    summary["reopened_layout"] = reopened
    if reopened != layout:
        raise nx.Failure(f"the layout was not restored: {layout} -> {reopened}")
    nx.press(operator, "alt+f")
    time.sleep(0.5)
    recent = app.control("bar-file-recent-0", operator)
    summary["recent"] = recent.name
    if NAME not in recent.name:
        raise nx.Failure(f"Open Recent does not list the schedule: {recent.name!r}")
    nx.click(operator, *recent.center)
    nx.wait_until(
        lambda: nx.window_name(operator) == f"{NAME}.selaschedule — Sela",
        10,
        "the reopened schedule under its clean title",
    )
    item = nx.wait_until(
        lambda: app.controls(operator).get("schedule-item-0"), 5, "the reopened item"
    )
    if "schedule-item-1" in app.controls(operator):
        raise nx.Failure("the reopened schedule has more than one item")
    session.save(operator, "09-reopened")

    # 7. Go Live with the real audience process.
    nx.click(operator, *app.control("live-output", operator).center)
    found = app.audience(20)
    if not found:
        raise nx.Failure(f"no audience window within 20s: {app.log()}")
    pid, screen = found
    time.sleep(1.5)
    nx.activate(operator)
    blank = nx.digest(screen)
    nx.click(operator, *item.center)
    time.sleep(1.0)
    if nx.digest(screen) != blank:
        raise nx.Failure("selecting the item changed the audience output")
    nx.press(operator, "pagedown")
    _, summary["verse_observed_ms"] = nx.wait_for_change(
        screen, blank, 5.0, "the reopened item going live"
    )
    image = session.save(nx.capture_stable(screen), "10-audience-verse")
    orange = image.count(ORANGE, tolerance=10, step=3)
    summary["audience_orange_samples"] = orange
    if orange < 2000:
        raise nx.Failure(f"the audience does not show the orange slide ({orange})")
    if image.ink_box(threshold=235) is None:
        raise nx.Failure("the audience slide shows no text")
    summary["live_status_verse"] = wait_status(
        app, operator, f"On screen: {TITLE} · Verse 1", "the verse in the Live status"
    )
    session.save(operator, "11-operator-live")
    nx.press(operator, "right")
    nx.wait_for_change(screen, image.digest(), 5.0, "Next to the chorus")
    image = session.save(nx.capture_stable(screen), "12-audience-chorus")
    if image.count(ORANGE, tolerance=10, step=3) > 100:
        raise nx.Failure("the chorus kept slide 1's background")
    black = image.count((0, 0, 0), tolerance=10, step=3)
    summary["audience_black_samples_chorus"] = black
    if black < 2000:
        raise nx.Failure(f"the chorus is not shown on black ({black})")
    if image.ink_box(threshold=235) is None:
        raise nx.Failure("the chorus slide shows no text")
    summary["live_status_chorus"] = wait_status(
        app, operator, f"On screen: {TITLE} · Chorus", "the chorus in the Live status"
    )
    nx.press(operator, "ctrl+q")
    summary["final_exit"] = app.wait_exit()
    if summary["final_exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['final_exit']}")
    if not nx.wait_dead(pid, 5):
        raise nx.Failure("quitting left the audience output running")
    return (
        "fresh profile -> announced Options -> New Song with a slide background and an "
        "announced close prompt -> schedule -> announced unsaved "
        "dialog -> saved file pins the stored revision -> quit -> reopen restores the layout "
        "and Open Recent -> Go Live shows the orange slide on the real audience and the Live "
        "status follows the renderer receipt"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "scenario-s01-x11", seed=False))
