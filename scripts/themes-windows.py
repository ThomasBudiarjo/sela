"""Windows native check: song themes (M1-08) on the real operator and audience.

Windows counterpart of scripts/themes-x11.py (same steps and assertions);
NOT RUN yet: written in the Linux container without a Windows host.

Usage: python scripts/themes-windows.py [--binary target\\debug\\sela.exe]
       [--seed target\\debug\\examples\\seed_library.exe] [--monitor secondary]
       [--out DIR]

Seeds `seed_library --themes` ("Signal Hymn" plus the themes Evening Navy,
Low Contrast, Missing Face and Wide Picture), launches the operator with the
audience on the secondary monitor and drives it by real clicks on controls
located through UI Automation (AccessKit author ids,
scripts/uia-names-windows.ps1):

1. Signal Hymn goes live in the built-in look (white text on black).
2. The Themes tab lists "Default (built-in)" (badged Default and In Preview)
   and the four library themes; their UIA names carry the warnings (Low
   Contrast: low contrast; Missing Face: font not installed; Wide Picture:
   Stretch distorts).
3. Apply to Song with Evening Navy stores the song's theme in SQLite while
   the audience frame stays identical.
4. Go Live shows navy, gold text, left aligned at the 160-point margin.
5. The song is added to the schedule; Apply to Item with Low Contrast goes
   live (pale background); Ctrl+S through the common Save dialog writes a
   .selaschedule whose item carries the explicit snapshot.
6. Edit… opens "Theme Editor - Evening Navy"; Reset Look and OK store the
   reset theme; the audience frame is unchanged until the library song is
   selected and re-cued (white on black again).
7. Ctrl+Q exits cleanly and ends the audience child.

Takes over the real keyboard and mouse; never run with EasyWorship open.
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
lo = a11y.lo

NAVY_RGB = (20, 30, 90)
GOLD_RGB = (255, 214, 90)
PALE_RGB = (240, 240, 236)
NAVY_LEFT_MARGIN = 160
THEMES = ("Evening Navy", "Low Contrast", "Missing Face", "Wide Picture")


def stored_themes(library: pathlib.Path) -> dict[str, bytes]:
    db = sqlite3.connect(f"file:{library}?mode=ro", uri=True)
    try:
        out = {}
        for (payload,) in db.execute("SELECT payload FROM themes"):
            payload = bytes(payload)
            out[payload[18 : 18 + payload[17]].decode()] = payload
        return out
    finally:
        db.close()


def song_theme(library: pathlib.Path) -> bytes | None:
    db = sqlite3.connect(f"file:{library}?mode=ro", uri=True)
    try:
        row = db.execute("SELECT theme FROM song_themes").fetchone()
        return bytes(row[0]) if row else None
    finally:
        db.close()


def neutral_payload(payload: bytes) -> bool:
    name_end = 18 + payload[17]
    format_len = payload[name_end]
    rest = payload[name_end + 1 + format_len :]
    return (
        format_len == 0
        and int.from_bytes(rest[:2], "little") == 0
        and rest[2:] == b"\x01"
    )


def corners(width: int, height: int, bgra: bytes) -> list[tuple[int, int, int]]:
    return [
        lo.at(width, bgra, x, y)
        for x, y in ((6, 6), (width - 6, 6), (6, height - 6), (width - 6, height - 6))
    ]


def wait(timeout: float, condition, what: str):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = condition()
        if value:
            return value
        time.sleep(0.1)
    raise nw.Failure(f"timed out waiting for {what}")


def changed(
    hwnd: int, before: str, what: str, timeout: float = 5.0
) -> tuple[int, int, bytes]:
    wait(timeout, lambda: lo.digest(hwnd) != before, what)
    time.sleep(0.4)
    a11y.stable_digest(hwnd)
    return nw.capture_client(hwnd)


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
    parser.add_argument("--monitor", default="secondary")
    parser.add_argument(
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/themes-windows"
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(
                f"FAIL: missing {path} (cargo build && cargo build --example seed_library)"
            )
            return 1
    summary: dict = {"status": "FAIL", "binary": str(args.binary.resolve())}
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-themes-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library), "--themes"], check=True)
    stored = stored_themes(library)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    app = nw.App(args.binary, ["--operator-library", str(library)])
    try:
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(1.2)

        def save(target: int, name: str) -> None:
            nw.write_png(args.out / f"{name}.png", *nw.capture_client(target))

        def node(aid: str, target: int = 0) -> dict:
            found = a11y.nodes_by_id(a11y.uia(target or hwnd))
            if aid not in found:
                raise nw.Failure(f"{aid} missing from the UIA tree")
            return found[aid]

        def click(aid: str, target: int = 0) -> None:
            a11y.click_node(target or hwnd, node(aid, target))

        # 1. Built-in look live.
        click("song-0")
        click("live-output")
        found = lo.audience(app.process.pid, 15)
        if not found:
            raise nw.Failure(f"no audience window: {app.log()}")
        pid, screen = found
        time.sleep(1.5)
        nw.ensure_foreground(hwnd, "after Live output on")
        blank = lo.digest(screen)
        click("go-live")
        width, height, bgra = changed(screen, blank, "built-in go live")
        nw.write_png(args.out / "audience-built-in.png", width, height, bgra)
        if lo.ink_box(width, height, bgra) is None or any(
            max(c) > 20 for c in corners(width, height, bgra)
        ):
            raise nw.Failure("built-in slide is not white on black")
        plain = lo.digest(screen)

        # 2. Themes tab and warnings.
        click("Themes")

        def rows():
            found = a11y.nodes_by_id(a11y.uia(hwnd))
            named = {
                n["name"].split(" · ")[0]: n
                for aid, n in found.items()
                if aid.startswith("theme-") and aid[6:].isdigit()
            }
            ok = (
                len(named) == 5
                and "Low contrast" in named.get("Low Contrast", {}).get("name", "")
                and "is not installed" in named.get("Missing Face", {}).get("name", "")
                and "Stretch distorts" in named.get("Wide Picture", {}).get("name", "")
            )
            return named if ok else None

        named = wait(30, rows, "five theme rows with warnings")
        summary["theme_rows"] = {k: v["name"] for k, v in named.items()}
        if (
            named["Default (built-in)"]["name"]
            != "Default (built-in) · Default · In Preview"
        ):
            raise nw.Failure(f"built-in row {named['Default (built-in)']['name']!r}")
        save(hwnd, "operator-themes-tab")

        # 3. Apply to Song; Live unchanged.
        a11y.click_node(hwnd, named["Evening Navy"])
        click("theme-apply-song")
        wait(
            5, lambda: song_theme(library) == stored["Evening Navy"][1:17], "song theme"
        )
        time.sleep(1.5)
        if lo.digest(screen) != plain:
            raise nw.Failure("applying a theme to the song changed the audience output")

        # 4. Go Live with the theme.
        click("go-live")
        width, height, bgra = changed(screen, plain, "navy go live")
        nw.write_png(args.out / "audience-navy.png", width, height, bgra)
        if not all(lo.near(c, NAVY_RGB, 10) for c in corners(width, height, bgra)):
            raise nw.Failure("navy background missing")
        gold = lo.color_box(width, height, bgra, GOLD_RGB, 40)
        margin = NAVY_LEFT_MARGIN * height / 1080
        summary["navy_gold_box"] = gold
        if gold is None or gold[0] < margin - 3 or gold[0] > width * 0.4:
            raise nw.Failure(f"gold text not at the left margin: {gold}")
        navy = lo.digest(screen)

        # 5. Item theme and schedule file.
        click("Songs")
        click("song-0")
        click("add-to-schedule")
        click("schedule-item-0")
        click("Themes")
        a11y.click_node(hwnd, named["Low Contrast"])
        click("theme-apply-item")
        click("go-live")
        width, height, bgra = changed(screen, navy, "item theme go live")
        nw.write_png(args.out / "audience-item-theme.png", width, height, bgra)
        if not all(lo.near(c, PALE_RGB, 10) for c in corners(width, height, bgra)):
            raise nw.Failure("item theme background missing")
        pale = lo.digest(screen)
        path = data / "Themed.selaschedule"
        nw.press(hwnd, "ctrl+s")
        nw.drive_file_dialog(app.process.pid, hwnd, str(path))
        wait(10, path.exists, "the schedule file")
        item = json.loads(path.read_text(encoding="utf-8"))["items"][0]
        if item.get("theme_explicit") is not True:
            raise nw.Failure("the saved item has no explicit theme snapshot")

        # 6. Theme editor.
        a11y.click_node(hwnd, named["Evening Navy"])
        known = set(nw.top_windows(app.process.pid))
        click("theme-edit")
        editor = wait(
            10,
            lambda: next(
                (w for w in nw.top_windows(app.process.pid) if w not in known), None
            ),
            "the theme editor",
        )
        if nw.title(editor) != "Theme Editor - Evening Navy":
            raise nw.Failure(f"editor title {nw.title(editor)!r}")
        nw.activate(editor)
        time.sleep(1.5)
        save(editor, "editor-navy")
        click("editor-button-30", editor)
        time.sleep(1.0)
        save(editor, "editor-reset")
        click("editor-button-15", editor)
        wait(
            10,
            lambda: editor not in nw.top_windows(app.process.pid),
            "the editor to close",
        )
        wait(
            5,
            lambda: neutral_payload(stored_themes(library)["Evening Navy"]),
            "reset theme",
        )
        nw.activate(hwnd)
        time.sleep(1.5)
        if lo.digest(screen) != pale:
            raise nw.Failure("saving the theme changed the audience output")
        click("Songs")
        click("song-0")
        time.sleep(1.0)
        click("go-live")
        width, height, bgra = changed(screen, pale, "re-cued go live")
        nw.write_png(args.out / "audience-recued.png", width, height, bgra)
        if any(max(c) > 20 for c in corners(width, height, bgra)):
            raise nw.Failure("re-cued slide is not in the reset look")

        # 7. Quit.
        nw.press(hwnd, "ctrl+q")
        summary["operator_exit"] = app.wait_exit()
        if summary["operator_exit"] != 0 or not lo.wait_dead(pid, 5):
            raise nw.Failure("Ctrl+Q did not exit cleanly")
        summary["status"] = "PASS"
        print(
            "PASS: themes tab, apply to song/item, theme editor, Live unchanged until re-cued"
        )
        return 0
    except nw.Failure as error:
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        (args.out / "summary.json").write_text(
            json.dumps(summary, indent=2), encoding="utf-8"
        )
        app.close()


if __name__ == "__main__":
    sys.exit(main())
