"""Windows native check: accessibility basics (M1-16, VAL-A11Y-001..005).

Usage: python scripts/a11y-windows.py [--binary sela.exe] [--seed seed_library.exe]
       [--monitor window] [--out DIR]

Inspects the running operator through UI Automation
(scripts/uia-names-windows.ps1, .NET UIAutomationClient) and drives it
natively:

- VAL-A11Y-001: a UIA dump shows the AccessKit role (control type) and a
  non-empty, correct English name for each critical control: menu bar and
  its six menus, Go Live, Black, Clear, Logo, Live previous/next, Live
  output, the five Resources tabs, and the Songs/Schedule/Preview/Live
  lists with their rows and slide tiles, and the Songs search box (Edit,
  "Search songs").
- VAL-A11Y-002: after Ctrl+B the Black button's UIA toggle state is On and
  Clear/Logo stay Off; a second Ctrl+B turns it off again.
- VAL-A11Y-003: after Options > General > Language > Bahasa Indonesia the
  same controls carry the Indonesian names without a restart; switching
  back restores English.
- VAL-A11Y-004: with no mouse input, Tab reaches every critical control
  (UIA HasKeyboardFocus + a FOCUS_BG/CANVAS_FOCUS focus-ring pixel probe +
  a capture per stop); Enter/Space on Go Live, Black, Clear, Logo, previous
  and next act on the audience output like a click. Shift+Tab walks back.
- VAL-A11Y-005: with focus on a Schedule row, Down/Up move the selection
  and UIA reports the newly selected row's name; same in the Songs list.

Takes over the real keyboard and mouse; never run with EasyWorship open.
"""

from __future__ import annotations

import argparse
import ctypes
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time
from ctypes import wintypes as w

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

ROOT = pathlib.Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location(
    "live_output_windows", ROOT / "scripts" / "live-output-windows.py"
)
lo = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(lo)

UIA_SCRIPT = ROOT / "scripts" / "uia-names-windows.ps1"
USER32 = ctypes.windll.user32

# Operator focus fills (src/ui/theme.rs): buttons/rows FOCUS_BG 0xdce3fa,
# slide tiles CANVAS_FOCUS 0x2c3550, as BGR capture triples.
FOCUS_BGR = (0xFA, 0xE3, 0xDC)
TILE_FOCUS_BGR = (0x50, 0x35, 0x2C)

# (AutomationId, UIA control type, English name, Indonesian name).
EXPECTED = [
    ("go-live", "Button", "Go Live", "Tayangkan"),
    ("live-previous", "Button", "‹ Previous", "‹ Sebelumnya"),
    ("live-next", "Button", "Next ›", "Berikutnya ›"),
    ("mask-black", "Button", "Black", "Hitam"),
    ("mask-clear", "Button", "Clear", "Kosong"),
    ("mask-logo", "Button", "Logo", "Logo"),
    ("schedule-list", "List", "Schedule", "Jadwal"),
    ("Preview", "List", "Preview", "Pratinjau"),
    ("Live", "List", "Live", "Langsung"),
    ("song-list", "List", "Songs", "Lagu"),
    ("song-search", "Edit", "Search songs", "Cari lagu"),
    ("Songs", "TabItem", "Songs", "Lagu"),
    ("Scriptures", "TabItem", "Scriptures", "Alkitab"),
    ("Media", "TabItem", "Media", "Media"),
    ("Presentations", "TabItem", "Presentations", "Presentasi"),
    ("Themes", "TabItem", "Themes", "Tema"),
    ("menu-bar", "MenuBar", "Menu bar", "Bilah menu"),
    ("bar-file", "MenuItem", "File", "Berkas"),
    ("bar-edit", "MenuItem", "Edit", "Sunting"),
    ("bar-live", "MenuItem", "Live", "Langsung"),
    ("bar-profiles", "MenuItem", "Profiles", "Profil"),
    ("bar-view", "MenuItem", "View", "Tampilan"),
    ("bar-help", "MenuItem", "Help", "Bantuan"),
]

# Stops VAL-A11Y-004 must reach by Tab (schedule/song rows need seeded data).
REQUIRED_STOPS = [
    "go-live",
    "live-previous",
    "live-next",
    "mask-black",
    "mask-clear",
    "mask-logo",
    "Songs",
    "Scriptures",
    "Media",
    "Presentations",
    "Themes",
    "song-0",
    "schedule-item-0",
]


