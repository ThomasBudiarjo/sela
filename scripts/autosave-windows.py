"""Windows native check: schedule autosave, crash recovery and the
single-instance lock (M1-12, scenario S03). Windows counterpart of
scripts/autosave-x11.py; NOT RUN yet (no Windows host when it was written).

Usage: python scripts/autosave-windows.py [--binary sela.exe] [--seed seed_library.exe]
       [--monitor secondary] [--out DIR]

Seeds a library with two original fixture songs into a private profile
folder and drives the real operator, locating controls by UI Automation id
(scripts/uia-names-windows.ps1 through a11y-windows.py's helpers):

1. Live output on; the first song is added to the Schedule and sent live.
   Within the autosave cadence (2 s quiet) one revision file appears in
   `<profile>\\Recovery\\`; its JSON line and SHA-256 trailer are checked and
   it pins exactly that song.
2. A second `sela.exe` on the same profile shows "Sela is already running"
   naming the profile, changes no profile file, and exits 0 on Enter.
3. The second song is added; as soon as `session.json` holds the uncommitted
   marker, the operator is ended with TerminateProcess (before the 2 s quiet
   period could commit it). The audience process survives and is then ended.
4. Relaunch: the recovery prompt says Sela did not close normally. The
   window is closed (WM_CLOSE) without answering: exit 0, nothing
   committed, `session.json` records `offered` with nothing acknowledged.
5. Relaunch: the prompt now says Sela was closed before the question was
   answered, the last autosave holds one item and the newer two-item edit
   cannot be recovered. Enter (Restore is the default) restores exactly one
   item, unsaved ("Untitled schedule* — Sela"); the restore is committed
   again and the previous run acknowledged.
6. Ctrl+Q asks to save; Don't Save quits with exit status 0 and
   `session.json` says `clean`.
7. A further launch shows no recovery prompt and quits at once with Ctrl+Q.

Also checks the Windows-specific parts the Linux run cannot: LockFileEx as
the profile lock and TerminateProcess as the kill. Takes over the real
keyboard and mouse; never run with EasyWorship open.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import native_win as nw

WM_CLOSE = 0x0010

ROOT = pathlib.Path(__file__).resolve().parents[1]


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lo = _load("live_output_windows", "live-output-windows.py")
a11y = _load("a11y_windows", "a11y-windows.py")

ALREADY_RUNNING_TITLE = "Sela is already running"


def keys(hwnd: int, *sequence: str) -> None:
    for key in sequence:
        nw.press(hwnd, key)
        time.sleep(0.12)


def wait_for(what: str, timeout: float, probe, interval: float = 0.05):
    deadline = time.monotonic() + timeout
    while True:
        value = probe()
        if value:
            return value
        if time.monotonic() >= deadline:
            raise nw.Failure(f"timed out after {timeout}s waiting for {what}")
        time.sleep(interval)


def revisions(profile: pathlib.Path) -> list[pathlib.Path]:
    return sorted((profile / "Recovery").glob("autosave-*.selarecovery"))


def read_revision(path: pathlib.Path) -> dict:
    data = path.read_bytes()
    parts = data.split(b"\n", 2)
    if len(parts) != 3:
        raise nw.Failure(f"{path.name} is not a complete checksummed revision")
    body, trailer, rest = parts
    if rest != b"" or trailer != b"sha256 " + hashlib.sha256(body).hexdigest().encode():
        raise nw.Failure(f"{path.name} is not a complete checksummed revision")
    revision = json.loads(body)
    if revision.get("format") != "sela-recovery" or revision.get("version") != 1:
        raise nw.Failure(f"unexpected revision header in {path.name}")
    return revision


def session_file(profile: pathlib.Path) -> dict | None:
    try:
        return json.loads((profile / "Recovery" / "session.json").read_text("utf-8"))
    except (OSError, ValueError):
        return None


def lock_state(path: pathlib.Path) -> tuple[int, int] | None:
    """Size and modification time of the profile lock file. Its content
    cannot be read while LockFileEx holds it, but its metadata can."""
    try:
        stat = path.stat()
    except OSError:
        return None
    return stat.st_size, stat.st_mtime_ns


def digest(path: pathlib.Path) -> str | None:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError:
        return None


def song_ids(library: pathlib.Path) -> list[bytes]:
    """Library rows in operator catalog order (title, ignoring case)."""
    import sqlite3

    with sqlite3.connect(library.as_uri() + "?mode=ro", uri=True) as db:
        return [
            row[0]
            for row in db.execute(
                "SELECT m.song FROM search_rows m JOIN song_search f ON f.rowid=m.row "
                "ORDER BY f.title COLLATE NOCASE"
            )
        ]


def nodes(hwnd: int) -> dict[str, dict]:
    return a11y.nodes_by_id(a11y.uia(hwnd))


def click(hwnd: int, aid: str, found: dict[str, dict] | None = None) -> None:
    found = found or nodes(hwnd)
    if aid not in found:
        raise nw.Failure(f"control {aid!r} missing from the UIA tree")
    a11y.click_node(hwnd, found[aid])


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
        "--out", type=pathlib.Path, default=ROOT / ".amp/in/artifacts/autosave-windows"
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
    profile = pathlib.Path(tempfile.mkdtemp(prefix="sela-autosave-"))
    library = profile / "library.sqlite"
    os.environ["SELA_AUDIENCE_MONITOR"] = args.monitor
    spawned: set[int] = set()
    apps: list[nw.App] = []

    def launch() -> nw.App:
        app = nw.App(args.binary, ["--operator-library", str(library)])
        apps.append(app)
        return app

    def save(hwnd: int, name: str) -> None:
        width, height, pixels = nw.capture_client(hwnd)
        nw.write_png(args.out / f"{name}.png", width, height, pixels)

    try:
        subprocess.run([str(args.seed), str(library), "--second-song"], check=True)
        first_id, second_id = song_ids(library)
        # 1. A committed revision while Live output is on.
        app = launch()
        operator = app.window()
        nw.activate(operator)
        time.sleep(1.0)
        found = nodes(operator)
        click(operator, "live-output", found)
        audience = lo.audience(app.process.pid, 15)
        if not audience:
            raise nw.Failure(f"no audience window within 15s: {app.log()}")
        audience_pid, screen = audience
        spawned.add(audience_pid)
        nw.activate(operator)
        time.sleep(1.0)
        blank = lo.digest(screen)
        click(operator, "song-0", found)
        click(operator, "add-to-schedule", found)
        wait_for(
            "the first autosave revision", 10, lambda: len(revisions(profile)) == 1
        )
        first = read_revision(revisions(profile)[0])
        pinned = [bytes.fromhex(i["song_id"]) for i in first["items"]]
        summary["first_revision"] = {
            "sequence": first["sequence"],
            "items": len(pinned),
        }
        if pinned != [first_id] or first["path"] is not None:
            raise nw.Failure(f"unexpected first revision {first}")
        click(operator, "schedule-item-0")
        click(operator, "go-live")
        wait_for("the audience to change", 5, lambda: lo.digest(screen) != blank)
        save(operator, "first-run-autosaved")

        # 2. A second instance refuses and changes nothing.
        watched = [
            profile / "Recovery" / "session.json",
            revisions(profile)[0],
            library,
        ]
        lock = profile / "sela.lock"
        before = {str(p): digest(p) for p in watched}
        before["lock"] = lock_state(lock)
        if before["lock"] is None:
            raise nw.Failure("the first operator holds no sela.lock")
        second = launch()
        notice = second.window()
        if nw.title(notice) != ALREADY_RUNNING_TITLE:
            raise nw.Failure(f"second instance window is {nw.title(notice)!r}")
        message = nodes(notice).get("already-running-message", {}).get("name", "")
        summary["second_instance_message"] = message
        if str(profile) not in message or "already using this profile" not in message:
            raise nw.Failure(f"unexpected second-instance message {message!r}")
        save(notice, "second-instance")
        nw.activate(notice)
        keys(notice, "enter")
        summary["second_instance_exit"] = second.wait_exit()
        if summary["second_instance_exit"] != 0:
            raise nw.Failure(
                f"second instance exit status {summary['second_instance_exit']}"
            )
        after = {str(p): digest(p) for p in watched}
        after["lock"] = lock_state(lock)
        if after != before:
            raise nw.Failure("the second instance changed profile files")

        # 3. A newer edit, then TerminateProcess before it is committed.
        nw.activate(operator)
        found = nodes(operator)
        click(operator, "song-1", found)
        click(operator, "add-to-schedule", found)
        marker = wait_for(
            "the uncommitted marker",
            1.5,
            lambda: (session_file(profile) or {}).get("uncommitted"),
            interval=0.02,
        )
        app.process.kill()  # TerminateProcess
        app.process.wait(5)
        summary["marker"] = marker
        if marker.get("items") != 2 or marker.get("after") != 1:
            raise nw.Failure(f"unexpected uncommitted marker {marker}")
        if len(revisions(profile)) != 1:
            raise nw.Failure(
                "the newer edit was committed before the kill; timing invalid"
            )
        if not lo.alive(audience_pid):
            raise nw.Failure("the audience died with the operator")
        lo.kill(audience_pid)

        # 4. Relaunch, then close with the prompt unanswered.
        app = launch()
        operator = app.window()
        nw.activate(operator)
        body = wait_for(
            "the recovery prompt", 15, lambda: nodes(operator).get("recovery-body"), 0.5
        )
        summary["unanswered_text"] = body.get("name", "")
        if not summary["unanswered_text"].startswith(
            "Sela did not close normally last time."
        ):
            raise nw.Failure(f"unexpected first prompt {summary['unanswered_text']!r}")
        save(operator, "recovery-prompt-first")
        nw.user32.PostMessageW(operator, WM_CLOSE, 0, 0)
        summary["unanswered_exit"] = app.wait_exit()
        if summary["unanswered_exit"] != 0:
            raise nw.Failure(f"unanswered close exit {summary['unanswered_exit']}")
        recorded = session_file(profile) or {}
        summary["unanswered_session"] = recorded
        if (
            recorded.get("state") != "offered"
            or recorded.get("acknowledged") != 0
            or not recorded.get("uncommitted")
        ):
            raise nw.Failure(f"the unanswered close was recorded as {recorded}")
        if len(revisions(profile)) != 1:
            raise nw.Failure("the unanswered close committed a revision")

        # 5. Relaunch: honest recovery prompt; Enter restores.
        app = launch()
        operator = app.window()
        nw.activate(operator)
        body = wait_for(
            "the recovery prompt", 15, lambda: nodes(operator).get("recovery-body"), 0.5
        )
        text = body.get("name", "")
        summary["recovery_text"] = text
        for expected in (
            "Sela was closed last time before this recovery question was answered.",
            "holds “Untitled schedule” with 1 item(s).",
            "Those changes were never saved to the schedule file.",
            "were not autosaved and cannot be recovered; the schedule then had 2 item(s).",
        ):
            if expected not in text:
                raise nw.Failure(f"recovery text lacks {expected!r}: {text!r}")
        save(operator, "recovery-prompt")
        keys(operator, "enter")
        time.sleep(0.5)
        restored = nodes(operator)
        if "recovery-body" in restored:
            raise nw.Failure("Enter did not close the recovery prompt")
        if "schedule-item-0" not in restored or "schedule-item-1" in restored:
            raise nw.Failure("expected exactly one restored item")
        wait_for(
            "the unsaved title",
            5,
            lambda: nw.title(operator) == "Untitled schedule* — Sela",
        )
        wait_for("the restore commit", 10, lambda: len(revisions(profile)) == 2)
        again = [
            bytes.fromhex(i["song_id"])
            for i in read_revision(revisions(profile)[1])["items"]
        ]
        if again != [first_id] or second_id in again:
            raise nw.Failure(f"the restore committed {again}")
        if (session_file(profile) or {}).get("acknowledged") != 1:
            raise nw.Failure("the previous run is not acknowledged")
        save(operator, "restored")

        # 6. A clean quit is recorded.
        keys(operator, "ctrl+q")
        time.sleep(1.0)
        if app.process.poll() is not None:
            raise nw.Failure("Ctrl+Q quit a restored, unsaved schedule without asking")
        keys(operator, "shift+tab", "enter")  # Don't Save
        summary["recovered_exit"] = app.wait_exit()
        if summary["recovered_exit"] != 0:
            raise nw.Failure(f"operator exit status {summary['recovered_exit']}")
        state = (session_file(profile) or {}).get("state")
        if state != "clean":
            raise nw.Failure(f"session.json after a clean quit says {state!r}")

        # 7. A clean start offers nothing.
        app = launch()
        operator = app.window()
        nw.activate(operator)
        time.sleep(2.0)
        if "recovery-body" in nodes(operator):
            raise nw.Failure("a recovery prompt after a clean quit")
        keys(operator, "ctrl+q")
        summary["clean_exit"] = app.wait_exit()
        if summary["clean_exit"] != 0:
            raise nw.Failure(f"clean-start exit status {summary['clean_exit']}")
        summary["status"] = "PASS"
        print(
            "PASS: autosave while Live; second instance refused untouched; TerminateProcess "
            "after a newer edit led to an honest recovery prompt restoring the committed "
            "one-item schedule; clean quit recorded; next start asked nothing"
        )
        return 0
    except (nw.Failure, AssertionError) as error:
        summary["error"] = str(error)
        print(f"FAIL: {error}")
        for index, app in enumerate(apps):
            if app.hwnd and nw.user32.IsWindow(app.hwnd):
                save(app.hwnd, f"failure-{index}")
        return 1
    finally:
        for app in apps:
            app.close()
        for pid in spawned:
            if lo.alive(pid):
                lo.kill(pid)
        # Only after every Sela process is gone: Windows keeps open files.
        for app in apps:
            if app.owned:
                shutil.rmtree(app.scratch, ignore_errors=True)
        shutil.rmtree(profile, ignore_errors=True)
        summary.setdefault("status", "FAIL")
        (args.out / "summary.json").write_text(json.dumps(summary, indent=2))


if __name__ == "__main__":
    sys.exit(main())
