"""Native check: standalone schedules, profile backup and restore on the real
Windows desktop (M1-13, backlog scenario S04).

Windows counterpart of scripts/bundle-x11.py, through UI Automation
(scripts/uia-names-windows.ps1) and the Win32 common dialogs:

- machine A: a library seeded with --portable-song ("Harbor Light": slide 1
  shows harbor.png, slide 2 lantern.png, which is not in the profile, slide
  3 the font "Sela Missing Serif"); the song is added to the schedule;
  File > Export Standalone Schedule... (UIA row bar-file-export) and the
  Save dialog write usb\\Sunday.selapackage; the result names lantern.png
  and the font; Python's zipfile checks every CRC and the manifest
  (sela-bundle 1, schedule, harbor.png embedded byte for byte, lantern.png
  listed, the font with embedded false); Profiles > Back Up Profile...
  writes usb\\Ana.selabackup (library.sqlite and harbor.png, no sela.lock or
  Recovery\\); Ctrl+Q, Don't Save;
- machine B (another folder depth, fresh library): Ctrl+O picks the
  package; harbor.png arrives byte for byte, Schedules\\Sunday.selaschedule
  is written and the preflight dialog shows the package notes and a
  Missing row for lantern.png; Locate... with the original repairs it; a
  package with a "..\\" entry is refused with "Nothing was changed." and
  nothing written; Profiles > Restore Profile... restores the backup as
  "Ana restored" beside B's profile; B's library is byte-identical;
- a third launch with the restored library lists "Harbor Light".

Windows-only checks the X11 script cannot make: the Win32 common dialogs,
an existing-folder restore through the Save dialog's overwrite prompt (Yes,
then Sela refuses with "already exists"), and file names that differ only
in case on NTFS (the package import names its copy "harbor (2).png" when a
different HARBOR.png is already in the profile).

Takes over the real keyboard and mouse; never run with EasyWorship open.
NOT RUN yet: written on Linux without a Windows host (bundle-backup-restore
stream, 2026-10-23); see docs/parity-pending.md (PP-BUN-08).

Run: python scripts/bundle-windows.py [--out DIR]
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import pathlib
import shutil
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import native_win as nw


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


a11y = _load("a11y_windows", "a11y-windows.py")

FONT = "Sela Missing Serif"


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def titles(library: pathlib.Path) -> list[str]:
    """Titles of the live songs, read-only."""
    uri = library.resolve().as_uri() + "?mode=ro"
    with sqlite3.connect(uri, uri=True, timeout=2) as db:
        rows = db.execute(
            "SELECT r.payload FROM songs s JOIN song_revisions r "
            "ON r.id=s.id AND r.revision=s.head WHERE s.deleted=0"
        ).fetchall()
    out = []
    for (data,) in rows:
        length = struct.unpack("<I", data[:4])[0]
        out.append(data[4 : 4 + length].decode("utf-8"))
    return sorted(out)


def manifest_of(path: pathlib.Path) -> dict:
    with zipfile.ZipFile(path) as archive:
        bad = archive.testzip()
        if bad is not None:
            raise nw.Failure(f"{path.name}: zipfile finds a bad CRC in {bad}")
        return json.loads(archive.read("manifest.json"))


def hostile_package(path: pathlib.Path) -> None:
    schedule = json.dumps(
        {"format": "sela-schedule", "version": 4, "title": "x", "items": []}
    )
    image = b"\x89PNG not really"
    digest = hashlib.sha256(image).hexdigest()
    manifest = {
        "format": "sela-bundle",
        "version": 1,
        "kind": "schedule",
        "generator": "bundle-windows.py",
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


class Operator:
    """One launched operator and the UIA helpers the steps share."""

    def __init__(self, binary: pathlib.Path, library: pathlib.Path, out: pathlib.Path):
        self.app = nw.App(binary, ["--operator-library", str(library)])
        self.hwnd = self.app.window()
        self.out = out
        nw.activate(self.hwnd)
        time.sleep(1.2)

    def nodes(self) -> dict[str, dict]:
        return a11y.nodes_by_id(a11y.uia(self.hwnd))

    def wait(self, what: str, probe, timeout: float = 30.0):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            found = probe(self.nodes())
            if found:
                return found
            time.sleep(0.4)
        raise nw.Failure(f"timed out waiting for {what}")

    def click(self, aid: str) -> None:
        node = self.wait(aid, lambda n: n.get(aid), 10)
        a11y.click_node(self.hwnd, node)
        a11y.park_cursor(self.hwnd)

    def menu(self, keys: str, aid: str) -> None:
        nw.press(self.hwnd, keys)
        time.sleep(0.4)
        self.click(aid)
        time.sleep(0.6)

    def lines(self, prefix: str, needle: str) -> list[str]:
        def probe(found: dict[str, dict]):
            lines = [
                found[f"{prefix}-{n}"]["name"]
                for n in range(40)
                if f"{prefix}-{n}" in found
            ]
            return lines if any(needle in line for line in lines) else None

        return self.wait(f"{prefix} lines with {needle!r}", probe)

    def dialog(self, path: pathlib.Path) -> None:
        nw.drive_file_dialog(self.app.process.pid, self.hwnd, str(path))

    def save(self, name: str) -> None:
        width, height, pixels = nw.capture_client(self.hwnd)
        nw.write_png(self.out / f"{name}.png", width, height, pixels)

    def close(self) -> None:
        self.app.close()
        if self.app.owned:
            shutil.rmtree(self.app.scratch, ignore_errors=True)


def has(lines: list[str], text: str) -> bool:
    return any(text in line for line in lines)


def run(args, data: pathlib.Path, summary: dict, check, apps: list) -> None:
    profile_a = data / "home" / "ana" / "sela"
    profile_a.mkdir(parents=True)
    library_a = profile_a / "library.sqlite"
    subprocess.run([str(args.seed), str(library_a), "--portable-song"], check=True)
    usb = data / "usb"
    usb.mkdir()
    a = Operator(args.binary, library_a, args.out)
    apps.append(a)
    a.click("song-0")
    a.click("add-to-schedule")
    a.menu("alt+f", "bar-file-export")
    a.dialog(usb / "Sunday")
    package = usb / "Sunday.selapackage"
    lines = a.lines("bundle-result", "Saved")
    summary["export_lines"] = lines
    a.save("01-export-result")
    check.expect(
        has(lines, "lantern.png") and has(lines, FONT), f"export result {lines}"
    )
    nw.press(a.hwnd, "enter")
    manifest = manifest_of(package)
    check.expect(
        (manifest["format"], manifest["version"], manifest["kind"])
        == ("sela-bundle", 1, "schedule"),
        "package manifest header",
    )
    images = {i["name"]: i["embedded"] for i in manifest["images"]}
    check.expect(
        images == {"harbor.png": True, "lantern.png": False}, f"images {images}"
    )
    with zipfile.ZipFile(package) as archive:
        carried = hashlib.sha256(archive.read("images/harbor.png")).hexdigest()
    check.expect(
        carried == sha256(profile_a / "Resources" / "Images" / "harbor.png"),
        "harbor.png carried byte for byte",
    )
    check.expect(
        [(f["family"], f["embedded"]) for f in manifest["fonts"]] == [(FONT, False)],
        "the font is listed, not embedded",
    )

    a.menu("alt+p", "bar-profiles-backup")
    a.dialog(usb / "Ana")
    backup = usb / "Ana.selabackup"
    summary["backup_lines"] = a.lines("bundle-result", "Backed up")
    a.save("02-backup-result")
    nw.press(a.hwnd, "enter")
    with zipfile.ZipFile(backup) as archive:
        entries = sorted(archive.namelist())
    summary["backup_entries"] = entries
    check.expect(manifest_of(backup)["kind"] == "profile", "backup manifest kind")
    check.expect(
        "library.sqlite" in entries and "Resources/Images/harbor.png" in entries,
        "library and image in the backup",
    )
    check.expect(
        not any(e == "sela.lock" or e.startswith("Recovery/") for e in entries),
        "no lock or autosave files in the backup",
    )
    nw.press(a.hwnd, "ctrl+q")
    time.sleep(1.0)
    nw.press(a.hwnd, "shift+tab")
    nw.press(a.hwnd, "enter")  # Don't Save
    summary["a_exit"] = a.app.wait_exit()
    check.expect(summary["a_exit"] == 0, f"machine A exit {summary['a_exit']}")

    profile_b = data / "D" / "Church Data" / "Profiles" / "sela"
    profile_b.mkdir(parents=True)
    library_b = profile_b / "library.sqlite"
    subprocess.run([str(args.seed), str(library_b)], check=True)
    # A different image whose name differs only in case (NTFS: same file).
    images_b = profile_b / "Resources" / "Images"
    images_b.mkdir(parents=True)
    shutil.copyfile(profile_a / "originals" / "lantern.png", images_b / "HARBOR.png")
    b = Operator(args.binary, library_b, args.out)
    apps.append(b)
    nw.press(b.hwnd, "ctrl+o")
    time.sleep(0.6)
    b.dialog(package)
    lines = b.lines("preflight-package", "Opened")
    summary["open_lines"] = lines
    check.expect(has(lines, "harbor.png → harbor (2).png"), f"renamed copy: {lines}")
    check.expect(has(lines, "does not carry") and has(lines, FONT), "package notes")
    row = b.wait("the lantern.png row", lambda n: n.get("preflight-row-0"), 15)
    summary["preflight_row"] = row["name"]
    check.expect("lantern.png" in row["name"], f"preflight row {row['name']!r}")
    b.save("03-preflight")
    check.expect(
        (profile_b / "Schedules" / "Sunday.selaschedule").is_file(),
        "the unpacked schedule file",
    )
    check.expect(
        sha256(images_b / "HARBOR.png")
        == sha256(profile_a / "originals" / "lantern.png"),
        "the existing HARBOR.png is untouched",
    )
    b.click("locate-0")
    b.dialog(profile_a / "originals" / "lantern.png")
    b.wait("the repaired report", lambda n: "preflight-row-0" not in n, 15)
    check.expect(
        sha256(images_b / "lantern.png")
        == sha256(profile_a / "originals" / "lantern.png"),
        "Locate... restores lantern.png",
    )
    b.save("04-repaired")
    b.click("dialog-cancel")

    hostile = usb / "Hostile.selapackage"
    hostile_package(hostile)
    before = sorted(str(p.relative_to(data)) for p in data.rglob("*"))
    nw.press(b.hwnd, "ctrl+o")
    time.sleep(0.6)
    b.dialog(hostile)
    lines = b.lines("bundle-result", "Nothing was changed.")
    summary["hostile_lines"] = lines
    check.expect(
        has(lines, "was refused") and has(lines, "../evil.png"),
        f"hostile package refused for its name: {lines}",
    )
    nw.press(b.hwnd, "enter")
    after = sorted(str(p.relative_to(data)) for p in data.rglob("*"))
    added = [p for p in after if p not in before and "Recovery" not in p]
    check.expect(not added, f"nothing written by the hostile package: {added}")

    library_before = library_b.read_bytes()
    destination = profile_b.parent / "Ana restored"
    b.menu("alt+p", "bar-profiles-restore")
    b.dialog(backup)
    time.sleep(0.6)
    b.dialog(destination)
    lines = b.lines("bundle-result", "Restored")
    summary["restore_lines"] = lines
    b.save("05-restore-result")
    check.expect(has(lines, "--operator-library"), "the result says how to open it")
    nw.press(b.hwnd, "enter")
    check.expect(
        titles(destination / "library.sqlite") == ["Harbor Light"], "restored songs"
    )
    # Restoring onto the now existing folder: the Save dialog asks to
    # replace (Yes), then Sela refuses and changes nothing.
    restored_library = (destination / "library.sqlite").read_bytes()
    b.menu("alt+p", "bar-profiles-restore")
    b.dialog(backup)
    time.sleep(0.6)
    b.dialog(destination)
    time.sleep(0.8)
    nw.press(b.hwnd, "alt+y")
    lines = b.lines("bundle-result", "already exists")
    check.expect(
        has(lines, "Nothing was changed."), f"existing folder refused: {lines}"
    )
    check.expect(
        (destination / "library.sqlite").read_bytes() == restored_library,
        "the existing restored profile is untouched",
    )
    nw.press(b.hwnd, "enter")
    nw.press(b.hwnd, "ctrl+q")
    summary["b_exit"] = b.app.wait_exit()
    check.expect(summary["b_exit"] == 0, f"machine B exit {summary['b_exit']}")
    check.expect(
        library_b.read_bytes() == library_before, "B's own library is unchanged"
    )

    c = Operator(args.binary, destination / "library.sqlite", args.out)
    apps.append(c)
    song = c.wait("the restored song list", lambda n: n.get("song-0"), 15)
    check.expect(
        "Harbor Light" in song["name"], f"restored list shows {song['name']!r}"
    )
    c.save("06-restored-operator")
    nw.press(c.hwnd, "ctrl+q")
    summary["restored_exit"] = c.app.wait_exit()
    check.expect(summary["restored_exit"] == 0, "restored operator exits 0")


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
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/bundle-windows"
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(f"FAIL: missing {path}")
            return 1
    summary: dict = {"status": "FAIL", "binary": str(args.binary.resolve())}
    check = a11y.Check(summary)
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-bundle-"))
    apps: list[Operator] = []
    try:
        run(args, data, summary, check, apps)
        summary["status"] = "FAIL" if check.failures else "PASS"
        return 0 if summary["status"] == "PASS" else 1
    except Exception as error:  # noqa: BLE001
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        for operator in apps:
            operator.close()
        (args.out / "summary.json").write_text(
            json.dumps(summary, indent=2, ensure_ascii=False)
        )
        print(json.dumps(summary, indent=2, ensure_ascii=False))
        shutil.rmtree(data, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
