"""Native check: the Songs resource browser and modal focus scopes
(focus-scopes-resource-browser; VAL-FOCUS-001/002/004, VAL-RES-001..004).

Small library (default), through UI Automation and real input:

- Shift+click selects a range, Ctrl+click toggles one row, Add to Schedule
  adds every selected song in list order (VAL-FOCUS-004);
- Sort by > Date modified / Descending / Title through the row context menu,
  with the choice ticked, and the order still applied after a restart
  (VAL-RES-001);
- Delete asks "Are you sure you want to permanently delete this song?" with
  Yes/No, focus on No; Enter and Esc keep the song, Yes removes it, the
  selection moves to a neighbour and the database row is gone (VAL-RES-004);
- Options opens centred over the window, Tab and Shift+Tab never leave it,
  clicks and Ctrl+B / Page Down do not reach the window behind, and Esc
  closes it and returns focus to where it was (VAL-FOCUS-001/002).

`--library PATH` (a 20k-song library from
`cargo run --release --example search_bench -- --songs 20000 --keep PATH`)
instead checks the virtualized list: only a window of rows is exposed,
Ctrl+End/Home reach the last and first row, repeated Page Down and a
scrollbar drag stay responsive, and the operator's own frame log
(`SELA_FRAME_LOG`) gives render-to-render intervals and row build times for
the scrolling windows (VAL-RES-002/003). Use the release binary for timing.

Takes over the real keyboard and mouse; never run with EasyWorship open.

Run: python scripts/resource-browser-windows.py [--out DIR] [--library PATH]
     [--binary PATH] [--seed PATH]
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import re
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

ROW = re.compile(r"^song-(\d+)$")


def song_rows(doc: dict) -> list[tuple[int, str, bool]]:
    """(index, title, selected) of the song rows UIA currently exposes."""
    rows = [
        (int(m.group(1)), n["name"], n.get("selected") is True)
        for n in doc["nodes"]
        if n["controlType"] == "ListItem" and (m := ROW.match(n["automationId"]))
    ]
    return sorted(rows)


def titles(doc: dict) -> list[str]:
    return [title for _, title, _ in song_rows(doc)]


def selected(doc: dict) -> list[int]:
    return [index for index, _, on in song_rows(doc) if on]


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, round(fraction * (len(ordered) - 1)))]


def song_count(library: pathlib.Path) -> int:
    connection = sqlite3.connect(f"file:{library}?mode=ro", uri=True)
    try:
        return connection.execute("SELECT COUNT(*) FROM songs WHERE deleted=0").fetchone()[0]
    finally:
        connection.close()


class Session:
    """One launched operator and the helpers every step shares."""

    def __init__(self, args, summary: dict, check, scratch: pathlib.Path, library: pathlib.Path):
        self.args, self.summary, self.check = args, summary, check
        self.app = nw.App(args.binary, ["--operator-library", str(library)], scratch=scratch)
        self.hwnd = self.app.window()
        nw.activate(self.hwnd)
        time.sleep(3.0 if args.library else 1.4)

    def save(self, name: str) -> None:
        width, height, pixels = nw.capture_client(self.hwnd)
        nw.write_png(self.args.out / f"{name}.png", width, height, pixels)

    def press(self, keys: str, pause: float = 0.15) -> None:
        try:
            nw.press(self.hwnd, keys)
        except nw.Failure:
            nw.activate(self.hwnd)
            self.summary["focus_recoveries"] = self.summary.get("focus_recoveries", 0) + 1
            nw.press(self.hwnd, keys)
        time.sleep(pause)

    def doc(self) -> dict:
        return a11y.uia(self.hwnd)

    def node(self, aid: str, doc: dict | None = None) -> dict | None:
        return a11y.nodes_by_id(doc or self.doc()).get(aid)

    def wait(self, what: str, condition, timeout: float = 8.0) -> dict:
        deadline = time.monotonic() + timeout
        doc = self.doc()
        while time.monotonic() < deadline:
            if condition(doc):
                self.check.expect(True, what)
                return doc
            time.sleep(0.2)
            doc = self.doc()
        self.check.expect(False, what)
        return doc

    def center(self, node: dict) -> tuple[int, int]:
        ox, oy = a11y.client_origin(self.hwnd)
        return node["x"] + node["width"] // 2 - ox, node["y"] + node["height"] // 2 - oy

    def click(self, node: dict, modifiers: str | None = None, right: bool = False) -> None:
        x, y = self.center(node)
        if right:
            nw.right_click(self.hwnd, x, y)
        elif modifiers:
            nw.click_with(self.hwnd, modifiers, x, y)
        else:
            nw.click(self.hwnd, x, y)
        time.sleep(0.35)

    def click_aid(self, aid: str, **kwargs) -> None:
        node = self.node(aid)
        if node is None:
            raise nw.Failure(f"{aid} is not in the UIA tree")
        self.click(node, **kwargs)

    def focused(self) -> str:
        nodes = a11y.uia(self.hwnd, focused=True)["nodes"]
        return nodes[0]["automationId"] if nodes else ""

    def quit(self) -> None:
        self.press("ctrl+q")
        time.sleep(0.6)
        if self.app.process.poll() is None:
            node = self.node("dialog-confirm")
            if node:
                self.click(node)
        self.app.wait_exit()


def small_run(args, summary: dict, check) -> None:
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="sela-resource-"))
    library = scratch / "library.sqlite"
    subprocess.run(
        [str(args.seed), str(library), "--second-song", "--extra-songs", "12"], check=True
    )
    session = Session(args, summary, check, scratch, library)
    try:
        # --- VAL-FOCUS-004: Shift/Ctrl multi-select, Add to Schedule ----
        doc = session.doc()
        order = titles(doc)
        summary["title_order"] = order
        check.expect(
            order[:2] == ["Quiet Canticle", "Signal Hymn"],
            f"default order is by title: {order[:4]}",
        )
        by_id = a11y.nodes_by_id(doc)
        session.click(by_id["song-0"])
        session.click(a11y.nodes_by_id(session.doc())["song-3"], modifiers="shift")
        doc = session.doc()
        check.expect(selected(doc) == [0, 1, 2, 3], f"Shift+click selects 0-3: {selected(doc)}")
        session.save("01-shift-range")
        session.click(a11y.nodes_by_id(doc)["song-2"], modifiers="ctrl")
        doc = session.doc()
        check.expect(selected(doc) == [0, 1, 3], f"Ctrl+click removes row 2: {selected(doc)}")
        names = titles(doc)
        session.click(a11y.nodes_by_id(doc)["add-to-schedule"])
        doc = session.wait(
            "Add to Schedule adds all selected songs",
            lambda d: "schedule-item-2" in a11y.nodes_by_id(d),
        )
        found = a11y.nodes_by_id(doc)
        added = [found[f"schedule-item-{i}"]["name"] for i in range(3)]
        check.expect(
            added == [names[0], names[1], names[3]],
            f"schedule holds the selection in list order: {added}",
        )
        session.save("02-added-to-schedule")
        session.click(a11y.nodes_by_id(doc)["song-1"])
        doc = session.doc()
        check.expect(selected(doc) == [1], f"a plain click selects one row: {selected(doc)}")

        # --- VAL-RES-001: Sort by, persisted ------------------------------
        def choose(*aids: str) -> None:
            session.click_aid("song-0", right=True)
            for aid in aids:
                time.sleep(0.2)
                session.click_aid(aid)

        choose("song-menu-sort", "song-sort-modified")
        doc = session.wait(
            "Sort by > Date modified reorders the list",
            lambda d: titles(d)[:2] == ["Signal Hymn", "Quiet Canticle"],
        )
        session.save("03-sorted-by-date")
        session.click_aid("song-0", right=True)
        session.click_aid("song-menu-sort")
        time.sleep(0.3)
        menu = session.doc()
        tick = a11y.nodes_by_id(menu).get("song-sort-modified", {})
        check.expect(tick.get("toggle") == "On", f"Date modified is ticked: {tick.get('toggle')}")
        session.save("04-sort-menu")
        session.click(a11y.nodes_by_id(menu)["song-sort-descending"])
        doc = session.wait(
            "Descending reverses the date order",
            lambda d: titles(d)[0] not in ("Signal Hymn", "Quiet Canticle"),
        )
        newest = titles(doc)[0]
        session.quit()
        session.summary["restarted"] = True
        session.app = nw.App(args.binary, ["--operator-library", str(library)], scratch=scratch)
        session.hwnd = session.app.window()
        nw.activate(session.hwnd)
        time.sleep(1.6)
        doc = session.doc()
        check.expect(
            titles(doc)[0] == newest,
            f"the date/descending order survives a restart: {titles(doc)[:3]}",
        )
        session.save("05-after-restart")
        choose("song-menu-sort", "song-sort-title")
        choose("song-menu-sort", "song-sort-ascending")
        session.wait(
            "Sort by > Title and Ascending restore alphabetical order",
            lambda d: titles(d)[:2] == ["Quiet Canticle", "Signal Hymn"],
        )

        # --- VAL-RES-004: delete the selected song ------------------------
        before = song_count(library)
        doc = session.doc()
        session.click(a11y.nodes_by_id(doc)["song-1"])
        victim = titles(session.doc())[1]
        session.press("delete", pause=0.5)
        doc = session.doc()
        found = a11y.nodes_by_id(doc)
        confirm, cancel = found.get("dialog-confirm"), found.get("dialog-cancel")
        check.expect(
            confirm is not None and confirm["name"] == "Yes",
            f"confirm button reads Yes: {confirm and confirm['name']!r}",
        )
        check.expect(
            cancel is not None and cancel["name"] == "No",
            f"cancel button reads No: {cancel and cancel['name']!r}",
        )
        check.expect(session.focused() == "dialog-cancel", f"focus starts on No: {session.focused()}")
        session.save("06-delete-dialog")
        session.press("enter", pause=0.5)
        check.expect(
            "dialog-cancel" not in a11y.nodes_by_id(session.doc()) and song_count(library) == before,
            "Enter (default No) keeps the song",
        )
        session.press("delete", pause=0.5)
        session.press("escape", pause=0.5)
        check.expect(
            "dialog-cancel" not in a11y.nodes_by_id(session.doc()) and song_count(library) == before,
            "Esc closes the confirmation as No",
        )
        session.press("delete", pause=0.5)
        session.click_aid("dialog-confirm")
        doc = session.wait(
            "the deleted song leaves the list",
            lambda d: victim not in titles(d) and len(titles(d)) >= 1,
        )
        check.expect(len(selected(doc)) == 1, f"a neighbour row is selected: {selected(doc)}")
        time.sleep(0.5)
        check.expect(
            song_count(library) == before - 1,
            "the database row is gone",
        )
        session.save("07-after-delete")
        session.click_aid("song-search")
        nw.type_text(session.hwnd, victim.split()[0][:2].lower())
        time.sleep(1.0)
        search_hit = titles(session.doc())
        check.expect(victim not in search_hit, "the deleted song is absent from search results")
        session.press("escape")

        # --- VAL-FOCUS-001/002: modal Options -----------------------------
        doc = session.doc()
        main_ids = {"go-live", "mask-black", "mask-clear", "mask-logo", "live-next", "Songs"}
        session.click(a11y.nodes_by_id(doc)["Songs"])
        before_status = session.node("Live-status", session.doc())
        session.press("alt+e")
        session.press("enter", pause=0.8)
        found = a11y.nodes_by_id(session.doc())
        check.expect("dialog-cancel" in found, "Options dialog is open")
        width, height = nw.client_size(session.hwnd)
        ok = found.get("dialog-confirm") or found.get("dialog-cancel")
        cx, cy = session.center(ok)
        check.expect(
            abs(cx - width // 2) < width * 0.25 and abs(cy - height // 2) < height * 0.3,
            f"Options buttons sit near the window centre: ({cx},{cy}) in {width}x{height}",
        )
        session.save("08-options-open")
        left = []
        for step in range(14):
            session.press("tab" if step < 9 else "shift+tab", pause=0.2)
            at = session.focused()
            if at in main_ids or at.startswith(("song-", "schedule-")):
                left.append(at)
        check.expect(not left, f"Tab and Shift+Tab never leave the dialog (escaped to {left})")
        session.press("ctrl+b")
        session.press("pagedown")
        found = a11y.nodes_by_id(session.doc())
        check.expect(
            found["mask-black"].get("toggle") == "Off",
            "Ctrl+B does not light Black behind the dialog",
        )
        session.click(found["mask-clear"])
        found = a11y.nodes_by_id(session.doc())
        check.expect(
            found["mask-clear"].get("toggle") == "Off" and "dialog-cancel" in found,
            "a click on the window behind does nothing",
        )
        if before_status:
            check.expect(
                (session.node("Live-status") or {}).get("name") == before_status["name"],
                "Live status is unchanged",
            )
        session.press("escape", pause=0.6)
        found = a11y.nodes_by_id(session.doc())
        check.expect("dialog-cancel" not in found, "Esc closes Options")
        landed = session.focused()
        summary["focus_after_options_escape"] = landed
        check.expect(
            landed in ("Songs", "bar-edit", "") or landed.startswith("song-"),
            f"focus returns to the opener, not into a stale control: {landed!r}",
        )
        session.save("09-options-closed")
        session.quit()
    finally:
        session.app.close()


def large_run(args, summary: dict, check) -> None:
    log = pathlib.Path(tempfile.mkdtemp(prefix="sela-frames-")) / "frames.log"
    os.environ["SELA_FRAME_LOG"] = str(log)
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="sela-resource20k-"))
    session = Session(args, summary, check, scratch, args.library)
    windows: dict[str, tuple[float, float]] = {}
    try:
        doc = session.doc()
        rows = song_rows(doc)
        summary["rows_exposed_initially"] = len(rows)
        check.expect(0 < len(rows) < 60, f"only a window of rows is exposed: {len(rows)} of 20000")
        session.save("20k-01-top")
        session.click(a11y.nodes_by_id(doc)["song-0"])

        session.press("ctrl+end", pause=0.8)
        doc = session.doc()
        last = song_rows(doc)[-1][0] if song_rows(doc) else -1
        check.expect(
            selected(doc) == [19999] and last == 19999,
            f"Ctrl+End selects and shows the last row: selected {selected(doc)}",
        )
        check.expect(session.focused() == "song-19999", f"last row has focus: {session.focused()}")
        session.save("20k-02-end")
        session.press("ctrl+home", pause=0.8)
        doc = session.doc()
        check.expect(selected(doc) == [0], f"Ctrl+Home returns to row 0: {selected(doc)}")
        session.save("20k-03-home")

        # Page Down repeatedly: every press paints a new window of rows.
        started = time.time() * 1000
        seen = set()
        for _ in range(60):
            session.press("pagedown", pause=0.0)
        windows["pagedown"] = (started, time.time() * 1000)
        time.sleep(0.8)
        doc = session.doc()
        cursor = selected(doc)
        check.expect(bool(cursor) and cursor[0] > 100, f"60 Page Downs moved the selection: {cursor}")
        check.expect(bool(song_rows(doc)), "rows are painted at the new position")
        seen.add(tuple(selected(doc)))
        session.save("20k-04-pagedown")

        # Scrollbar drag from top to bottom and back.
        session.press("ctrl+home", pause=0.8)
        bar = session.node("song-scrollbar")
        if bar is None:
            check.expect(False, "scrollbar is exposed")
        else:
            x, top = session.center({**bar, "y": bar["y"] + 6, "height": 0})
            bottom = bar["y"] + bar["height"] - a11y.client_origin(session.hwnd)[1] - 4
            started = time.time() * 1000
            nw.mouse_down(session.hwnd, x, top)
            nw.drag_to(session.hwnd, x, bottom, step=6, pause=0.008)
            time.sleep(0.2)
            nw.mouse_up()
            windows["drag-down"] = (started, time.time() * 1000)
            time.sleep(0.8)
            doc = session.doc()
            rows = song_rows(doc)
            check.expect(
                bool(rows) and rows[-1][0] >= 19990,
                f"dragging the thumb to the bottom shows the end: {rows[-1][0] if rows else None}",
            )
            session.save("20k-05-drag-bottom")
            mid = (top + bottom) // 2
            nw.mouse_down(session.hwnd, x, bottom)
            nw.drag_to(session.hwnd, x, mid, step=6, pause=0.008)
            time.sleep(0.2)
            nw.mouse_up()
            time.sleep(0.8)
            rows = song_rows(session.doc())
            check.expect(
                bool(rows) and 5000 < rows[0][0] < 15000,
                f"a drag to the middle paints rows from the middle: {rows[0][0] if rows else None}",
            )
            session.save("20k-06-drag-middle")

        # Search results use the same list.
        session.click_aid("song-search")
        nw.type_text(session.hwnd, "ma")
        doc = session.wait(
            "a two-letter query fills the list with results", lambda d: len(song_rows(d)) > 0
        )
        found = a11y.nodes_by_id(doc)
        check.expect(
            "song-search-more" in found,
            "a result set cut at 200 shows the 'more results' hint",
        )
        session.save("20k-07-search-more")
        session.press("escape")
        session.quit()
    finally:
        session.app.close()

    if not log.is_file():
        check.expect(False, "frame log was written")
        return
    frames = []
    for line in log.read_text().splitlines():
        unix, gap, rows = (float(v) for v in line.split())
        frames.append((unix, gap, rows))
    stats = {}
    for name, (start, end) in windows.items():
        # The first frame of a window follows an idle gap, not scrolling.
        inside = [(g, r) for t, g, r in frames if start <= t <= end and g > 0][1:]
        if len(inside) < 5:
            check.expect(False, f"{name}: enough frames in the window ({len(inside)})")
            continue
        gaps = [g for g, _ in inside]
        builds = [r for _, r in inside]
        stats[name] = {
            "frames": len(inside),
            "interval_p50_ms": round(percentile(gaps, 0.5), 2),
            "interval_p95_ms": round(percentile(gaps, 0.95), 2),
            "interval_max_ms": round(max(gaps), 2),
            "row_build_p95_ms": round(percentile(builds, 0.95), 3),
            "row_build_max_ms": round(max(builds), 3),
        }
        check.expect(
            stats[name]["interval_p95_ms"] < 33,
            f"{name}: render-to-render p95 {stats[name]['interval_p95_ms']} ms < 33 ms",
        )
    summary["frame_stats"] = stats
    summary["frame_log_frames"] = len(frames)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=pathlib.Path, default=ROOT / "target/debug/sela.exe")
    parser.add_argument(
        "--seed", type=pathlib.Path, default=ROOT / "target/debug/examples/seed_library.exe"
    )
    parser.add_argument(
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/resource-browser-windows"
    )
    parser.add_argument("--library", type=pathlib.Path)
    parser.add_argument("--monitor", default="secondary")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(f"FAIL: missing {path}")
            return 1
    summary: dict = {"status": "FAIL", "binary": str(args.binary.resolve())}
    check = a11y.Check(summary)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    try:
        (large_run if args.library else small_run)(args, summary, check)
        summary["status"] = "FAIL" if check.failures else "PASS"
        return 0 if summary["status"] == "PASS" else 1
    except Exception as error:  # noqa: BLE001
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        return 1
    finally:
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))
        print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
