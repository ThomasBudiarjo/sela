"""Windows native E2E scenario S02 (backlog "Repeatable end-to-end
scenarios"): present A -> edit/select B -> next/previous -> mask toggles ->
Go Live under a mask, with the real audience process.

Usage: python scripts/scenario-s02-windows.py [--binary sela.exe]
       [--seed seed_library.exe] [--out DIR]

Windows counterpart of scripts/scenario-s02-x11.py, same checks in the same
order (see that docstring): seeded "Signal Hymn" (A) and "Quiet Canticle"
(B) scheduled as A, B with Live output on (SELA_AUDIENCE_MONITOR=secondary);
A goes live by Page Down; with the Songs row menu open B and Page Down change
nothing (SC2-FOCUS-01), and with the Schedule item menu open B, Page Down and
Ctrl+Delete change nothing and Esc closes it (SC2-FOCUS-02); with New ▾ open
B and Page Down change nothing, a press on New ▾ released 300 px away closes
it and the next Enter opens it again (SC2-FOCUS-03); Songs menu › Edit Song… on B adds a line and Ctrl+S
stores B's revision 2 while A keeps revision 1 and the audience frame and the
Live status (UIA `Live-status`) stay on A's Verse 1; selecting B is Preview
only; Next/Previous act on A; Ctrl+B / Ctrl+C toggle and restore the exact
frame and Ctrl+L without a logo is refused; under Black, Go Live on B and Page
Up (back to A, Live pane focused) keep the audience black while the status
shows "Under mask: …"; Ctrl+B reveals A's Verse 1, Page Down shows B; Ctrl+Q,
Don't Save, the audience process ends.

Prerequisites: Windows 11, `cargo build --locked` and `cargo build --locked
--examples`, a secondary monitor for the audience, the keyboard and mouse idle
for the whole run. Takes over the real keyboard; never run with EasyWorship
open. Status: NOT RUN (no Windows host when written, 2026-10-22).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parents[1]
A = "Signal Hymn"
B = "Quiet Canticle"
ADDED = "a line added while A was live"


def load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lo = load("live_output_windows", "live-output-windows.py")
a11y = load("a11y_windows", "a11y-windows.py")
editor_script = load("song_editor_windows", "song-editor-windows.py")


def revisions(library: pathlib.Path) -> dict[str, int]:
    """Head revision of each song by title."""
    uri = library.resolve().as_uri() + "?mode=ro"
    with sqlite3.connect(uri, uri=True) as db:
        heads = dict(db.execute("SELECT id, head FROM songs WHERE deleted=0").fetchall())
        rows = db.execute(
            "SELECT s.id, r.payload FROM songs s JOIN song_revisions r "
            "ON r.id=s.id AND r.revision=s.head WHERE s.deleted=0"
        ).fetchall()
    titles = {}
    for song_id, payload in rows:
        length = int.from_bytes(payload[:4], "little")
        titles[payload[4 : 4 + length].decode("utf-8")] = heads[song_id]
    return titles


def payload_fields(library: pathlib.Path, title: str) -> list[str]:
    """Head revision payload fields of the song titled `title` (title,
    metadata, then label/lyrics per section), as `native_x11.song_payloads`."""
    uri = library.resolve().as_uri() + "?mode=ro"
    with sqlite3.connect(uri, uri=True) as db:
        rows = db.execute(
            "SELECT r.payload FROM songs s JOIN song_revisions r "
            "ON r.id=s.id AND r.revision=s.head WHERE s.deleted=0"
        ).fetchall()
    for (data,) in rows:
        fields = []
        while data:
            length = int.from_bytes(data[:4], "little")
            fields.append(data[4 : 4 + length].decode("utf-8"))
            data = data[4 + length :]
        if fields and fields[0] == title:
            return fields
    raise nw.Failure(f"no stored song titled {title!r}")


def wait_for(timeout: float, what: str, probe):
    deadline = time.monotonic() + timeout
    while True:
        value = probe()
        if value:
            return value
        if time.monotonic() >= deadline:
            raise nw.Failure(f"timed out waiting for {what}")
        time.sleep(0.2)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=pathlib.Path, default=ROOT / "target/debug/sela.exe")
    parser.add_argument(
        "--seed", type=pathlib.Path, default=ROOT / "target/debug/examples/seed_library.exe"
    )
    parser.add_argument(
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/scenario-s02-windows"
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    summary: dict = {"status": "FAIL"}
    app = None
    try:
        data = pathlib.Path(tempfile.mkdtemp(prefix="sela-s02-"))
        library = data / "library.sqlite"
        subprocess.run([str(args.seed), str(library), "--second-song"], check=True)
        os.environ["SELA_AUDIENCE_MONITOR"] = "secondary"
        app = nw.App(args.binary, ["--operator-library", str(library)])
        operator = app.window()
        nw.activate(operator)
        time.sleep(1.5)

        def nodes() -> dict[str, dict]:
            return a11y.nodes_by_id(a11y.uia(operator))

        def press_control(aid: str) -> None:
            a11y.click_node(operator, wait_for(5, aid, lambda: nodes().get(aid)))

        def status(needle: str, what: str) -> str:
            return a11y.wait_live_status(operator, needle, 10, what)

        def shot(hwnd: int, name: str) -> None:
            width, height, pixels = nw.capture_client(hwnd)
            nw.write_png(args.out / f"{name}.png", width, height, pixels)

        for row in ("song-1", "song-0"):
            press_control(row)
            press_control("add-to-schedule")
        a11y.show_live_output(operator)
        found = lo.audience(app.process.pid, 20)
        if not found:
            raise nw.Failure("no audience window after Live output on")
        audience_pid, screen = found
        a11y.wait_live_status(operator, "×", 30, "audience output never became ready")
        nw.activate(operator)

        # 1. Present A.
        press_control("schedule-item-0")
        nw.press(operator, "pagedown")
        summary["a_live"] = status(f"On screen: {A} · Verse 1", "A live")
        a1 = a11y.stable_digest(screen)
        shot(screen, "01-a-verse")

        def unchanged(what: str) -> None:
            time.sleep(1.0)
            if lo.digest(screen) != a1:
                raise nw.Failure(f"{what} changed the audience output")
            if f"On screen: {A} · Verse 1" not in a11y.live_status(operator):
                raise nw.Failure(f"{what} changed the Live status")

        # 2. Songs row menu modality.
        song = nodes()["song-0"]
        origin_x, origin_y = a11y.client_origin(operator)
        nw.right_click(
            operator,
            song["x"] + song["width"] // 2 - origin_x,
            song["y"] + song["height"] // 2 - origin_y,
        )
        wait_for(5, "the Songs menu", lambda: "song-menu" in nodes())
        nw.press(operator, "b")
        nw.press(operator, "pagedown")
        unchanged("B and Page Down under the Songs menu")
        if "Mask:" in a11y.live_status(operator):
            raise nw.Failure("a mask went on under the Songs menu")
        nw.press(operator, "escape")

        # 2b. Schedule item menu modality (SC2-FOCUS-02).
        row = nodes()["schedule-item-1"]
        nw.right_click(
            operator,
            row["x"] + row["width"] // 2 - origin_x,
            row["y"] + row["height"] // 2 - origin_y,
        )
        wait_for(5, "the Schedule item menu", lambda: "item-menu-popup" in nodes())
        for keys in ("b", "pagedown", "ctrl+delete"):
            nw.press(operator, keys)
        unchanged("B, Page Down and Ctrl+Delete under the Schedule item menu")
        if "Mask:" in a11y.live_status(operator):
            raise nw.Failure("a mask went on under the Schedule item menu")
        found = nodes()
        if "schedule-item-1" not in found or "item-menu-popup" not in found:
            raise nw.Failure("a key under the Schedule item menu removed or closed it")
        nw.press(operator, "escape")
        wait_for(5, "Esc to close the item menu", lambda: "item-menu-popup" not in nodes())

        # 2c. New ▾ modality and a press released elsewhere (SC2-FOCUS-03).
        press_control("new-menu")
        wait_for(5, "the New menu", lambda: "new-menu-popup" in nodes())
        nw.press(operator, "b")
        nw.press(operator, "pagedown")
        unchanged("B and Page Down under the New menu")
        new = nodes()["new-menu"]
        new_x = new["x"] + new["width"] // 2 - origin_x
        new_y = new["y"] + new["height"] // 2 - origin_y
        nw.mouse_down(operator, new_x, new_y)
        try:
            nw.drag_to(operator, new_x, new_y + 300)
        finally:
            nw.mouse_up()
        wait_for(5, "the press to close New ▾", lambda: "new-menu-popup" not in nodes())
        nw.press(operator, "enter")
        wait_for(5, "the first Enter to open New ▾", lambda: "new-menu-popup" in nodes())
        nw.press(operator, "escape")
        wait_for(5, "Esc to close New ▾", lambda: "new-menu-popup" not in nodes())
        unchanged("the New menu steps")

        # 3. Edit B while A is live.
        before = revisions(library)
        nw.right_click(
            operator,
            song["x"] + song["width"] // 2 - origin_x,
            song["y"] + song["height"] // 2 - origin_y,
        )
        press_control("song-menu-edit")
        editor = wait_for(
            15, "B's editor",
            lambda: next(
                (h for h in nw.top_windows(app.process.pid) if nw.title(h) == f"Song Editor - {B}"),
                None,
            ),
        )
        nw.activate(editor)
        time.sleep(1.0)
        editor_script.keys(editor, "enter", "end", "enter")
        editor_script.write(editor, ADDED)
        nw.press(editor, "ctrl+s")
        after = wait_for(
            10, "B's next revision",
            lambda: (r := revisions(library))[B] == before[B] + 1 and r,
        )
        summary["revisions"] = [before, after]
        if after[A] != before[A]:
            raise nw.Failure("editing B changed A's revision")
        stored = payload_fields(library, B)
        if not stored[5].endswith("\n" + ADDED):
            raise nw.Failure(f"unexpected stored B lyrics {stored[5]!r}")
        time.sleep(1.0)  # the editor takes the save reply off its worker
        nw.press(editor, "alt+f4")
        wait_for(10, "the saved editor to close", lambda: not nw.user32.IsWindow(editor))
        nw.activate(operator)
        unchanged("editing B")

        # 4. Select B.
        press_control("schedule-item-1")
        unchanged("selecting B")

        # 5. Next/Previous act on A.
        press_control("live-next")
        status(f"On screen: {A} · Chorus", "A's chorus")
        press_control("live-previous")
        status(f"On screen: {A} · Verse 1", "A's verse again")
        if a11y.stable_digest(screen) != a1:
            raise nw.Failure("Previous did not return to A's verse frame")

        # 6. Mask toggles.
        nw.press(operator, "ctrl+b")
        status("Mask: Black", "Black")
        if not a11y.wait_for(5, lambda: lo.brightest(nw.capture_client(screen)[2]) < 16):
            raise nw.Failure("Black did not blank the audience")
        shot(screen, "06-black")
        nw.press(operator, "ctrl+b")
        if not a11y.wait_for(5, lambda: lo.digest(screen) == a1):
            raise nw.Failure("Black off did not restore A's verse")
        nw.press(operator, "ctrl+c")
        status("Mask: Clear", "Clear")
        if not a11y.wait_for(5, lambda: not a11y.audience_ink(screen)):
            raise nw.Failure("Clear did not hide the text")
        nw.press(operator, "ctrl+c")
        if not a11y.wait_for(5, lambda: lo.digest(screen) == a1):
            raise nw.Failure("Clear off did not restore A's verse")
        nw.press(operator, "ctrl+l")
        status("No logo set", "the refused Logo")
        unchanged("Ctrl+L without a logo")

        # 7. Go Live under Black.
        nw.press(operator, "ctrl+b")
        status("Mask: Black", "Black again")
        black = a11y.stable_digest(screen)
        press_control("go-live")
        summary["b_under_mask"] = status(f"Under mask: {B} · Verse 1", "B under the mask")
        if lo.digest(screen) != black:
            raise nw.Failure("Go Live under Black showed B")
        nw.press(operator, "pageup")
        summary["a_under_mask"] = status(f"Under mask: {A} · Verse 1", "A under the mask")
        if lo.digest(screen) != black:
            raise nw.Failure("Page Up under Black showed A")
        nw.press(operator, "ctrl+b")
        if not a11y.wait_for(5, lambda: lo.digest(screen) == a1):
            raise nw.Failure("Black off did not reveal A's verse")
        nw.press(operator, "pagedown")
        status(f"On screen: {B} · Verse 1", "B live")
        shot(screen, "09-b-live")

        # 8. Quit without saving.
        nw.press(operator, "ctrl+q")
        time.sleep(0.8)
        nw.press(operator, "shift+tab")
        nw.press(operator, "enter")
        summary["exit"] = app.wait_exit()
        if summary["exit"] != 0:
            raise nw.Failure(f"operator exit status {summary['exit']}")
        if not lo.wait_dead(audience_pid, 5):
            raise nw.Failure("quitting left the audience output running")
        summary["status"] = "PASS"
    except nw.Failure as failure:
        summary["error"] = str(failure)
        print(f"FAIL: {failure}")
    finally:
        if app is not None:
            app.close()
        (args.out / "summary.json").write_text(
            json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8"
        )
        print(json.dumps(summary, indent=2, ensure_ascii=False))
    return 0 if summary["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
