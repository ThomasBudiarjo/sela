"""Native check: song metadata fields, the copyright strip on the audience,
metadata search and modal focus scopes (song-metadata-fields; VAL-SONG-001..003,
VAL-FOCUS-001).

Run 1 (`seed_library --metadata-songs`, Live output on):

- Go Live on "Metadata Hymn" draws the copyright strip on the audience at the
  bottom of its first and last slide, "Plain Hymn" (no copyright text) draws
  none (VAL-SONG-002; EW8-OBS-063: every slide unless "first slide only");
- the Songs search finds it by author, copyright, administrator and song
  number (VAL-SONG-003; docs/search.md);
- Edit Song opens the editor: the Inspector shows Administrator and Song
  number, a non-numeric Song number is refused with a visible message and the
  database keeps the old revision, a valid edit saves a new revision whose
  `song_metadata` row holds the values, and reopening shows the same
  Inspector (VAL-SONG-001);
- closing a dirty editor opens the Yes/No/Cancel prompt as a modal: the focus
  fill never leaves it under Tab/Shift+Tab, typed keys, Ctrl+B, Page Down and
  clicks on the editor behind change nothing, the live audience frame does not
  change and its change count (the receipt proxy) stays 0 (VAL-FOCUS-001);
- Options (Edit > Options) is checked the same way, with the audience frame
  and change count as evidence.

Run 2 (`seed_library --vanished-schedule`): the preflight report opened by
Ctrl+O traps Tab, ignores Ctrl+B and Page Down, and leaves the audience frame
untouched.

"Receipts" are audience frame changes sampled every 100 ms while the
operator is driven: the renderer acknowledges a cue only by changing its
frame, so 0 changes means no cue was applied.

Takes over the real keyboard and mouse; never run with EasyWorship open.

Run: python scripts/song-metadata-windows.py [--out DIR] [--binary PATH] [--seed PATH]
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
lo = a11y.lo

FOCUS_BGR = (0xFA, 0xE3, 0xDC)
# Logical-pixel anchors of the standalone-sized editor window (1180x740).
INSPECTOR = (-44, 34)
FIELD_X = 790
FIELD_Y = {
    "authors": 184,
    "copyright": 255,
    "license": 325,
    "administrator": 395,
    "number": 465,
}
TITLE = (80, 20)
MAIN_IDS = {"go-live", "mask-black", "mask-clear", "mask-logo", "live-next", "Songs"}


def revisions(library: pathlib.Path, title: str) -> list[tuple[int, str, str]]:
    """(revision, administrator, song number) of every revision of `title`."""
    db = sqlite3.connect(f"file:{library}?mode=ro", uri=True)
    try:
        rows = db.execute(
            "SELECT r.revision, COALESCE(m.administrator,''), COALESCE(m.song_number,'') "
            "FROM song_revisions r LEFT JOIN song_metadata m "
            "ON m.song=r.id AND m.revision=r.revision "
            "WHERE instr(r.payload, CAST(? AS BLOB)) > 0 ORDER BY r.revision",
            (title,),
        ).fetchall()
        return [tuple(row) for row in rows]
    finally:
        db.close()


def song_modified(library: pathlib.Path) -> list[int]:
    db = sqlite3.connect(f"file:{library}?mode=ro", uri=True)
    try:
        return [r[0] for r in db.execute("SELECT modified FROM songs WHERE deleted=0")]
    finally:
        db.close()


def strip_ink(pixels: bytes, width: int, height: int, top: float, bottom: float) -> int:
    """Light pixels in a horizontal band of the audience frame."""
    count = 0
    for y in range(int(height * top), int(height * bottom), 2):
        row = y * width * 4
        for x in range(0, width, 2):
            i = row + x * 4
            if pixels[i] > 160 and pixels[i + 1] > 160 and pixels[i + 2] > 160:
                count += 1
    return count


def focus_fill_outside(
    pixels: bytes, width: int, height: int, box: tuple[int, int, int, int]
) -> tuple[int, int]:
    """(focus-fill pixels outside `box`, inside it) over a sampled grid."""
    outside = inside = 0
    left, top, right, bottom = box
    for y in range(0, height, 3):
        row = y * width * 4
        for x in range(0, width, 3):
            i = row + x * 4
            if lo.near(tuple(pixels[i : i + 3]), FOCUS_BGR, 4):
                if left <= x < right and top <= y < bottom:
                    inside += 1
                else:
                    outside += 1
    return outside, inside


class Audience:
    """The audience window and its frame-change counter."""

    def __init__(self, session):
        found = lo.audience(session.app.process.pid, 15)
        if not found:
            raise nw.Failure(f"no audience window: {session.app.log()}")
        self.pid, self.hwnd = found
        time.sleep(1.0)

    def digest(self) -> str:
        return lo.digest(self.hwnd)

    def frame(self) -> tuple[int, int, bytes]:
        return nw.capture_client(self.hwnd)

    def save(self, out: pathlib.Path, name: str) -> None:
        nw.write_png(out / f"{name}.png", *self.frame())

    def settled(self) -> str:
        return a11y.stable_digest(self.hwnd)

    def changes(self, seconds: float, action=None) -> int:
        """Frame changes seen while `action` runs and for `seconds` after."""
        last = self.digest()
        seen = 0
        if action:
            action()
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            time.sleep(0.1)
            now = self.digest()
            if now != last:
                seen += 1
                last = now
        return seen


def editor_window(session, timeout: float = 10.0) -> int:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        for hwnd in nw.top_windows(session.app.process.pid):
            if nw.title(hwnd).startswith("Song Editor"):
                return hwnd
        time.sleep(0.2)
    raise nw.Failure("the Song Editor window did not open")


class Editor:
    def __init__(self, session, hwnd: int):
        self.session, self.hwnd = session, hwnd
        nw.activate(hwnd)
        time.sleep(0.8)
        self.scale = nw.dpi(hwnd) / 96

    def at(self, x: int, y: int) -> tuple[int, int]:
        width, _ = nw.client_size(self.hwnd)
        x = x if x >= 0 else int(width / self.scale) + x
        return int(x * self.scale), int(y * self.scale)

    def click(self, x: int, y: int) -> None:
        nw.click(self.hwnd, *self.at(x, y))
        time.sleep(0.35)

    def press(self, keys: str, pause: float = 0.2) -> None:
        nw.press(self.hwnd, keys)
        time.sleep(pause)

    def fill(self, field: str, text: str) -> None:
        self.click(FIELD_X, FIELD_Y[field])
        self.press("ctrl+a")
        if text:
            nw.type_text(self.hwnd, text)
        else:
            self.press("delete")
        time.sleep(0.3)

    def pixels(self) -> tuple[int, int, bytes]:
        return nw.capture_client(self.hwnd)

    def save(self, out: pathlib.Path, name: str) -> bytes:
        shot = self.pixels()
        nw.write_png(out / f"{name}.png", *shot)
        return shot[2]

    def region(self, top: float, bottom: float) -> bytes:
        width, height, pixels = self.pixels()
        return pixels[int(height * top) * width * 4 : int(height * bottom) * width * 4]


def differing_pixels(a: bytes, b: bytes) -> int:
    """Pixels that differ between two equally sized captures."""
    if len(a) != len(b):
        return len(a) + len(b)
    return sum(1 for i in range(0, len(a), 4) if a[i : i + 3] != b[i : i + 3])


def go_live(session, row: int) -> None:
    session.click_aid(f"schedule-item-{row}")
    session.press("pagedown", pause=1.0)


def metadata_run(args, summary: dict, check) -> None:
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="sela-metadata-"))
    library = scratch / "library.sqlite"
    subprocess.run([str(args.seed), str(library), "--metadata-songs"], check=True)
    session = rb.Session(args, summary, check, scratch, library)
    out = args.out
    try:
        a11y.show_live_output(session.hwnd)
        audience = Audience(session)
        blank = audience.settled()
        found = a11y.nodes_by_id(session.doc())
        session.click(found["song-0"])
        session.click(a11y.nodes_by_id(session.doc())["add-to-schedule"])
        session.click(a11y.nodes_by_id(session.doc())["song-1"])
        session.click(a11y.nodes_by_id(session.doc())["add-to-schedule"])
        session.wait(
            "both songs are scheduled",
            lambda d: "schedule-item-1" in a11y.nodes_by_id(d),
        )

        # --- VAL-SONG-002: the copyright strip on the audience -----------
        go_live(session, 0)
        first = audience.settled()
        check.expect(first != blank, "Go Live on Metadata Hymn changes the audience")
        width, height, pixels = audience.frame()
        footer_first = strip_ink(pixels, width, height, 0.86, 0.99)
        body_first = strip_ink(pixels, width, height, 0.30, 0.70)
        summary["metadata_first_slide"] = {
            "strip_ink": footer_first,
            "body_ink": body_first,
        }
        check.expect(
            footer_first > 200,
            f"first slide shows the copyright strip ({footer_first})",
        )
        check.expect(
            body_first > 500, f"first slide still shows its lyrics ({body_first})"
        )
        audience.save(out, "01-audience-first-slide")
        for _ in range(2):
            session.click_aid("live-next")
            time.sleep(0.8)
        audience.settled()
        width, height, pixels = audience.frame()
        footer_last = strip_ink(pixels, width, height, 0.86, 0.99)
        summary["metadata_last_slide"] = {"strip_ink": footer_last}
        check.expect(
            footer_last > 200, f"last slide shows the copyright strip ({footer_last})"
        )
        audience.save(out, "02-audience-last-slide")
        go_live(session, 1)
        audience.settled()
        width, height, pixels = audience.frame()
        plain = strip_ink(pixels, width, height, 0.86, 0.99)
        body_plain = strip_ink(pixels, width, height, 0.30, 0.70)
        summary["plain_first_slide"] = {"strip_ink": plain, "body_ink": body_plain}
        check.expect(plain < 20, f"a song without copyright shows no strip ({plain})")
        check.expect(body_plain > 500, "the plain song's lyrics are shown")
        audience.save(out, "03-audience-plain-song")
        go_live(session, 0)
        live_frame = audience.settled()

        # --- VAL-SONG-003: metadata search ---------------------------------
        searched: dict[str, list[str]] = {}
        for term, expected in (
            ("quill", True),
            ("7654321", True),
            ("acme", True),
            ("synthetic", True),
            ("nomatchterm", False),
        ):
            session.click_aid("song-search")
            session.press("ctrl+a", pause=0.1)
            nw.type_text(session.hwnd, term)
            time.sleep(1.0)
            found = rb.titles(session.doc())
            searched[term] = found
            check.expect(
                ("Metadata Hymn" in found) == expected,
                f"search {term!r} {'finds' if expected else 'does not find'} Metadata Hymn: {found}",
            )
            if term == "7654321":
                session.save("04-search-song-number")
        summary["search"] = searched
        session.press("escape", pause=0.4)
        session.click_aid("song-search")
        session.press("ctrl+a", pause=0.1)
        session.press("delete", pause=0.6)

        # --- VAL-SONG-001: the editor shows, validates and saves ----------
        session.click(a11y.nodes_by_id(session.doc())["song-0"], right=True)
        time.sleep(0.4)
        session.click(a11y.nodes_by_id(session.doc())["song-menu-edit"])
        editor = Editor(session, editor_window(session))
        time.sleep(1.5)
        editor.click(*INSPECTOR)
        time.sleep(0.6)
        editor.save(out, "05-editor-inspector-loaded")
        summary["editor_title"] = nw.title(editor.hwnd)
        base = revisions(library, "Metadata Hymn")
        check.expect(
            base and base[-1][1:] == ("Acme Administration", "7654321"),
            f"the database holds the seeded metadata: {base}",
        )
        editor.fill("number", "12x")
        editor.press("ctrl+s", pause=0.8)
        editor.save(out, "06-editor-invalid-song-number")
        check.expect(
            revisions(library, "Metadata Hymn") == base,
            "a non-numeric Song number does not save a revision",
        )
        editor.fill("number", "42")
        editor.fill("administrator", "Second Administrator")
        editor.press("ctrl+s", pause=1.2)
        editor.save(out, "07-editor-saved")
        after = revisions(library, "Metadata Hymn")
        summary["revisions"] = {"before": base, "after": after}
        check.expect(
            len(after) == len(base) + 1
            and after[-1][1:] == ("Second Administrator", "42"),
            f"Save writes a new revision with the new metadata: {after}",
        )
        check.expect(
            all(isinstance(m, int) and m > 0 for m in song_modified(library)),
            "every song has a modified stamp",
        )
        saved_shot = editor.region(0.2, 0.7)

        # --- VAL-FOCUS-001: the close prompt -------------------------------
        editor.click(*TITLE)
        nw.type_text(editor.hwnd, " draft")
        time.sleep(0.4)
        live_before = audience.digest()
        panel = (
            int(nw.client_size(editor.hwnd)[0] / 2 - 220 * editor.scale),
            int(nw.client_size(editor.hwnd)[1] / 2 - 100 * editor.scale),
            int(nw.client_size(editor.hwnd)[0] / 2 + 220 * editor.scale),
            int(nw.client_size(editor.hwnd)[1] / 2 + 100 * editor.scale),
        )
        prompt_changes = 0
        editor.press("ctrl+q", pause=0.8)
        editor.save(out, "08-editor-close-prompt")
        width, height, pixels = editor.pixels()
        outside, inside = focus_fill_outside(pixels, width, height, panel)
        check.expect(inside > 20, f"the prompt shows a focused button ({inside})")
        check.expect(outside == 0, f"no focus fill outside the prompt ({outside})")
        leaks = 0
        for step in range(8):
            editor.press("tab" if step < 5 else "shift+tab", pause=0.25)
            width, height, pixels = editor.pixels()
            outside, inside = focus_fill_outside(pixels, width, height, panel)
            if outside or not inside:
                leaks += 1
        check.expect(
            leaks == 0, f"Tab and Shift+Tab stay in the prompt ({leaks} leaks)"
        )
        editor.save(out, "09-prompt-after-tab-walk")

        def hammer() -> None:
            for keys in (
                "x",
                "ctrl+b",
                "pagedown",
                "ctrl+z",
                "ctrl+s",
                "ctrl+c",
                "ctrl+l",
            ):
                editor.press(keys, pause=0.15)
            editor.click(60, 20)
            editor.click(INSPECTOR[0], INSPECTOR[1])
            editor.click(300, 400)

        prompt_changes += audience.changes(1.0, hammer)
        check.expect(
            audience.digest() == live_before == live_frame,
            "the audience frame is unchanged by keys and clicks under the prompt",
        )
        check.expect(
            prompt_changes == 0, f"editor prompt: {prompt_changes} audience receipts"
        )
        summary["receipts"] = {"editor_prompt": prompt_changes}
        editor.save(out, "10-prompt-after-keys-and-clicks")
        # Cancel (Esc) keeps the editor open and dirty.
        editor.press("escape", pause=0.7)
        check.expect(
            nw.user32.IsWindow(editor.hwnd)
            and nw.title(editor.hwnd).startswith("Song Editor"),
            "Esc cancels the prompt and keeps the editor",
        )
        editor.save(out, "11-prompt-cancelled")
        # Reopen shows the saved values: undo the draft, close, reopen.
        editor.press("ctrl+z", pause=0.3)
        editor.press("ctrl+q", pause=0.8)
        for _ in range(3):
            if not nw.user32.IsWindow(editor.hwnd):
                break
            editor.press("shift+tab", pause=0.2)
            editor.press("enter", pause=0.6)
        # Discard through No when the draft was not undone completely.
        if nw.user32.IsWindow(editor.hwnd):
            editor.press("ctrl+q", pause=0.8)
            editor.press("shift+tab", pause=0.2)
            editor.press("enter", pause=0.8)
        nw.activate(session.hwnd)
        time.sleep(0.8)
        session.click(a11y.nodes_by_id(session.doc())["song-0"], right=True)
        time.sleep(0.4)
        session.click(a11y.nodes_by_id(session.doc())["song-menu-edit"])
        reopened = Editor(session, editor_window(session))
        time.sleep(1.5)
        reopened.click(*INSPECTOR)
        time.sleep(0.6)
        reopened.save(out, "12-editor-reopened")
        # The saved capture still has the caret and focus ring of the last
        # field, so only the field's border, caret and the status text may differ.
        moved = differing_pixels(reopened.region(0.2, 0.7), saved_shot)
        summary["reopened_differing_pixels"] = moved
        check.expect(
            moved < 3000,
            f"reopening the song shows the same Inspector values ({moved} px differ)",
        )
        reopened.press("ctrl+q", pause=0.8)

        # --- VAL-FOCUS-001: Options modal ----------------------------------
        nw.activate(session.hwnd)
        time.sleep(0.6)
        session.click(a11y.nodes_by_id(session.doc())["Songs"])
        status = session.node("Live-status", session.doc())
        options_before = audience.digest()

        def options() -> None:
            session.press("alt+e")
            session.press("enter", pause=0.8)
            found = a11y.nodes_by_id(session.doc())
            check.expect("dialog-cancel" in found, "Options dialog is open")
            session.save("13-options-open")
            left = []
            for step in range(12):
                session.press("tab" if step < 7 else "shift+tab", pause=0.2)
                at = session.focused()
                if at in MAIN_IDS or at.startswith(("song-", "schedule-")):
                    left.append(at)
            check.expect(not left, f"Options: Tab never leaves the dialog ({left})")
            session.press("ctrl+b")
            session.press("pagedown")
            session.click(a11y.nodes_by_id(session.doc())["mask-clear"])
            found = a11y.nodes_by_id(session.doc())
            check.expect(
                found["mask-black"].get("toggle") == "Off"
                and found["mask-clear"].get("toggle") == "Off"
                and "dialog-cancel" in found,
                "Options: Ctrl+B, Page Down and a click behind change nothing",
            )
            session.press("escape", pause=0.7)

        options_changes = audience.changes(1.0, options)
        check.expect(
            options_changes == 0, f"Options: {options_changes} audience receipts"
        )
        check.expect(
            audience.digest() == options_before, "Options: audience frame unchanged"
        )
        if status:
            check.expect(
                (session.node("Live-status") or {}).get("name") == status["name"],
                "Options: Live status unchanged",
            )
        summary["receipts"]["options"] = options_changes
        audience.save(out, "14-audience-during-options")
        session.quit()
    finally:
        session.app.close()


def preflight_run(args, summary: dict, check) -> None:
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="sela-metadata-preflight-"))
    library = scratch / "library.sqlite"
    subprocess.run([str(args.seed), str(library), "--vanished-schedule"], check=True)
    session = rb.Session(args, summary, check, scratch, library)
    try:
        a11y.show_live_output(session.hwnd)
        audience = Audience(session)
        blank = audience.settled()

        def report() -> None:
            session.press("ctrl+o", pause=0.8)
            nw.drive_file_dialog(
                session.app.process.pid,
                session.hwnd,
                str(scratch / "Sunday.selaschedule"),
            )
            time.sleep(2.0)
            found = a11y.nodes_by_id(session.doc())
            check.expect("dialog-cancel" in found, "the preflight report is open")
            session.save("15-preflight-report")
            left = []
            for step in range(10):
                session.press("tab" if step < 6 else "shift+tab", pause=0.2)
                at = session.focused()
                if at in MAIN_IDS or at.startswith(("song-", "schedule-")):
                    left.append(at)
            check.expect(not left, f"preflight: Tab never leaves the report ({left})")
            session.press("ctrl+b")
            session.press("pagedown")
            session.click(a11y.nodes_by_id(session.doc())["mask-black"])
            found = a11y.nodes_by_id(session.doc())
            check.expect(
                found["mask-black"].get("toggle") == "Off" and "dialog-cancel" in found,
                "preflight: Ctrl+B, Page Down and a click behind change nothing",
            )
            session.press("escape", pause=0.7)
            check.expect(
                "dialog-cancel" not in a11y.nodes_by_id(session.doc()),
                "Esc closes the preflight report",
            )

        changes = audience.changes(1.0, report)
        check.expect(changes == 0, f"preflight report: {changes} audience receipts")
        check.expect(audience.digest() == blank, "preflight: audience frame unchanged")
        summary["receipts"]["preflight"] = changes
        audience.save(args.out, "16-audience-during-preflight")
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
        default=ROOT / ".amp/in/artifacts/song-metadata-windows",
    )
    parser.add_argument("--monitor", default="secondary")
    args = parser.parse_args()
    args.library = None
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(f"FAIL: missing {path}")
            return 1
    summary: dict = {"status": "FAIL", "binary": str(args.binary.resolve())}
    check = a11y.Check(summary)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    try:
        metadata_run(args, summary, check)
        preflight_run(args, summary, check)
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
