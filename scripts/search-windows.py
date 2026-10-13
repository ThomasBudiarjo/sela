"""Native check: the Songs search box on the real Windows desktop (M1-07b).

Covers VAL-SEARCH-001..008 and 010 through UI Automation and screen
captures of the real operator:

- one character shows the whole list, two or more filter by title and by
  lyric words, an empty result shows the "No songs found" state, the clear
  button and Esc restore the list;
- a result survives while it still matches, and drops out of the selection
  without replacing Preview;
- with a song live, typing B, C, L (and shifted forms) in the box neither
  lights a mask nor changes the audience, and Page Down / Right do not
  change Live;
- `--latency-library PATH` (a 20k-song library made by
  `cargo run --release --example search_bench -- --keep PATH`) measures
  keystroke-to-painted-list latency of two-character queries.

Takes over the real keyboard and mouse; never run with EasyWorship open.

Run: python scripts/search-windows.py [--out DIR] [--latency-library PATH]
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import re
import statistics
import subprocess
import sys
import tempfile
import time
import zlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import native_win as nw


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lo = _load("live_output_windows", "live-output-windows.py")
a11y = _load("a11y_windows", "a11y-windows.py")

ROW_ID = re.compile(r"^song-\d+$")
# Two-letter prefixes of the synthetic corpus vocabulary, plus common ones.
LATENCY_QUERIES = [
    "ma", "ka", "sa", "la", "ra", "na", "ta", "ba", "da", "ha",
    "ke", "se", "me", "te", "be", "ni", "li", "ri", "si", "mi",
    "ko", "so", "mo", "to", "bo", "ku", "su", "mu", "tu", "lu",
]  # fmt: skip


def rows(doc: dict) -> list[str]:
    return sorted(
        n["name"]
        for n in doc["nodes"]
        if ROW_ID.match(n["automationId"]) and n["controlType"] == "ListItem"
    )


def song_list_rect(hwnd: int) -> tuple[int, int, int, int]:
    found = a11y.nodes_by_id(a11y.uia(hwnd))
    node = found["song-list"]
    ox, oy = a11y.client_origin(hwnd)
    # Rows start at the top of the list; a 400x200 patch holds six of them.
    return (node["x"] - ox, node["y"] - oy, min(400, node["width"]), min(200, node["height"]))


def region_digest(hwnd: int, rect: tuple[int, int, int, int]) -> int:
    """CRC of a small client-area rectangle; a cheap BitBlt (about 1 ms) so the
    polling loop adds little to the latency it measures."""
    x, y, width, height = rect
    origin = nw.w.POINT(0, 0)
    nw.user32.ClientToScreen(hwnd, nw.ctypes.byref(origin))
    screen = nw.user32.GetDC(None)
    memory = nw.gdi32.CreateCompatibleDC(screen)
    bitmap = nw.gdi32.CreateCompatibleBitmap(screen, width, height)
    previous = nw.gdi32.SelectObject(memory, bitmap)
    try:
        nw.gdi32.BitBlt(
            memory, 0, 0, width, height, screen, origin.x + x, origin.y + y,
            0x00CC0020 | 0x40000000,
        )  # fmt: skip
        header = nw.BITMAPINFOHEADER(
            biSize=nw.ctypes.sizeof(nw.BITMAPINFOHEADER),
            biWidth=width,
            biHeight=-height,
            biPlanes=1,
            biBitCount=32,
            biCompression=0,
        )
        pixels = nw.ctypes.create_string_buffer(width * height * 4)
        info = nw.ctypes.create_string_buffer(bytes(header) + b"\0" * 16)
        nw.gdi32.GetDIBits(memory, bitmap, 0, height, pixels, info, 0)
        return zlib.crc32(pixels.raw)
    finally:
        nw.gdi32.SelectObject(memory, previous)
        nw.gdi32.DeleteObject(bitmap)
        nw.gdi32.DeleteDC(memory)
        nw.user32.ReleaseDC(None, screen)

def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, round(fraction * (len(ordered) - 1)))]


def measure_latency(hwnd: int, queries: list[str], summary: dict, box: dict) -> None:
    rect = song_list_rect(hwnd)
    a11y.click_node(hwnd, box)
    samples: list[dict] = []
    for query in queries:
        nw.press(hwnd, "ctrl+a")
        nw.press(hwnd, "backspace")
        time.sleep(0.6)
        nw.press(hwnd, query[0])
        time.sleep(0.4)
        before = region_digest(hwnd, rect)
        started = time.perf_counter()
        nw.press(hwnd, query[1])
        deadline = started + 3.0
        elapsed = None
        polls = 0
        while time.perf_counter() < deadline:
            polls += 1
            if region_digest(hwnd, rect) != before:
                elapsed = (time.perf_counter() - started) * 1000
                break
        samples.append({"query": query, "ms": elapsed, "polls": polls})
        time.sleep(0.2)
    values = [s["ms"] for s in samples if s["ms"] is not None]
    summary["latency"] = {
        "queries": len(samples),
        "painted": len(values),
        "p50_ms": statistics.median(values) if values else None,
        "p95_ms": percentile(values, 0.95) if values else None,
        "max_ms": max(values) if values else None,
        "samples": samples,
        "method": (
            "SendInput of the second character to the first captured frame whose "
            "song-list region differs (BitBlt polling; includes capture cost)"
        ),
    }


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
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/search-windows"
    )
    parser.add_argument("--monitor", default="secondary")
    parser.add_argument("--latency-library", type=pathlib.Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for path in (args.binary, args.seed):
        if not path.is_file():
            print(f"FAIL: missing {path}")
            return 1
    summary: dict = {"status": "FAIL", "binary": str(args.binary.resolve())}
    check = a11y.Check(summary)
    data = pathlib.Path(tempfile.mkdtemp(prefix="sela-search-"))
    library = data / "library.sqlite"
    if args.latency_library:
        library = args.latency_library
        scale_run = True
    else:
        scale_run = False
        subprocess.run([str(args.seed), str(library), "--second-song"], check=True)
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    app = nw.App(args.binary, ["--operator-library", str(library)])
    hwnd = 0
    audience_hwnd = 0
    try:
        hwnd = app.window()
        nw.activate(hwnd)
        time.sleep(3.0 if scale_run else 1.2)

        def save(name: str) -> None:
            width, height, pixels = nw.capture_client(hwnd)
            nw.write_png(args.out / f"{name}.png", width, height, pixels)

        def press(keys: str) -> None:
            try:
                nw.press(hwnd, keys)
            except nw.Failure:
                nw.activate(hwnd)
                summary["focus_recoveries"] = summary.get("focus_recoveries", 0) + 1
                nw.press(hwnd, keys)
            time.sleep(0.12)

        def wait_rows(expected: list[str], what: str, timeout: float = 8.0) -> dict:
            deadline = time.monotonic() + timeout
            doc = a11y.uia(hwnd)
            while time.monotonic() < deadline:
                if rows(doc) == sorted(expected):
                    check.expect(True, what)
                    return doc
                doc = a11y.uia(hwnd)
            check.expect(False, f"{what} (got {rows(doc)})")
            return doc

        doc = a11y.uia(hwnd, args.out / "00-dump-initial.json")
        found = a11y.nodes_by_id(doc)
        box = found.get("song-search")
        if box is None:
            raise nw.Failure("no song-search node in the UIA tree")
        check.expect(
            box["controlType"] == "Edit" and box["name"] == "Search songs",
            f"search box is an Edit named {box['name']!r}",
        )
        if scale_run:
            measure_latency(hwnd, LATENCY_QUERIES, summary, box)
            save("latency-end")
            summary["status"] = "FAIL" if check.failures else "PASS"
            return 0 if summary["status"] == "PASS" else 1

        both = ["Quiet Canticle", "Signal Hymn"]
        check.expect(rows(doc) == both, f"unfiltered list {rows(doc)}")
        a11y.click_node(hwnd, box)
        # VAL-SEARCH-003: one character leaves the list unfiltered.
        press("s")
        time.sleep(0.8)
        check.expect(rows(a11y.uia(hwnd)) == both, "one character: list unfiltered")
        save("01-one-character")
        # VAL-SEARCH-001: two characters filter.
        press("i")
        wait_rows(["Signal Hymn"], "'si' filters to Signal Hymn")
        save("02-filtered")
        # VAL-SEARCH-002: lyric words.
        press("ctrl+a")
        for letter in "refrain":
            press(letter)
        wait_rows(["Signal Hymn"], "lyric word 'refrain' finds Signal Hymn")
        press("ctrl+a")
        for letter in "orig":
            press(letter)
        wait_rows(both, "'orig' (both lyrics) lists both songs")
        # VAL-SEARCH-006: no results.
        press("ctrl+a")
        for letter in "zzqx":
            press(letter)
        wait_rows([], "'zzqx' shows an empty list")
        save("03-no-results")
        # Clear button and Esc.
        found = a11y.nodes_by_id(a11y.uia(hwnd))
        clear = found.get("song-search-clear")
        check.expect(clear is not None, "clear button is exposed while the box has text")
        if clear:
            a11y.click_node(hwnd, clear)
        wait_rows(both, "clear button restores the full list")
        for letter in "quiet":
            press(letter)
        wait_rows(["Quiet Canticle"], "'quiet' finds Quiet Canticle")
        press("escape")
        wait_rows(both, "Esc restores the full list")

        # VAL-SEARCH-007: selection after search.
        for letter in "sig":
            press(letter)
        wait_rows(["Signal Hymn"], "'sig' filters before selecting")
        doc = a11y.uia(hwnd)
        found = a11y.nodes_by_id(doc)
        row = next(
            n
            for n in doc["nodes"]
            if ROW_ID.match(n["automationId"]) and n["name"] == "Signal Hymn"
        )
        a11y.click_node(hwnd, row)
        time.sleep(1.0)
        doc = a11y.uia(hwnd)
        found = a11y.nodes_by_id(doc)
        check.expect(
            found[row["automationId"]].get("selected") is True
            and "preview-slide-0" in found,
            "clicking a result selects it and previews it",
        )
        save("04-result-selected")
        a11y.click_node(hwnd, box)
        press("ctrl+a")
        for letter in "quiet":
            press(letter)
        doc = wait_rows(["Quiet Canticle"], "'quiet' replaces the results")
        found = a11y.nodes_by_id(doc)
        check.expect(
            found.get("song-0", {}).get("selected") is not True
            and "preview-slide-0" in found,
            "the dropped selection is cleared and Preview keeps its content",
        )
        save("05-selection-dropped")
        a11y.click_node(hwnd, found["song-search"])
        press("ctrl+a")
        press("backspace")
        wait_rows(both, "emptying the box restores the full list")

        # VAL-SEARCH-004: typing never fires Black/Clear/Logo or Go Live.
        doc = a11y.uia(hwnd)
        hymn = next(
            n
            for n in doc["nodes"]
            if ROW_ID.match(n["automationId"]) and n["name"] == "Signal Hymn"
        )
        a11y.click_node(hwnd, hymn)
        time.sleep(0.8)
        a11y.show_live_output(hwnd)
        seen = lo.audience(app.process.pid, 15)
        if not seen:
            raise nw.Failure("no audience window after Live output on")
        _, audience_hwnd = seen
        time.sleep(1.0)
        nw.activate(hwnd)
        found = a11y.nodes_by_id(a11y.uia(hwnd))
        a11y.click_node(hwnd, found["go-live"])
        live = a11y.wait_for(5.0, lambda: a11y.audience_ink(audience_hwnd))
        check.expect(live, "a song is live before typing in the box")
        before = a11y.stable_digest(audience_hwnd)
        a11y.click_node(hwnd, found["song-search"])
        for chord in ("b", "c", "l", "shift+b", "shift+c", "shift+l", "pagedown", "right"):
            press(chord)
        time.sleep(1.0)
        found = a11y.nodes_by_id(a11y.uia(hwnd))
        toggles = {
            aid: found[aid].get("toggle")
            for aid in ("mask-black", "mask-clear", "mask-logo")
        }
        check.expect(
            all(state == "Off" for state in toggles.values()),
            f"no mask lit after typing b/c/l: {toggles}",
        )
        check.expect(
            lo.digest(audience_hwnd) == before,
            "the audience frame is unchanged after typing and Page Down/Right",
        )
        save("06-typed-while-live")

        press("ctrl+q")
        time.sleep(0.6)
        if app.process.poll() is None:
            found = a11y.nodes_by_id(a11y.uia(hwnd))
            if "dialog-confirm" in found:
                a11y.click_node(hwnd, found["dialog-confirm"])
        app.wait_exit()
        hwnd = 0
        summary["status"] = "FAIL" if check.failures else "PASS"
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
