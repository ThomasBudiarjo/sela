#!/usr/bin/env python3
"""Linux/X11 native check: standalone schedules, profile backup and restore
(M1-13, backlog scenario S04), end to end on two private profiles.

Built on the shared harness (scripts/native_x11.py, docs/e2e-linux.md); the
Windows counterpart is scripts/bundle-windows.py. Format and policy:
docs/bundle.md.

Usage: python3 scripts/bundle-x11.py [--display :122 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR] [--build]

Machine A: a library seeded with `--portable-song` ("Harbor Light": slide 1
shows harbor.png, slide 2 lantern.png, which is NOT in the profile (its
original sits in originals/), slide 3 names the font "Sela Missing Serif").
The song is added to the schedule, then File > Export Standalone
Schedule... (clicked by AccessKit id) and the portal Save dialog write
usb/Sunday.selapackage. The result dialog must say what was saved and name
lantern.png as not included; the package is then read with Python's own
zipfile module (an independent ZIP reader): CRCs check, the manifest is
format sela-bundle version 1, kind schedule, harbor.png embedded byte for
byte, lantern.png listed as not embedded, the font listed with embedded
false. Profiles > Back Up Profile... writes usb/Ana.selabackup (manifest
kind profile, library.sqlite and harbor.png inside, no sela.lock or
Recovery/). Ctrl+Q, Don't Save quits A.

Machine B: a fresh library at another folder depth. Ctrl+O and the portal
Open dialog pick the package. B already has a different HARBOR.png (a name
that differs only in case), so Sela stores the carried image byte for byte
as "harbor (2).png", leaves HARBOR.png alone, writes
Schedules/Sunday.selaschedule, and the preflight dialog opens with the
package notes ("harbor.png → harbor (2).png", "does not carry ...", the
font) and a Missing row for lantern.png.
Locate... with the original repairs it (lantern.png appears in B with the
pinned SHA-256 and the row goes away). A hostile package (a "../" entry,
written by zipfile) is refused with "Nothing was changed." and nothing is
written outside B. Profiles > Restore Profile... picks the backup and
names a new folder "Ana restored"; the result names --operator-library,
B's own library is byte-identical, and a third launch with the restored
library shows "Harbor Light" in Songs.

X11 differences: the dialogs are the GTK file chooser of
xdg-desktop-portal-gtk (Windows: the common dialogs), driven by typing the
path into its location entry; restoring onto an existing folder is covered
by GPUI tests only, because the GTK chooser opens a typed folder instead of
returning it. Fonts differ from Windows; "Sela Missing Serif" is absent on
both. On the default xvfb display the audience output is not started.
"""

from __future__ import annotations

import hashlib
import json
import sys
import time
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

FONT = "Sela Missing Serif"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def manifest_of(path: Path) -> dict:
    with zipfile.ZipFile(path) as archive:
        bad = archive.testzip()
        if bad is not None:
            raise nx.Failure(f"{path.name}: zipfile finds a bad CRC in {bad}")
        return json.loads(archive.read("manifest.json"))


def names_in(path: Path) -> list[str]:
    with zipfile.ZipFile(path) as archive:
        return sorted(archive.namelist())


def click_id(app: nx.App, window: int, aid: str) -> None:
    control = nx.wait_until(lambda: app.controls(window).get(aid), 10, f"control {aid}")
    nx.click(window, *control.center)
    # Park the pointer away from where a portal dialog may open: a GTK
    # tooltip under it would be the dialog's first visible window.
    nx.mouse_move(window, 1270, 790)


def menu(app: nx.App, window: int, keys: str, item: str) -> None:
    nx.press(window, keys)
    time.sleep(0.4)
    click_id(app, window, item)
    time.sleep(0.6)


