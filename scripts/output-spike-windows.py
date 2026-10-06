"""Windows supervisor for the M0-04 audience spike (Win32 counterpart of output-spike.py).

Usage: python scripts/output-spike-windows.py [artifact-directory]

Requires a built `target/debug/examples/output_spike.exe`, at least two monitors,
and exclusive keyboard focus while it runs. The audience runs as an independent
DX12 process on a non-primary monitor; the GPUI operator stalls, prepares, exits
and is killed while audience presentation timestamps are recorded. Additional
phases move the audience between monitors with different scale factors, run it
borderless fullscreen, and compare low/high power adapter preference per monitor.
All launched processes are reaped; only owned window client areas are captured.
"""

from __future__ import annotations

import itertools
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_win as nw

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug" / "examples" / "output_spike.exe"
SWP_NOSIZE = 0x0001
FREEZE_MS = 500  # Diagnostic freeze detector, not a presentation SLO.
AUDIENCE = "Sela audience spike"


def now_us() -> int:
    return time.time_ns() // 1000


class Session:
    def __init__(self, out: Path):
        self.out = out
        out.mkdir(parents=True, exist_ok=True)
        self.children: list[subprocess.Popen] = []

    def launch(self, name: str, *args: str, **env: object) -> subprocess.Popen:
        environment = dict(os.environ, SELA_SPIKE_BACKEND="dx12")
        environment.update({key: str(value) for key, value in env.items()})
        with open(self.out / f"{name}.log", "w") as log:
            process = subprocess.Popen([str(BINARY), *args], env=environment, stdout=log,
                                       stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
        self.children.append(process)
        return process

    def close(self) -> None:
        for process in self.children:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(3)


def window(process: subprocess.Popen, title: str | None = None, timeout: float = 15) -> int:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise nw.Failure(f"process {process.pid} exited before window discovery")
        found = [h for h in nw.top_windows(process.pid) if title is None or nw.title(h) == title]
        if found:
            return found[0]
        time.sleep(0.1)
    raise nw.Failure(f"window missing for process {process.pid}")


def capture(out: Path, name: str, windows: dict[str, int]) -> None:
    for role, hwnd in windows.items():
        if nw.user32.IsWindow(hwnd):
            width, height, pixels = nw.capture_client(hwnd)
            nw.write_png(out / f"{name}-{role}.png", width, height, pixels)


def rows(path: Path) -> list[list[str]]:
    return [line.split() for line in path.read_text(errors="replace").splitlines()]


def frames(path: Path) -> list[tuple[int, int]]:
    return [(int(p[1]), int(p[4])) for p in rows(path) if len(p) >= 5 and p[2] == "present_call"]


def monitors(path: Path) -> list[dict]:
    found = []
    for p in rows(path):
        if len(p) >= 9 and p[2] == "monitor":
            match = re.fullmatch(r"(\d+)x(\d+)\+(-?\d+)\+(-?\d+)", p[4])
            width, height, x, y = map(int, match.groups())
            found.append({"index": int(p[3]), "width": width, "height": height, "x": x, "y": y,
                          "scale": float(p[6]), "refresh_mhz": int(p[8]), "name": " ".join(p[9:])})
    return found


def adapter(path: Path) -> str:
    match = re.search(r'adapter AdapterInfo \{ name: "([^"]*)"', path.read_text(errors="replace"))
    return match.group(1) if match else "unknown"


def surface(path: Path) -> str:
    return next((" ".join(p[3:]) for p in rows(path) if len(p) > 3 and p[2] == "surface"), "unknown")


def gap(frame_list: list[tuple[int, int]], start: int, end: int) -> tuple[float, int]:
    inside = [t for t, _ in frame_list if start <= t <= end]
    edges = [start, *inside, end]
    return max(b - a for a, b in itertools.pairwise(edges)) / 1000, len(inside)


def distribution(frame_list: list[tuple[int, int]]) -> dict:
    gaps = sorted((b[1] - a[1]) / 1000 for a, b in itertools.pairwise(frame_list))
    return {"frames": len(frame_list), "gap_p50_ms": gaps[len(gaps) // 2],
            "gap_p95_ms": gaps[int(len(gaps) * 0.95)], "gap_p99_ms": gaps[int(len(gaps) * 0.99)],
            "gap_max_ms": gaps[-1]}


def wait_exit(process: subprocess.Popen, timeout: float = 10) -> int:
    try:
        return process.wait(timeout)
    except subprocess.TimeoutExpired as error:
        raise nw.Failure(f"process {process.pid} did not exit") from error


def probe(out: Path) -> list[dict]:
    session = Session(out)
    try:
        process = session.launch("probe", "--audience", SELA_SPIKE_SECONDS=2)
        if wait_exit(process, 20) != 0:
            raise nw.Failure("probe audience failed")
    finally:
        session.close()
    found = monitors(out / "probe.log")
    if len(found) < 2:
        raise nw.Failure(f"need two monitors, found {len(found)}")
    return found


def main_phase(out: Path, audience_monitor: dict, move_monitor: dict) -> dict:
    session = Session(out)
    try:
        audience = session.launch("audience", "--audience", SELA_SPIKE_MONITOR=audience_monitor["index"],
                                  SELA_SPIKE_SECONDS=90)
        aw = window(audience, AUDIENCE)
        operator = session.launch("operator")
        ow = window(operator)
        nw.activate(ow)
        time.sleep(1)
        capture(out, "baseline", {"operator": ow, "audience": aw})
        nw.chord(ow, "ctrl+p")
        nw.chord(ow, "ctrl+p")  # Must reject, not enqueue a second preparation.
        time.sleep(0.2)
        capture(out, "preparing", {"operator": ow})
        nw.chord(ow, "ctrl+1")  # Handled while preparation is pending.
        time.sleep(2.2)
        capture(out, "prepared", {"operator": ow})
        for stroke, delay in [("ctrl+1", 0.4), ("ctrl+2", 0.8), ("ctrl+3", 2.4)]:
            nw.chord(ow, stroke)
            if stroke == "ctrl+3":
                time.sleep(0.25)
                capture(out, "stall-a", {"audience": aw})
                time.sleep(0.5)
                capture(out, "stall-b", {"audience": aw})
                nw.chord(ow, "ctrl+1")  # Queued behind the stall.
            time.sleep(delay)

        moves = []
        for target in (move_monitor, audience_monitor):
            moved_at = now_us()
            nw.user32.SetWindowPos(aw, None, target["x"] + 100, target["y"] + 100, 0, 0,
                                   SWP_NOSIZE | nw.SWP_NOZORDER | nw.SWP_NOACTIVATE)
            time.sleep(1.5)
            capture(out, f"moved-{target['index']}", {"audience": aw})
            moves.append({"monitor": target["index"], "at_us": moved_at, "dpi_after": nw.dpi(aw),
                          "client_after": nw.client_size(aw)})
        resized = nw.resize_client(aw, 600, 320)
        time.sleep(0.5)

        nw.chord(ow, "ctrl+q")
        if wait_exit(operator, 5) != 0:
            raise nw.Failure("operator did not exit cleanly")
        exit_us = now_us()
        time.sleep(1)
        capture(out, "after-exit-a", {"audience": aw})
        time.sleep(0.5)
        capture(out, "after-exit-b", {"audience": aw})
        killed = session.launch("operator-killed")
        window(killed)
        time.sleep(0.5)
        killed.kill()
        killed.wait(5)
        killed_us = now_us()
        time.sleep(1)
        capture(out, "after-kill", {"audience": aw})
        nw.chord(aw, "alt+f4")
        if wait_exit(audience, 5) != 0:
            raise nw.Failure("audience did not exit cleanly")
    finally:
        session.close()

    frame_list = frames(out / "audience.log")
    operator_rows = rows(out / "operator.log")
    intervals, begin = [], None
    for p in operator_rows:
        if len(p) >= 4 and p[2] == "stall_begin":
            begin = (int(p[1]), int(p[3]))
        if len(p) >= 4 and p[2] == "stall_end":
            end = int(p[1])
            assert begin is not None and int(p[3]) == begin[1]
            assert end - begin[0] >= begin[1] * 1000
            worst, count = gap(frame_list, begin[0], end)
            assert count >= 2, (begin, end)
            if begin[1] == 2000:
                assert worst < FREEZE_MS, f"audience froze during UI stall: {worst}ms"
            intervals.append({"start_us": begin[0], "end_us": end, "requested_ms": begin[1],
                              "present_calls": count, "max_gap_ms": worst})
            begin = None
    assert [i["requested_ms"] for i in intervals] == [100, 100, 500, 2000, 100], intervals
    preparation = {p[2]: int(p[1]) for p in operator_rows if len(p) == 3 and p[2].startswith("preparation_")}
    assert set(preparation) == {"preparation_begin", "preparation_end", "preparation_busy", "preparation_observed"}
    start, end = preparation["preparation_begin"], preparation["preparation_end"]
    assert end - start >= 2_000_000
    assert start <= intervals[0]["start_us"] < intervals[0]["end_us"] < end
    assert preparation["preparation_busy"] < end <= preparation["preparation_observed"]
    prep_gap, prep_count = gap(frame_list, start, end)
    assert prep_count > 12 and prep_gap < FREEZE_MS, (prep_gap, prep_count)
    for move in moves:
        move["max_gap_ms"], move["present_calls"] = gap(frame_list, move["at_us"], move["at_us"] + 1_500_000)
        assert move["max_gap_ms"] < FREEZE_MS, f"audience froze during monitor move: {move}"
    audience_rows = rows(out / "audience.log")
    scale_events = [float(p[3]) for p in audience_rows if len(p) >= 4 and p[2] == "scale_factor"]
    current = [" ".join(p[3:]) for p in audience_rows if len(p) > 3 and p[2] == "monitor_current"]
    if move_monitor["scale"] != audience_monitor["scale"]:
        assert len(scale_events) >= 2, f"expected scale changes, got {scale_events}"
    after_exit = sum(t > exit_us for t, _ in frame_list)
    after_kill = sum(t > killed_us for t, _ in frame_list)
    assert after_exit > 10 and after_kill > 10
    return {"adapter": adapter(out / "audience.log"), "surface": surface(out / "audience.log"),
            "audience_monitor": audience_monitor["index"], "move_monitor": move_monitor["index"],
            "stalls": intervals, "preparation": preparation, "preparation_max_gap_ms": prep_gap,
            "moves": moves, "scale_factor_events": scale_events, "monitor_current": current,
            "resized_client": resized, "operator_exit_observed_us": exit_us,
            "operator_kill_observed_us": killed_us, "calls_after_exit": after_exit,
            "calls_after_kill": after_kill, **distribution(frame_list)}


def fullscreen_phase(out: Path, monitor: dict) -> dict:
    session = Session(out)
    try:
        audience = session.launch("fullscreen-audience", "--audience", SELA_SPIKE_MONITOR=monitor["index"],
                                  SELA_SPIKE_FULLSCREEN=1, SELA_SPIKE_SECONDS=30)
        aw = window(audience, AUDIENCE)
        time.sleep(1)
        operator = session.launch("fullscreen-operator")
        ow = window(operator)
        nw.activate(ow)
        time.sleep(1)
        nw.chord(ow, "ctrl+3")
        time.sleep(0.5)
        capture(out, "fullscreen-stall", {"audience": aw})
        time.sleep(2.2)
        nw.chord(ow, "ctrl+q")
        if wait_exit(operator, 5) != 0:
            raise nw.Failure("fullscreen operator did not exit cleanly")
        time.sleep(1)
        client = nw.client_size(aw)
        nw.chord(aw, "alt+f4")
        if wait_exit(audience, 5) != 0:
            raise nw.Failure("fullscreen audience did not exit cleanly")
    finally:
        session.close()
    frame_list = frames(out / "fullscreen-audience.log")
    stall = [p for p in rows(out / "fullscreen-operator.log") if len(p) >= 4 and p[2].startswith("stall_")]
    assert [p[2] for p in stall] == ["stall_begin", "stall_end"], stall
    worst, count = gap(frame_list, int(stall[0][1]), int(stall[1][1]))
    assert worst < FREEZE_MS, f"fullscreen audience froze during UI stall: {worst}ms"
    expected = (monitor["width"], monitor["height"])
    assert tuple(client) == expected, f"fullscreen client {client} != monitor {expected}"
    return {"monitor": monitor["index"], "client": client, "adapter": adapter(out / "fullscreen-audience.log"),
            "surface": surface(out / "fullscreen-audience.log"), "stall_max_gap_ms": worst,
            "stall_present_calls": count, **distribution(frame_list)}


def adapter_matrix(out: Path, monitor_list: list[dict]) -> list[dict]:
    results = []
    for monitor in monitor_list:
        for power in ("low", "high"):
            name = f"matrix-m{monitor['index']}-{power}"
            session = Session(out)
            try:
                process = session.launch(name, "--audience", SELA_SPIKE_MONITOR=monitor["index"],
                                         WGPU_POWER_PREF=power, SELA_SPIKE_SECONDS=6)
                window(process, AUDIENCE)
                if wait_exit(process, 20) != 0:
                    raise nw.Failure(f"{name} failed")
            finally:
                session.close()
            frame_list = frames(out / f"{name}.log")
            # Skip the first second: window creation and first presents are not steady state.
            steady = [f for f in frame_list if f[1] >= 1_000_000]
            results.append({"monitor": monitor["index"], "power": power, "adapter": adapter(out / f"{name}.log"),
                            "surface": surface(out / f"{name}.log"), **distribution(steady)})
    return results


def main() -> int:
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / ".amp" / "in" / "artifacts" / "output-spike-windows"
    if not BINARY.is_file():
        print(f"FAIL: build the output_spike example first: {BINARY}")
        return 1
    try:
        found = probe(out)
        secondary = [m for m in found if (m["x"], m["y"]) != (0, 0)]
        audience_monitor = secondary[0]
        others = [m for m in found if m["index"] != audience_monitor["index"]]
        move_monitor = next((m for m in others if m["scale"] != audience_monitor["scale"]), others[0])
        summary = {"monitors": found, "binary": str(BINARY)}
        summary["main"] = main_phase(out, audience_monitor, move_monitor)
        print("PASS: independent DX12 audience under UI stalls, preparation, monitor moves, exit and kill")
        summary["fullscreen"] = fullscreen_phase(out, audience_monitor)
        print("PASS: borderless fullscreen audience under 2s UI stall")
        summary["adapter_matrix"] = adapter_matrix(out, found)
        print("PASS: adapter preference matrix")
    except (nw.Failure, AssertionError) as error:
        print(f"FAIL: {error!r}")
        return 1
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
