#!/usr/bin/env python3
"""Linux/X11 native check: operator Live output drives the real audience renderer.

Port of scripts/live-output-windows.py on the shared harness
(scripts/native_x11.py, docs/e2e-linux.md).

Usage: python3 scripts/live-output-x11.py [--display :100 | --kind xvfb|dual]
       [--binary target/debug/sela] [--seed .../seed_library] [--out DIR]
       [--build] [--tolerate-audience-focus]

Seeds a new library with original fixture songs and an original logo image,
launches the operator with a private profile, and drives it by keyboard (plus
real clicks on Previous/Next, located through AT-SPI like the Windows script
locates them through UI Automation) while checking the operator keeps the
input focus (no re-activation between keys unless a theft is logged).
Checks: no audience before Live is on; the audience child appears without
taking the focus; a new session shows nothing until Go Live (Page Down); Go
Live, Next (stopping at the last slide) and Previous change the captured
audience output; Ctrl+B shows black, Ctrl+C hides the text, Ctrl+L shows the
logo, each with its operator button lit, and a second press restores the
slide; a Logo mask survives Live off/on; Live off ends the child; Ctrl+Q exits
cleanly and ends the child. Then a second operator on a formatted-song library
(M1-05g2): Go Live shows the bold gold right/bottom-aligned slide, Next shows
the italic underlined centered slide and the "Arial" slide, Next stops at the
last slide, and Ctrl+Q exits cleanly. Then a third operator on a
background-song library (M1-05h): Zoom (the green center band covers the
corners), Maintain (black bars, red and blue bands at the edges), black for
the missing image (warning captured from the operator) and the song master's
color.

X11 differences (see docs/e2e-linux.md): on the default `xvfb` display the
audience runs in its development `window` placement (640x360 beside the
operator; Xvfb has one RandR CRTC, so there is no secondary monitor) and the
harness window manager does not focus it, so the focus assertion only proves
the operator kept the focus through Live on. `--kind dual` (Xorg dummy, two
monitors) runs the real `secondary` placement; there winit 0.30's X11 backend
ignores `with_active(false)`, Openbox focuses the audience and the check fails
unless `--tolerate-audience-focus` records it as the known Linux difference.
Arial is not installed here, so the third formatted slide uses the fallback
face; the check is that it changes and shows text, as on Windows.
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_x11 as nx

# Operator tab order: controls 0-9, Live output, Go Live, Previous, Next,
# Logo, Black, Clear, Normalize text size (Songs tab), Open, Save, Add to
# Schedule, schedule Up/Down/Remove, songs, schedule items, slides.
TABS_TO_FIRST_SONG = 25
NEXT = "click:live-next"
PREVIOUS = "click:live-previous"
# Original fixture logo: one opaque color, neither black nor text white.
LOGO_RGB = (30, 110, 210)
# Formatted song slide 1: bold gold fill, right/bottom aligned.
GOLD_RGB = (255, 210, 0)
# Operator `MASK_ON` fill of an acknowledged mask button (RGB).
MASK_ON_RGB = (0xF6, 0xDF, 0xDC)
# `seed_library --background-song`: the image's red, green and blue bands
# and the song master's color.
BANDS_RGB = ((200, 40, 40), (40, 160, 60), (40, 60, 200))
MASTER_RGB = (20, 30, 90)
# Gold samples (every 2nd pixel) the Windows check requires; kept unchanged
# although the xvfb audience is only 624x351 (about 1200 are seen there).
GOLD_SAMPLES = 400


def seed_logo(profile: Path) -> None:
    nx.solid_png(profile / "Resources" / "Images" / "sela-logo.png", 64, 36, LOGO_RGB)
    (profile / "logo.txt").write_text("sela-logo.png\n", encoding="utf-8")


class Run:
    def __init__(self, session: nx.Session, app: nx.App, operator: int, strict: bool):
        self.session, self.app, self.operator, self.strict = (
            session,
            app,
            operator,
            strict,
        )
        self.summary = session.summary
        self.targets: dict[str, tuple[int, int]] = {}

    def control_point(self, aid: str) -> tuple[int, int]:
        """Client-area center of a control found by its AccessKit author id."""
        if aid not in self.targets:
            self.targets[aid] = self.app.control(aid, self.operator).center
        return self.targets[aid]

    def keys(self, *sequence: str) -> None:
        # "click:<id>" presses a control by id. Go Live leaves focus on the
        # Live pane, so counted Tabs to Previous/Next are not stable.
        for keys in sequence:
            if keys.startswith("click:"):
                nx.click(
                    self.operator, *self.control_point(keys[6:]), strict=self.strict
                )
            else:
                nx.press(self.operator, keys, strict=self.strict)

    def save(self, wid: int, name: str) -> nx.Image:
        return self.session.save(nx.capture_stable(wid), name)

    def change(
        self, screen: int, before: str, keys: str, name: str, timeout: float = 3.0
    ) -> str:
        """Press keys and wait for the captured output to differ from `before`."""
        self.keys(keys)
        image, elapsed = nx.wait_for_change(screen, before, timeout, name)
        self.summary[f"{name}_observed_ms"] = elapsed
        self.session.save(image, name)
        box = image.ink_box()
        if box is None:
            raise nx.Failure(f"{name}: no text visible on the audience output")
        self.summary[f"{name}_ink_box"] = box
        return image.digest()

    def lit_pixels(self) -> int:
        """Acknowledged-mask fill samples in the operator's toolbar band (the
        28 px menu bar sits above the 40 px toolbar)."""
        shot = nx.capture(self.operator)
        return shot.count(
            MASK_ON_RGB, tolerance=3, region=(0, 28, shot.width, min(shot.height, 68))
        )

    def wait_lit(self, name: str, lit: bool, timeout: float = 2.0) -> None:
        state = "lit" if lit else "unlit"
        nx.wait_until(
            lambda: (self.lit_pixels() >= 20) == lit,
            timeout,
            f"{name}: operator mask button {state}",
        )

    def mask(self, screen: int, before: str, keys: str, name: str, check) -> None:
        """Toggle a mask on, check the audience frame, then that its button is lit."""
        self.keys(keys)
        image, elapsed = nx.wait_for_change(screen, before, 3.0, name)
        self.summary[f"{name}_observed_ms"] = elapsed
        self.session.save(image, name)
        check(name, image)
        self.wait_lit(name, True)
        self.save(self.operator, f"operator-{name}")

    def unmask(self, screen: int, slide: str, keys: str, name: str) -> None:
        """Toggle the mask off: the same slide frame returns and no button is lit."""
        sent = time.monotonic()
        self.keys(keys)
        nx.wait_until(
            lambda: nx.digest(screen) == slide, 3.0, f"{name}: the slide to return"
        )
        self.summary[f"{name}_restored_ms"] = round((time.monotonic() - sent) * 1000)
        self.wait_lit(name, False)

    def audience(self, app: nx.App, label: str, tolerate: bool) -> tuple[int, int]:
        started = time.monotonic()
        found = app.audience(15)
        if not found:
            raise nx.Failure(f"{label}: no audience window within 15s: {app.log()}")
        self.summary[f"{label}_audience_window_seconds"] = round(
            time.monotonic() - started, 3
        )
        time.sleep(1.5)
        owner = nx.focused()
        if owner != self.operator:
            message = f"{label}: the audience output took the focus from the operator; focus is {nx.describe_window(owner)}"
            if not tolerate:
                raise nx.Failure(message)
            # Known X11 difference (winit ignores with_active(false)); recorded,
            # then the operator is re-activated so the rest can be checked.
            self.summary.setdefault("tolerated_audience_focus", []).append(message)
            nx.log(f"tolerated: {message}")
            nx.activate(self.operator)
        return found


def is_black(name: str, image: nx.Image) -> None:
    if image.brightest() > 16:
        raise nx.Failure(f"{name}: the audience output is not black")


def is_cleared(name: str, image: nx.Image) -> None:
    # The fixture slides have a black background, so Clear looks like Black
    # here; Clear over an image background is checked by native-cues.py.
    if image.ink_box() is not None:
        raise nx.Failure(f"{name}: slide text is still visible")


def is_logo(name: str, image: nx.Image) -> None:
    if not nx.near(image.center(), LOGO_RGB):
        raise nx.Failure(
            f"{name}: audience center is {image.center()}, not the logo {LOGO_RGB}"
        )
    if image.ink_box() is not None:
        raise nx.Failure(f"{name}: slide text is visible over the logo")


def backdrop(name: str, image: nx.Image, expected: dict) -> dict:
    """Check sample points inside the 32 px text inset against colors."""
    width, height = image.width, image.height
    seen = {}
    for label, (x, y) in {
        "top_left": (10, 10),
        "bottom_right": (width - 10, height - 10),
        "middle_left": (10, height // 2),
        "middle_right": (width - 10, height // 2),
    }.items():
        seen[label] = image.pixel(x, y)
        if label in expected and not nx.near(seen[label], expected[label], 16):
            raise nx.Failure(f"{name}: {label} is {seen[label]}, not {expected[label]}")
    if image.ink_box() is None:
        raise nx.Failure(f"{name}: no text over the background")
    return seen


def start_operator(
    session: nx.Session, library: Path, name: str, strict: bool
) -> tuple[nx.App, Run]:
    app = session.app(["--operator-library", library], name=name)
    operator = app.window()
    title = nx.window_name(operator)
    session.summary[f"{name}_title"] = title
    if title != nx.OPERATOR_TITLE:
        raise nx.Failure(f"unexpected operator window {title!r}")
    nx.activate(operator)
    time.sleep(1.0)  # the storage worker loads the catalog off the UI thread
    return app, Run(session, app, operator, strict)


def body(session: nx.Session) -> str:
    args, summary = session.args, session.summary
    tolerate = args.tolerate_audience_focus
    summary["audience_monitor"] = session.display.audience_monitor
    data = session.temp("sela-live-")
    library = nx.seed_library(data / "library.sqlite", seed=args.seed)
    seed_logo(data)
    app, run = start_operator(session, library, "operator", strict=False)
    operator = run.operator

    run.keys(*["tab"] * TABS_TO_FIRST_SONG, "enter", "tab", "enter")
    time.sleep(0.3)
    run.save(operator, "operator-preview")
    if app.audience(0.5):
        raise nx.Failure("an audience output exists before Live output is on")

    # Slide 0 -> song 0 -> Remove, Down, Up, Add, Save, Open -> Normalize ->
    # Clear, Black, Logo -> Next -> Previous -> Go Live -> Live output.
    run.keys(*["shift+tab"] * 15, "enter")
    pid, screen = run.audience(app, "first", tolerate)
    summary["audience_client"] = [nx.geometry(screen).width, nx.geometry(screen).height]
    image = run.save(screen, "audience-new-session")
    if image.ink_box() is not None:
        raise nx.Failure("a new output session showed content before Go Live")
    run.save(operator, "operator-live-on")

    blank = nx.digest(screen)
    # Page Down is Go Live from any non-text control (EW8-OBS-018).
    first = run.change(screen, blank, "pagedown", "go_live")
    run.save(operator, "operator-go-live")
    second = run.change(screen, first, NEXT, "next_1")
    third = run.change(screen, second, NEXT, "next_2")
    run.keys(NEXT)
    time.sleep(1.0)
    if nx.digest(screen) != third:
        raise nx.Failure("Next past the last slide changed the audience output")
    back = run.change(screen, third, PREVIOUS, "previous")
    if back != second:
        summary["previous_matches_second"] = False
        raise nx.Failure("Previous did not restore the second slide's output")
    run.save(operator, "operator-previous")

    # Masks on the real renderer, each from the unmasked slide.
    run.mask(screen, back, "ctrl+b", "black", is_black)
    run.unmask(screen, back, "ctrl+b", "black_off")
    run.mask(screen, back, "ctrl+c", "clear", is_cleared)
    run.unmask(screen, back, "ctrl+c", "clear_off")
    run.mask(screen, back, "ctrl+l", "logo", is_logo)
    run.unmask(screen, back, "ctrl+l", "logo_off")
    # Leave Logo on: it must come back in the next session (EW8-OBS-017).
    run.mask(screen, back, "ctrl+l", "logo_again", is_logo)

    run.keys("shift+tab", "shift+tab", "enter")  # Live output off
    if not nx.wait_dead(pid, 5):
        raise nx.Failure("Live output off did not end the audience child")
    summary["live_off_ended_child"] = True
    run.save(operator, "operator-live-off")

    run.keys("enter")  # Live output on again: a new session, no replay.
    pid, screen = run.audience(app, "second", tolerate)
    nx.wait_until(
        lambda: nx.near(nx.capture(screen).center(), LOGO_RGB),
        5,
        "the Logo mask in the new session",
    )
    time.sleep(0.3)
    image = run.save(screen, "audience-second-session")
    if image.ink_box() is not None:
        raise nx.Failure("a new output session replayed an earlier slide")
    run.wait_lit("second_session_logo", True)
    summary["logo_survived_live_off"] = True

    run.keys("ctrl+q")
    summary["operator_exit"] = app.wait_exit()
    if summary["operator_exit"] != 0:
        raise nx.Failure(f"operator exit status {summary['operator_exit']}")
    summary["quit_ended_child"] = nx.wait_dead(pid, 5)
    if not summary["quit_ended_child"]:
        raise nx.Failure("Ctrl+Q left the audience output running")
    session.close_app(app)

    # Second operator on a formatted-song library: the styled slides go live
    # through the same path (M1-05g2). The gold slide's fill is not
    # near-white, so its checks are color-aware instead of ink_box.
    formatted = nx.seed_library(
        data / "formatted" / "library.sqlite", "--formatted-song", seed=args.seed
    )
    app2, run2 = start_operator(session, formatted, "formatted", strict=False)
    run2.keys(*["tab"] * TABS_TO_FIRST_SONG, "enter", "tab", "enter")
    time.sleep(0.3)
    run2.save(run2.operator, "formatted-preview")
    run2.keys(*["shift+tab"] * 15, "enter")
    pid2, screen2 = run2.audience(app2, "formatted", tolerate)

    # Go Live: bold bundled face, gold fill, right/bottom aligned, with Outer
    # outline and shadow (invisible on black; their blending is unit-checked).
    blank2 = nx.digest(screen2)
    run2.keys("pagedown")
    image, elapsed = nx.wait_for_change(screen2, blank2, 5.0, "formatted go_live")
    summary["formatted_go_live_observed_ms"] = elapsed
    session.save(image, "formatted-gold")
    width, height = image.width, image.height
    gold = image.count(GOLD_RGB)
    summary["formatted_gold_samples"] = gold
    if gold < GOLD_SAMPLES:
        raise nx.Failure(f"formatted-gold: only {gold} gold samples")
    # Right/bottom aligned: the block hugs the lower-right inset, so its
    # right/bottom margins are smaller than its left/top ones.
    box = image.box(GOLD_RGB)
    if box is None:
        raise nx.Failure("formatted-gold: no gold box")
    if (
        width - box[2] > box[0]
        or height - box[3] > box[1]
        or width - box[2] > width * 0.15
        or height - box[3] > height * 0.15
    ):
        raise nx.Failure(f"formatted-gold: not right/bottom aligned: {box}")
    summary["formatted_gold_box"] = box
    gold_frame = image.digest()

    # Next: synth italic + underline, centered white fill.
    italic = run2.change(screen2, gold_frame, NEXT, "formatted-italic")
    box = summary["formatted-italic_ink_box"]
    midpoint = ((box[0] + box[2]) / 2, (box[1] + box[3]) / 2)
    if not (
        width * 0.33 < midpoint[0] < width * 0.67
        and height * 0.25 < midpoint[1] < height * 0.75
    ):
        raise nx.Failure(f"formatted-italic: not centered: {box}")
    # Next: the "Arial" bold italic slide (fallback face when Arial is absent).
    arial = run2.change(screen2, italic, NEXT, "formatted-arial")
    run2.keys(NEXT)
    time.sleep(1.0)
    if nx.digest(screen2) != arial:
        raise nx.Failure("Next past the last formatted slide changed the output")
    run2.keys("ctrl+q")
    summary["formatted_operator_exit"] = app2.wait_exit()
    if summary["formatted_operator_exit"] != 0:
        raise nx.Failure(
            f"formatted operator exit status {summary['formatted_operator_exit']}"
        )
    if not nx.wait_dead(pid2, 5):
        raise nx.Failure("the formatted operator left its audience running")
    session.close_app(app2)

    # Third operator on a background-song library (M1-05h): image fits, a
    # missing image substituted by black, and the master's color.
    backdrop_library = nx.seed_library(
        data / "backdrop" / "library.sqlite", "--background-song", seed=args.seed
    )
    app3, run3 = start_operator(session, backdrop_library, "backdrop", strict=False)
    run3.keys(*["tab"] * TABS_TO_FIRST_SONG, "enter", "tab", "enter")
    time.sleep(0.3)
    run3.save(run3.operator, "backdrop-preview")
    run3.keys(*["shift+tab"] * 15, "enter")
    pid3, screen3 = run3.audience(app3, "backdrop", tolerate)
    red, green, blue = BANDS_RGB
    black = (0, 0, 0)
    steps = [
        ("pagedown", "backdrop-zoom", {"top_left": green, "bottom_right": green}),
        (
            NEXT,
            "backdrop-maintain",
            {"top_left": black, "middle_left": red, "middle_right": blue},
        ),
        (NEXT, "backdrop-missing", {"top_left": black, "middle_left": black}),
        (NEXT, "backdrop-master", {"top_left": MASTER_RGB, "middle_right": MASTER_RGB}),
    ]
    frame = nx.digest(screen3)
    for keys, name, expected in steps:
        run3.keys(keys)
        image, elapsed = nx.wait_for_change(screen3, frame, 5.0, name)
        summary[f"{name}_observed_ms"] = elapsed
        session.save(image, name)
        summary[name] = backdrop(name, image, expected)
        frame = image.digest()
        # The missing image's warning shows in the Live status.
        run3.save(run3.operator, f"operator-{name}")
    run3.keys("ctrl+q")
    summary["backdrop_operator_exit"] = app3.wait_exit()
    if summary["backdrop_operator_exit"] != 0:
        raise nx.Failure(
            f"backdrop operator exit status {summary['backdrop_operator_exit']}"
        )
    if not nx.wait_dead(pid3, 5):
        raise nx.Failure("the backdrop operator left its audience running")

    tolerated = (
        " (audience focus theft tolerated)"
        if summary.get("tolerated_audience_focus")
        else ""
    )
    return (
        f"Live on spawns the audience ({session.display.audience_monitor}){tolerated}, Go Live/Next/Previous "
        "change it, Next stops at the end, Black/Clear/Logo show and restore with lit buttons, Logo survives "
        "Live off/on, new sessions replay no slide, Live off and Ctrl+Q end it; the formatted song shows gold "
        "right/bottom, italic centered underlined and fallback-face slides; the background song shows Zoom, "
        "Maintain bars, black for a missing image and the master color"
    )


def options(parser) -> None:
    parser.add_argument(
        "--tolerate-audience-focus",
        action="store_true",
        help="record (instead of failing on) the audience taking the focus (known on X11 dual displays)",
    )


if __name__ == "__main__":
    sys.exit(nx.main(body, __doc__, "live-output-x11", options=options))
