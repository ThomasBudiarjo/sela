#!/usr/bin/env python3
"""Linux/X11 native check: stage (Foldback) output next to the audience output.

Built on the shared harness (scripts/native_x11.py, docs/e2e-linux.md);
Windows counterpart: scripts/stage-display-windows.py.

Usage: python3 scripts/stage-display-x11.py [--display :118 |
       --kind xvfb|triple] [--tolerate-output-focus]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR]
       [--build]

Seeds a library with the original fixture songs and a profile whose
settings enable Foldback (`stage_enabled=1`), launches the real operator and
drives it with real clicks on controls located through AT-SPI, reading the
Live pane status group (`Live-status`) for the renderer-acknowledged state of
each output. Checks, in order:

1. No output before Live is on; Live output on starts both children (the
   audience `sela --audience` and the stage `sela --stage`, separate PIDs),
   and the operator keeps the input focus.
2. The idle stage shows its dark background, the amber "Nothing live" label
   and the white clock; the audience shows nothing.
3. Go Live on Signal Hymn: the audience shows the verse (white text only, no
   stage colors anywhere); the stage shows the verse in white in its upper
   body and the next slide (Chorus) in sky blue below; both are acknowledged
   in the status ("On screen: Signal Hymn · Verse 1", "Stage … showing
   Signal Hymn · Verse 1").
4. Next: both outputs change; the stage names the Chorus.
5. Live › Stage Message…, typed text with a "b" in it (no Black toggles while
   typing), Show on Stage: the stage gains the amber message band, the
   audience frame is byte-identical.
6. The stage process is killed (SIGKILL): the operator reports "Stage output
   lost", the audience process and window stay, and Next still changes the
   audience frame with its acknowledgment.
7. One Live › Show Stage Output, as the lost status says, restarts the
   stage: a new stage process shows the current slide (Bridge) with "End of
   schedule" next, without the audience PID or frame changing.
8. Edit › Options… › Foldback shows the enable check box, the Automatic and
   monitor rows and the content check boxes (AT-SPI roles and names are
   recorded and the page is captured); Esc cancels without touching either
   output.
9. Ctrl+Q exits 0 and ends both children.

With three monitors (`--kind triple`, Xorg dummy DUMMY0 primary plus DUMMY1
and DUMMY2, or an attached display with three RandR monitors) no placement
override is set: the audience takes its production `secondary` placement and
the stage the Options › Foldback "Automatic" monitor, and step 1 also checks
that both windows cover a whole monitor and that the stage is on neither the
primary nor the audience's monitor.

X11 differences: Xvfb has one RandR CRTC (docs/e2e-linux.md), and an Xvfb
with two `-screen`s is two X screens that winit lists one at a time, so on
`xvfb` both outputs run in their development `window` placement
(`SELA_AUDIENCE_MONITOR=window`, `SELA_STAGE_MONITOR=window`); the harness
Openbox rules keep the audience top-right and the stage below it, unfocused,
both 624x351. On real monitors (`triple`) Openbox focuses each output window
when it maps (winit 0.30's X11 backend ignores `with_active(false)`, the M1-10
Linux finding); the check fails on that unless `--tolerate-output-focus`
records it and re-activates the operator. Windows is unaffected. Captures are
lavapipe software renders, not display timing.
"""

from __future__ import annotations

import os
import signal
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

# Stage colors (src/audience/stage.rs, src/stage.rs): background, current
# slide and clock, next slide, labels and message.
STAGE_BG = (16, 17, 20)
WHITE = (255, 255, 255)
NEXT_RGB = (125, 211, 252)
NOTE_RGB = (251, 191, 36)


