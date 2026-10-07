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
            fields.append(data[4 : 4 + length].decode("utf-8"))
            data = data[4 + length :]
        result.append(fields)
    return result


def section_ids(path):
    with sqlite3.connect(path, timeout=1) as db:
        assert db.execute("PRAGMA user_version").fetchone()[0] == 4
        return [
            row[0]
            for row in db.execute(
                "SELECT i.section FROM songs s JOIN section_ids i "
                "ON i.song=s.id AND i.revision=s.head WHERE s.deleted=0 "
                "ORDER BY s.id,i.position"
            )
        ]


def main():
    artifacts = Path(".amp/in/artifacts/song-library")
    artifacts.mkdir(parents=True, exist_ok=True)
    binary = Path(
        os.environ.get(
            "SELA_BINARY",
            str(
                Path(os.environ.get("CARGO_TARGET_DIR", "target")).resolve()
                / "debug/sela"
            ),
        )
    )
    with tempfile.TemporaryDirectory(prefix="sela-library-") as tmp:
        tmp = Path(tmp)
        database = tmp / "profile" / "library.sqlite"
        with (tmp / "app.log").open("w") as log:

            def start():
                app = subprocess.Popen(
                    [str(binary), "--library", str(database)],
                    env=dict(os.environ, XDG_RUNTIME_DIR=str(tmp)),
                    stdout=log,
                    stderr=log,
                )
                window = ""
                for _ in range(100):
                    assert app.poll() is None, "application exited during startup"
                    found = subprocess.run(
                        ["xdotool", "search", "--onlyvisible", "--pid", str(app.pid)],
                        text=True,
                        capture_output=True,
                        timeout=5,
                        check=False,
                    )
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
                        for key, label in [
                            ("x", "Absolute upper-left X:"),
                            ("y", "Absolute upper-left Y:"),
                            ("w", "Width:"),
                            ("h", "Height:"),
                        ]:
                            if label in line:
                                values[key] = int(line.split(":")[-1].strip())
                    return values

                def click(x, y):
                    focus()
                    g = geometry()
                    run(
                        "xdotool",
                        "mousemove",
                        str(g["x"] + x),
                        str(g["y"] + y),
                        "click",
                        "1",
                    )
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
                    time.sleep(
                        0.3
                    )  # Wait for real native redraw, not a synthesized image.
                    g = geometry()
                    root = str(tmp / "root.png")
                    run("import", "-window", "root", root)
                    run(
                        "convert",
                        root,
                        "-crop",
                        f"{g['w']}x{g['h']}+{g['x']}+{g['y']}",
                        "+repage",
                        str(artifacts / f"{name}.png"),
                    )

                focus()
                time.sleep(1)
                capture("initial")
                key("ctrl+s")
                assert payload(database) == [], "blank title must not save"
                capture("validation")
                click(885, 740)
                assert app.poll() is None, "invalid OK must keep the editor open"
                assert payload(database) == [], "invalid OK must not commit"
                capture("validation-ok")
                click(200, 135)
                write("Original native song")
                click(930, 20)  # Inspector owns metadata, never the Words pane.
                capture("inspector")
                click(600, 175)
                write("Sela fixture author")
                key("Tab")
                write("Original test content, 2026")
                key("Tab")
                write("fixture-only")
                capture("inspector-filled")
                click(930, 20)
                click(150, 405)
                write("First verse")
                key("Return")
                write("A different second line")
                key("Return")
                click(70, 740)
                click(150, 356)
                key("ctrl+a")
                write("Chorus")
                click(150, 440)
                write("Sing together")
                key("Return")
                write("Original chorus ending")
                capture("draft")
                click(100, 175)
                capture("slides")
                click(45, 175)
                key("ctrl+s")
                for _ in range(100):
                    if database.exists() and payload(database):
                        break
                    time.sleep(0.05)
                records = payload(database)
                identities = section_ids(database)
                assert len(identities) == 2 and len(set(identities)) == 2
                expected = [
                    [
                        "Original native song",
                        "Sela fixture author",
                        "Original test content, 2026",
                        "fixture-only",
                        "Verse 1",
                        "First verse\nA different second line\n",
                        "Chorus",
                        "Sing together\nOriginal chorus ending",
                    ]
                ]
                assert records == expected, records
                time.sleep(0.3)
                capture("saved")
                # Save keeps document history. Undo from Title must undo the
                # latest lyric edit, not a separate title-field history.
                click(200, 135)
                key("ctrl+z")
                capture("saved-document-undo")
                key("alt+F4")
                assert app.poll() is None, "undo after save must make the draft dirty"
                assert payload(database) == expected
                click(360, 95)  # Keep editing in the explicit dirty-close strip.
                click(200, 135)
                key("ctrl+shift+z")
                click(150, 740)  # Remove at retained native control focus.
                capture("removed-section")
                key("ctrl+z", "ctrl+shift+z")
                key("ctrl+s")
                time.sleep(0.5)
                assert payload(database) == [expected[0][:6]], (
                    "Remove must remove only Chorus"
                )
                assert section_ids(database) == identities[:1]
                click(200, 135)
                key("ctrl+z")
                capture("restored-section")
                key("ctrl+shift+z", "ctrl+z", "ctrl+s")
                time.sleep(0.5)
                assert payload(database) == expected, (
                    "structural undo must restore both distinct sections"
                )
                assert section_ids(database) == identities
                click(200, 135)
                key("ctrl+a")
                write("Unsaved replacement")
                key("alt+F4")
                assert app.poll() is None, "WM close must retain dirty edits"
                assert payload(database) == expected
                capture("unsaved-close")
                click(360, 95)
                assert app.poll() is None, "Keep editing must not close"
                capture("keep-editing")
                click(200, 135)
                key("ctrl+q")
                click(220, 95)
                assert app.wait(timeout=10) == 0
                assert payload(database) == expected, (
                    "discard must preserve committed content"
                )
                app, window = start()
                focus()
                time.sleep(1)
                click(135, 20)  # Explicit saved-song selector, separate from Words.
                click(80, 142)
                time.sleep(0.5)
                click(135, 20)
                capture("reopened")
                click(80, 250)  # Select second section, not a saved-song row.
                capture("reopened-chorus")
                run("xdotool", "windowsize", "--sync", window, "720", "440")
                time.sleep(0.3)
                capture("compact")
                click(670, 20)
                capture("compact-inspector")
                click(670, 20)
                click(135, 20)
                capture("compact-library-selector")
                click(135, 20)
                g = geometry()
                run(
                    "xdotool",
                    "mousemove",
                    str(g["x"] + 150),
                    str(g["y"] + 320),
                    "click",
                    "--repeat",
                    "8",
                    "--delay",
                    "80",
                    "5",
                )
                time.sleep(0.3)
                capture("compact-scrolled")
                run("xdotool", "windowsize", "--sync", window, "980", "760")
                time.sleep(0.3)
                click(885, 740)  # OK closes only after an actual Saved receipt.
                assert app.wait(timeout=10) == 0
                assert payload(database) == expected
                print(
                    "PASS: native validation, two-section multiline entry, save/document undo/redo, structural undo, independent SQLite bytes, "
                    "WM dirty-close guard, keep editing, discard-close, reopen, resize and clean close"
                )
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
