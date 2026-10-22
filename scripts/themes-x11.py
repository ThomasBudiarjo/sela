#!/usr/bin/env python3
"""Linux/X11 native check: song themes (M1-08) on the real operator and audience.

Built on the shared harness (scripts/native_x11.py, docs/e2e-linux.md);
the Windows counterpart is scripts/themes-windows.py.

Usage: python3 scripts/themes-x11.py [--display :117 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR]
       [--build] [--tolerate-audience-focus]

Seeds a library with `seed_library --themes` ("Signal Hymn" plus the themes
Evening Navy, Low Contrast, Missing Face and Wide Picture), launches the
operator with a private profile and drives it by real clicks on controls
located by their AccessKit ids over AT-SPI. Checks, in order:

1. Signal Hymn goes live in the built-in look (white text on black).
2. The Themes tab lists the built-in theme first ("Default (built-in)",
   badged Default and In Preview) and the four library themes with rendered
   thumbnails; their accessible names carry the warnings: Low Contrast
   reports low contrast, Missing Face the substituted font, Wide Picture the
   Stretch distortion of its 4:1 image. A capture of the tab is saved.
3. Apply to Song with Evening Navy stores the song's theme (SQLite) and
   restyles Preview, while the audience output stays byte-identical.
4. Go Live then shows the theme on the audience: navy background, gold
   text, left aligned inside the theme's 160-point left margin.
5. The song is added to the schedule; Apply to Item with Low Contrast
   restyles that item only; Go Live shows its pale background; Ctrl+S
   through the GTK portal dialog writes a .selaschedule whose item carries
   the explicit Low Contrast snapshot (theme_explicit true).
6. Edit… opens "Theme Editor - Evening Navy"; Reset Look turns the editor
   preview back to the built-in look; OK saves the theme (SQLite payload
   without format or background) and closes the editor. The audience keeps
   its slide byte-identical; selecting the library song and Go Live re-cues
   it in the reset look (white on black).
7. Ctrl+Q exits cleanly (nothing unsaved) and ends the audience child.

X11 differences (see docs/e2e-linux.md): on the default `xvfb` display the
audience is the 624x351 development window, not a monitor-covering window,
so margins and colors are checked at that size (margins scale with the
output height); `--kind dual` uses the real secondary placement, where the
known X11 audience focus theft fails the run unless
`--tolerate-audience-focus` records it. The Save dialog is the GTK portal
chooser. Fonts are DejaVu; "Sela Missing Face" is absent everywhere by
design. Operator controls are located once and their centers reused: after
the theme editor window closed, AT-SPI walks of the operator tree exceeded
the harness bound on X11, so the editor step comes last; likewise once the
operator's X title names the unsaved schedule the AT-SPI frame lookup by
title fails, so the schedule item is selected with Down.
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

# `seed_library --themes` colors (examples/seed_library.rs).
NAVY_RGB = (20, 30, 90)
GOLD_RGB = (255, 214, 90)
PALE_RGB = (240, 240, 236)
# Evening Navy's left margin, in 1080-line points.
NAVY_LEFT_MARGIN = 160
THEMES = ("Evening Navy", "Low Contrast", "Missing Face", "Wide Picture")


def theme_rows(app: nx.App, operator: int) -> dict[str, tuple[str, nx.Control]]:
    """Theme rows by theme name: (accessible name, control)."""
    rows = {}
    for aid, control in app.controls(operator).items():
        if aid.startswith("theme-") and aid[6:].isdigit():
            name = control.name.split(" · ")[0]
            rows[name] = (control.name, control)
    return rows


def stored_themes(library: Path) -> dict[str, bytes]:
    """Library themes by name, as their stored payloads (`Theme::encode`:
    codec, 16-byte id, name length and name, ...)."""
    with nx.sqlite_ro(library) as db:
        out = {}
        for (payload,) in db.execute("SELECT payload FROM themes"):
            payload = bytes(payload)
            length = payload[17]
            out[payload[18 : 18 + length].decode()] = payload
        return out


def song_theme(library: Path) -> bytes | None:
    with nx.sqlite_ro(library) as db:
        row = db.execute("SELECT theme FROM song_themes").fetchone()
        return bytes(row[0]) if row else None


def neutral_payload(payload: bytes) -> bool:
    """A stored theme without format and background, copyright shown."""
    name_end = 18 + payload[17]
    format_len = payload[name_end]
    rest = payload[name_end + 1 + format_len :]
    background_len = int.from_bytes(rest[:2], "little")
    return format_len == 0 and background_len == 0 and rest[2:] == b"\x01"


def corners(image: nx.Image) -> list[tuple[int, int, int]]:
    w, h = image.width, image.height
    return [
        image.pixel(x, y) for x, y in ((6, 6), (w - 6, 6), (6, h - 6), (w - 6, h - 6))
    ]


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    data = session.temp("sela-themes-")
    library = nx.seed_library(data / "library.sqlite", "--themes", seed=args.seed)
    stored = stored_themes(library)
    if sorted(stored) != sorted(THEMES):
        raise nx.Failure(f"seeded themes {sorted(stored)}")
    app = session.app(["--operator-library", library], name="operator")
    operator = app.window()
    if nx.window_name(operator) != nx.OPERATOR_TITLE:
        raise nx.Failure(f"unexpected operator window {nx.window_name(operator)!r}")
    nx.activate(operator)
    nx.resize(operator, 1290, 1050)  # left of the 624x351 audience window
    time.sleep(1.0)  # the storage worker loads Songs and themes off the UI thread
    # Control centers by id, looked up once and reused (see the docstring).
    centers: dict[str, tuple[int, int]] = {}

    def click(aid: str, wid: int = operator) -> None:
        if wid != operator:
            nx.click(wid, *app.control(aid, wid).center)
        else:
            if aid not in centers:
                centers[aid] = app.control(aid, operator).center
            nx.click(operator, *centers[aid])
        time.sleep(0.25)

    # 1. Built-in look live.
    click("song-0")
    click("live-output")
    found = app.audience(15)
    if not found:
        raise nx.Failure(f"no audience window: {app.log()}")
    pid, screen = found
    time.sleep(1.5)
    if nx.focused() != operator:
        message = f"the audience took the focus ({nx.describe_window(nx.focused())})"
        if not args.tolerate_audience_focus:
            raise nx.Failure(message)
        summary.setdefault("tolerated_audience_focus", []).append(message)
        nx.activate(operator)
    blank = nx.digest(screen)
    click("go-live")
    nx.wait_for_change(screen, blank, 5.0, "built-in go live")
    image = nx.capture_stable(screen)
    session.save(image, "audience-built-in")
    if image.ink_box() is None or any(max(c) > 20 for c in corners(image)):
        raise nx.Failure(f"built-in slide: corners {corners(image)}")
    plain = image.digest()

    # 2. The Themes tab with thumbnails and warnings.
    click("Themes")

    def listed():
        rows = theme_rows(app, operator)
        names = {n: label for n, (label, _) in rows.items()}
        ready = (
            len(rows) == 5
            and "Low contrast" in names.get("Low Contrast", "")
            and "is not installed" in names.get("Missing Face", "")
            and "Stretch distorts" in names.get("Wide Picture", "")
        )
        return rows if ready else None

    rows = nx.wait_until(listed, 30, "five theme rows with their warnings")
    names = {n: label for n, (label, _) in rows.items()}
    summary["theme_rows"] = names
    # The previewed song renders with the built-in theme, so its row is
    # also badged In Preview.
    if names.get("Default (built-in)") != "Default (built-in) · Default · In Preview":
        raise nx.Failure(f"built-in row {names.get('Default (built-in)')!r}")
    if names["Evening Navy"] != "Evening Navy":
        raise nx.Failure(
            f"Evening Navy should have no warning: {names['Evening Navy']!r}"
        )
    for name, (_, control) in rows.items():
        centers[f"row:{name}"] = control.center
    for aid in ("theme-apply-song", "theme-apply-item", "theme-edit", "Songs"):
        centers[aid] = app.control(aid, operator).center
    time.sleep(0.5)
    tab = session.save(operator, "operator-themes-tab")
    summary["themes_tab_colors"] = tab.colors()

    # 3. Apply to Song: Preview restyles, Live does not.
    click("row:Evening Navy")
    click("theme-apply-song")
    nx.wait_until(
        lambda: song_theme(library) == stored["Evening Navy"][1:17],
        5,
        "the song's theme stored",
    )
    time.sleep(1.5)
    if nx.digest(screen) != plain:
        raise nx.Failure("applying a theme to the song changed the audience output")
    session.save(operator, "operator-applied-song")
    summary["live_unchanged_after_apply"] = True

    # 4. Go Live: the theme reaches the audience.
    click("go-live")
    _, elapsed = nx.wait_for_change(screen, plain, 5.0, "navy go live")
    image = nx.capture_stable(screen)
    session.save(image, "audience-navy")
    summary["navy_observed_ms"] = elapsed
    if not all(nx.near(c, NAVY_RGB, 10) for c in corners(image)):
        raise nx.Failure(f"navy background: corners {corners(image)}")
    gold = image.box(GOLD_RGB, tolerance=40)
    summary["navy_gold_box"] = gold
    margin = NAVY_LEFT_MARGIN * image.height / 1080
    if gold is None or gold[0] < margin - 3 or gold[0] > image.width * 0.4:
        raise nx.Failure(
            f"gold text not left aligned at the margin {margin:.0f}: {gold}"
        )
    navy = image.digest()

    # 5. A schedule item's own theme, saved in the schedule file.
    click("Songs")
    click("song-0")
    click("add-to-schedule")
    # Down selects the first schedule item (the X title now names the
    # unsaved schedule, which the AT-SPI frame lookup does not follow).
    nx.press(operator, "down")
    time.sleep(0.5)
    click("Themes")
    click("row:Low Contrast")
    click("theme-apply-item")
    time.sleep(1.0)
    click("go-live")
    nx.wait_for_change(screen, navy, 5.0, "item theme go live")
    image = nx.capture_stable(screen)
    session.save(image, "audience-item-theme")
    if not all(nx.near(c, PALE_RGB, 10) for c in corners(image)):
        raise nx.Failure(f"item theme background: corners {corners(image)}")
    pale = image.digest()
    path = data / "Themed.selaschedule"
    nx.press(operator, "ctrl+s")
    app.drive_file_dialog(path, owner=operator)
    nx.wait_until(path.exists, 10, "the schedule file")
    document = json.loads(path.read_text(encoding="utf-8"))
    item = document["items"][0]
    summary["schedule_item_theme_explicit"] = item.get("theme_explicit")
    if document["version"] != 4 or item.get("theme_explicit") is not True:
        raise nx.Failure(f"schedule file item theme: {item.get('theme_explicit')!r}")
    if bytes.fromhex(item["theme"])[1:17] != stored["Low Contrast"][1:17]:
        raise nx.Failure("the saved snapshot is not Low Contrast")
    session.save(operator, "operator-item-theme")

    # 6. Theme editor: Reset Look, OK; Live waits for a new Go Live.
    summary["centers"] = {k: list(v) for k, v in centers.items()}
    click("row:Evening Navy")
    known = app.windows()
    click("theme-edit")
    editor = app.new_window(known)
    title = nx.window_name(editor)
    summary["editor_title"] = title
    if title != "Theme Editor - Evening Navy":
        raise nx.Failure(f"theme editor title {title!r}")
    nx.activate(editor)
    time.sleep(1.5)
    before = session.save(editor, "editor-navy")
    click("editor-button-30", editor)  # Reset Look
    time.sleep(1.5)
    after = session.save(editor, "editor-reset")
    if after.count(NAVY_RGB, tolerance=8) >= before.count(NAVY_RGB, tolerance=8):
        raise nx.Failure("Reset Look did not take the navy out of the editor preview")
    click("editor-button-15", editor)  # OK
    nx.wait_until(
        lambda: not nx.window_exists(editor) or not nx.geometry(editor).viewable,
        10,
        "the theme editor to close",
    )
    nx.wait_until(
        lambda: neutral_payload(stored_themes(library)["Evening Navy"]),
        5,
        "the reset theme stored",
    )
    nx.activate(operator)
    time.sleep(1.5)
    if nx.digest(screen) != pale:
        raise nx.Failure("saving the theme changed the audience output before Go Live")
    summary["live_unchanged_after_edit"] = True
    # The library song (Evening Navy, now reset) re-cued: the edited look.
    click("Songs")
    click("song-0")
    time.sleep(1.0)
    click("go-live")
    nx.wait_for_change(screen, pale, 5.0, "re-cued go live")
    image = nx.capture_stable(screen)
    session.save(image, "audience-recued")
    if image.ink_box() is None or any(max(c) > 20 for c in corners(image)):
        raise nx.Failure(
            f"re-cued slide not in the reset look: corners {corners(image)}"
        )

    # 7. Quit.
    nx.press(operator, "ctrl+q")
    summary["operator_exit"] = app.wait_exit()
    if summary["operator_exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['operator_exit']}")
    if not nx.wait_dead(pid, 5):
        raise nx.Failure("Ctrl+Q left the audience output running")
    return (
        "the Themes tab lists the built-in and four library themes with thumbnails and font, contrast "
        "and aspect warnings; Apply to Song restyles Preview while Live stays identical until Go Live "
        "shows navy, gold, left-margined text; a schedule item's own theme goes live and is saved as an "
        "explicit snapshot; the theme editor's Reset Look and OK save the theme while Live keeps its "
        "scene until the song is re-cued in the reset look; Ctrl+Q exits cleanly"
    )


def options(parser) -> None:
    parser.add_argument(
        "--tolerate-audience-focus",
        action="store_true",
        help="record (instead of failing on) the audience taking the focus (known on X11 dual displays)",
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "themes-x11", options=options))