class Run:
    def __init__(self, session: nx.Session, app: nx.App, operator: int):
        self.session, self.app, self.operator = session, app, operator
        self.summary = session.summary

    def click(self, aid: str) -> None:
        """Click a control by AccessKit id (positions read fresh each time:
        menus and dialogs move things)."""
        nx.click(self.operator, *self.app.control(aid, self.operator).center)

    def menu(self, item: str) -> None:
        self.click("bar-live")
        nx.wait_until(
            lambda: item in self.app.controls(self.operator),
            5,
            f"Live menu row {item}",
        )
        self.click(item)

    def status(self) -> str:
        found = self.app.controls(self.operator).get("Live-status")
        return found.name if found else ""

    def wait_status(self, text: str, timeout: float = 10.0) -> str:
        def probe() -> str | None:
            status = self.status()
            return status if text in status else None

        return nx.wait_until(probe, timeout, f"Live status containing {text!r}")

    def save(self, wid: int, name: str) -> nx.Image:
        return self.session.save(nx.capture_stable(wid), name)

    def keep_focus(self, what: str) -> None:
        time.sleep(1.0)
        owner = nx.focused()
        if owner != self.operator:
            message = f"{what}: focus is {nx.describe_window(owner)}"
            if not self.session.args.tolerate_output_focus:
                self.summary.setdefault("focus_after", []).append(message)
                raise nx.Failure(f"{what}: the operator lost the input focus")
            # Known X11 difference (winit 0.30 ignores with_active(false) on
            # X11, docs/e2e-linux.md): recorded, then the operator re-activated.
            self.summary.setdefault("tolerated_output_focus", []).append(message)
            nx.log(f"tolerated: {message}")
            nx.activate(self.operator)


def colors(image: nx.Image) -> dict[str, int]:
    return {
        "white": image.count(WHITE, tolerance=40, step=2),
        "next": image.count(NEXT_RGB, tolerance=40, step=2),
        "note": image.count(NOTE_RGB, tolerance=40, step=2),
    }


def stage_panel(name: str, image: nx.Image, message: bool = False) -> dict:
    """Stage frame checks: dark background, a white current slide in the
    upper body, sky-blue next text below it, amber label (and message)."""
    corner = image.pixel(4, image.height - 4)
    if not nx.near(corner, STAGE_BG, 6):
        raise nx.Failure(f"{name}: stage background is {corner}, not {STAGE_BG}")
    h = image.height
    upper = image.count(
        WHITE,
        tolerance=40,
        step=2,
        region=(0, int(h * 0.15), image.width, int(h * 0.45)),
    )
    lower = image.count(
        NEXT_RGB, tolerance=40, step=2, region=(0, int(h * 0.62), image.width, h)
    )
    found = colors(image) | {"current_white": upper, "next_blue": lower}
    if upper < 30:
        raise nx.Failure(f"{name}: no current slide text on the stage ({found})")
    if found["note"] < 10:
        raise nx.Failure(f"{name}: no amber label on the stage ({found})")
    if message:
        band = image.count(
            NOTE_RGB,
            tolerance=40,
            step=2,
            region=(0, int(h * 0.12), image.width, int(h * 0.25)),
        )
        if band < 10:
            raise nx.Failure(f"{name}: no message band on the stage ({found})")
        found["message_band"] = band
    return found


def body_digest(wid: int) -> str:
    """Stage frame digest below the header, so a clock tick is not a change."""
    image = nx.capture(wid)
    top = int(image.height * 0.12)
    return image.crop(0, top, image.width, image.height - top).digest()


def message_band(image: nx.Image) -> int:
    h = image.height
    return image.count(
        NOTE_RGB,
        tolerance=40,
        step=2,
        region=(0, int(h * 0.12), image.width, int(h * 0.25)),
    )