def dialog_lines(app: nx.App, window: int, prefix: str, done) -> list[str]:
    """Names of the `<prefix>-<n>` result lines once `done(lines)` holds."""

    def probe():
        controls = app.controls(window)
        lines = [
            controls[f"{prefix}-{n}"].name
            for n in range(40)
            if f"{prefix}-{n}" in controls
        ]
        return lines if lines and done(lines) else None

    return nx.wait_until(probe, 30, f"the {prefix} lines")


def has(lines: list[str], text: str) -> bool:
    return any(text in line for line in lines)


def machine_a(session: nx.Session, data: Path) -> tuple[Path, Path, Path]:
    summary = session.summary
    profile = data / "home" / "ana" / "sela"
    profile.mkdir(parents=True)
    library = nx.seed_library(
        profile / "library.sqlite", "--portable-song", seed=session.args.seed
    )
    usb = data / "usb"
    usb.mkdir()
    app = session.app(["--operator-library", library], name="machine-a")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)
    click_id(app, operator, "song-0")
    click_id(app, operator, "add-to-schedule")
    time.sleep(0.4)

    menu(app, operator, "alt+f", "bar-file-export")
    app.drive_file_dialog(usb / "Sunday", owner=operator)
    package = usb / "Sunday.selapackage"
    lines = dialog_lines(app, operator, "bundle-result", lambda ls: has(ls, "Saved"))
    summary["export_lines"] = lines
    session.save(operator, "a-export-result")
    if not has(lines, "Saved “Sunday.selapackage”: 1 items"):
        raise nx.Failure(f"unexpected export result {lines}")
    if not has(lines, "lantern.png") or not has(lines, FONT):
        raise nx.Failure(
            f"the result must name the missing image and the font: {lines}"
        )
    nx.press(operator, "enter")
    manifest = manifest_of(package)
    summary["package_entries"] = names_in(package)
    if (manifest["format"], manifest["version"], manifest["kind"]) != (
        "sela-bundle",
        1,
        "schedule",
    ):
        raise nx.Failure(f"unexpected manifest header {manifest}")
    images = {i["name"]: i["embedded"] for i in manifest["images"]}
    if images != {"harbor.png": True, "lantern.png": False}:
        raise nx.Failure(f"unexpected image records {manifest['images']}")
    with zipfile.ZipFile(package) as archive:
        carried = hashlib.sha256(archive.read("images/harbor.png")).hexdigest()
    if carried != sha256(profile / "Resources" / "Images" / "harbor.png"):
        raise nx.Failure("harbor.png was not carried byte for byte")
    fonts = [(f["family"], f["embedded"]) for f in manifest["fonts"]]
    if fonts != [(FONT, False)]:
        raise nx.Failure(f"unexpected font records {fonts}")

    menu(app, operator, "alt+p", "bar-profiles-backup")
    app.drive_file_dialog(usb / "Ana", owner=operator)
    backup = usb / "Ana.selabackup"
    lines = dialog_lines(
        app, operator, "bundle-result", lambda ls: has(ls, "Backed up")
    )
    summary["backup_lines"] = lines
    session.save(operator, "a-backup-result")
    nx.press(operator, "enter")
    entries = names_in(backup)
    summary["backup_entries"] = entries
    if manifest_of(backup)["kind"] != "profile":
        raise nx.Failure("the backup manifest is not a profile backup")
    for needed in ("library.sqlite", "Resources/Images/harbor.png", "manifest.json"):
        if needed not in entries:
            raise nx.Failure(f"{needed} missing from the backup: {entries}")
    if any(e == "sela.lock" or e.startswith("Recovery/") for e in entries):
        raise nx.Failure(f"lock or autosave files in the backup: {entries}")

    nx.press(operator, "ctrl+q")
    time.sleep(1.0)
    nx.press(operator, "shift+tab", "enter")  # Don't Save
    summary["a_exit"] = app.wait_exit()
    if summary["a_exit"] != 0:
        raise nx.Failure(f"machine A exit status {summary['a_exit']}")
    session.close_app(app)
    return package, backup, profile


