"""Native check: File > Import > EasyWorship Library... on the real Windows
desktop (M4-07 EasyWorship 6-8 child).

Windows counterpart of scripts/ew-import-x11.py, through UI Automation
(scripts/uia-names-windows.ps1) and screen captures of the real operator:

- seeds a new library and writes the synthetic EasyWorship library of
  scripts/ew_library_fixture.py (original text, UNOBSERVED schema guess) to
  <tmp>\\EasyWorship\\Default\\v6.1\\Databases\\Data, recording the SHA-256,
  size and mtime of every file under <tmp>\\EasyWorship;
- Alt+F, Down x8, Enter opens Import > EasyWorship Library...; the common
  folder dialog gets the EasyWorship *profile* folder; the staged preview
  must list five rows in title order with
  "Ready: 4 · Not importable: 1 · Duplicates: 1" and Import (3) while the
  library is unchanged;
- Shift+Tab, Enter presses Import (3): the library must hold exactly the
  three new songs with author, copyright, administrator, CCLI number and
  words read back from SQLite;
- the same library again shows four duplicates with Skip chosen and
  Import (0); Esc cancels and the library is unchanged;
- every EasyWorship file keeps its SHA-256, size and mtime and no file is
  added beside them; Ctrl+Q exits 0.

Never point it at a real EasyWorship installation: it writes its own
synthetic library to a temporary folder. Takes over the real keyboard and
mouse. NOT RUN yet: written on Linux without a Windows host
(ew-library-import stream, 2026-10-23); see docs/parity-pending.md
(PP-EWL-07).

Run: python scripts/ew-import-windows.py [--out DIR] [--monitor secondary]
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import ew_library_fixture as fx
import native_win as nw


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


a11y = _load("a11y_windows", "a11y-windows.py")
song_import = _load("song_import_windows", "song-import-windows.py")

EW_ITEM_DOWNS = 8  # New Song -> ... -> Import > EasyWorship Library...


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
        default=ROOT / ".amp/in/artifacts/ew-import-windows",
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
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-ew-import-"))
    library = data / "library.sqlite"
    ew_root = data / "EasyWorship"
    profile = ew_root / "Default"
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    app: nw.App | None = None
    hwnd = 0
    try:
        subprocess.run([str(args.seed), str(library)], check=True)
        fx.write_ew_library(profile / "v6.1" / "Databases" / "Data")
        sources = fx.fingerprint(ew_root)
        summary["source_files"] = [(path, digest) for path, digest, _, _ in sources]
        before = song_import.titles(library)
        summary["library_before"] = before
        app = nw.App(args.binary, ["--operator-library", str(library)])
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(1.2)

        def save(name: str) -> None:
            width, height, pixels = nw.capture_client(hwnd)
            nw.write_png(args.out / f"{name}.png", width, height, pixels)

        def press(keys: str) -> None:
            nw.press(hwnd, keys)
            time.sleep(0.12)

        def open_library_import() -> None:
            a11y.park_cursor(hwnd)
            press("alt+f")
            for _ in range(EW_ITEM_DOWNS):
                press("down")
            press("enter")
            time.sleep(0.8)
            song_import.drive_folder_dialog(app.process.pid, hwnd, profile)

        def preview(writes: int, timeout: float = 30.0) -> dict:
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                found = a11y.nodes_by_id(a11y.uia(hwnd))
                confirm = found.get("import-confirm")
                if (
                    confirm
                    and confirm["name"] == f"Import ({writes})"
                    and "import-status" not in found
                ):
                    return found
                time.sleep(0.3)
            raise nw.Failure(
                f"the staged preview with Import ({writes}) did not appear"
            )

        open_library_import()
        found = preview(3)
        save("01-ew-preview")
        summary["summary"] = found["import-summary"]["name"]
        check.expect(
            found["import-summary"]["name"] == fx.SUMMARY,
            f"preview summary {found['import-summary']['name']!r}",
        )
        rows = [
            found[f"import-row-{i}"]["name"]
            for i in range(len(fx.ROWS))
            if f"import-row-{i}" in found
        ]
        check.expect(rows == fx.ROWS, f"preview rows {rows}")
        check.expect(
            "import-choice-4" not in found and "import-choice-0" in found,
            "choices for songs only, none for the song without words",
        )
        check.expect(
            song_import.titles(library) == before, "nothing is saved while staged"
        )

        press("shift+tab")
        press("enter")
        deadline = time.monotonic() + 15
        after = song_import.titles(library)
        while len(after) != len(before) + 3 and time.monotonic() < deadline:
            time.sleep(0.3)
            after = song_import.titles(library)
        summary["library_after"] = after
        check.expect(
            set(after) - set(before) == fx.IMPORTED, f"imported titles {after}"
        )
        harbor = fx.library_song(library, "Harbor Morning")
        kasih = fx.library_song(library, "Kasih Setia")
        check.expect(harbor is not None and kasih is not None, "songs saved")
        if harbor is not None:
            fields, (admin, number) = harbor
            summary["harbor"] = {
                "fields": fields,
                "administrator": admin,
                "song_number": number,
            }
            problems = fx.check_harbor(fields, admin, number)
            check.expect(not problems, f"Harbor Morning mapping: {problems}")
        if kasih is not None:
            check.expect(
                kasih[0][4:6] == fx.KASIH_SECTION,
                f"\\uN words decoded: {kasih[0][4:6]}",
            )
        time.sleep(0.8)
        save("02-ew-imported")

        open_library_import()
        found = preview(0)
        save("03-ew-duplicates")
        summary["duplicate_summary"] = found["import-summary"]["name"]
        check.expect(
            found["import-summary"]["name"].endswith(fx.REIMPORT_DUPLICATES),
            f"four duplicates: {found['import-summary']['name']!r}",
        )
        check.expect(
            all(f"import-choice-{row}-skip" in found for row in (0, 1, 2, 3)),
            "duplicate rows offer Skip",
        )
        press("escape")
        time.sleep(0.8)
        check.expect(
            "import-dialog" not in a11y.nodes_by_id(a11y.uia(hwnd)),
            "Esc cancels the import",
        )
        check.expect(song_import.titles(library) == after, "cancelling changed nothing")
        check.expect(
            fx.fingerprint(ew_root) == sources,
            "every EasyWorship file keeps its SHA-256, size and mtime",
        )

        press("ctrl+q")
        summary["exit"] = app.wait_exit()
        hwnd = 0
        check.expect(summary["exit"] == 0, f"operator exit status {summary['exit']}")
        summary["status"] = "FAIL" if check.failures else "PASS"
        return 0 if summary["status"] == "PASS" else 1
    except Exception as error:  # noqa: BLE001
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        if app is not None:
            app.close()
            if app.owned:
                shutil.rmtree(app.scratch, ignore_errors=True)
        (args.out / "summary.json").write_text(
            json.dumps(summary, indent=2, ensure_ascii=False)
        )
        print(json.dumps(summary, indent=2, ensure_ascii=False))
        shutil.rmtree(data, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
