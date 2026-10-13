"""Windows native check: duplicate, multi-select, drag reorder, Update items.

Usage: python scripts/schedule-remainder-windows.py [--binary sela.exe]
       [--seed seed_library.exe] [--edit edit_song.exe] [--monitor secondary]
       [--out DIR]

Drives the real operator with real mouse input (SendInput, intermediate drag
moves, Shift/Ctrl held during clicks) against a seeded library of five songs.
Row order, selection and names are read from UI Automation; the audience output
by capture; saved .selaschedule files as JSON.

VAL-SCHED-001/002: the context-menu Duplicate puts a selected copy right after
its original; the copy is a separate entry (removing it leaves the original;
a saved file keeps both).
VAL-FOCUS-003: click, Shift+click, Ctrl+click and Shift+Down select the ranges
the contract lists. VAL-SCHED-003: Ctrl+click 2 and 4, drag to the end gives
1,3,5,6,2,4 with both moved items still selected. VAL-FOCUS-005: with several
selected, Go Live shows only the focused item (the audience equals the focused
item alone and differs from the other selected item).
VAL-SCHED-004: a 26-item schedule; dragging the first row into the bottom edge
autoscrolls and the drop lands at the indicator row; an Esc during another drag
leaves the order unchanged. VAL-SCHED-005: after the library song is edited,
Songs > Update items in Schedule lists only the affected items with old/new
text; Cancel leaves the saved snapshots, Confirm updates them. VAL-SCHED-006: a
live updated item keeps the audience until Go Live. VAL-SCHED-007: removing
the live item keeps the audience.
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

SONGS = 5
ROW_H = 28  # logical px, src/operator/schedule.rs SCHEDULE_ROW_H


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
        "--edit",
        type=pathlib.Path,
        default=ROOT / "target/debug/examples/edit_song.exe",
    )
    parser.add_argument("--monitor", default="secondary")
    parser.add_argument(
        "--out",
        type=pathlib.Path,
        default=ROOT / ".amp/in/artifacts/schedule-remainder-windows",
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed, args.edit):
        if not path.is_file():
            print(f"FAIL: missing {path} (cargo build --examples && cargo build)")
            return 1
    summary: dict = {"binary": str(args.binary.resolve()), "monitor": args.monitor}
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-remainder-"))
    library = data / "library.sqlite"
    subprocess.run(
        [str(args.seed), str(library), "--second-song", "--extra-songs", "3"],
        check=True,
    )
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    spawned: set[int] = set()
    app = nw.App(args.binary, ["--operator-library", str(library)])
    hwnd = 0

    def shot(name: str) -> None:
        width, height, pixels = nw.capture_client(hwnd)
        nw.write_png(args.out / f"{name}.png", width, height, pixels)

    def tree() -> dict[str, dict]:
        return a11y.nodes_by_id(a11y.uia(hwnd))

    def nodes() -> list[dict]:
        return a11y.uia(hwnd)["nodes"]

    def row_nodes(doc: list[dict] | None = None) -> list[dict]:
        found = [
            n
            for n in (doc if doc is not None else nodes())
            if n["automationId"].startswith("schedule-item-")
        ]
        return sorted(found, key=lambda n: int(n["automationId"].rsplit("-", 1)[1]))

    def titles() -> list[str]:
        return [n["name"] for n in row_nodes()]

    def picked() -> list[int]:
        return [i + 1 for i, n in enumerate(row_nodes()) if n["selected"]]

    def origin() -> tuple[int, int]:
        return a11y.client_origin(hwnd)

    def centre(node: dict) -> tuple[int, int]:
        ox, oy = origin()
        return node["x"] + node["width"] // 2 - ox, node["y"] + node["height"] // 2 - oy

    def click_id(name: str) -> None:
        nw.click(hwnd, *centre(tree()[name]))
        time.sleep(0.4)

    def row_xy(index: int, doc: list[dict] | None = None) -> tuple[int, int]:
        """Client point of the (1-based) schedule row; the title column, so
        the row's number and live marker are never the target."""
        node = row_nodes(doc)[index - 1]
        return node["x"] - origin()[0] + node["width"] // 2, centre(node)[1]

    def expect(label: str, got, want) -> None:
        summary[label] = got
        if got != want:
            raise nw.Failure(f"{label}: expected {want}, got {got}")

    def add_song(row: int) -> None:
        """Select library row `row` (0-based) and press Add to Schedule."""
        doc = tree()
        nw.click(hwnd, *centre(song_node(songs[row])))
        time.sleep(0.2)
        nw.click(hwnd, *centre(doc["add-to-schedule"]))
        time.sleep(0.35)

    def song_node(title: str) -> dict:
        for n in nodes():
            if n["automationId"].startswith("song-") and n["name"] == title:
                return n
        raise nw.Failure(f"no library row for {title}")

    def audience_state(screen: int) -> str:
        time.sleep(0.5)
        return lo.digest(screen)

    def wait_change(screen: int, before: str, name: str) -> str:
        sent = time.monotonic()
        while lo.digest(screen) == before:
            if time.monotonic() - sent > 3.0:
                raise nw.Failure(f"{name}: audience output did not change within 3s")
            time.sleep(0.05)
        time.sleep(0.3)
        return lo.digest(screen)

    def file_items(path: pathlib.Path) -> list[tuple[str, int, str]]:
        document = json.loads(path.read_text(encoding="utf-8"))
        return [
            (i["song_id"], i["revision"], i["song"]["sections"][0]["lyrics"])
            for i in document["items"]
        ]

    def save_again() -> None:
        nw.click(hwnd, *row_xy(1))
        time.sleep(0.3)
        nw.press(hwnd, "ctrl+s")
        time.sleep(1.0)

    def save_as(path: pathlib.Path) -> None:
        nw.click(hwnd, *row_xy(1))
        time.sleep(0.3)
        nw.press(hwnd, "ctrl+s")
        time.sleep(0.8)
        nw.drive_file_dialog(app.process.pid, hwnd, str(path))
        deadline = time.monotonic() + 5
        while not path.is_file():
            if time.monotonic() > deadline:
                raise nw.Failure("Ctrl+S did not save the schedule file")
            time.sleep(0.05)
        time.sleep(0.4)

    try:
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(1.5)
        songs = [tree()[f"song-{k}"]["name"] for k in range(SONGS)]
        summary["songs"] = songs
        if len(set(songs)) != SONGS:
            raise nw.Failure(f"expected five distinct songs, got {songs}")

        # Six items s1..s5, s1.
        order = [0, 1, 2, 3, 4, 0]
        for song in order:
            add_song(song)
        expect("six_items", titles(), [songs[i] for i in order])
        shot("01-six-items")

        # VAL-SCHED-001/002: Duplicate through the context menu on item 2.
        nw.right_click(hwnd, *row_xy(2))
        time.sleep(0.5)
        shot("02-item-menu")
        click_id("item-menu-duplicate")
        names = titles()
        expect(
            "duplicate_titles",
            names,
            [songs[i] for i in order[:2] + [order[1]] + order[2:]],
        )
        expect("duplicate_selected", picked(), [3])
        shot("03-duplicated")
        # The copy is its own entry: Ctrl+Delete removes only the copy.
        nw.click(hwnd, *row_xy(3))
        time.sleep(0.3)
        nw.press(hwnd, "ctrl+delete")
        time.sleep(0.4)
        expect("copy_removed_leaves_original", titles(), [songs[i] for i in order])
        # Duplicate again and keep it: saving keeps both occurrences.
        nw.right_click(hwnd, *row_xy(2))
        time.sleep(0.5)
        click_id("item-menu-duplicate")
        expect("duplicate_again", len(titles()), 7)
        saved = data / "Remainder.selaschedule"
        save_as(saved)
        stored = file_items(saved)
        expect("saved_duplicate_items", len(stored), 7)
        expect("saved_duplicate_same_song", stored[1][:2], stored[2][:2])
        nw.click(hwnd, *row_xy(3))
        time.sleep(0.3)
        nw.press(hwnd, "ctrl+delete")  # back to six
        time.sleep(0.4)
        expect("six_again", titles(), [songs[i] for i in order])

        # VAL-FOCUS-003: click 2, Shift+click 5, Ctrl+click 3, Ctrl+click 6.
        nw.click(hwnd, *row_xy(2))
        time.sleep(0.3)
        nw.click_with(hwnd, "shift", *row_xy(5))
        time.sleep(0.4)
        expect("shift_range", picked(), [2, 3, 4, 5])
        shot("04-shift-range")
        nw.click_with(hwnd, "ctrl", *row_xy(3))
        time.sleep(0.4)
        expect("ctrl_deselect", picked(), [2, 4, 5])
        nw.click_with(hwnd, "ctrl", *row_xy(6))
        time.sleep(0.4)
        expect("ctrl_add", picked(), [2, 4, 5, 6])
        shot("05-ctrl-selection")
        nw.click(hwnd, *row_xy(1))
        time.sleep(0.3)
        nw.press(hwnd, "shift+down")
        time.sleep(0.2)
        nw.press(hwnd, "shift+down")
        time.sleep(0.4)
        expect("shift_down_extends", picked(), [1, 2, 3])

        # VAL-SCHED-003: Ctrl+click 2 and 4, drag to the end.
        nw.click(hwnd, *row_xy(2))
        time.sleep(0.3)
        nw.click_with(hwnd, "ctrl", *row_xy(4))
        time.sleep(0.4)
        expect("two_and_four", picked(), [2, 4])
        before = [songs[i] for i in order]
        doc = nodes()
        bottom = row_nodes(doc)[-1]
        oy = origin()[1]
        end_y = bottom["y"] + bottom["height"] - oy - 3
        start = row_xy(2, doc)
        nw.mouse_down(hwnd, *start)
        nw.drag_to(hwnd, start[0], start[1] + 40)
        nw.drag_to(hwnd, start[0], end_y)
        time.sleep(0.4)
        shot("06-drag-to-end")
        nw.mouse_up()
        time.sleep(0.6)
        moved = [before[i - 1] for i in (1, 3, 5, 6, 2, 4)]
        expect("multi_move_order", titles(), moved)
        expect("multi_move_selected", picked(), [5, 6])
        shot("07-moved")

        # VAL-FOCUS-005: Go Live applies only the focused item of a selection.
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
        nw.click(hwnd, *row_xy(1))  # s1
        time.sleep(0.3)
        nw.click_with(hwnd, "ctrl", *row_xy(3))  # s5, now focused
        time.sleep(0.4)
        expect("live_selection", picked(), [1, 3])
        click_id("go-live")
        multi = wait_change(screen, blank, "go live with a selection")
        shot("08-operator-multi-live")
        w, h, px = nw.capture_client(screen)
        nw.write_png(args.out / "08-audience-multi-live.png", w, h, px)
        nw.click(hwnd, *row_xy(3))  # the focused item alone
        time.sleep(0.4)
        click_id("go-live")
        time.sleep(0.8)
        alone = lo.digest(screen)
        if alone != multi:
            raise nw.Failure("selection Go Live differs from the focused item alone")
        nw.click(hwnd, *row_xy(1))
        time.sleep(0.4)
        click_id("go-live")
        other = wait_change(screen, multi, "the other selected item alone")
        summary["multi_selection_only_focused_live"] = other != multi
        nw.click(hwnd, *row_xy(1))
        time.sleep(0.2)
        nw.click_with(hwnd, "ctrl", *row_xy(3))
        time.sleep(0.4)
        click_id("go-live")
        again = wait_change(screen, other, "go live again with the selection")
        expect("multi_live_is_focused_item", again == multi, True)

        # VAL-SCHED-004: 26 items; autoscroll drag and Esc cancel.
        schedule_before = titles()
        while len(schedule_before) < 26:
            add_song((len(schedule_before) * 2) % SONGS)
            schedule_before = titles()
        expect("long_schedule", len(schedule_before), 26)
        nw.click(hwnd, *row_xy(1))
        time.sleep(0.3)
        doc = nodes()
        list_node = a11y.nodes_by_id({"nodes": doc})["schedule-list"]
        list_bottom = list_node["y"] + list_node["height"] - oy
        y0 = row_nodes(doc)[0]["y"]
        start = row_xy(1, doc)
        nw.mouse_down(hwnd, *start)
        nw.drag_to(hwnd, start[0], start[1] + 40)
        nw.drag_to(hwnd, start[0], list_bottom - 6)
        deadline = time.monotonic() + 8
        while True:
            time.sleep(0.5)
            if row_nodes()[0]["y"] < y0 - 3 * ROW_H:
                break
            if time.monotonic() > deadline:
                raise nw.Failure("dragging at the bottom edge never scrolled the list")
        scrolled = row_nodes()[0]["y"]
        summary["autoscroll_first_row_moved_px"] = y0 - scrolled
        shot("09-autoscroll-drag")
        pointer = list_bottom - 6
        nw.mouse_up()
        time.sleep(0.8)
        after = titles()
        # Dropped lower than where it started and placed where the pointer was.
        index = max(i for i in range(26) if after[i] == schedule_before[0])
        landed = row_nodes()
        landed_y = landed[index]["y"] - oy
        rest = after[:index] + after[index + 1 :]
        expect("drag_rest_keeps_order", rest, schedule_before[1:])
        summary["drag_landed_index"] = index
        if index < 14:
            raise nw.Failure(f"drop did not land after autoscroll: index {index}")
        if abs(landed_y + ROW_H - pointer) > 2 * ROW_H * nw.dpi(hwnd) / 96:
            raise nw.Failure(
                f"drop landed away from the indicator: row y {landed_y}, pointer {pointer}"
            )
        shot("10-after-drop")

        # Esc during another drag leaves the order unchanged.
        order_now = titles()
        doc = nodes()
        start = row_xy(2, doc)
        target = row_xy(5, doc)
        nw.mouse_down(hwnd, *start)
        nw.drag_to(hwnd, start[0], start[1] + 40)
        nw.drag_to(hwnd, target[0], target[1])
        time.sleep(0.4)
        shot("11-drag-before-esc")
        nw.press(hwnd, "escape")
        time.sleep(0.4)
        nw.mouse_up()
        time.sleep(0.6)
        expect("esc_cancel_order", titles(), order_now)
        shot("12-after-esc")

        # Back to a short schedule: rows 1..3 are enough for the update checks.
        while len(row_nodes()) > 3:
            nw.click(hwnd, *row_xy(len(row_nodes())))
            nw.press(hwnd, "ctrl+delete")
            time.sleep(0.25)
        names = titles()
        summary["short_schedule"] = names

        # VAL-SCHED-005/006: edit "Study Hymn 1" in the library file.
        target_song = songs[2]
        target_rows = [i + 1 for i, n in enumerate(names) if n == target_song]
        if not target_rows:
            nw.click(hwnd, *row_xy(1))
            add_song(2)
            names = titles()
            target_rows = [i + 1 for i, n in enumerate(names) if n == target_song]
        # Put the target item live (old text), then edit the library song.
        nw.click(hwnd, *row_xy(target_rows[0]))
        time.sleep(0.4)
        click_id("go-live")
        old_live = wait_change(screen, again, "target item live")
        saved2 = saved
        save_again()
        old_items = file_items(saved2)
        subprocess.run(
            [str(args.edit), str(library), target_song, "Revised audience line"],
            check=True,
        )
        # Songs list > right-click > Refresh, then Update items in Schedule.
        nw.right_click(hwnd, *centre(song_node(target_song)))
        time.sleep(0.5)
        shot("13-song-menu")
        menu = {n["name"]: n for n in nodes() if n["name"]}
        nw.click(hwnd, *centre(menu["Refresh"]))
        time.sleep(1.5)
        nw.right_click(hwnd, *centre(song_node(target_song)))
        time.sleep(0.5)
        menu = {n["name"]: n for n in nodes() if n["name"]}
        nw.click(hwnd, *centre(menu["Update items in Schedule"]))
        time.sleep(1.0)
        dialog = nodes()
        shot("14-update-diff")
        listed = [n for n in dialog if n["automationId"].startswith("update-item-")]
        expect("diff_lists_only_affected", len(listed), len(target_rows))
        expect(
            "diff_item_titles",
            [n["name"] for n in listed],
            [target_song] * len(target_rows),
        )
        # Old/new slide text is painted, not exposed to UIA: it is checked in
        # 14-update-diff.png and, as data, by the saved file after Confirm.
        # Cancel leaves the snapshots.
        click_id("dialog-cancel")
        time.sleep(0.4)
        save_again()
        expect("cancel_keeps_snapshots", file_items(saved2), old_items)
        shot("15-after-cancel")
        # Confirm updates them, the live audience unchanged until Go Live.
        nw.right_click(hwnd, *centre(song_node(target_song)))
        time.sleep(0.5)
        menu = {n["name"]: n for n in nodes() if n["name"]}
        nw.click(hwnd, *centre(menu["Update items in Schedule"]))
        time.sleep(1.0)
        click_id("dialog-confirm")
        time.sleep(1.0)
        save_again()
        updated = file_items(saved2)
        for index, (before_item, after_item) in enumerate(zip(old_items, updated)):
            changed = names[index] == target_song
            if changed:
                expect(
                    f"item_{index}_new_lyrics", after_item[2], "Revised audience line"
                )
                if after_item[1] <= before_item[1]:
                    raise nw.Failure(
                        "confirm did not move the item to the new revision"
                    )
            else:
                expect(f"item_{index}_unchanged", after_item, before_item)
        shot("16-after-confirm")
        time.sleep(0.8)
        if lo.digest(screen) != old_live:
            raise nw.Failure("updating the live item changed the audience")
        w, h, px = nw.capture_client(screen)
        nw.write_png(args.out / "16-audience-after-update.png", w, h, px)
        summary["update_kept_audience"] = True
        nw.click(hwnd, *row_xy(target_rows[0]))
        time.sleep(0.4)
        click_id("go-live")
        new_live = wait_change(screen, old_live, "Go Live after update")
        w, h, px = nw.capture_client(screen)
        nw.write_png(args.out / "17-audience-after-go-live.png", w, h, px)
        summary["go_live_shows_update"] = new_live != old_live

        # VAL-SCHED-007: removing the live item keeps the audience.
        count = len(titles())
        nw.click(hwnd, *row_xy(target_rows[0]))
        time.sleep(0.3)
        nw.press(hwnd, "ctrl+delete")
        time.sleep(1.0)
        expect("live_item_removed", len(titles()), count - 1)
        if lo.digest(screen) != new_live:
            raise nw.Failure("removing the live item changed the audience")
        shot("18-live-item-removed")
        summary["removed_live_item_kept_audience"] = True

        summary["status"] = "PASS"
        print(
            "PASS: duplicate, Shift/Ctrl selection, multi-item drag move, autoscroll "
            "and Esc-cancelled drag, Update items diff (Cancel/Confirm), live update "
            "and live removal keep the audience, multi-selection sends one cue"
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
        nw.user32.mouse_event(0x0004, 0, 0, 0, 0)
        app.close()
        for pid in spawned:
            if lo.alive(pid):
                lo.kill(pid)
        summary.setdefault("status", "FAIL")
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