def hostile_package(path: Path) -> None:
    schedule = json.dumps(
        {"format": "sela-schedule", "version": 4, "title": "x", "items": []}
    )
    image = b"\x89PNG not really"
    digest = hashlib.sha256(image).hexdigest()
    manifest = {
        "format": "sela-bundle",
        "version": 1,
        "kind": "schedule",
        "generator": "bundle-x11.py",
        "title": "x",
        "files": [
            {
                "path": "schedule.selaschedule",
                "size": len(schedule),
                "sha256": hashlib.sha256(schedule.encode()).hexdigest(),
                "role": "schedule",
            },
            {
                "path": "../evil.png",
                "size": len(image),
                "sha256": digest,
                "role": "image",
            },
        ],
        "images": [{"name": "evil.png", "sha256": digest, "embedded": True}],
    }
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("schedule.selaschedule", schedule)
        archive.writestr("../evil.png", image)
        archive.writestr("manifest.json", json.dumps(manifest))


def machine_b(
    session: nx.Session, data: Path, package: Path, backup: Path, original: Path
) -> Path:
    summary = session.summary
    profile = data / "D" / "Church Data" / "Profiles" / "sela"
    profile.mkdir(parents=True)
    library = nx.seed_library(profile / "library.sqlite", seed=session.args.seed)
    # A different image whose name differs only in case: the same file on
    # Windows and macOS volumes, so Sela never reuses or replaces it.
    images = profile / "Resources" / "Images"
    images.mkdir(parents=True)
    (images / "HARBOR.png").write_bytes(original.read_bytes())
    app = session.app(["--operator-library", library], name="machine-b")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)

    nx.mouse_move(operator, 1270, 790)
    nx.press(operator, "ctrl+o")
    time.sleep(0.6)
    app.drive_file_dialog(package, owner=operator)
    lines = dialog_lines(
        app, operator, "preflight-package", lambda ls: has(ls, "Opened")
    )
    summary["open_lines"] = lines
    for text in ("harbor.png → harbor (2).png", "does not carry", FONT):
        if not has(lines, text):
            raise nx.Failure(f"package notes lack {text!r}: {lines}")
    rows = nx.wait_until(
        lambda: (c := app.controls(operator)).get("preflight-row-0") and c,
        15,
        "the preflight row for lantern.png",
    )
    summary["preflight_row"] = rows["preflight-row-0"].name
    if "lantern.png" not in rows["preflight-row-0"].name:
        raise nx.Failure(f"unexpected preflight row {rows['preflight-row-0'].name}")
    session.save(operator, "b-preflight")
    unpacked = profile / "Schedules" / "Sunday.selaschedule"
    if not unpacked.is_file():
        raise nx.Failure(f"no unpacked schedule in {profile / 'Schedules'}")
    harbor_a = data / "home" / "ana" / "sela" / "Resources" / "Images" / "harbor.png"
    if sha256(images / "harbor (2).png") != sha256(harbor_a):
        raise nx.Failure("harbor.png differs after the transfer")
    if sha256(images / "HARBOR.png") != sha256(original):
        raise nx.Failure("the existing HARBOR.png was replaced")

    click_id(app, operator, "locate-0")
    app.drive_file_dialog(original, owner=operator)
    nx.wait_until(
        lambda: (
            (images / "lantern.png").is_file()
            and sha256(images / "lantern.png") == sha256(original)
        ),
        10,
        "Locate... to restore lantern.png",
    )
    nx.wait_until(
        lambda: "preflight-row-0" not in app.controls(operator),
        10,
        "the repaired row to leave the report",
    )
    session.save(operator, "b-repaired")
    summary["relinked"] = True
    # The pointer: keyboard focus stays with the portal dialog's owner
    # window, not necessarily inside the report, after Locate....
    click_id(app, operator, "dialog-cancel")
    time.sleep(0.4)

    hostile = data / "usb" / "Hostile.selapackage"
    hostile_package(hostile)
    before = sorted(p.relative_to(data).as_posix() for p in data.rglob("*"))
    nx.press(operator, "ctrl+o")
    time.sleep(0.6)
    app.drive_file_dialog(hostile, owner=operator)
    lines = dialog_lines(
        app, operator, "bundle-result", lambda ls: has(ls, "Nothing was changed.")
    )
    summary["hostile_lines"] = lines
    if not has(lines, "was refused") or not has(lines, "../evil.png"):
        raise nx.Failure(f"the hostile package was not refused: {lines}")
    session.save(operator, "b-hostile-refused")
    nx.press(operator, "enter")
    after = sorted(p.relative_to(data).as_posix() for p in data.rglob("*"))
    added = [p for p in after if p not in before and "/Recovery/" not in p]
    if (
        added
        or (data / "usb" / "evil.png").exists()
        or (profile.parent / "evil.png").exists()
    ):
        raise nx.Failure(f"the hostile package wrote files: {added}")

    library_before = (profile / "library.sqlite").read_bytes()
    menu(app, operator, "alt+p", "bar-profiles-restore")
    app.drive_file_dialog(backup, owner=operator)
    time.sleep(0.6)
    destination = profile.parent / "Ana restored"
    app.drive_file_dialog(destination, owner=operator)
    lines = dialog_lines(app, operator, "bundle-result", lambda ls: has(ls, "Restored"))
    summary["restore_lines"] = lines
    session.save(operator, "b-restore-result")
    if not has(lines, "--operator-library"):
        raise nx.Failure(f"the restore result does not say how to open it: {lines}")
    nx.press(operator, "enter")
    if not (destination / "library.sqlite").is_file():
        raise nx.Failure(f"nothing restored at {destination}")
    titles = sorted(f[0] for f in nx.song_payloads(destination / "library.sqlite"))
    summary["restored_titles"] = titles
    if titles != ["Harbor Light"]:
        raise nx.Failure(f"unexpected restored songs {titles}")
    leftovers = [
        p.name for p in profile.parent.iterdir() if p.name.startswith(".sela-")
    ]
    if leftovers:
        raise nx.Failure(f"staging left behind: {leftovers}")

    nx.press(operator, "ctrl+q")
    summary["b_exit"] = app.wait_exit()
    if summary["b_exit"] != 0:
        raise nx.Failure(f"machine B exit status {summary['b_exit']}")
    session.close_app(app)
    if (profile / "library.sqlite").read_bytes() != library_before:
        raise nx.Failure("the restore changed machine B's own library")
    return destination