def uia(hwnd: int, out: pathlib.Path | None = None, focused: bool = False) -> dict:
    command = [
        "powershell.exe",
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        str(UIA_SCRIPT),
        "-Hwnd",
        str(hwnd),
    ]
    if focused:
        command.append("-FocusedOnly")
    if out is not None:
        command += ["-OutJson", str(out)]
    run = subprocess.run(
        command,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=180,
        check=False,
    )
    if run.returncode != 0:
        raise nw.Failure(f"uia-names-windows.ps1 failed: {run.stderr.strip()[:400]}")
    return json.loads(run.stdout)


def nodes_by_id(doc: dict) -> dict[str, dict]:
    found: dict[str, dict] = {}
    for node in doc["nodes"]:
        aid = node["automationId"]
        if aid and aid not in found:
            found[aid] = node
    return found


def client_origin(hwnd: int) -> tuple[int, int]:
    point = w.POINT(0, 0)
    USER32.ClientToScreen(hwnd, ctypes.byref(point))
    return point.x, point.y


def click_node(hwnd: int, node: dict) -> None:
    """Left click at the center of a UIA node's bounding rectangle."""
    origin_x, origin_y = client_origin(hwnd)
    x = node["x"] + node["width"] // 2 - origin_x
    y = node["y"] + node["height"] // 2 - origin_y
    nw.click(hwnd, x, y)
    time.sleep(0.3)


def show_live_output(hwnd: int) -> None:
    """Live > Show Live Output through the open menu's UIA row."""
    nw.press(hwnd, "alt+l")
    time.sleep(0.4)
    found = nodes_by_id(uia(hwnd))
    if "bar-live-show" not in found:
        raise nw.Failure("Live menu row bar-live-show missing from the UIA tree")
    click_node(hwnd, found["bar-live-show"])


def park_cursor(hwnd: int) -> None:
    """Park the pointer over an inert area inside the operator window.

    Parking outside the window leaves hover stuck on the last-clicked
    element: GPUI's Windows backend never receives WM_MOUSELEAVE, so only a
    WM_MOUSEMOVE over the window clears hover. A stuck hover fill masks the
    focus fill the walk probes for. The empty area below the (few) seeded
    song rows is inert and holds no probe target.
    """
    width, height = nw.client_size(hwnd)
    point = w.POINT(int(width * 0.55), height - 70)
    USER32.ClientToScreen(hwnd, ctypes.byref(point))
    USER32.SetCursorPos(point.x, point.y)
    time.sleep(0.3)


def focus_fill_hits(
    hwnd: int, node: dict, bgr: tuple[int, int, int]
) -> tuple[int, int]:
    """Sample interior points of the node for the focus fill color."""
    width, height, pixels = nw.capture_client(hwnd)
    origin_x, origin_y = client_origin(hwnd)
    x0 = node["x"] - origin_x
    y0 = node["y"] - origin_y
    hits = total = 0
    for fx in (0.25, 0.5, 0.75):
        for fy in (0.25, 0.5, 0.75):
            x = x0 + int(node["width"] * fx)
            y = y0 + int(node["height"] * fy)
            if 0 <= x < width and 0 <= y < height:
                total += 1
                i = (y * width + x) * 4
                if lo.near(tuple(pixels[i : i + 3]), bgr, 6):
                    hits += 1
    return hits, total


def audience_ink(hwnd: int) -> bool:
    width, height, pixels = nw.capture_client(hwnd)
    return lo.ink_box(width, height, pixels) is not None


def logo_on_audience(hwnd: int) -> bool:
    width, height, pixels = nw.capture_client(hwnd)
    return lo.hits(width, height, pixels, lo.LOGO_RGB) > 200


def focused_toggle(hwnd: int, aid: str) -> str | None:
    node = uia(hwnd, focused=True)["nodes"][0]
    if node["automationId"] != aid:
        return None
    return node.get("toggle")


def wait_for(timeout: float, condition) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if condition():
            return True
        time.sleep(0.1)
    return False


def live_status(hwnd: int) -> str:
    """The Live pane status lines (joined), as the operator announces them."""
    node = nodes_by_id(uia(hwnd)).get("Live-status")
    return node["name"] if node else ""


def wait_live_status(
    hwnd: int, needle: str, timeout: float, what: str, unless: str = ""
) -> str:
    """Waits (bounded) until the Live status contains `needle` and not
    `unless`; on timeout the failure carries the operator's own state so a
    flake is diagnosable."""
    deadline = time.monotonic() + timeout
    while True:
        status = live_status(hwnd)
        if needle in status and not (unless and unless in status):
            return status
        if time.monotonic() >= deadline:
            raise nw.Failure(
                f"{what}: Live status never showed {needle!r} within "
                f"{timeout:.0f}s; operator state: {status!r}"
            )
        time.sleep(0.3)


