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
