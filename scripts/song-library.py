#!/usr/bin/env python3
"""Native authoring checks against a disposable profile, not a mock UI."""
import os
import sqlite3
import struct
import subprocess
import tempfile
import time
from pathlib import Path


def run(*args):
    return subprocess.check_output(args, text=True, timeout=10).strip()


def payload(path):
    with sqlite3.connect(path, timeout=1) as db:
        rows = db.execute(
            "SELECT r.payload FROM songs s JOIN song_revisions r "
            "ON r.id=s.id AND r.revision=s.head WHERE s.deleted=0"
        ).fetchall()
    result = []
    for (data,) in rows:
        fields = []
        while data:
            length = struct.unpack("<I", data[:4])[0]
            fields.append(data[4:4 + length].decode("utf-8"))
            data = data[4 + length:]
        result.append(fields)
    return result


def main():
    artifacts = Path(".amp/in/artifacts/song-library")
    artifacts.mkdir(parents=True, exist_ok=True)
    binary = Path(os.environ.get("SELA_BINARY", str(
        Path(os.environ.get("CARGO_TARGET_DIR", "target")).resolve() / "debug/sela")))
    with tempfile.TemporaryDirectory(prefix="sela-library-") as tmp:
        tmp = Path(tmp)
        database = tmp / "profile" / "library.sqlite"
        with (tmp / "app.log").open("w") as log:
            def start():
                app = subprocess.Popen([str(binary), "--library", str(database)],
                                       env=dict(os.environ, XDG_RUNTIME_DIR=str(tmp)),
                                       stdout=log, stderr=log)
                window = ""
                for _ in range(100):
                    assert app.poll() is None, "application exited during startup"
                    found = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(app.pid)],
                                           text=True, capture_output=True, timeout=5, check=False)
                    if found.stdout.strip():
                        window = found.stdout.splitlines()[0]
                        break
                    time.sleep(0.1)
                assert window, "window startup deadline"
                return app, window

            app, window = start()
            try:

                def focus():
                    run("xdotool", "windowactivate", "--sync", window)
                    assert run("xdotool", "getwindowfocus") == window

                def geometry():
                    values = {}
                    for line in run("xwininfo", "-id", window).splitlines():
                        for key, label in [("x", "Absolute upper-left X:"), ("y", "Absolute upper-left Y:"),
                                           ("w", "Width:"), ("h", "Height:")]:
                            if label in line:
                                values[key] = int(line.split(":")[-1].strip())
                    return values

                def click(x, y):
                    focus()
                    g = geometry()
                    run("xdotool", "mousemove", str(g["x"] + x), str(g["y"] + y), "click", "1")
                    time.sleep(0.15)

                def key(*keys):
                    focus()
                    run("xdotool", "key", "--clearmodifiers", *keys)
                    time.sleep(0.15)

                def write(text):
                    focus()
                    run("xdotool", "type", "--clearmodifiers", "--delay", "1", text)
                    time.sleep(0.15)

                def capture(name):
                    g = geometry()
                    root = str(tmp / "root.png")
                    run("import", "-window", "root", root)
                    run("convert", root, "-crop", f"{g['w']}x{g['h']}+{g['x']}+{g['y']}",
                        "+repage", str(artifacts / f"{name}.png"))

                focus()
                time.sleep(1)
                capture("initial")
                key("ctrl+s")
                assert payload(database) == [], "blank title must not save"
                capture("validation")
                click(400, 130)
                write("Original native song")
                key("Tab")
                write("Sela fixture author")
                key("Tab")
                write("Original test content, 2026")
                key("Tab")
                write("fixture-only")
                key("Tab", "Tab")
                write("First verse")
                key("Return")
                write("A different second line")
                key("Return")
                click(492, 384)
                click(400, 446)
                key("ctrl+a")
                write("Chorus")
                key("Tab")
                write("Sing together")
                key("Return")
                write("Original chorus ending")
                capture("draft")
                key("ctrl+s")
                for _ in range(100):
                    if database.exists() and payload(database):
                        break
                    time.sleep(0.05)
                records = payload(database)
                expected = [["Original native song", "Sela fixture author",
                             "Original test content, 2026", "fixture-only",
                             "Verse 1", "First verse\nA different second line\n",
                             "Chorus", "Sing together\nOriginal chorus ending"]]
                assert records == expected, records
                time.sleep(0.3)
                capture("saved")
                # Save keeps document history. Undo from Title must undo the
                # latest lyric edit, not a separate title-field history.
                click(400, 130)
                key("ctrl+z")
                capture("saved-document-undo")
                key("alt+F4")
                assert app.poll() is None, "undo after save must make the draft dirty"
                assert payload(database) == expected
                key("shift+Tab", "Return")
                click(400, 130)
                key("ctrl+shift+z")
                # Section controls precede fields in the existing tab order:
                # Title -> Refresh -> Remove. Undo/redo must work at control focus.
                key("shift+Tab", "shift+Tab", "Return")
                capture("removed-section")
                key("ctrl+z", "ctrl+shift+z")
                key("ctrl+s")
                time.sleep(0.5)
                assert payload(database) == [expected[0][:6]], "Remove must remove only Chorus"
                click(400, 130)
                key("ctrl+z")
                capture("restored-section")
                key("ctrl+shift+z", "ctrl+z", "ctrl+s")
                time.sleep(0.5)
                assert payload(database) == expected, "structural undo must restore both distinct sections"
                click(400, 130)
                key("ctrl+a")
                write("Unsaved replacement")
                key("alt+F4")
                assert app.poll() is None, "WM close must retain dirty edits"
                assert payload(database) == expected
                capture("unsaved-close")
                key("shift+Tab", "Return")
                assert app.poll() is None, "Keep editing must not close"
                capture("keep-editing")
                click(400, 130)
                key("ctrl+q")
                # From Title, the two preceding tab stops are the close choices.
                key("shift+Tab", "shift+Tab", "Return")
                assert app.wait(timeout=10) == 0
                assert payload(database) == expected, "discard must preserve committed content"
                app, window = start()
                focus()
                time.sleep(1)
                click(80, 142)
                time.sleep(0.5)
                capture("reopened")
                click(423, 384)
                capture("reopened-chorus")
                run("xdotool", "windowsize", "--sync", window, "720", "440")
                time.sleep(0.3)
                capture("compact")
                g = geometry()
                run("xdotool", "mousemove", str(g["x"] + 500), str(g["y"] + 320),
                    "click", "--repeat", "8", "--delay", "80", "5")
                time.sleep(0.3)
                capture("compact-scrolled")
                key("ctrl+q")
                assert app.wait(timeout=10) == 0
                assert payload(database) == expected
                print("PASS: native validation, two-section multiline entry, save/document undo/redo, structural undo, independent SQLite bytes, "
                      "WM dirty-close guard, keep editing, discard-close, reopen, resize and clean close")
            finally:
                if app.poll() is None:
                    app.terminate()
                    try:
                        app.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        app.kill()
                        app.wait(timeout=5)


if __name__ == "__main__":
    main()
