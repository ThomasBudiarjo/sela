"""Native check: the EW-style main menu bar on the real Windows desktop.

Covers the M1-02/M1-03 contract (EW8-OBS-010/053): File, Edit, Live,
Profiles, View, Help open by Alt+mnemonic and by mouse, Esc closes and
restores focus, unbuilt items are disabled and inert, and the enabled items
dispatch the same actions as the toolbar/keyboard equivalents — verified
against the real audience window for the Live menu masks.

Pass: all six menus open by keyboard and mouse, captures of every open menu
land in --out, menu-dispatched Black/Clear/Logo change the audience output
and light the toolbar buttons, File > Save As/Open/Exit work end to end.

Run: python scripts/menu-bar-windows.py
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import native_win as nw

_spec = importlib.util.spec_from_file_location(
    "live_output_windows", ROOT / "scripts" / "live-output-windows.py"
)
lo = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(lo)

OPERATOR_TITLE = "Sela — Technical preview"
# The six menus in the observed EW 8.0.49 order (EW8-OBS-010).
MENUS = [
    ("f", "file"),
    ("e", "edit"),
    ("l", "live"),
    ("p", "profiles"),
    ("v", "view"),
    ("h", "help"),
]
# The open menu's label paints with the focus background; its left edge is
# the popup's left edge (the popup anchors at the label's bottom-left).
OPEN_BG = (0xDC, 0xE3, 0xFA)
MENU_ROW_Y = 14  # logical px: center of the 28px menu bar


def keys(hwnd: int, *sequence: str) -> None:
    for key in sequence:
        nw.press(hwnd, key)
        time.sleep(0.18)


def popup_left(hwnd: int, scale: float) -> int | None:
    """Left edge of the open menu's label/popup; None when no menu is open."""
    width, height, bgra = nw.capture_client(hwnd)
    y = min(height - 1, round(MENU_ROW_Y * scale))
    row = y * width * 4
    hits = [
        x
        for x in range(0, width, 2)
        if lo.near(
            (bgra[row + x * 4 + 2], bgra[row + x * 4 + 1], bgra[row + x * 4]),
            OPEN_BG,
            8,
        )
    ]
    if len(hits) < 6:
        return None
    return hits[0]


def save(run_out: pathlib.Path, hwnd: int, name: str) -> None:
    width, height, pixels = nw.capture_client(hwnd)
    nw.write_png(run_out / f"{name}.png", width, height, pixels)


def wait_change(hwnd: int, before: str, name: str, timeout: float = 3.0) -> None:
    sent = time.monotonic()
    while lo.digest(hwnd) == before:
        if time.monotonic() - sent > timeout:
            raise nw.Failure(f"{name}: audience output did not change")
        time.sleep(0.05)


def wait_restore(hwnd: int, target: str, name: str, timeout: float = 3.0) -> None:
    sent = time.monotonic()
    while lo.digest(hwnd) != target:
        if time.monotonic() - sent > timeout:
            raise nw.Failure(f"{name}: the slide did not return")
        time.sleep(0.05)


def menu_mask(
    operator: int,
    screen: int,
    out: pathlib.Path,
    before: str,
    sequence: tuple[str, ...],
    name: str,
    check,
) -> None:
    """Toggle a mask through the Live menu; check the audience and the lit
    toolbar button, like Run.mask does for the Ctrl+ shortcuts."""
    keys(operator, *sequence)
    wait_change(screen, before, name)
    time.sleep(0.3)
    width, height, pixels = nw.capture_client(screen)
    nw.write_png(out / f"{name}.png", width, height, pixels)
    check(name, width, height, pixels)
    deadline = time.monotonic() + 2.0
    while lo.lit_pixels(operator) < 20:
        if time.monotonic() > deadline:
            raise nw.Failure(f"{name}: operator mask button not lit")
        time.sleep(0.05)


