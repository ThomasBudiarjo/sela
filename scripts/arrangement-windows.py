"""Windows native check: per-schedule-item arrangement, pagination, save/reopen.

Usage: python scripts/arrangement-windows.py [--binary sela.exe]
       [--seed seed_library.exe] [--monitor secondary] [--out DIR]

Drives the real operator with real mouse and keyboard input against a library
holding one song with three arrangements ("Full", "Chorus Twice", "Long"; the
last is one 40-line section that does not fit a slide).

VAL-SCHED-002: Duplicate puts a copy after the original; the Preview toolbar's
arrangement button changes only the copy's arrangement. The original keeps its
three slides, the copy shows two, and the audience differs per item. Saving
writes format version 2 with the copy's arrangement only; reopening the file
with Ctrl+O shows both items with their own slides and the same audience.
VAL-ARR-003: choosing "Long" lists several Preview slides whose captions carry
the page numbers ("(1/N)"), the original unaffected.
VAL-ARR-004: a live item keeps its audience while the previewed copy changes
arrangement; Go Live then sends the new slides.
Requires exclusive use of the keyboard and mouse and an unobstructed audience
monitor while it runs.
"""

from __future__ import annotations

import argparse
import contextlib
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import native_win as nw


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lo = _load("live_output_windows", "live-output-windows.py")
a11y = _load("a11y_windows", "a11y-windows.py")


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
        "--out",
        type=pathlib.Path,
        default=ROOT / ".amp/in/artifacts/arrangement-windows",
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(f"FAIL: missing {path} (cargo build --examples && cargo build)")
            return 1
    summary: dict = {"binary": str(args.binary.resolve()), "monitor": args.monitor}
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-arrangement-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library), "--arranged-song"], check=True)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    spawned: set[int] = set()
    app = nw.App(args.binary, ["--operator-library", str(library)])
    hwnd = 0

    def shot(name: str) -> None:
        width, height, pixels = nw.capture_client(hwnd)
        nw.write_png(args.out / f"{name}.png", width, height, pixels)

    def audience_shot(name: str, screen: int) -> None:
        width, height, pixels = nw.capture_client(screen)
        nw.write_png(args.out / f"{name}.png", width, height, pixels)

    def tree() -> dict[str, dict]:
        return a11y.nodes_by_id(a11y.uia(hwnd))

    def nodes() -> list[dict]:
        return a11y.uia(hwnd)["nodes"]

    def origin() -> tuple[int, int]:
        return a11y.client_origin(hwnd)

    def centre(node: dict) -> tuple[int, int]:
        ox, oy = origin()
        return node["x"] + node["width"] // 2 - ox, node["y"] + node["height"] // 2 - oy

    def click_id(name: str) -> None:
        nw.click(hwnd, *centre(tree()[name]))
        time.sleep(0.5)

    def rows() -> list[dict]:
        found = [n for n in nodes() if n["automationId"].startswith("schedule-item-")]
        return sorted(found, key=lambda n: int(n["automationId"].rsplit("-", 1)[1]))

    def click_row(index: int) -> None:
        node = rows()[index]
        _, y = centre(node)
        nw.click(hwnd, node["x"] - origin()[0] + node["width"] // 2, y)
        time.sleep(0.6)

    def tiles() -> list[str]:
        found = [n for n in nodes() if n["automationId"].startswith("preview-slide-")]
        found.sort(key=lambda n: int(n["automationId"].rsplit("-", 1)[1]))
        return [n["name"] for n in found]

    def expect(label: str, got, want) -> None:
        summary[label] = got
        if got != want:
            raise nw.Failure(f"{label}: expected {want}, got {got}")

    def wait_change(screen: int, before: str, name: str) -> str:
        sent = time.monotonic()
        while lo.digest(screen) == before:
            if time.monotonic() - sent > 3.0:
                raise nw.Failure(f"{name}: audience output did not change within 3s")
            time.sleep(0.05)
        time.sleep(0.3)
        return lo.digest(screen)

    def go_live(screen: int, before: str, name: str) -> str:
        click_id("go-live")
        return wait_change(screen, before, name)

    def items(path: pathlib.Path) -> tuple[int, list]:
        document = json.loads(path.read_text(encoding="utf-8"))
        return document["version"], document["items"]

    try:
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(1.5)
        nw.click(hwnd, *centre(tree()["song-0"]))
        time.sleep(0.3)
        click_id("add-to-schedule")
        click_row(0)
        full = tiles()
        expect("original_slides", len(full), 3)
        shot("01-original-full")

        nw.right_click(hwnd, *centre(rows()[0]))
        time.sleep(0.5)
        click_id("item-menu-duplicate")
        expect("two_items", len(rows()), 2)
        time.sleep(0.6)
        expect("copy_starts_as_the_song_arrangement", len(tiles()), 3)

        # Preview toolbar: Full -> Chorus Twice -> Long.
        click_id("preview-arrangement")
        expect("copy_chorus_twice_slides", len(tiles()), 2)
        shot("02-copy-chorus-twice")
        click_id("preview-arrangement")
        pages = tiles()
        summary["copy_long_tiles"] = pages
        if len(pages) < 3 or not all(
            f"({i + 1}/{len(pages)})" in p for i, p in enumerate(pages)
        ):
            raise nw.Failure(f"overflow did not paginate with page numbers: {pages}")
        shot("03-copy-long-paginated")
        click_id("preview-arrangement")  # Full again
        click_id("preview-arrangement")  # Chorus Twice
        expect("copy_ends_on_chorus_twice", len(tiles()), 2)

        click_row(0)
        expect("original_unchanged", tiles(), full)
        shot("04-original-after-copy-change")
        click_row(1)
        expect("copy_two_slides", len(tiles()), 2)

        # VAL-ARR-004: audience per item, and Live is stable while Preview changes.
        click_id("live-output")
        found = lo.audience(app.process.pid, 15)
        if not found:
            raise nw.Failure(f"no audience window within 15s: {app.log()}")
        pid, screen = found
        spawned.add(pid)
        time.sleep(1.5)
        if nw.user32.GetForegroundWindow() != hwnd:
            nw.activate(hwnd)
        blank = lo.digest(screen)
        click_row(0)
        original_live = go_live(screen, blank, "original live")
        audience_shot("05-audience-original", screen)
        click_row(1)
        copy_live = go_live(screen, original_live, "copy live")
        audience_shot("06-audience-copy", screen)
        summary["audiences_differ"] = original_live != copy_live
        if original_live == copy_live:
            raise nw.Failure("the original and the copy show the same audience")
        # Change the copy's arrangement while it is live: audience unchanged.
        click_id("preview-arrangement")  # Long
        time.sleep(0.8)
        if lo.digest(screen) != copy_live:
            raise nw.Failure("changing the previewed arrangement changed Live")
        summary["live_stable_during_arrangement_change"] = True
        # Sending the new slides is explicit: Go Live shows page 1 of the overflow.
        long_live = go_live(screen, copy_live, "long page one live")
        audience_shot("06b-audience-long-page-one", screen)
        summary["long_page_differs"] = long_live != copy_live
        click_id("preview-arrangement")  # Full
        click_id("preview-arrangement")  # Chorus Twice
        expect("copy_back_on_chorus_twice", len(tiles()), 2)

        saved = data / "Arranged.selaschedule"
        click_row(0)
        nw.press(hwnd, "ctrl+s")
        time.sleep(0.8)
        nw.drive_file_dialog(app.process.pid, hwnd, str(saved))
        deadline = time.monotonic() + 5
        while not saved.is_file():
            if time.monotonic() > deadline:
                raise nw.Failure("Ctrl+S did not save the schedule file")
            time.sleep(0.05)
        time.sleep(0.6)
        version, stored = items(saved)
        expect("file_version", version, 2)
        expect("original_has_no_arrangement", "arrangement" in stored[0], False)
        expect(
            "copy_has_arrangement", isinstance(stored[1].get("arrangement"), str), True
        )
        shot("07-saved")

        # Reopen in a fresh session with Ctrl+O.
        app.close()
        for pid in spawned:
            if lo.alive(pid):
                lo.kill(pid)
        spawned.clear()
        app = nw.App(args.binary, ["--operator-library", str(library)])
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(1.5)
        nw.press(hwnd, "ctrl+o")
        time.sleep(0.8)
        nw.drive_file_dialog(app.process.pid, hwnd, str(saved))
        deadline = time.monotonic() + 8
        while len(rows()) < 2:
            if time.monotonic() > deadline:
                raise nw.Failure("Ctrl+O did not reopen the schedule")
            time.sleep(0.2)
        time.sleep(0.6)
        click_id("live-output")
        found = lo.audience(app.process.pid, 15)
        if not found:
            raise nw.Failure(f"no audience window within 15s: {app.log()}")
        pid, screen = found
        spawned.add(pid)
        time.sleep(1.5)
        if nw.user32.GetForegroundWindow() != hwnd:
            nw.activate(hwnd)
        blank = lo.digest(screen)
        click_row(0)
        expect("reopened_original", tiles(), full)
        reopened_original = go_live(screen, blank, "reopened original")
        click_row(1)
        expect("reopened_copy_slides", len(tiles()), 2)
        reopened_copy = go_live(screen, reopened_original, "reopened copy")
        expect("reopened_original_audience", reopened_original, original_live)
        expect("reopened_copy_audience", reopened_copy, copy_live)
        shot("08-reopened")
        audience_shot("09-audience-reopened-copy", screen)

        summary["status"] = "PASS"
        print(
            "PASS: per-item arrangement survives duplicate, change, save (v2) and "
            "reopen; overflow paginates with page numbers; Live stable"
        )
        return 0
    except (nw.Failure, AssertionError, KeyError, IndexError) as error:
        summary["error"] = repr(error)
        print(f"FAIL: {error!r}")
        if hwnd and nw.user32.IsWindow(hwnd):
            shot("failure-operator")
            with contextlib.suppress(OSError, nw.Failure, ValueError):
                (args.out / "failure-uia.json").write_text(
                    json.dumps(a11y.uia(hwnd), indent=1)
                )
        return 1
    finally:
        app.close()
        for pid in spawned:
            if lo.alive(pid):
                lo.kill(pid)
        summary.setdefault("status", "FAIL")
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