def audience_private(name: str, image: nx.Image) -> dict:
    found = colors(image)
    if found["next"] or found["note"]:
        raise nx.Failure(f"{name}: stage colors on the audience output ({found})")
    if image.ink_box() is None:
        raise nx.Failure(f"{name}: no slide text on the audience output")
    return found


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    data = session.temp("sela-stage-")
    library = nx.seed_library(data / "library.sqlite", seed=args.seed)
    (data / "settings.txt").write_text(
        "sela-settings 1\nstage_enabled=1\n", encoding="utf-8"
    )
    # Three monitors (`--kind triple`): production placement, Automatic must
    # pick the monitor that is neither primary nor the audience's. Fewer:
    # the development `window` placement beside the audience.
    monitors = session.display.monitors
    production = len(monitors) >= 3
    summary["stage_placement"] = "automatic" if production else "window"
    app = session.app(
        ["--operator-library", library],
        name="operator",
        env={} if production else {"SELA_STAGE_MONITOR": "window"},
    )
    operator = app.window()
    if nx.window_name(operator) != nx.OPERATOR_TITLE:
        raise nx.Failure(f"unexpected operator window {nx.window_name(operator)!r}")
    nx.activate(operator)
    time.sleep(1.0)  # catalog and settings load off the UI thread
    run = Run(session, app, operator)
    if app.audience(0.3) or app.stage(0.3):
        raise nx.Failure("an output exists before Live output is on")

    # 1. Live output on: two separate renderer processes.
    run.click("live-output")
    audience = app.audience(20)
    stage = app.stage(20)
    if not audience or not stage:
        raise nx.Failure(
            f"outputs did not appear (audience {audience}, stage {stage}): {app.log()}"
        )
    (audience_pid, audience_wid), (stage_pid, stage_wid) = audience, stage
    if audience_pid == stage_pid:
        raise nx.Failure("audience and stage share one process")
    summary["pids"] = {"audience": audience_pid, "stage": stage_pid}
    run.keep_focus("Live output on")
    if production:
        placed = {}
        for name, wid in (("audience", audience_wid), ("stage", stage_wid)):

            def covering(wid: int = wid) -> nx.Geometry | None:
                box = nx.geometry(wid)
                return box if (box.width, box.height) == (1920, 1080) else None

            box = nx.wait_until(covering, 10, f"the {name} window covering a monitor")
            placed[name] = next(
                (m.name for m in monitors if (m.x, m.y) == (box.x, box.y)), None
            )
        summary["placement"] = placed
        summary["monitors"] = [m.__dict__ for m in monitors]
        primary = next((m.name for m in monitors if m.primary), monitors[0].name)
        if placed["stage"] in (placed["audience"], primary):
            raise nx.Failure(f"Automatic put the stage on {placed['stage']}: {placed}")
    status = run.wait_status("showing Nothing live")
    summary["status_idle"] = status

    # 2. Idle stage and blank audience.
    idle = run.save(stage_wid, "stage-idle")
    corner = idle.pixel(4, idle.height - 4)
    if not nx.near(corner, STAGE_BG, 6):
        raise nx.Failure(f"idle stage background is {corner}")
    summary["stage_idle_colors"] = colors(idle)
    if (
        summary["stage_idle_colors"]["note"] < 10
        or summary["stage_idle_colors"]["white"] < 10
    ):
        raise nx.Failure(
            f"idle stage lacks its label or clock: {summary['stage_idle_colors']}"
        )
    if run.save(audience_wid, "audience-idle").ink_box() is not None:
        raise nx.Failure("the audience shows content before Go Live")

    # 3. Go Live on the first song.
    run.click("song-0")
    time.sleep(0.5)
    audience_before, stage_before = nx.digest(audience_wid), nx.digest(stage_wid)
    run.click("go-live")
    verse, _ = nx.wait_for_change(audience_wid, audience_before, 10, "audience verse")
    stage_verse, _ = nx.wait_for_change(stage_wid, stage_before, 10, "stage verse")
    run.wait_status("On screen: Signal Hymn · Verse 1")
    run.wait_status("showing Signal Hymn · Verse 1")
    time.sleep(0.5)
    verse = run.save(audience_wid, "audience-verse")
    summary["audience_verse"] = audience_private("audience verse", verse)
    stage_verse = run.save(stage_wid, "stage-verse")
    summary["stage_verse"] = stage_panel("stage verse", stage_verse)
    if summary["stage_verse"]["next_blue"] < 20:
        raise nx.Failure(f"stage verse: no next slide text ({summary['stage_verse']})")

    # 4. Next on both outputs.
    audience_before, stage_before = nx.digest(audience_wid), nx.digest(stage_wid)
    run.click("live-next")
    nx.wait_for_change(audience_wid, audience_before, 10, "audience chorus")
    nx.wait_for_change(stage_wid, stage_before, 10, "stage chorus")
    run.wait_status("showing Signal Hymn · Chorus")
    time.sleep(0.5)
    summary["audience_chorus"] = audience_private(
        "audience chorus", run.save(audience_wid, "audience-chorus")
    )
    summary["stage_chorus"] = stage_panel(
        "stage chorus", run.save(stage_wid, "stage-chorus")
    )

    # 5. Stage message: private while typed, stage-only once shown. The
    # header (clock) is left out of the comparison: the minute may change.
    audience_frame, stage_before = nx.digest(audience_wid), body_digest(stage_wid)
    run.menu("bar-live-stage-message")
    nx.wait_until(
        lambda: "stage-message-input" in app.controls(operator),
        5,
        "the stage message field",
    )
    nx.type_text(operator, "Wrap up by eleven b")
    time.sleep(0.5)
    if nx.digest(audience_wid) != audience_frame:
        raise nx.Failure(
            "typing in the stage message changed the audience (a mask key fired?)"
        )
    if body_digest(stage_wid) != stage_before:
        raise nx.Failure("the stage changed before Show on Stage")
    nx.press(operator, "BackSpace", "BackSpace")
    run.click("dialog-confirm")
    run.wait_status("Stage message: Wrap up by eleven")
    nx.wait_until(
        lambda: message_band(nx.capture(stage_wid)) >= 10,
        10,
        "the message band on the stage",
    )
    time.sleep(0.5)
    summary["stage_message"] = stage_panel(
        "stage message", run.save(stage_wid, "stage-message"), message=True
    )
    if nx.digest(audience_wid) != audience_frame:
        raise nx.Failure("the stage message changed the audience frame")
    summary["audience_unchanged_by_message"] = True

    # 6. Kill the stage renderer: the audience keeps running and responding.
    os.kill(stage_pid, signal.SIGKILL)
    if not nx.wait_dead(stage_pid, 5):
        raise nx.Failure("the stage process did not end")
    summary["status_stage_lost"] = run.wait_status("Stage output lost")
    if not nx.alive(audience_pid) or not nx.window_exists(audience_wid):
        raise nx.Failure("killing the stage ended the audience output")
    audience_before = nx.digest(audience_wid)
    run.click("live-next")
    nx.wait_for_change(
        audience_wid, audience_before, 10, "audience bridge after stage loss"
    )
    run.wait_status("On screen: Signal Hymn · Bridge")
    time.sleep(0.5)
    summary["audience_bridge"] = audience_private(
        "audience bridge", run.save(audience_wid, "audience-bridge-stage-lost")
    )
    run.save(operator, "operator-stage-lost")

    # 7. Restart only the stage from the Live menu.
    audience_frame = nx.digest(audience_wid)
    if "Live › Show Stage Output restarts it" not in summary["status_stage_lost"]:
        raise nx.Failure("the lost status does not name the restart")
    run.menu("bar-live-stage")  # one activation restarts a lost stage
    restarted = app.stage(20)
    if not restarted or restarted[0] == stage_pid:
        raise nx.Failure(f"no fresh stage process after the restart ({restarted})")
    stage_pid, stage_wid = restarted
    run.keep_focus("stage restart")
    run.wait_status("showing Signal Hymn · Bridge")
    time.sleep(0.8)
    bridge = run.save(stage_wid, "stage-bridge-restarted")
    summary["stage_bridge"] = stage_panel("stage bridge", bridge, message=True)
    if summary["stage_bridge"]["next_blue"] > summary["stage_verse"]["next_blue"]:
        raise nx.Failure("the last slide shows next slide text on the stage")
    if nx.digest(audience_wid) != audience_frame or not nx.alive(audience_pid):
        raise nx.Failure("restarting the stage touched the audience")
    run.save(operator, "operator-final")

    # 8. Options > Foldback shows the stored choice; Cancel changes nothing.
    nx.press(operator, "alt+e", "Return")
    nx.wait_until(
        lambda: "options-section-foldback" in app.controls(operator),
        5,
        "the Options dialog",
    )
    run.click("options-section-foldback")

    def foldback_page() -> dict | None:
        controls = app.controls(operator)
        return controls if "options-stage-enabled" in controls else None

    found = nx.wait_until(foldback_page, 5, "the Foldback page")
    summary["foldback_controls"] = {
        aid: [found[aid].role, found[aid].name]
        for aid in sorted(found)
        if aid.startswith(("options-stage", "options-section"))
    }
    run.save(operator, "options-foldback")
    nx.press(operator, "Escape")
    nx.wait_until(
        lambda: "options-section-foldback" not in app.controls(operator),
        5,
        "Options to close",
    )
    if not nx.alive(stage_pid) or not nx.alive(audience_pid):
        raise nx.Failure("cancelling Options ended an output")

    # 9. Quit ends both children.
    nx.press(operator, "ctrl+q")
    summary["operator_exit"] = app.wait_exit()
    if summary["operator_exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['operator_exit']}")
    for pid in (audience_pid, stage_pid):
        if not nx.wait_dead(pid, 5):
            raise nx.Failure(f"Ctrl+Q left output process {pid} running")
    session.close_app(app)
    return (
        "stage and audience run as separate processes; Go Live/Next reach both with "
        "their own acknowledgments; the stage message and next slide never reach the "
        "audience; killing the stage leaves the audience running and responsive; the "
        "stage restarts alone on the current slide"
    )


def options(parser) -> None:
    parser.add_argument(
        "--tolerate-output-focus",
        action="store_true",
        help="record (instead of failing on) an output window taking the X focus",
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "stage-display-x11", options=options))