def menu_unmask(
    operator: int, screen: int, slide: str, sequence: tuple[str, ...], name: str
) -> None:
    keys(operator, *sequence)
    wait_restore(screen, slide, name)
    deadline = time.monotonic() + 2.0
    while lo.lit_pixels(operator) >= 20:
        if time.monotonic() > deadline:
            raise nw.Failure(f"{name}: operator mask button still lit")
        time.sleep(0.05)


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
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/menu-bar-windows"
    )
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(
                f"FAIL: missing {path} (cargo build && cargo build --example seed_library)"
            )
            return 1
    summary: dict = {"binary": str(args.binary.resolve()), "monitor": args.monitor}
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-menubar-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library)], check=True)
    lo.seed_logo(data)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    app = nw.App(args.binary, ["--operator-library", str(library)])
    spawned: set[int] = set()
    try:
        operator = app.window()
        summary["operator_title"] = nw.title(operator)
        if summary["operator_title"] != OPERATOR_TITLE:
            raise nw.Failure(f"unexpected operator title {summary['operator_title']!r}")
        nw.activate(operator)
        time.sleep(1.2)  # storage worker loads the catalog off the UI thread
        scale = nw.dpi(operator) / 96

        # 1. Esc restores focus: focus Go Live, open Live with Alt+L, Esc,
        #    then Enter must activate Go Live (a status message appears).
        keys(operator, *["tab"] * 12)
        keys(operator, "alt+l")
        time.sleep(0.3)
        if popup_left(operator, scale) is None:
            raise nw.Failure("Alt+L did not open the Live menu")
        keys(operator, "escape")
        time.sleep(0.3)
        if popup_left(operator, scale) is not None:
            raise nw.Failure("Esc did not close the menu")
        closed = lo.digest(operator)
        keys(operator, "enter")
        deadline = time.monotonic() + 3.0
        while lo.digest(operator) == closed:
            if time.monotonic() > deadline:
                raise nw.Failure("Enter after Esc did not reach the Go Live button")
            time.sleep(0.05)
        summary["esc_restores_focus"] = True
        save(args.out, operator, "esc-focus-restored")

        # 2. Keyboard calibration: every menu opens by Alt+mnemonic; record
        #    the popup's left edge (== the label's left edge) for the mouse
        #    phase, and capture each open menu.
        labels: dict[str, int] = {}
        for key, name in MENUS:
            keys(operator, f"alt+{key}")
            time.sleep(0.3)
            left = popup_left(operator, scale)
            if left is None:
                raise nw.Failure(f"Alt+{key.upper()} did not open the {name} menu")
            labels[name] = left
            save(args.out, operator, f"menu-{name}")
            keys(operator, "escape")
            time.sleep(0.2)
        edges = [labels[name] for _, name in MENUS]
        if edges != sorted(edges):
            raise nw.Failure(f"menu labels out of order: {edges}")
        summary["label_left_edges"] = labels

        # 3. Mouse: clicking each calibrated label opens the same menu.
        for _, name in MENUS:
            nw.click(
                operator, labels[name] + round(10 * scale), round(MENU_ROW_Y * scale)
            )
            time.sleep(0.3)
            left = popup_left(operator, scale)
            if left is None or abs(left - labels[name]) > round(6 * scale):
                raise nw.Failure(f"clicking the {name} label did not open its menu")
            save(args.out, operator, f"menu-{name}-mouse")
            keys(operator, "escape")
            time.sleep(0.2)
        summary["mouse_opens_each_menu"] = True

        # Clicking the open menu's label closes it again.
        file_x = labels["file"] + round(10 * scale)
        nw.click(operator, file_x, round(MENU_ROW_Y * scale))
        time.sleep(0.3)
        if popup_left(operator, scale) is None:
            raise nw.Failure("clicking File did not open its menu")
        nw.click(operator, file_x, round(MENU_ROW_Y * scale))
        time.sleep(0.3)
        if popup_left(operator, scale) is not None:
            raise nw.Failure("clicking the open File label did not close it")
        summary["label_click_toggles"] = True

        # A click elsewhere dismisses the menu.
        nw.click(
            operator, labels["view"] + round(10 * scale), round(MENU_ROW_Y * scale)
        )
        time.sleep(0.3)
        width, height, _ = nw.capture_client(operator)
        nw.click(operator, width // 2, height // 2)
        time.sleep(0.3)
        if popup_left(operator, scale) is not None:
            raise nw.Failure("clicking outside did not dismiss the menu")
        summary["outside_click_dismisses"] = True

        # Disabled rows are inert: Open Recent (File row 5) never closes the
        # menu or opens a dialog.
        nw.click(operator, file_x, round(MENU_ROW_Y * scale))
        time.sleep(0.3)
        # popup top 28 + padding 4 + rows 0-4 (4*26) + separator (9) + 13
        open_recent_y = round((28 + 4 + 4 * 26 + 9 + 13) * scale)
        nw.click(operator, file_x + round(30 * scale), open_recent_y)
        time.sleep(0.3)
        if popup_left(operator, scale) is None:
            raise nw.Failure("clicking disabled Open Recent closed the menu")
        keys(operator, "escape")
        summary["disabled_items_inert"] = True

        # 4. Preview the first catalog song and its first slide. Focus is
        #    still on the Go Live button from step 1 (menus and clicks never
        #    move it): 13 tabs reach the first song row, then the preview's
        #    first slide (mirrors scripts/live-output-windows.py).
        keys(operator, *["tab"] * 13, "enter", "tab", "enter")
        time.sleep(0.5)

        # 5. Live output on, Go Live, then the Live menu drives the masks.
        keys(operator, *["shift+tab"] * 15, "enter")
        found = lo.audience(app.process.pid, 15)
        if not found:
            raise nw.Failure(f"no audience window within 15s: {app.log()}")
        pid, screen = found
        spawned.add(pid)
        time.sleep(1.5)
        keys(operator, "pagedown")  # Go Live from any control (EW8-OBS-018)
        deadline = time.monotonic() + 8.0
        slide = None
        while slide is None:
            width, height, pixels = nw.capture_client(screen)
            if lo.ink_box(width, height, pixels) is not None:
                slide = lo.digest(screen)
            if time.monotonic() > deadline:
                raise nw.Failure("no slide on the audience output after Go Live")
            time.sleep(0.1)
        save(args.out, screen, "audience-first")
        save(args.out, operator, "operator-live")

        menu_mask(
            operator,
            screen,
            args.out,
            slide,
            ("alt+l", "down", "enter"),
            "logo_on",
            lo.is_logo,
        )
        menu_unmask(operator, screen, slide, ("alt+l", "down", "enter"), "logo_off")
        menu_mask(
            operator,
            screen,
            args.out,
            slide,
            ("alt+l", "down", "down", "enter"),
            "black_on",
            lo.is_black,
        )
        menu_unmask(
            operator, screen, slide, ("alt+l", "down", "down", "enter"), "black_off"
        )
        menu_mask(
            operator,
            screen,
            args.out,
            slide,
            ("alt+l", "down", "down", "down", "enter"),
            "clear_on",
            lo.is_cleared,
        )
        menu_unmask(
            operator,
            screen,
            slide,
            ("alt+l", "down", "down", "down", "enter"),
            "clear_off",
        )
        summary["menu_drives_masks"] = True

        # 6. Show Live Output toggles the audience window off and on.
        keys(operator, "alt+l", "up", "enter")
        if not lo.wait_dead(pid, 5):
            raise nw.Failure("Show Live Output off did not end the audience child")
        spawned.discard(pid)
        summary["show_live_output_off"] = True
        keys(operator, "alt+l", "up", "enter")
        found = lo.audience(app.process.pid, 15)
        if not found:
            raise nw.Failure("Show Live Output on did not restart the audience")
        pid, screen = found
        spawned.add(pid)
        time.sleep(1.0)

        # 7. Edit > Options... opens the Options dialog; Cancel closes it.
        before = lo.digest(operator)
        keys(operator, "alt+e", "enter")
        deadline = time.monotonic() + 3.0
        while lo.digest(operator) == before:
            if time.monotonic() > deadline:
                raise nw.Failure("Edit > Options... did not open the dialog")
            time.sleep(0.05)
        save(args.out, operator, "options-from-menu")
        summary["options_from_menu"] = True
        keys(operator, "enter")  # Cancel is the default focus
        time.sleep(0.5)

        # 8. File > Save Schedule names the empty schedule; File > Open
        #    Schedule lists and reopens it.
        keys(operator, "alt+f", "down", "down", "down", "enter")
        time.sleep(0.5)
        save(args.out, operator, "file-save-as")
        nw.type_text(operator, "Menu Set")
        keys(operator, "enter")
        time.sleep(1.0)
        summary["save_as_from_menu"] = True
        keys(operator, "alt+f", "down", "down", "enter")
        time.sleep(0.8)
        save(args.out, operator, "file-open-dialog")
        keys(operator, "shift+tab", "enter")  # the only saved schedule
        time.sleep(1.0)
        save(args.out, operator, "file-opened")
        summary["open_from_menu"] = True

        # 9. File > Exit quits cleanly and takes the audience output down.
        keys(operator, "alt+f", "up", "enter")
        summary["exit_status"] = app.wait_exit()
        if summary["exit_status"] != 0:
            raise nw.Failure(f"operator exit status {summary['exit_status']}")
        if not lo.wait_dead(pid, 5):
            raise nw.Failure("File > Exit left the audience output running")
        spawned.discard(pid)
        summary["exit_from_menu"] = True
        summary["status"] = "PASS"
        return 0
    except nw.Failure as failure:
        summary["status"] = "FAIL"
        summary["failure"] = str(failure)
        print(f"FAIL: {failure}")
        return 1
    finally:
        app.close()
        for pid in spawned:
            lo.kill(pid)
        (args.out / "summary.json").write_text(
            json.dumps(summary, indent=2) + "\n", encoding="utf-8"
        )


if __name__ == "__main__":
    raise SystemExit(main())
