"""Windows native E2E scenario S01 (backlog "Repeatable end-to-end
scenarios"): fresh offline profile -> create a song -> schedule -> save ->
close/reopen -> Go Live with the real audience process.

Usage: python scripts/scenario-s01-windows.py [--binary sela.exe] [--out DIR]

Windows counterpart of scripts/scenario-s01-x11.py, same checks in the same
order (see that docstring): `sela.exe` with no arguments in a new private
profile (LOCALAPPDATA/APPDATA/TEMP of one temporary folder, nothing seeded),
Edit > Options is a UIA Window element named "Options" (Esc closes it),
New ▾ › New Song in the editor window with two slides and a #FFAA00 Color Fill
on slide 1 (decoded from SQLite), Alt+F4 shows the close prompt as a UIA
Window named "Save changes before closing?" and Esc cancels it, Ctrl+S, Add
to Schedule, Ctrl+Q shows the unsaved-changes dialog as a UIA Window element
(controlType and name checked) named with its question (SC2-A11Y-01), Cancel, Ctrl+S through the native Save dialog, the saved file
pins the stored revision, a splitter drag, Ctrl+Q, a second launch with the
same profile restores the pane rectangles and File › Open Recent reopens the
schedule, Live output on (SELA_AUDIENCE_MONITOR=secondary), Page Down shows the
orange slide on the audience and the Live status names "S01 Hymn · Verse 1",
Right shows the Chorus, Ctrl+Q ends the audience process.

Prerequisites: Windows 11, `cargo build --locked`, a secondary monitor for the
audience, the keyboard and mouse idle for the whole run (the script verifies
the foreground before each input). Takes over the real keyboard; never run
with EasyWorship open. Status: NOT RUN (no Windows host when written,
2026-10-22).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import sqlite3
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parents[1]
TITLE = "S01 Hymn"
NAME = "s01-service"
ORANGE = (0xFF, 0xAA, 0x00)
UNSAVED_QUESTION = "The current Schedule is not saved. Would you like to save it now?"
# Yes saves and closes, No discards (EW8-OBS-012), so the editor asks to save.
CLOSE_QUESTION = "Save changes before closing?"
# AccessKit 0.34 maps Role::Dialog to the UIA Window control type.
DIALOG = "Window"
LYRICS = [
    "Verse 1",
    "Morning light on quiet water\nevery shore awake and singing",
    "Chorus",
    "Lift the morning song",
]


def load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lo = load("live_output_windows", "live-output-windows.py")
a11y = load("a11y_windows", "a11y-windows.py")
editor_script = load("song_editor_windows", "song-editor-windows.py")


def songs(library: pathlib.Path) -> list[tuple[str, int]]:
    if not library.exists():
        return []
    uri = library.resolve().as_uri() + "?mode=ro"
    with sqlite3.connect(uri, uri=True) as db:
        return db.execute(
            "SELECT lower(hex(id)), head FROM songs WHERE deleted=0 ORDER BY id"
        ).fetchall()


def wait_for(timeout: float, what: str, probe):
    deadline = time.monotonic() + timeout
    while True:
        value = probe()
        if value:
            return value
        if time.monotonic() >= deadline:
            raise nw.Failure(f"timed out waiting for {what}")
        time.sleep(0.2)


def nodes(hwnd: int) -> dict[str, dict]:
    return a11y.nodes_by_id(a11y.uia(hwnd))


def pane_rects(hwnd: int) -> dict[str, list[int]]:
    found = nodes(hwnd)
    rects = {
        aid: [found[aid]["x"], found[aid]["y"], found[aid]["width"], found[aid]["height"]]
        for aid in ("Preview", "Live")
    }
    rects["schedule-list"] = [found["schedule-list"]["x"], found["schedule-list"]["width"]]
    return rects


def orange_count(hwnd: int) -> int:
    width, height, pixels = nw.capture_client(hwnd)
    return lo.hits(width, height, pixels, ORANGE)


def create_song(app: nw.App, operator: int, library: pathlib.Path, out: pathlib.Path,
                summary: dict) -> None:
    a11y.click_node(operator, nodes(operator)["new-menu"])
    time.sleep(0.5)
    a11y.click_node(operator, nodes(operator)["new-song-menu"])
    editor = wait_for(
        15, "the Song Editor window",
        lambda: next((h for h in nw.top_windows(app.process.pid) if h != operator), None),
    )
    time.sleep(1.5)
    if nw.title(editor) != "Song Editor - Untitled":
        raise nw.Failure(f"New Song opened {nw.title(editor)!r}")
    nw.activate(editor)
    scale = nw.dpi(editor) / 96

    def keys(*sequence: str) -> None:
        editor_script.keys(editor, *sequence)

    def write(text: str) -> None:
        editor_script.write(editor, text)

    def shot(name: str) -> tuple[int, int, bytes]:
        time.sleep(0.8)
        width, height, pixels = nw.capture_client(editor)
        nw.write_png(out / f"{name}.png", width, height, pixels)
        return width, height, pixels

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
    nw.click(editor, int(80 * scale), int(20 * scale))
    time.sleep(0.3)
    write(TITLE)
    shot("02-editor-words")
    nw.click(editor, int(89 * scale), int(113 * scale))
    time.sleep(1.0)
    nw.click(editor, int(140 * scale), int(210 * scale))
    button = editor_script.format_button(*shot("03-editor-slides"), scale)
    if not button:
        raise nw.Failure("no Format toggle in the editor toolbar")
    nw.click(editor, *button)
    time.sleep(0.5)
    nw.click(editor, int(140 * scale), int(210 * scale))
    keys("tab", "tab", "enter", "tab", "tab", "enter", "down", "down", "enter")
    keys("tab", "enter", "ctrl+a")
    write("#FFAA00")
    keys("enter")
    shot("04-editor-orange")
    summary["editor_orange_samples"] = orange_count(editor)
    if summary["editor_orange_samples"] < 2000:
        raise nw.Failure("the editor preview shows no Color Fill")
    # Closing with unsaved edits asks whether to save, as an announced dialog
    # (SC2-A11Y-01, SC2-I18N-02); Esc is Cancel and keeps the editor.
    nw.press(editor, "alt+f4")
    prompt = wait_for(10, "the close prompt", lambda: nodes(editor).get("close-prompt"))
    summary["close_prompt"] = [prompt["controlType"], prompt["name"]]
    if summary["close_prompt"] != [DIALOG, CLOSE_QUESTION]:
        raise nw.Failure(f"the close prompt is not announced: {summary['close_prompt']}")
    shot("04b-editor-close-prompt")
    keys("escape")
    wait_for(5, "Esc to cancel the close prompt", lambda: "close-prompt" not in nodes(editor))
    if not nw.user32.IsWindow(editor):
        raise nw.Failure("Esc on the close prompt closed the editor")
    keys("ctrl+s")
    expected = [TITLE, "", "", "", *LYRICS]
    stored = wait_for(
        10, "Ctrl+S to store the new song",
        lambda: [p for p in editor_script.payloads(library) if p[0] == TITLE],
    )
    if stored != [expected]:
        raise nw.Failure(f"unexpected stored song {stored}")
    slides, master = editor_script.backgrounds(library)
    if slides != [(0, bytes([1, 1, 0xFF, 0xAA, 0, 2]))] or master != []:
        raise nw.Failure(f"unexpected stored backgrounds {slides} {master}")
    time.sleep(1.0)  # the editor takes the save reply off its worker
    nw.press(editor, "alt+f4")
    wait_for(10, "the saved editor to close", lambda: not nw.user32.IsWindow(editor))
    nw.activate(operator)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=pathlib.Path, default=ROOT / "target/debug/sela.exe")
    parser.add_argument(
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/scenario-s01-windows"
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    summary: dict = {"status": "FAIL"}
    apps: list[nw.App] = []
    profile = pathlib.Path(tempfile.mkdtemp(prefix="sela-s01-"))
    library = profile / "local" / "sela" / "library.sqlite"
    os.environ["SELA_AUDIENCE_MONITOR"] = "secondary"

    def shot(hwnd: int, name: str) -> None:
        width, height, pixels = nw.capture_client(hwnd)
        nw.write_png(args.out / f"{name}.png", width, height, pixels)

    try:
        # 1. Fresh profile.
        app = nw.App(args.binary, [], scratch=profile)
        apps.append(app)
        operator = app.window()
        nw.activate(operator)
        wait_for(10, "the operator to create its library", library.exists)
        time.sleep(1.0)
        if songs(library) or "song-0" in nodes(operator):
            raise nw.Failure("a fresh profile already has songs")
        shot(operator, "01-fresh")
        # Options (Edit > Options..., Alt+E) is an announced dialog.
        nw.press(operator, "alt+e")
        time.sleep(0.4)
        nw.press(operator, "enter")
        options = wait_for(5, "the Options dialog", lambda: nodes(operator).get("options-dialog"))
        summary["options_dialog"] = [options["controlType"], options["name"]]
        if summary["options_dialog"] != [DIALOG, "Options"]:
            raise nw.Failure(f"Options is not an announced dialog: {summary['options_dialog']}")
        nw.press(operator, "escape")
        wait_for(5, "Esc to close Options", lambda: "options-dialog" not in nodes(operator))

        # 2. Create a song.
        create_song(app, operator, library, args.out, summary)
        [(song_id, revision)] = songs(library)
        summary["song"] = [song_id, revision]

        # 3. Schedule it; the unsaved prompt is an announced dialog.
        a11y.click_node(operator, wait_for(10, "the new song", lambda: nodes(operator).get("song-0")))
        a11y.click_node(operator, nodes(operator)["add-to-schedule"])
        wait_for(5, "the dirty title", lambda: nw.title(operator) == "Untitled schedule* — Sela")
        nw.press(operator, "ctrl+q")
        time.sleep(0.8)
        dialog = nodes(operator).get("schedule-dialog")
        summary["unsaved_dialog"] = dialog and [dialog["controlType"], dialog["name"]]
        if summary["unsaved_dialog"] != [DIALOG, UNSAVED_QUESTION]:
            raise nw.Failure(f"the unsaved prompt is not announced: {summary['unsaved_dialog']}")
        shot(operator, "06-unsaved-dialog")
        nw.press(operator, "enter")  # Cancel
        time.sleep(0.5)

        # 4. Save through the native dialog.
        nw.press(operator, "ctrl+s")
        nw.drive_file_dialog(app.process.pid, operator, str(profile / NAME))
        schedule_file = profile / f"{NAME}.selaschedule"
        document = wait_for(
            10, "the saved schedule",
            lambda: schedule_file.is_file() and json.loads(schedule_file.read_text("utf-8")),
        )
        items = [[i["song_id"], i["revision"]] for i in document["items"]]
        summary["saved"] = [document["title"], items]
        if document["title"] != NAME or items != [[song_id, revision]]:
            raise nw.Failure(f"unexpected saved schedule {summary['saved']}")
        before = pane_rects(operator)
        found = nodes(operator)
        origin_x, origin_y = a11y.client_origin(operator)
        split_x = found["Preview"]["x"] - origin_x - 3
        split_y = found["Preview"]["y"] - origin_y + 200
        nw.mouse_down(operator, split_x, split_y)
        nw.drag_to(operator, split_x + 80, split_y)
        nw.mouse_up()
        time.sleep(0.5)
        layout = pane_rects(operator)
        summary["layout"] = layout
        if layout == before:
            raise nw.Failure("the splitter drag did not change the layout")
        shot(operator, "08-saved")

        # 5. Close.
        nw.press(operator, "ctrl+q")
        summary["first_exit"] = app.wait_exit()
        if summary["first_exit"] != 0:
            raise nw.Failure(f"operator exit status {summary['first_exit']}")

        # 6. Reopen with the same profile.
        app = nw.App(args.binary, [], scratch=profile)
        apps.append(app)
        operator = app.window()
        nw.activate(operator)
        time.sleep(1.5)
        summary["reopened_layout"] = pane_rects(operator)
        if summary["reopened_layout"] != layout:
            raise nw.Failure("the layout was not restored")
        nw.press(operator, "alt+f")
        time.sleep(0.5)
        recent = nodes(operator).get("bar-file-recent-0")
        if not recent or NAME not in recent["name"]:
            raise nw.Failure(f"Open Recent does not list the schedule: {recent}")
        a11y.click_node(operator, recent)
        wait_for(10, "the reopened schedule", lambda: nw.title(operator) == f"{NAME}.selaschedule — Sela")
        item = wait_for(5, "the reopened item", lambda: nodes(operator).get("schedule-item-0"))
        if "schedule-item-1" in nodes(operator):
            raise nw.Failure("the reopened schedule has more than one item")
        shot(operator, "09-reopened")

        # 7. Go Live with the real audience process.
        a11y.show_live_output(operator)
        found_audience = lo.audience(app.process.pid, 20)
        if not found_audience:
            raise nw.Failure("no audience window after Live output on")
        audience_pid, screen = found_audience
        a11y.wait_live_status(operator, "×", 30, "audience output never became ready")
        nw.activate(operator)
        blank = lo.digest(screen)
        a11y.click_node(operator, item)
        time.sleep(1.0)
        if lo.digest(screen) != blank:
            raise nw.Failure("selecting the item changed the audience output")
        nw.press(operator, "pagedown")
        summary["live_status_verse"] = a11y.wait_live_status(
            operator, f"On screen: {TITLE} · Verse 1", 10, "the verse in the Live status"
        )
        verse = a11y.stable_digest(screen)
        summary["audience_orange_samples"] = orange_count(screen)
        if summary["audience_orange_samples"] < 2000 or not a11y.audience_ink(screen):
            raise nw.Failure("the audience does not show the orange slide with text")
        shot(screen, "10-audience-verse")
        nw.press(operator, "right")
        summary["live_status_chorus"] = a11y.wait_live_status(
            operator, f"On screen: {TITLE} · Chorus", 10, "the chorus in the Live status"
        )
        if a11y.stable_digest(screen) == verse or orange_count(screen) > 100:
            raise nw.Failure("the chorus did not replace the orange verse")
        nw.press(operator, "ctrl+q")
        summary["final_exit"] = app.wait_exit()
        if summary["final_exit"] != 0:
            raise nw.Failure(f"operator exit status {summary['final_exit']}")
        if not lo.wait_dead(audience_pid, 5):
            raise nw.Failure("quitting left the audience output running")
        summary["status"] = "PASS"
    except nw.Failure as failure:
        summary["error"] = str(failure)
        print(f"FAIL: {failure}")
    finally:
        for app in apps:
            app.close()
        (args.out / "summary.json").write_text(
            json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8"
        )
        print(json.dumps(summary, indent=2, ensure_ascii=False))
    return 0 if summary["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