def stable_digest(hwnd: int, timeout: float = 5.0) -> str:
    """The audience frame digest once it stops changing (Go Live fades in)."""
    deadline = time.monotonic() + timeout
    last = lo.digest(hwnd)
    while time.monotonic() < deadline:
        time.sleep(0.6)
        current = lo.digest(hwnd)
        if current == last:
            return current
        last = current
    return last


def monitors(binary: pathlib.Path) -> list[str]:
    out = subprocess.run(
        [str(binary), "--list-monitors"], capture_output=True, text=True, check=True
    ).stdout
    return [line.split("\t")[0] for line in out.splitlines() if line.strip()]


class Check:
    def __init__(self, summary: dict):
        self.summary = summary
        self.failures: list[str] = summary.setdefault("failures", [])

    def expect(self, condition: bool, what: str) -> bool:
        print(f"{'ok' if condition else 'FAIL'}: {what}")
        if not condition:
            self.failures.append(what)
        return condition

    def expect_controls(self, doc: dict, name_index: int, language: str) -> None:
        found = nodes_by_id(doc)
        for aid, ctype, english, indonesian in EXPECTED:
            name = (english, indonesian)[name_index - 2]
            node = found.get(aid)
            if node is None:
                self.expect(False, f"{language}: missing UIA node {aid}")
                continue
            self.expect(
                node["controlType"] == ctype and node["name"] == name,
                f"{language}: {aid} is {node['controlType']} named {node['name']!r} "
                f"(want {ctype} named {name!r})",
            )


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
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/a11y-windows"
    )
    # Fullscreen on a secondary monitor keeps the audience output from
    # overlapping the operator: captures are screen BitBlts (native_win.py),
    # so an overlapping transparent window would pollute every probe.
    parser.add_argument("--monitor", default="secondary")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(
                f"FAIL: missing {path} (cargo build && cargo build --example seed_library)"
            )
            return 1
    summary: dict = {"status": "FAIL", "binary": str(args.binary.resolve())}
    check = Check(summary)
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-a11y-"))
    library = data / "library.sqlite"
    subprocess.run([str(args.seed), str(library), "--second-song"], check=True)
    lo.seed_logo(data)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    app = nw.App(args.binary, ["--operator-library", str(library)])
    hwnd = 0
    audience_hwnd = 0
    try:
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(1.2)
        scale = nw.dpi(hwnd) / 96
        summary["dpi_scale"] = scale

        def save(name: str) -> None:
            width, height, pixels = nw.capture_client(hwnd)
            nw.write_png(args.out / f"{name}.png", width, height, pixels)

        def press(keys: str) -> None:
            # The windowed audience output occasionally takes the foreground;
            # the checks here target the operator, so reclaim it and note it.
            try:
                nw.press(hwnd, keys)
            except nw.Failure:
                nw.activate(hwnd)
                summary["focus_recoveries"] = summary.get("focus_recoveries", 0) + 1
                nw.press(hwnd, keys)

        # --- VAL-A11Y-001: roles and names, English ---------------------
        doc = uia(hwnd, args.out / "01-dump-english.json")
        save("01-english")
        check.expect_controls(doc, 2, "en")
        found = nodes_by_id(doc)
        edit_nodes = [n for n in doc["nodes"] if n["controlType"] == "Edit"]
        check.expect(
            [n["automationId"] for n in edit_nodes] == ["song-search"],
            f"the library search box is the only Edit: {[n['automationId'] for n in edit_nodes]}",
        )
        # Menu titles expose Expanded/Collapsed.
        for top in (
            "bar-file",
            "bar-edit",
            "bar-live",
            "bar-profiles",
            "bar-view",
            "bar-help",
        ):
            node = found.get(top, {})
            check.expect(
                node.get("expanded") in ("Collapsed", "Expanded"),
                f"menu title {top} reports expand state ({node.get('expanded')!r})",
            )
        # Open the File menu: the popup is a Menu of named MenuItems, the
        # title reports Expanded, and not-yet-built items report disabled.
        click_node(hwnd, found["bar-file"])
        doc = uia(hwnd, args.out / "01b-dump-file-menu.json")
        found = nodes_by_id(doc)
        menu = found.get("bar-menu")
        check.expect(
            menu is not None and menu["controlType"] == "Menu" and bool(menu["name"]),
            f"open File popup is a named Menu ({menu and menu['controlType']!r})",
        )
        items = [
            n
            for n in doc["nodes"]
            if n["parent"] == "bar-menu" and n["controlType"].startswith("MenuItem")
        ]
        check.expect(
            items and all(bool(n["name"]) for n in items),
            f"every File menu item is named ({len(items)} items)",
        )
        check.expect(
            any(not n["isEnabled"] for n in items),
            "disabled File menu items report IsEnabled=false",
        )
        check.expect(
            found["bar-file"].get("expanded") == "Expanded",
            "open menu title reports Expanded",
        )
        save("01b-file-menu")
        press("escape")
        time.sleep(0.4)
        doc = uia(hwnd)
        found = nodes_by_id(doc)

        # Seed the schedule: select each song row, click Add to Schedule.
        click_node(hwnd, found["song-0"])
        click_node(hwnd, found["add-to-schedule"])
        click_node(hwnd, found["song-1"])
        click_node(hwnd, found["add-to-schedule"])
        # Select the three-slide song so Live previous/next have somewhere to go.
        hymn_aid = next(
            (
                aid
                for aid in ("song-0", "song-1")
                if found[aid]["name"] == "Signal Hymn"
            ),
            None,
        )
        if hymn_aid is None:
            raise nw.Failure("seeded library has no 'Signal Hymn' song row")
        click_node(hwnd, found[hymn_aid])
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            doc = uia(hwnd)
            found = nodes_by_id(doc)
            if "preview-slide-2" in found and "schedule-item-1" in found:
                break
            time.sleep(0.4)
        doc = uia(hwnd, args.out / "02-dump-seeded.json")
        found = nodes_by_id(doc)
        entry_titles = [
            found["schedule-item-0"]["name"],
            found["schedule-item-1"]["name"],
        ]
        song_titles = [found["song-0"]["name"], found["song-1"]["name"]]
        for aid in ("schedule-item-0", "schedule-item-1", "song-0", "song-1"):
            node = found.get(aid)
            check.expect(
                node is not None
                and node["controlType"] == "ListItem"
                and bool(node["name"]),
                f"row {aid} is a named ListItem",
            )
        check.expect(
            all(
                f"preview-slide-{i}" in found
                and found[f"preview-slide-{i}"]["controlType"] == "ListItem"
                for i in range(3)
            ),
            "three preview slide tiles are ListItems",
        )

        # --- VAL-A11Y-002: mask toggle state ----------------------------
        click_node(hwnd, found["live-output"])
        seen = lo.audience(app.process.pid, 15)
        if not seen:
            raise nw.Failure("no audience window after Live output on")
        _, audience_hwnd = seen
        time.sleep(1.0)
        # In --monitor window mode the audience window can open foreground.
        nw.activate(hwnd)
        press("ctrl+b")
        time.sleep(0.8)
        doc = uia(hwnd, args.out / "03-dump-black-on.json")
        found = nodes_by_id(doc)
        check.expect(
            found["mask-black"].get("toggle") == "On", "Black toggled on (UIA)"
        )
        check.expect(found["mask-clear"].get("toggle") == "Off", "Clear stays off")
        check.expect(found["mask-logo"].get("toggle") == "Off", "Logo stays off")
        save("02-black-lit")
        press("ctrl+b")
        time.sleep(0.8)
        doc = uia(hwnd)
        found = nodes_by_id(doc)
        check.expect(
            found["mask-black"].get("toggle") == "Off", "Black toggled off again (UIA)"
        )
        check.expect(
            found["live-output"].get("toggle") == "On", "Live output reports on (UIA)"
        )

        # --- VAL-A11Y-005: rows navigate and report selection -----------
        click_node(hwnd, found["schedule-item-0"])
        press("down")
        time.sleep(0.6)
        doc = uia(hwnd, args.out / "04-dump-schedule-down.json")
        found = nodes_by_id(doc)
        node = found["schedule-item-1"]
        check.expect(
            node.get("selected") is True and node["name"] == entry_titles[1],
            f"Down in the Schedule selects {node['name']!r} (UIA selected)",
        )
        check.expect(
            found["schedule-item-0"].get("selected") is False,
            "the previous Schedule row reports unselected",
        )
        press("up")
        time.sleep(0.6)
        doc = uia(hwnd)
        found = nodes_by_id(doc)
        check.expect(
            found["schedule-item-0"].get("selected") is True,
            "Up in the Schedule selects the first row again",
        )
        click_node(hwnd, found["song-0"])
        press("down")
        time.sleep(0.6)
        doc = uia(hwnd, args.out / "05-dump-songs-down.json")
        found = nodes_by_id(doc)
        node = found["song-1"]
        check.expect(
            node.get("selected") is True and node["name"] == song_titles[1],
            f"Down in the Songs list selects {node['name']!r} (UIA selected)",
        )
        press("up")
        time.sleep(0.6)
        doc = uia(hwnd)
        found = nodes_by_id(doc)
        check.expect(
            found["song-0"].get("selected") is True,
            "Up in the Songs list selects the first row again",
        )
        save("03-selection")

        # --- VAL-A11Y-004: keyboard-only walk ---------------------------
        # Reselect the three-slide song for the Live previous/next checks.
        click_node(hwnd, found[hymn_aid])
        time.sleep(1.0)
        park_cursor(hwnd)  # clear hover off the clicked row before probing
        # A mouse-seeded start can leave UIA focus on the root window; Tab
        # until a real control (non-empty AutomationId) holds it instead of
        # assuming the click focused one.
        for _ in range(4):
            seeded = uia(hwnd, focused=True)["nodes"]
            if seeded and seeded[0]["automationId"] and seeded[0]["controlType"] != "Window":
                break
            press("tab")
            time.sleep(0.4)
        sequence: list[str] = []
        probed: set[str] = set()
        acted: set[str] = set()
        slide_digest: str | None = None
        walk_limit = 3 * (len(REQUIRED_STOPS) + 30)
        for _ in range(walk_limit):
            focused = uia(hwnd, focused=True)
            if not focused["nodes"]:
                raise nw.Failure("UIA reports no focused element during the walk")
            node = focused["nodes"][0]
            aid = node["automationId"]
            sequence.append(aid)
            if aid in REQUIRED_STOPS and aid not in probed:
                probed.add(aid)
                save(f"walk-{aid}")
                hits, total = focus_fill_hits(hwnd, node, FOCUS_BGR)
                if not (hits * 2 > total > 0):
                    # A mid-relayout UIA rect can be stale; re-query once.
                    time.sleep(0.5)
                    node = uia(hwnd, focused=True)["nodes"][0]
                    hits, total = focus_fill_hits(hwnd, node, FOCUS_BGR)
                ring = hits * 2 > total > 0
                check.expect(
                    node["hasKeyboardFocus"] and ring,
                    f"Tab stop {aid}: UIA focus={node['hasKeyboardFocus']} "
                    f"focus ring pixels {hits}/{total} "
                    f"rect=({node['x']},{node['y']} {node['width']}x{node['height']})",
                )

            # Activation parity with click, verified on the audience. The
            # mask and previous/next checks need a live slide, so they wait
            # for the cycle after Enter on Go Live. Masks acknowledge
            # asynchronously (the indicator lights on renderer ack), so each
            # check polls instead of sleeping a fixed time.
            if aid == "go-live" and aid not in acted:
                acted.add(aid)
                press("enter")
                live = wait_for(4.0, lambda: audience_ink(audience_hwnd))
                slide_digest = stable_digest(audience_hwnd) if live else None
                check.expect(live, "Enter on Go Live puts the slide on the audience")
            elif aid == "live-next" and aid not in acted and slide_digest:
                acted.add(aid)
                slide = slide_digest
                press("space")
                moved = wait_for(3.0, lambda s=slide: lo.digest(audience_hwnd) != s)
                check.expect(moved, "Space on Next advances the audience slide")
            elif aid == "live-previous" and aid not in acted and slide_digest:
                acted.add(aid)
                slide = slide_digest
                press("enter")
                back = wait_for(3.0, lambda s=slide: lo.digest(audience_hwnd) == s)
                check.expect(back, "Enter on Previous returns the audience slide")
            elif aid == "mask-black" and aid not in acted and slide_digest:
                acted.add(aid)
                # The walk passes live-next before the masks, so the live
                # slide may not be slide 0: reference the frame on screen now.
                before = stable_digest(audience_hwnd)
                press("space")
                black = wait_for(
                    3.0, lambda: lo.brightest(nw.capture_client(audience_hwnd)[2]) <= 16
                )
                on = wait_for(2.0, lambda a=aid: focused_toggle(hwnd, a) == "On")
                check.expect(
                    black and on, "Space on Black blacks the audience + toggles on"
                )
                press("space")
                restored = wait_for(3.0, lambda b=before: lo.digest(audience_hwnd) == b)
                check.expect(restored, "Space on Black again restores the slide")
            elif aid == "mask-clear" and aid not in acted and slide_digest:
                acted.add(aid)
                before = stable_digest(audience_hwnd)
                press("enter")
                cleared = wait_for(3.0, lambda: not audience_ink(audience_hwnd))
                on = wait_for(2.0, lambda a=aid: focused_toggle(hwnd, a) == "On")
                check.expect(
                    cleared and on, "Enter on Clear clears the audience + toggles on"
                )
                press("enter")
                restored = wait_for(3.0, lambda b=before: lo.digest(audience_hwnd) == b)
                check.expect(restored, "Enter on Clear again restores the slide")
            elif aid == "mask-logo" and aid not in acted and slide_digest:
                acted.add(aid)
                before = stable_digest(audience_hwnd)
                press("space")
                shown = wait_for(4.0, lambda: logo_on_audience(audience_hwnd))
                on = wait_for(2.0, lambda a=aid: focused_toggle(hwnd, a) == "On")
                check.expect(shown and on, "Space on Logo shows the logo + toggles on")
                press("space")
                restored = wait_for(3.0, lambda b=before: lo.digest(audience_hwnd) == b)
                if not restored:
                    # Distinguish "key did not dispatch" from "renderer kept
                    # the logo": record the toggle and the audience frame.
                    summary["logo_toggle_after_restore_press"] = focused_toggle(
                        hwnd, "mask-logo"
                    )
                    width, height, pixels = nw.capture_client(audience_hwnd)
                    nw.write_png(
                        args.out / "fail-logo-off-audience.png", width, height, pixels
                    )
                check.expect(restored, "Space on Logo again restores the slide")

            if set(REQUIRED_STOPS) <= probed and len(acted) >= 6:
                break
            press("tab")
            time.sleep(0.35)
        missing = [aid for aid in REQUIRED_STOPS if aid not in probed]
        check.expect(
            not missing, f"Tab reached every critical stop (missing: {missing})"
        )
        check.expect(
            len(acted) >= 6,
            f"Enter/Space activated Go Live, masks and previous/next (acted: {sorted(acted)})",
        )
        summary["tab_order"] = sequence
        # Shift+Tab walks back to the stop visited before the current one.
        press("shift+tab")
        time.sleep(0.5)
        node = uia(hwnd, focused=True)["nodes"][0]
        check.expect(
            node["automationId"] == sequence[-2],
            f"Shift+Tab steps back to {sequence[-2]!r} (at {node['automationId']!r})",
        )

        # --- VAL-A11Y-003: names follow the UI language ------------------
        rows = len(monitors(args.binary)) + 1
        press("alt+e")
        time.sleep(0.3)
        press("enter")
        time.sleep(0.8)
        # Focus starts on Cancel; rows are OK, monitors from the last, then
        # the language rows (Indonesian, English) walking backwards.
        for _ in range(2 + rows):
            press("shift+tab")
            time.sleep(0.1)
        press("space")
        time.sleep(0.3)
        for _ in range(rows + 1):
            press("tab")
            time.sleep(0.1)
        press("enter")
        time.sleep(0.8)
        doc = uia(hwnd, args.out / "06-dump-indonesian.json")
        save("04-indonesian")
        check.expect_controls(doc, 3, "id")
        found = nodes_by_id(doc)
        check.expect(
            found["live-output"]["name"] == "Langsung ● Aktif",
            f"id: live output is {found['live-output']['name']!r}",
        )
        # And back to English without a restart.
        press("alt+e")
        time.sleep(0.3)
        press("enter")
        time.sleep(0.8)
        for _ in range(2 + rows + 1):
            press("shift+tab")
            time.sleep(0.1)
        press("space")
        time.sleep(0.3)
        for _ in range(rows + 2):
            press("tab")
            time.sleep(0.1)
        press("enter")
        time.sleep(0.8)
        doc = uia(hwnd, args.out / "07-dump-english-again.json")
        check.expect_controls(doc, 2, "en (restored)")

        press("ctrl+q")
        time.sleep(0.6)
        if app.process.poll() is None:
            # Quit is guarded while the schedule is dirty: confirm the dialog.
            found = nodes_by_id(uia(hwnd))
            if "dialog-confirm" in found:
                click_node(hwnd, found["dialog-confirm"])
        app.wait_exit()
        hwnd = 0
        summary["status"] = "FAIL" if summary["failures"] else "PASS"
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
