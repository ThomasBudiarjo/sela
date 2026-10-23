#!/usr/bin/env python3
"""Linux/X11 native check: schedule autosave, crash recovery and the
single-instance lock (M1-12, scenario S03).

Built on the shared harness (scripts/native_x11.py, docs/e2e-linux.md).

Usage: python3 scripts/autosave-x11.py [--display :113 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR] [--build]

Seeds a library with two original fixture songs into a private profile
folder and drives the real operator by mouse (controls located by AccessKit
id over AT-SPI) and keyboard:

1. First run: Live output on; the first song is added to the Schedule and
   sent live. Within the autosave cadence (2 s quiet) one revision file
   appears in `<profile>/Recovery/`; it is read back (JSON line plus its
   SHA-256 trailer) and pins exactly that song.
2. Second instance: a second `sela` on the same profile shows "Sela is
   already running" naming the profile, changes no profile file (session,
   revision and library bytes compared) and exits 0 on Enter.
3. Newer edit, then SIGKILL: the second song is added; as soon as the
   uncommitted marker is in `session.json` (and before the 2 s quiet period
   lets a second revision land) the operator process is killed with SIGKILL.
   The audience process survives the kill (it keeps its scene) and is then
   ended by the script.
4. Relaunch: the recovery prompt appears (captured) and says Sela did not
   close normally. The window is closed (WM close) without answering: the
   process exits 0, nothing is committed, and `session.json` records the
   run as `offered` with nothing acknowledged and the marker kept.
5. Relaunch: the prompt appears again (captured); it now says Sela was
   closed before the question was answered, the last autosave holds one
   item, and changes from the newer edit (two items) cannot be recovered.
   Enter (Restore is the
   default) restores exactly the committed one-item schedule as unsaved
   ("Untitled schedule* — Sela"), and the restore is committed again with
   the previous run acknowledged.
6. Ctrl+Q asks to save; Don't Save quits with exit status 0 and
   `session.json` says `clean`.
7. A further launch shows no recovery prompt and quits at once with Ctrl+Q.

X11 differences: none in the checked behaviour. The kill is SIGKILL (the
Windows script uses TerminateProcess). On the default xvfb display the
audience is the 624x351 development window beside the operator. Lock
semantics are flock(2) here and LockFileEx on Windows.
"""

from __future__ import annotations

import hashlib
import json
import os
import signal
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

ALREADY_RUNNING_TITLE = "Sela is already running"


def revisions(profile: Path) -> list[Path]:
    return sorted((profile / "Recovery").glob("autosave-*.selarecovery"))


def read_revision(path: Path) -> dict:
    """The revision's JSON line, after checking its SHA-256 trailer."""
    data = path.read_bytes()
    body, trailer, rest = data.split(b"\n", 2)
    if rest != b"" or trailer != b"sha256 " + hashlib.sha256(body).hexdigest().encode():
        raise nx.Failure(f"{path.name} is not a complete checksummed revision")
    revision = json.loads(body)
    if revision.get("format") != "sela-recovery" or revision.get("version") != 1:
        raise nx.Failure(f"unexpected revision header in {path.name}")
    return revision


def session_file(profile: Path) -> dict | None:
    try:
        return json.loads((profile / "Recovery" / "session.json").read_text())
    except (OSError, ValueError):
        return None


def digest(path: Path) -> str | None:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError:
        return None


