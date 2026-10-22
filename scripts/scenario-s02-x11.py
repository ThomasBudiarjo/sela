#!/usr/bin/env python3
"""Linux/X11 native E2E scenario S02 (backlog "Repeatable end-to-end
scenarios"): present A -> edit/select B -> next/previous -> mask toggles ->
Go Live under a mask, with the real audience process.

Usage: python3 scripts/scenario-s02-x11.py [--display :111 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR]
       [--build]

Seeds a library with two original songs (A = "Signal Hymn": Verse 1, Chorus,
Bridge; B = "Quiet Canticle": Verse 1), schedules A then B and turns Live
output on. Checks, in order:

1. Present A: the first schedule item and Page Down; the audience changes and
   the Live status (AT-SPI `Live-status`) names "Signal Hymn · Verse 1".
2. Songs menu modality (SC2-FOCUS-01): with the Songs row menu open, B and
   Page Down change neither the audience nor the mask; Esc closes the menu.
   The Schedule item menu (right-click on item 2) is modal the same way
   (SC2-FOCUS-02): B, Page Down and Ctrl+Delete change nothing and the entry
   stays; Esc closes it.
3. Edit B while A is live: Songs menu › Edit Song… opens "Song Editor - Quiet
   Canticle"; a line is added to its verse and Ctrl+S stores revision 2 of B
   (SQLite) while A keeps revision 1. The audience frame and the Live status
   are unchanged; the editor closes without a prompt.
4. Select B: the second schedule item goes to Preview only (audience
   unchanged).
5. Next/Previous act on A, not on the selection: Next shows the Chorus,
   Previous returns to exactly the Verse 1 frame.
6. Mask toggles (EW8-OBS-014..019): Ctrl+B blacks the audience and the status
   shows "Mask: Black", Ctrl+B again restores the Verse 1 frame; Ctrl+C clears
   the text (no near-white ink) and Ctrl+C again restores it; Ctrl+L with no
   logo set is refused with "No logo set" and changes nothing.
7. Go Live under a mask (EW8-OBS-017: masks survive Go Live): Ctrl+B, then Go
   Live on the selected B keeps the audience black while the status shows
   "Under mask: Quiet Canticle · Verse 1"; Go Live handed the page keys to
   the Live pane, so Page Up takes A live, still under the mask. Ctrl+B
   reveals exactly the Verse 1 frame of A, Page Down shows B.
8. Ctrl+Q asks about the unsaved schedule; Don't Save quits (exit status 0)
   and the audience process ends.

X11 differences: on the default xvfb display the audience is the 624x351
development window beside the operator (use --kind dual for the
monitor-covering window); Alt+F4 (Openbox) stands in for WM_CLOSE; fonts are
DejaVu. No EasyWorship reference runs here: the mask and focus rules checked
are the ones Sela documents (EW8-OBS IDs above), not parity evidence.
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

A = "Signal Hymn"
B = "Quiet Canticle"
ADDED = "a line added while A was live"


def revisions(library: Path) -> dict[str, int]:
    """Head revision of each song by title."""
    with nx.sqlite_ro(library) as db:
        rows = db.execute("SELECT id, head FROM songs WHERE deleted=0").fetchall()
    heads = dict(rows)
    return {
        payload[0]: heads[song_id]
        for song_id, payload in zip(
            sorted(heads), nx.song_payloads(library), strict=True
        )
    }


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


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    data = session.temp("sela-s02-")
    library = nx.seed_library(data / "library.sqlite", "--second-song", seed=args.seed)
    app = session.app(["--operator-library", library], name="operator")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.5)

    def control(aid: str) -> nx.Control:
        return nx.wait_until(
            lambda: app.controls(operator).get(aid), 5, f"control {aid}"
        )

    def press_control(aid: str) -> None:
        nx.click(operator, *control(aid).center)

    def status() -> str:
        return app.controls(operator)["Live-status"].name

    def wait_status(text: str, what: str) -> str:
        def probe():
            current = status()
            return current if text in current else None

        return nx.wait_until(probe, 5, what)

    # Schedule A then B (Songs are sorted by title: B is song-0, A song-1).
    for row in ("song-1", "song-0"):
        press_control(row)
        time.sleep(0.4)
        press_control("add-to-schedule")
    control("schedule-item-1")
    press_control("live-output")
    found = app.audience(20)
    if not found:
        raise nx.Failure(f"no audience window within 20s: {app.log()}")
    pid, screen = found
    time.sleep(1.5)
    nx.activate(operator)
    blank = nx.digest(screen)

    # 1. Present A.
    press_control("schedule-item-0")
    time.sleep(0.8)
    nx.press(operator, "pagedown")
    nx.wait_for_change(screen, blank, 5.0, "A going live")
    verse = session.save(nx.capture_stable(screen), "01-a-verse")
    a1 = verse.digest()
    if verse.ink_box(threshold=235) is None:
        raise nx.Failure("A's verse shows no text")
    summary["a_live"] = wait_status(f"On screen: {A} · Verse 1", "A in the Live status")

    def unchanged(what: str) -> None:
        time.sleep(1.0)
        if nx.digest(screen) != a1:
            raise nx.Failure(f"{what} changed the audience output")
        if f"On screen: {A} · Verse 1" not in status():
            raise nx.Failure(f"{what} changed the Live status: {status()!r}")

    # 2. Show keys are inert while the Songs row menu is open.
    nx.right_click(operator, *control("song-0").center)
    control("song-menu")
    nx.press(operator, "b", "pagedown")
    unchanged("B and Page Down under the Songs menu")
    if "Mask:" in status():
        raise nx.Failure(f"a mask went on under the Songs menu: {status()!r}")
    session.save(operator, "02-songs-menu")
    nx.press(operator, "escape")
    time.sleep(0.4)
    if "song-menu" in app.controls(operator):
        raise nx.Failure("Esc did not close the Songs menu")

    # 2b. The same for the Schedule item menu (SC2-FOCUS-02): B, Page Down and
    # Ctrl+Delete neither drive Live nor remove the entry it points at.
    nx.right_click(operator, *control("schedule-item-1").center)
    control("item-menu-popup")
    nx.press(operator, "b", "pagedown", "ctrl+delete")
    unchanged("B, Page Down and Ctrl+Delete under the Schedule item menu")
    if "Mask:" in status():
        raise nx.Failure(f"a mask went on under the item menu: {status()!r}")
    found = app.controls(operator)
    if "schedule-item-1" not in found or "item-menu-popup" not in found:
        raise nx.Failure("a key under the Schedule item menu removed or closed it")
    session.save(operator, "02b-item-menu")
    nx.press(operator, "escape")
    time.sleep(0.4)
    if "item-menu-popup" in app.controls(operator):
        raise nx.Failure("Esc did not close the Schedule item menu")

    # 3. Edit B while A is live.
    before = revisions(library)
    nx.right_click(operator, *control("song-0").center)
    nx.click(operator, *control("song-menu-edit").center)
    editor = app.new_window([operator], 15)
    nx.wait_until(
        lambda: nx.window_name(editor) == f"Song Editor - {B}", 10, "B's editor"
    )
    nx.activate(editor)
    time.sleep(1.0)
    # The caret starts in slide 1's label; Enter moves to its lyrics.
    nx.press(editor, "enter", "end", "enter", pause=0.15)
    nx.type_text(editor, ADDED, pause=0.25)
    session.save(nx.capture_stable(editor), "03-editor-b")
    nx.press(editor, "ctrl+s")
    after = nx.wait_until(
        lambda: (r := revisions(library))[B] == before[B] + 1 and r,
        10,
        "Ctrl+S to store B's next revision",
    )
    summary["revisions"] = [before, after]
    if after[A] != before[A]:
        raise nx.Failure("editing B changed A's revision")
    [b_payload] = [p for p in nx.song_payloads(library) if p[0] == B]
    if not b_payload[5].endswith("\n" + ADDED):
        raise nx.Failure(f"unexpected stored B lyrics {b_payload[5]!r}")
    close_saved_editor(app, editor)
    nx.activate(operator)
    unchanged("editing B")

    # 4. Select B: Preview only.
    press_control("schedule-item-1")
    unchanged("selecting B")
    session.save(operator, "04-b-selected")

    # 5. Next/Previous act on the live A.
    press_control("live-next")
    nx.wait_for_change(screen, a1, 5.0, "Next to A's chorus")
    wait_status(f"On screen: {A} · Chorus", "A's chorus in the Live status")
    session.save(nx.capture_stable(screen), "05-a-chorus")
    press_control("live-previous")
    nx.wait_until(lambda: nx.digest(screen) == a1, 5, "Previous back to A's verse")
    wait_status(f"On screen: {A} · Verse 1", "A's verse again")

    # 6. Mask toggles.
    nx.press(operator, "ctrl+b")
    black = nx.wait_until(
        lambda: (i := nx.capture(screen)).brightest() < 16 and i,
        5,
        "Black on the audience",
    )
    session.save(black, "06-black")
    wait_status("Mask: Black", "Black in the Live status")
    nx.press(operator, "ctrl+b")
    nx.wait_until(lambda: nx.digest(screen) == a1, 5, "Black off restores A's verse")
    nx.press(operator, "ctrl+c")
    cleared = nx.wait_until(
        lambda: (i := nx.capture(screen)).ink_box(threshold=235) is None and i,
        5,
        "Clear hides the text",
    )
    session.save(cleared, "07-clear")
    wait_status("Mask: Clear", "Clear in the Live status")
    nx.press(operator, "ctrl+c")
    nx.wait_until(lambda: nx.digest(screen) == a1, 5, "Clear off restores A's verse")
    nx.press(operator, "ctrl+l")
    wait_status("No logo set", "the refused Logo")
    unchanged("Ctrl+L without a logo")

    # 7. Go Live under a mask.
    nx.press(operator, "ctrl+b")
    nx.wait_until(lambda: nx.capture(screen).brightest() < 16, 5, "Black again")
    black_digest = nx.digest(screen)
    press_control("go-live")
    summary["b_under_mask"] = wait_status(
        f"Under mask: {B} · Verse 1", "B acknowledged under the mask"
    )
    time.sleep(0.5)
    if nx.digest(screen) != black_digest:
        raise nx.Failure("Go Live under Black showed B")
    session.save(operator, "08-b-under-mask")
    # Go Live handed the page keys to the Live pane: Page Up is the previous
    # schedule item (EW8-OBS-050; Sela's PageUp rule), still masked.
    nx.press(operator, "pageup")
    summary["a_under_mask"] = wait_status(
        f"Under mask: {A} · Verse 1", "Page Up took A live under the mask"
    )
    if nx.digest(screen) != black_digest:
        raise nx.Failure("Page Up under Black showed A")
    nx.press(operator, "ctrl+b")
    nx.wait_until(lambda: nx.digest(screen) == a1, 5, "Black off reveals A's verse")
    nx.press(operator, "pagedown")
    nx.wait_for_change(screen, a1, 5.0, "Page Down to B")
    image = session.save(nx.capture_stable(screen), "09-b-live")
    if image.ink_box(threshold=235) is None:
        raise nx.Failure("B shows no text")
    wait_status(f"On screen: {B} · Verse 1", "B in the Live status")

    # 8. Quit without saving the schedule.
    nx.press(operator, "ctrl+q")
    time.sleep(0.8)
    if not app.running():
        raise nx.Failure("Ctrl+Q quit with an unsaved schedule")
    nx.press(operator, "shift+tab", "enter")  # Don't Save
    summary["exit"] = app.wait_exit()
    if summary["exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['exit']}")
    if not nx.wait_dead(pid, 5):
        raise nx.Failure("quitting left the audience output running")
    return (
        "A live; show keys inert under the Songs menu; editing B stores a new revision and "
        "never touches A on the audience; selecting B is Preview only; Next/Previous act on "
        "A; Black/Clear toggle and restore the exact frame, Logo refused without a logo; Go "
        "Live and Page Up under Black stay black until Black is lifted"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "scenario-s02-x11"))
