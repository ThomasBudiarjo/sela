"""Native check: the Scriptures tab on the real Windows desktop (M4-03/M4-04).

Windows counterpart of scripts/scripture-x11.py, through UI Automation and
screen captures of the real operator and audience:

- opening the Scriptures tab installs the bundled public-domain KJV and WEB
  off the UI thread (both translation rows appear; both
  `<profile>\\Scripture\\*.selabible` files exist with 31,102 KJV verses);
- typing "John 3:16-18" in the reference box selects three verse rows and
  previews them; Live output on, then Go Live changes the audience output,
  which shows text;
- Add to Schedule makes schedule item 0 "John 3:16-18 (KJV)";
- B, period and Page Down typed into the box change neither the masks nor
  the audience output;
- "John 3:99" shows the note "John 3 has 36 verses in KJV" and leaves the
  audience as it was;
- with "John 3:16-18" again, the WEB row then Go Live changes the output;
- Keyword mode, "only begotten": verse rows appear; a double-click on the
  first one goes live.
- Import Bible… through the native Open dialog (a small synthetic Zefania
  file): the footer reports "Imported SZEF", the new row is labelled
  "(restricted)", the selection count and the audience are unchanged.
- Ctrl+S through the native Save dialog writes a version-5 .selaschedule
  whose first item is the KJV scripture item with its verse text pinned.

Takes over the real keyboard and mouse; never run with EasyWorship open.
Needs a secondary monitor for the audience (`--monitor`, default secondary).

Run: python scripts/scripture-windows.py [--binary target\\debug\\sela.exe]
     [--seed target\\debug\\examples\\seed_library.exe] [--out DIR]
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


lo = _load("live_output_windows", "live-output-windows.py")
a11y = _load("a11y_windows", "a11y-windows.py")

APPLICATION_ID = 0x53424942  # "SBIB", src/scripture/store.rs
SCHEDULE = "Scripture service"
SYNTHETIC_ZEFANIA = """<XMLBIBLE biblename="Synthetic Zefania"><INFORMATION>
<identifier>SZEF</identifier><language>IND</language>
<rights>Synthetic rights</rights></INFORMATION>
<BIBLEBOOK bnumber="43"><CHAPTER cnumber="3">
<VERS vnumber="16">Teks sintetis enam belas</VERS>
<VERS vnumber="17">Teks sintetis tujuh belas</VERS>
</CHAPTER></BIBLEBOOK></XMLBIBLE>
"""


def saved_schedule(path: pathlib.Path) -> dict | None:
    """The saved .selaschedule document, once it is complete JSON."""
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


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
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/scripture-windows"
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
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-scripture-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library)], check=True)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    app = nw.App(args.binary, ["--operator-library", str(library)])
    hwnd = 0
    audience_hwnd = 0
    try:
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(1.2)

        def save(name: str, window: int | None = None) -> None:
            width, height, pixels = nw.capture_client(window or hwnd)
            nw.write_png(args.out / f"{name}.png", width, height, pixels)

        def press(keys: str) -> None:
            try:
                nw.press(hwnd, keys)
            except nw.Failure:
                nw.activate(hwnd)
                summary["focus_recoveries"] = summary.get("focus_recoveries", 0) + 1
                nw.press(hwnd, keys)
            time.sleep(0.12)

        def type_text(text: str) -> None:
            nw.type_text(hwnd, text)
            time.sleep(0.3)

        def node(aid: str, timeout: float = 20.0) -> dict:
            deadline = time.monotonic() + timeout
            while True:
                found = a11y.nodes_by_id(a11y.uia(hwnd))
                if aid in found:
                    return found[aid]
                if time.monotonic() >= deadline:
                    raise nw.Failure(f"control {aid!r} never appeared")
                time.sleep(0.3)

        def click(aid: str) -> None:
            a11y.click_node(hwnd, node(aid))

        click("Scriptures")
        started = time.monotonic()
        node("translation-KJV", 60.0)
        node("translation-WEB", 60.0)
        summary["bundled_install_s"] = round(time.monotonic() - started, 2)
        for name in ("KJV.selabible", "WEB.selabible"):
            path = data / "Scripture" / name
            check.expect(path.is_file(), f"{name} installed")
            if path.is_file():
                with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True) as db:
                    app_id = db.execute("PRAGMA application_id").fetchone()[0]
                    verses = db.execute("SELECT count(*) FROM verses").fetchone()[0]
                check.expect(app_id == APPLICATION_ID, f"{name} application id")
                summary[f"{name}_verses"] = verses
        check.expect(
            summary.get("KJV.selabible_verses") == 31102, "KJV has 31,102 verses"
        )
        save("01-scriptures-tab")

        click("scripture-reference")
        type_text("John 3:16-18")
        time.sleep(1.5)
        doc = a11y.uia(hwnd)
        selected = [
            n["name"]
            for n in doc["nodes"]
            if n["automationId"].startswith("verse-") and n.get("selected") is True
        ]
        summary["selected_rows"] = selected
        check.expect(len(selected) == 3, f"three verse rows selected ({len(selected)})")
        save("02-john-3-16-18")

        a11y.show_live_output(hwnd)
        seen = lo.audience(app.process.pid, 15)
        if not seen:
            raise nw.Failure("no audience window after Live output on")
        _, audience_hwnd = seen
        time.sleep(1.0)
        nw.activate(hwnd)
        blank = lo.digest(audience_hwnd)
        click("go-live")
        live = a11y.wait_for(5.0, lambda: lo.digest(audience_hwnd) != blank)
        check.expect(live and a11y.audience_ink(audience_hwnd), "John 3:16-18 is live")
        live_digest = a11y.stable_digest(audience_hwnd)
        save("03-audience-kjv", audience_hwnd)

        click("scripture-add")
        item = node("schedule-item-0", 10.0)
        summary["schedule_item"] = item["name"]
        check.expect(
            "John 3:16-18 (KJV)" in item["name"], "the passage is a schedule item"
        )

        click("scripture-reference")
        press("end")
        type_text("b.")
        press("pagedown")
        time.sleep(1.0)
        found = a11y.nodes_by_id(a11y.uia(hwnd))
        toggles = {
            aid: found[aid].get("toggle")
            for aid in ("mask-black", "mask-clear", "mask-logo")
        }
        check.expect(
            all(state == "Off" for state in toggles.values()), f"no mask lit: {toggles}"
        )
        check.expect(
            lo.digest(audience_hwnd) == live_digest,
            "typing left the audience unchanged",
        )

        press("ctrl+a")
        press("backspace")
        type_text("John 3:99")
        note = node("scripture-note", 10.0)["name"]
        summary["malformed_note"] = note
        check.expect(note == "John 3 has 36 verses in KJV", f"note {note!r}")
        check.expect(
            lo.digest(audience_hwnd) == live_digest, "a bad reference left Live alone"
        )
        save("04-malformed-note")

        press("ctrl+a")
        press("backspace")
        type_text("John 3:16-18")
        time.sleep(1.0)
        click("translation-WEB")
        time.sleep(1.0)
        click("go-live")
        web = a11y.wait_for(5.0, lambda: lo.digest(audience_hwnd) != live_digest)
        check.expect(web, "the WEB passage went live")
        web_digest = a11y.stable_digest(audience_hwnd)
        save("05-audience-web", audience_hwnd)

        click("translation-KJV")
        click("scripture-mode-keyword")
        click("scripture-reference")
        type_text("only begotten")
        first = node("verse-0", 15.0)
        summary["first_hit"] = first["name"][:80]
        check.expect("begotten" in first["name"], "keyword hits listed")
        origin_x, origin_y = a11y.client_origin(hwnd)
        x = first["x"] + first["width"] // 2 - origin_x
        y = first["y"] + first["height"] // 2 - origin_y
        # Two clicks well inside the system double-click time.
        nw.click(hwnd, x, y)
        nw.click(hwnd, x, y)
        keyword = a11y.wait_for(5.0, lambda: lo.digest(audience_hwnd) != web_digest)
        check.expect(keyword, "a double-clicked hit went live")
        save("06-keyword-live", audience_hwnd)
        keyword_digest = a11y.stable_digest(audience_hwnd)

        # Import Bible… through the native Open dialog; the catalog refresh
        # after it changes nothing the operator chose.
        count_before = node("scripture-selection-count")["name"]
        bible = data / "synthetic-zefania.xml"
        bible.write_text(SYNTHETIC_ZEFANIA, encoding="utf-8")
        click("scripture-import")
        time.sleep(0.8)
        nw.drive_file_dialog(app.process.pid, hwnd, str(bible))
        row = node("translation-SZEF", 15.0)
        summary["imported_row"] = row["name"]
        check.expect(
            row["name"].endswith("(restricted)"), "the imported row says restricted"
        )
        deadline = time.monotonic() + 15
        message = ""
        while time.monotonic() < deadline:
            found = a11y.nodes_by_id(a11y.uia(hwnd)).get("scripture-message")
            message = found["name"] if found else ""
            if message.startswith("Imported"):
                break
            time.sleep(0.3)
        summary["import_message"] = message
        check.expect("SZEF" in message, f"import message {message!r}")
        check.expect(
            (data / "Scripture" / "SZEF.selabible").is_file(), "SZEF.selabible written"
        )
        time.sleep(1.0)
        count_after = node("scripture-selection-count")["name"]
        summary["selection_after_import"] = count_after
        check.expect(count_after == count_before, "the import kept the selection")
        check.expect(
            lo.digest(audience_hwnd) == keyword_digest, "the import left Live alone"
        )
        save("07-imported")

        # Ctrl+S: version 5 with the scripture item pinned in the file.
        press("ctrl+s")
        time.sleep(0.8)
        nw.drive_file_dialog(app.process.pid, hwnd, str(data / SCHEDULE))
        schedule_file = data / f"{SCHEDULE}.selaschedule"
        deadline = time.monotonic() + 5
        while saved_schedule(schedule_file) is None:
            if time.monotonic() > deadline:
                raise nw.Failure("Ctrl+S did not save the schedule file")
            time.sleep(0.05)
        document = saved_schedule(schedule_file)
        items = document.get("items") or []
        first_item = items[0] if items else {}
        summary["saved"] = {
            "version": document.get("version"),
            "kinds": [i.get("kind") for i in items],
            "title": first_item.get("title"),
            "translation": first_item.get("translation"),
        }
        check.expect(document.get("version") == 5, "the schedule file is version 5")
        check.expect(
            first_item.get("kind") == "scripture"
            and first_item.get("translation") == "KJV"
            and any("loved the world" in str(v) for v in first_item.get("verses", [])),
            f"the scripture item is pinned in the file: {summary['saved']}",
        )

        press("ctrl+q")
        time.sleep(0.6)
        if app.process.poll() is None:
            found = a11y.nodes_by_id(a11y.uia(hwnd))
            if "dialog-confirm" in found:
                a11y.click_node(hwnd, found["dialog-confirm"])
        app.wait_exit()
        hwnd = 0
        summary["status"] = "FAIL" if check.failures else "PASS"
        return 0 if summary["status"] == "PASS" else 1
    except Exception as error:  # noqa: BLE001
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        if audience_hwnd:
            for pid in lo.children(app.process.pid):
                lo.kill(pid)
        app.close()
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))
        print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