def body(session: nx.Session) -> str:
    data = session.temp("sela-bundle-")
    package, backup, profile_a = machine_a(session, data)
    original = profile_a / "originals" / "lantern.png"
    destination = machine_b(session, data, package, backup, original)

    app = session.app(
        ["--operator-library", destination / "library.sqlite"], name="restored"
    )
    operator = app.window()
    nx.activate(operator)
    song = nx.wait_until(
        lambda: app.controls(operator).get("song-0"), 15, "the restored song list"
    )
    session.summary["restored_song_row"] = song.name
    if "Harbor Light" not in song.name:
        raise nx.Failure(f"the restored profile lists {song.name!r}")
    session.save(operator, "restored-operator")
    nx.press(operator, "ctrl+q")
    session.summary["restored_exit"] = app.wait_exit()
    if session.summary["restored_exit"] != 0:
        raise nx.Failure(
            f"restored operator exit status {session.summary['restored_exit']}"
        )
    return (
        "Export Standalone Schedule writes a ZIP that Python's zipfile verifies (manifest v1, image byte for "
        "byte, missing image and font listed); Back Up Profile writes a profile backup without lock or "
        "autosave files; on a second profile Open Schedule unpacks the package, preflight names the "
        "missing image and font and Locate... repairs the image; a '../' package is refused with nothing "
        "written; Restore Profile builds a new profile that a third launch opens"
    )


sys.exit(nx.main(body, __doc__, "bundle-x11"))