def click_control(app: nx.App, window: int, aid: str) -> None:
    control = app.control(aid, window)
    nx.click(window, *control.center)


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    profile = session.temp("sela-autosave-")
    library = nx.seed_library(
        profile / "library.sqlite", "--second-song", seed=args.seed
    )
    first_id, second_id = nx.catalog_song_ids(library)

    # 1. First run: a committed revision while Live output is on.
    app = session.app(["--operator-library", library], name="first")
    operator = app.window()
    nx.activate(operator)
    time.sleep(1.0)  # the storage worker loads the catalog off the UI thread
    controls = app.controls(operator)
    for needed in ("song-0", "song-1", "add-to-schedule", "live-output", "go-live"):
        if needed not in controls:
            raise nx.Failure(f"control {needed!r} missing ({sorted(controls)[:30]})")
    nx.click(operator, *controls["live-output"].center)
    found = app.audience(15)
    if not found:
        raise nx.Failure(f"no audience window within 15s: {app.log()}")
    audience_pid, screen = found
    nx.activate(operator)
    time.sleep(1.0)
    blank = nx.digest(screen)
    nx.click(operator, *controls["song-0"].center)
    nx.click(operator, *controls["add-to-schedule"].center)
    nx.wait_until(
        lambda: len(revisions(profile)) == 1, 10, "the first autosave revision"
    )
    first = read_revision(revisions(profile)[0])
    pinned = [bytes.fromhex(i["song_id"]) for i in first["items"]]
    summary["first_revision"] = {"sequence": first["sequence"], "items": len(pinned)}
    if pinned != [first_id] or first["path"] is not None:
        raise nx.Failure(f"unexpected first revision {first}")
    click_control(app, operator, "schedule-item-0")
    nx.click(operator, *app.control("go-live", operator).center)
    image, summary["go_live_observed_ms"] = nx.wait_for_change(
        screen, blank, 5.0, "go_live"
    )
    session.save(image, "audience-live")
    session.save(operator, "first-run-autosaved")

    # 2. A second instance on the same profile refuses and changes nothing.
    watched = [
        profile / "Recovery" / "session.json",
        revisions(profile)[0],
        library,
        profile / "sela.lock",
    ]
    before = {str(p): digest(p) for p in watched}
    second = session.app(["--operator-library", library], name="second-instance")
    notice = second.window(title=ALREADY_RUNNING_TITLE)
    message = second.control("already-running-message", notice).name
    summary["second_instance_message"] = message
    if str(profile) not in message or "already using this profile" not in message:
        raise nx.Failure(f"unexpected second-instance message {message!r}")
    session.save(nx.capture_stable(notice), "second-instance")
    nx.activate(notice)
    nx.press(notice, "enter")
    summary["second_instance_exit"] = second.wait_exit()
    if summary["second_instance_exit"] != 0:
        raise nx.Failure(
            f"second instance exit status {summary['second_instance_exit']}"
        )
    after = {str(p): digest(p) for p in watched}
    if after != before:
        raise nx.Failure(
            f"the second instance changed profile files: {before} -> {after}"
        )
    if [w for w in app.windows() if nx.window_name(w) == ALREADY_RUNNING_TITLE]:
        raise nx.Failure("the first operator showed the notice")
    session.close_app(second)

    # 3. A newer edit, then SIGKILL before it can be committed.
    nx.activate(operator)
    controls = app.controls(operator)
    nx.click(operator, *controls["song-1"].center)
    nx.click(operator, *controls["add-to-schedule"].center)
    edited = time.monotonic()
    marker = nx.wait_until(
        lambda: (session_file(profile) or {}).get("uncommitted"),
        1.5,
        "the uncommitted marker of the newer edit",
        interval=0.02,
    )
    os.kill(app.pid, signal.SIGKILL)
    app.process.wait(5)
    summary["killed_after_edit_ms"] = round((time.monotonic() - edited) * 1000)
    summary["marker"] = marker
    if marker.get("items") != 2 or marker.get("after") != 1:
        raise nx.Failure(f"unexpected uncommitted marker {marker}")
    if len(revisions(profile)) != 1:
        raise nx.Failure("the newer edit was committed before the kill; timing invalid")
    if (session_file(profile) or {}).get("state") != "running":
        raise nx.Failure("the killed run is not recorded as running")
    if not nx.alive(audience_pid):
        raise nx.Failure(
            "the audience died with the operator (expected it to keep its scene)"
        )
    summary["audience_survived_kill"] = True
    os.kill(audience_pid, signal.SIGTERM)
    if not nx.wait_dead(audience_pid, 5):
        os.kill(audience_pid, signal.SIGKILL)
    session.close_app(app)

    # 4. Relaunch, then close the window with the prompt unanswered.
    app = session.app(["--operator-library", library], name="unanswered")
    operator = app.window()
    nx.activate(operator)
    body_control = nx.wait_until(
        lambda: app.controls(operator).get("recovery-body"), 15, "the recovery prompt"
    )
    summary["unanswered_text"] = body_control.name
    if not body_control.name.startswith("Sela did not close normally last time."):
        raise nx.Failure(f"unexpected first prompt {body_control.name!r}")
    time.sleep(0.3)
    session.save(nx.capture_stable(operator), "recovery-prompt-first")
    nx.wm_close(operator)
    summary["unanswered_exit"] = app.wait_exit()
    if summary["unanswered_exit"] != 0:
        raise nx.Failure(f"unanswered close exit status {summary['unanswered_exit']}")
    recorded = session_file(profile) or {}
    summary["unanswered_session"] = recorded
    if (
        recorded.get("state") != "offered"
        or recorded.get("acknowledged") != 0
        or not recorded.get("uncommitted")
    ):
        raise nx.Failure(f"the unanswered close was recorded as {recorded}")
    if len(revisions(profile)) != 1:
        raise nx.Failure("the unanswered close committed a revision")
    session.close_app(app)

    # 5. Relaunch: the recovery prompt, honest about the lost edit; Restore.
    app = session.app(["--operator-library", library], name="recovered")
    operator = app.window()
    nx.activate(operator)
    body_control = nx.wait_until(
        lambda: app.controls(operator).get("recovery-body"), 15, "the recovery prompt"
    )
    text = body_control.name
    summary["recovery_text"] = text
    for expected in (
        "Sela was closed last time before this recovery question was answered.",
        "holds “Untitled schedule” with 1 item(s).",
        "Those changes were never saved to the schedule file.",
        "were not autosaved and cannot be recovered; the schedule then had 2 item(s).",
    ):
        if expected not in text:
            raise nx.Failure(f"recovery text lacks {expected!r}: {text!r}")
    time.sleep(0.3)
    session.save(nx.capture_stable(operator), "recovery-prompt")
    nx.press(operator, "enter")  # Restore is the default button
    nx.wait_until(
        lambda: "recovery-body" not in app.controls(operator), 5, "the prompt to close"
    )
    restored = app.controls(operator)
    if "schedule-item-0" not in restored or "schedule-item-1" in restored:
        raise nx.Failure(
            f"expected exactly one restored item ({sorted(restored)[:40]})"
        )
    nx.wait_until(
        lambda: nx.window_name(operator) == "Untitled schedule* — Sela",
        5,
        "the unsaved title",
    )
    nx.wait_until(lambda: len(revisions(profile)) == 2, 10, "the restore commit")
    again = read_revision(revisions(profile)[1])
    if [bytes.fromhex(i["song_id"]) for i in again["items"]] != [first_id]:
        raise nx.Failure(f"the restore committed {again}")
    if second_id in [bytes.fromhex(i["song_id"]) for i in again["items"]]:
        raise nx.Failure("the lost edit reappeared")
    acknowledged = (session_file(profile) or {}).get("acknowledged")
    if acknowledged != 1:
        raise nx.Failure(f"the previous run is not acknowledged ({acknowledged})")
    time.sleep(0.3)
    session.save(operator, "restored")

    # 6. Quit cleanly with Don't Save: the run is recorded clean.
    nx.press(operator, "ctrl+q")
    time.sleep(1.0)
    if not app.running():
        raise nx.Failure("Ctrl+Q quit a restored, unsaved schedule without asking")
    session.save(operator, "quit-guard")
    nx.press(operator, "shift+tab", "enter")  # Don't Save
    summary["recovered_exit"] = app.wait_exit()
    if summary["recovered_exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['recovered_exit']}")
    state = (session_file(profile) or {}).get("state")
    if state != "clean":
        raise nx.Failure(f"session.json after a clean quit says {state!r}")
    session.close_app(app)

    # 7. A clean start offers nothing.
    app = session.app(["--operator-library", library], name="clean-start")
    operator = app.window()
    nx.activate(operator)
    time.sleep(2.0)
    if "recovery-body" in app.controls(operator):
        raise nx.Failure("a recovery prompt after a clean quit")
    session.save(operator, "clean-start")
    nx.press(operator, "ctrl+q")
    summary["clean_exit"] = app.wait_exit()
    if summary["clean_exit"] != 0:
        raise nx.Failure(f"clean-start exit status {summary['clean_exit']}")
    summary["revisions_kept"] = [p.name for p in revisions(profile)]
    return (
        "autosave committed while Live was on; a second instance refused without touching "
        "the profile; SIGKILL after a newer edit led to a recovery prompt that restored the "
        "last committed one-item schedule and disclosed the lost two-item edit (after a close "
        "with the prompt unanswered had kept it and said so); a clean "
        "quit was recorded and the next start asked nothing"
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "autosave-x11"))
