#!/usr/bin/env python3
"""Linux/X11 native check: Scriptures tab end to end (M4-03/M4-04).

Runs on the shared harness (scripts/native_x11.py, docs/e2e-linux.md).

Usage: python3 scripts/scripture-x11.py [--display :116 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR] [--build]

Seeds a new library (for a private profile) and launches the real operator.
Opening the Scriptures tab installs the bundled public-domain KJV and WEB
off the UI thread: the check waits for both translation rows, then verifies
both `<profile>/Scripture/*.selabible` files exist with the Sela Bible
application id. By mouse and keyboard, with the operator keeping the input
focus:

1. Click the reference box and type "John 3:16-18": three verse rows are
   selected and Preview shows them.
2. Turn Live output on, then Go Live: the captured audience output changes
   and shows text. On the `dual` display (a real 1920x1080 audience) the
   per-slide reference strip is also checked: ink in the strip's band at
   the bottom of the output, where slide text never goes.
3. Type "b", "." and Page Down into the box (Black and Go Live outside text
   fields): the audience output does not change.
4. Type the malformed "John 3:99": the note says "John 3 has 36 verses in
   KJV"; Preview and the audience stay as they were.
5. Retype "John 3:16-18" and click the WEB row: the list switches
   translation, Go Live changes the audience output again.
6. Keyword mode, "only begotten" in KJV: verse rows appear (search starts
   at two characters); a double-click on the first one goes live.
7. Import Bible… through the portal Open dialog (a small synthetic
   Zefania file): the footer reports the import, the new row appears
   labelled "(restricted)", the keyword hits keep their selection (the catalog refresh
   changes nothing the operator chose) and the audience is unchanged.
8. Add to Schedule: the schedule gains the item (title "Untitled
   schedule*"), so Ctrl+Q asks first; Cancel keeps it open.
9. Ctrl+S through the portal Save dialog: the .selaschedule file is
   version 5 with the scripture item (kind, KJV, the verse text pinned in
   it); Ctrl+Q then quits without asking.
10. With KJV.selabible deleted, a second launch opens the file with Ctrl+O
   through the portal Open dialog: the scripture item is listed, previews
   from its snapshot and goes live (the audience shows text).

X11 differences: on the default xvfb display the audience is the 624x351
development window beside the operator, not a monitor-covering window; the
fonts are DejaVu (no Arial). Text in the audience capture is checked as
present (ink box) and changed (digest), not read back. The 351-px-high
development window is below the 360 px the reference strip needs
(`slides::FOOTER_MIN_HEIGHT`), so on xvfb the strip is not drawn and not
checked; run `--kind dual` for the native strip check. File dialogs are the
GTK chooser of xdg-desktop-portal-gtk (docs/e2e-linux.md).
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

APPLICATION_ID = 0x53424942  # "SBIB", src/scripture/store.rs
# The reference strip's band at the bottom of the output: two lines at
# 3.2 % of the height with line height 1.3 (src/slides.rs FOOTER_EM,
# LINE_HEIGHT), so about 8.6 % of a 1080-px output. Slide text is fitted
# above it, so ink in this band is the strip.
STRIP_BAND = 0.085
SCHEDULE = "Scripture service"
SYNTHETIC_ZEFANIA = """<XMLBIBLE biblename="Synthetic Zefania"><INFORMATION>
<identifier>SZEF</identifier><language>IND</language>
<rights>Synthetic rights</rights></INFORMATION>
<BIBLEBOOK bnumber="43"><CHAPTER cnumber="3">
<VERS vnumber="16">Teks sintetis enam belas</VERS>
<VERS vnumber="17">Teks sintetis tujuh belas</VERS>
</CHAPTER></BIBLEBOOK></XMLBIBLE>
"""


def strip_box(image: nx.Image) -> tuple[int, int, int, int] | None:
    top = int(image.height * (1 - STRIP_BAND))
    return image.ink_box(region=(0, top, image.width, image.height))


def saved_schedule(path: Path) -> dict | None:
    """The saved .selaschedule document, once it is complete JSON."""
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    data = session.temp("sela-scripture-")
    library = nx.seed_library(data / "library.sqlite", seed=args.seed)
    app = session.app(["--operator-library", library], name="operator")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)

    def control(aid: str) -> nx.Control:
        return nx.wait_until(
            lambda: app.controls(operator).get(aid), 20, f"control {aid}"
        )

    def click(aid: str) -> None:
        x, y = control(aid).center
        nx.click(operator, x, y)

    def note() -> str | None:
        found = app.controls(operator).get("scripture-note")
        return found.name if found else None

    click("Scriptures")
    started = time.monotonic()
    control("translation-KJV")
    control("translation-WEB")
    summary["bundled_install_s"] = round(time.monotonic() - started, 2)
    folder = data / "Scripture"
    for name in ("KJV.selabible", "WEB.selabible"):
        path = folder / name
        if not path.is_file():
            raise nx.Failure(f"{path} was not installed")
        with nx.sqlite_ro(path) as db:
            app_id = db.execute("PRAGMA application_id").fetchone()[0]
            verses = db.execute("SELECT count(*) FROM verses").fetchone()[0]
        if app_id != APPLICATION_ID:
            raise nx.Failure(f"{name} has application id {app_id:#x}")
        summary[f"{name}_verses"] = verses
    if summary["KJV.selabible_verses"] != 31102:
        raise nx.Failure(f"KJV has {summary['KJV.selabible_verses']} verses")
    session.save(operator, "operator-scriptures-tab")

    click("scripture-reference")
    nx.type_text(operator, "John 3:16-18")
    rows = nx.wait_until(
        lambda: [
            c
            for aid, c in app.controls(operator).items()
            if aid.startswith("verse-") and c.name.startswith("3:1")
        ],
        15,
        "John 3 verse rows",
    )
    summary["verse_rows_seen"] = len(rows)
    time.sleep(0.5)
    session.save(operator, "operator-john-3-16-18")

    click("live-output")
    found = app.audience(15)
    if not found:
        raise nx.Failure(f"no audience window within 15s: {app.log()}")
    _, screen = found
    time.sleep(1.5)
    nx.ensure_focus(operator, "after Live output on")
    blank = nx.digest(screen)
    click("go-live")
    image, summary["go_live_observed_ms"] = nx.wait_for_change(
        screen, blank, 5.0, "John 3:16-18 live"
    )
    if image.ink_box() is None:
        raise nx.Failure("the live scripture slide shows no text")
    session.save(image, "audience-john-3-16-18-kjv")
    live = image.digest()
    summary["audience_size"] = [image.width, image.height]
    if session.display.kind == "dual":
        # A real-size output draws the per-slide reference under the text.
        image = nx.capture_stable(screen)
        strip = strip_box(image)
        summary["reference_strip_box"] = strip
        if strip is None:
            raise nx.Failure(
                f"no reference strip in the bottom {STRIP_BAND:.1%} of the "
                f"{image.width}x{image.height} audience output"
            )
        live = image.digest()

    # Typing never triggers live shortcuts: B, period, Page Down in the box.
    click("scripture-reference")
    nx.press(operator, "End")
    nx.type_text(operator, "b.")
    nx.press(operator, "pagedown")
    time.sleep(1.0)
    if nx.digest(screen) != live:
        raise nx.Failure("typing B, period or Page Down in the box changed the output")
    summary["typing_kept_output"] = True

    nx.press(operator, "ctrl+a", "BackSpace")
    nx.type_text(operator, "John 3:99")
    message = nx.wait_until(note, 10, "the malformed-reference note")
    summary["malformed_note"] = message
    if "36" not in message or "KJV" not in message:
        raise nx.Failure(f"unexpected note {message!r}")
    time.sleep(0.5)
    if nx.digest(screen) != live:
        raise nx.Failure("a malformed reference changed the audience output")
    session.save(operator, "operator-malformed-note")

    nx.press(operator, "ctrl+a", "BackSpace")
    nx.type_text(operator, "John 3:16-18")
    nx.wait_until(lambda: note() is None, 10, "the note to clear")
    click("translation-WEB")
    time.sleep(1.0)
    session.save(operator, "operator-web")
    click("go-live")
    image, summary["web_live_observed_ms"] = nx.wait_for_change(
        screen, live, 5.0, "John 3:16-18 WEB live"
    )
    if image.ink_box() is None:
        raise nx.Failure("the WEB slide shows no text")
    session.save(image, "audience-john-3-16-18-web")
    web = image.digest()

    click("translation-KJV")
    click("scripture-mode-keyword")
    click("scripture-reference")
    nx.type_text(operator, "only begotten")
    first = nx.wait_until(
        lambda: app.controls(operator).get("verse-0"), 15, "keyword hits"
    )
    summary["first_hit"] = first.name[:80]
    if "begotten" not in first.name:
        raise nx.Failure(f"unexpected first hit {first.name!r}")
    session.save(operator, "operator-keyword-hits")
    x, y = first.center
    nx.double_click(operator, x, y)
    image, summary["keyword_live_observed_ms"] = nx.wait_for_change(
        screen, web, 5.0, "keyword hit live"
    )
    session.save(image, "audience-keyword-hit")
    keyword_live = nx.capture_stable(screen).digest()

    # Import Bible… through the portal Open dialog. The catalog refresh
    # after it must leave the hit list and its selection alone.
    count_before = control("scripture-selection-count").name
    bible = data / "synthetic-zefania.xml"
    bible.write_text(SYNTHETIC_ZEFANIA, encoding="utf-8")
    click("scripture-import")
    time.sleep(0.8)
    session.save(nx.capture_stable(app.file_dialog()), "import-dialog")
    app.drive_file_dialog(bible, owner=operator)
    row = control("translation-SZEF")
    summary["imported_row"] = row.name
    if not row.name.endswith("(restricted)"):
        raise nx.Failure(
            f"the imported row does not say it is restricted: {row.name!r}"
        )

    def message() -> str | None:
        found = app.controls(operator).get("scripture-message")
        return found.name if found and found.name.startswith("Imported") else None

    summary["import_message"] = nx.wait_until(message, 15, "the import message")
    if "SZEF" not in summary["import_message"]:
        raise nx.Failure(f"unexpected import message {summary['import_message']!r}")
    if not (folder / "SZEF.selabible").is_file():
        raise nx.Failure("the imported translation file was not written")
    time.sleep(1.0)
    if nx.digest(screen) != keyword_live:
        raise nx.Failure("importing a Bible changed the audience output")
    count_after = control("scripture-selection-count").name
    summary["selection_after_import"] = count_after
    if count_after != count_before:
        raise nx.Failure(
            f"the import changed the selection: {count_before!r} -> {count_after!r}"
        )
    session.save(operator, "operator-imported")

    # Add to Schedule pins the selected verse as a schedule item: the
    # schedule becomes unsaved (window title) and Ctrl+Q asks first. The
    # AT-SPI frame is found by title, so this runs last.
    def unsaved_title() -> str | None:
        title = nx.window_name(operator)
        return title if title.startswith("Untitled schedule*") else None

    click("scripture-add")
    summary["title_after_add"] = nx.wait_until(
        unsaved_title, 10, "the schedule to gain the scripture item"
    )
    time.sleep(0.5)
    session.save(operator, "operator-schedule-item")
    nx.press(operator, "ctrl+q")
    time.sleep(1.0)
    if not app.running():
        raise nx.Failure("Ctrl+Q quit with an unsaved scripture schedule item")
    nx.press(operator, "enter")  # Cancel
    time.sleep(0.5)

    # Save: version 5 with the scripture item pinned in the file.
    nx.press(operator, "ctrl+s")
    time.sleep(0.8)
    session.save(nx.capture_stable(app.file_dialog()), "save-dialog")
    app.drive_file_dialog(data / SCHEDULE, owner=operator)
    schedule_file = data / f"{SCHEDULE}.selaschedule"
    document = nx.wait_until(
        lambda: saved_schedule(schedule_file), 5, "Ctrl+S to save the schedule file"
    )
    items = document.get("items") or []
    item = items[0] if items else {}
    verses = item.get("verses") or []
    summary["saved"] = {
        "version": document.get("version"),
        "kinds": [i.get("kind") for i in items],
        "title": item.get("title"),
        "translation": item.get("translation"),
        "verses": len(verses),
    }
    if (
        document.get("version") != 5
        or summary["saved"]["kinds"] != ["scripture"]
        or item.get("translation") != "KJV"
        or not any("begotten" in str(v) for v in verses)
    ):
        raise nx.Failure(f"unexpected saved schedule {summary['saved']}")
    nx.press(operator, "ctrl+q")  # saved: no question
    summary["exit"] = app.wait_exit()
    if summary["exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['exit']}")
    session.close_app(app)

    # Reopen with the translation file gone: the snapshot still plays.
    (folder / "KJV.selabible").unlink()
    app = session.app(["--operator-library", library], name="reopened")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)
    nx.press(operator, "ctrl+o")
    time.sleep(0.8)
    session.save(nx.capture_stable(app.file_dialog()), "open-dialog")
    app.drive_file_dialog(schedule_file, owner=operator)

    def reopened_title() -> str | None:
        title = nx.window_name(operator)
        return title if title.startswith(SCHEDULE) else None

    summary["reopened_title"] = nx.wait_until(
        reopened_title, 10, "the window title to name the reopened schedule"
    )

    def reopened_item() -> nx.Control | None:
        # The AT-SPI frame is found by title, and its name can trail the X
        # window title just after Ctrl+O: retry instead of failing at once.
        try:
            return app.controls(operator, timeout=5).get("schedule-item-0")
        except nx.Failure:
            return None

    reopened = nx.wait_until(reopened_item, 40, "the reopened scripture item")
    summary["reopened_item"] = reopened.name
    if "(KJV)" not in reopened.name:
        raise nx.Failure(f"unexpected reopened item {reopened.name!r}")
    click("schedule-item-0")
    click("live-output")
    found = app.audience(15)
    if not found:
        raise nx.Failure(f"no audience window within 15s: {app.log()}")
    _, screen = found
    time.sleep(1.5)
    nx.ensure_focus(operator, "after Live output on (reopened)")
    blank = nx.digest(screen)
    click("go-live")
    image, summary["reopened_live_observed_ms"] = nx.wait_for_change(
        screen, blank, 5.0, "reopened scripture item live"
    )
    if image.ink_box() is None:
        raise nx.Failure("the reopened scripture item shows no text")
    session.save(image, "audience-reopened-item")
    session.save(operator, "operator-reopened")
    nx.press(operator, "ctrl+q")
    summary["reopened_exit"] = app.wait_exit()
    if summary["reopened_exit"] != 0:
        raise nx.Failure(f"reopened operator exit status {summary['reopened_exit']}")
    return (
        "bundled KJV/WEB installed off the UI thread (31,102 KJV verses); 'John 3:16-18' typed, "
        "previewed and sent live (audience changed); "
        "B/./PageDown in the box left the output; "
        "'John 3:99' noted without changing Preview or Live; WEB switch went live; "
        "keyword 'only begotten' found and went live by double-click; "
        "Import Bible… through the portal dialog added SZEF; "
        + (
            "reference strip drawn on the 1080p output; "
            if "reference_strip_box" in summary
            else ""
        )
        + "Add to Schedule made the schedule unsaved (guarded quit); Ctrl+S saved it as "
        "version 5 through the portal Save dialog; with KJV.selabible deleted, Ctrl+O "
        "reopened it through the portal Open dialog and the item went live from its snapshot"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "scripture-x11"))
