# Linux native E2E harness (X11)

Status: **implemented-unqualified** (M0-03 slice, 2026-10-22). The harness
drives the real GPUI operator and the real `sela --audience` renderer on a
virtual X display. It is feature-level evidence that the application behaves
on Linux; it is **not** hardware, GPU, display-timing, DPI, Windows, screen
reader or EasyWorship-parity qualification (see [Known limits](#known-limits)).
Windows runs stay in [native-testing.md](native-testing.md); what the owner
still has to run on Windows and observe in EasyWorship is in
[parity-pending.md](parity-pending.md).

Files:

- `scripts/native_x11.py`: shared harness (Python standard library plus
  external tools; no pip packages).
- `scripts/<feature>-x11.py`: one script per feature. Ported so far:
  `live-output-x11.py`, `schedule-x11.py`, `song-editor-x11.py` (from the
  `*-windows.py` scripts of the same name). New with a Windows counterpart:
  `song-import-x11.py` (import-framework; `song-import-windows.py`).
  End-to-end scenarios (backlog "Repeatable end-to-end scenarios"):
  `scenario-s01-x11.py` (fresh profile, New Song, schedule, save, reopen,
  Go Live) and `scenario-s02-x11.py` (present A, edit B, Next/Previous,
  masks, Go Live under Black), with Windows counterparts
  `scenario-s0{1,2}-windows.py` (scrutiny-r2). Feature scripts written on
  the harness: `autosave-x11.py` (M1-12, S03; Windows counterpart
  `autosave-windows.py`).
- Older X11 scripts (`native-smoke.sh`, `operator-shell.py`,
  `song-library.py`) predate the harness and were not run or ported in this
  slice; the documented `native-smoke.sh` command still uses
  `VK_DRIVER_FILES=/dev/null`, which no longer works (see below).

## Prerequisites

Ubuntu 24.04 packages (this container has all of them; the last four groups
were installed on 2026-10-22 for this slice):

| Need | Packages |
| --- | --- |
| Virtual display, WM, input, capture | `xvfb openbox xdotool x11-utils imagemagick` |
| Software Vulkan for GPUI and the audience | `mesa-vulkan-drivers` (lavapipe ICD `/usr/share/vulkan/icd.d/lvp_icd.json`) |
| Private session bus | `dbus-daemon libglib2.0-bin` (`gdbus`) |
| Control lookup by id (AT-SPI) | `at-spi2-core` |
| File dialogs (GPUI uses xdg-desktop-portal on Linux) | `xdg-desktop-portal xdg-desktop-portal-gtk` |
| Two-monitor `dual` display | `xserver-xorg-video-dummy x11-xserver-utils` (pulls `xserver-xorg-core`) |

`python3 scripts/native_x11.py doctor` lists what is present. Build the
debug binary and the fixture seeder first (or pass `--build` to a script):

```sh
cargo build --locked -j 3 --bin sela --example seed_library
```

## Running

Every feature script takes the same options:

```sh
python3 scripts/live-output-x11.py                 # private Xvfb display (default)
python3 scripts/live-output-x11.py --kind dual     # private two-monitor Xorg display
python3 scripts/live-output-x11.py --display :100  # attach to an existing display
#   --binary PATH  --seed PATH  --out DIR  --build
```

By default a run starts its **own** X server (the server picks a free display
number through `-displayfd`) and its own Openbox, and stops both at the end.
Several scripts can therefore run at the same time without sharing focus or
the XTEST pointer (checked: `schedule-x11.py` and `song-editor-x11.py` in
parallel on `:0` and `:1`, both PASS). Lavapipe is CPU heavy: on a 4-CPU
machine keep at most two or three runs in parallel next to a cargo build.

To keep one display for repeated runs (the 2026-10-22 runs used `:100`):

```sh
python3 scripts/native_x11.py serve --display :100 &   # Xvfb :100 1920x1080x24 -nolisten tcp + Openbox
python3 scripts/schedule-x11.py --display :100
kill %1                                                 # stops Xvfb and Openbox
```

`serve` starts Openbox with the harness rules (below). An attached display
whose window manager lacks those rules still works, but the xvfb-kind
audience window may overlap the operator. Only one run may use an attached
display at a time.

## Display findings (2026-10-22)

- **GPUI needs Vulkan.** At the pinned zed `a846890` GPUI renders through
  wgpu's Vulkan backend on Linux. `VK_DRIVER_FILES=/dev/null` (the old
  software-GL workaround in [native-testing.md](native-testing.md)) now makes
  the operator exit with "Failed to create surface for any enabled backend".
  The harness sets `VK_DRIVER_FILES` to the lavapipe ICD for every app it
  starts (set `SELA_E2E_KEEP_VULKAN=1` to keep the caller's drivers). The
  audience renderer (winit + wgpu) also renders correctly on lavapipe; its
  `gl` backend is not compiled into this build.
- **Xvfb has one RandR CRTC.** winit (0.30.12, `x11/monitor.rs`) builds its
  monitor list from CRTCs, so under Xvfb the audience sees one monitor named
  `screen`, and `secondary` fails with "no secondary audience monitor".
  `xrandr --setmonitor` (RandR 1.5 monitors) does not help: checked on a
  3840x1080 Xvfb, `sela --list-monitors` still printed only `screen 3840 1080`.
  The `xvfb` kind therefore runs the audience in its development `window`
  placement (`SELA_AUDIENCE_MONITOR=window`), and the harness Openbox rules
  put the operator at the top-left and the audience at the top-right as a
  624x351 window that is not focused on map, so neither covers the other on a
  1920x1080 screen. The audience renders and reports "Live output 624×351".
- **`dual` kind: a real second monitor.** Xorg with the dummy driver exposes
  sixteen outputs; the harness enables DUMMY0 (primary) and DUMMY1 at
  1920x1080 side by side, so winit sees two monitors and the audience opens
  with the production `secondary` placement, borderless and covering DUMMY1
  (1920x1080 captures). No window rules are applied there.
- **Linux finding: the audience takes the focus.** On the `dual` display
  Openbox gives the input focus to the audience window when it maps: winit
  0.30's X11 backend ignores `with_active(false)`, which keeps it unfocused
  on Windows. `live-output-x11.py --kind dual` fails on this unless
  `--tolerate-audience-focus` records it (each Live on is listed in
  `summary.json`) and re-activates the operator. Tracked in
  [backlog M1-10](backlog.md#m1-10--previewlive-controls-and-acknowledged-state);
  Windows is unaffected (`live-output-windows.py` asserts it).
- File dialogs are the GTK file chooser of `xdg-desktop-portal-gtk`, served by
  the run's private D-Bus session (portal choice pinned by a private
  `portals.conf`). They open in a separate process and are found through the
  run's process tag, never by title.

## Harness API (`import native_x11 as nx`)

Script scaffolding:

- `nx.main(body, __doc__, "<feature>-x11", seed=True, options=None)`: parses
  the common options, checks tools and binaries, starts or attaches the
  display, calls `body(session)` (which returns the PASS message), prints the
  final `PASS:`/`FAIL:`/`ENVIRONMENT:` line, captures every window of every
  app on failure, writes `summary.json` and always cleans up. `options` adds
  script-specific arguments.
- `Session`: `args`, `display`, `out`, `summary` (dict written to
  `summary.json`), `app(args, name=..., scratch=None, env=None, a11y=True,
  portal=True, audience_monitor=None, audience_backend=None)`,
  `temp(prefix)` (deleted at the end), `save(window_or_image, name)`,
  `close_app(app)` (keeps its log).
- `Failure` (exit 1) and `EnvironmentProblem` (exit 2). A plain `assert`
  also counts as a failure, with its traceback in `summary.json`.

Processes and profile:

- `App` launches the binary with a new temporary profile (`HOME`,
  `XDG_RUNTIME_DIR` mode 700, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
  `XDG_CACHE_HOME`) and a private `dbus-daemon`; with `--operator-library`
  or `--library` the Sela profile is that file's folder. Every process it
  starts, and everything they spawn (audience, D-Bus services, portal),
  carries a private `SELA_E2E_RUN=<tag>` environment entry; `close()` kills
  every live process with that tag (TERM, then KILL) and deletes the profile.
  The harness is a child subreaper, so orphans are reaped, not left as
  zombies.
- `app.window(timeout, title=None)`, `app.windows()`, `app.all_windows()`
  (with the audience), `app.new_window(known)`, `app.audience(timeout)` →
  `(pid, window)` of the child titled "Sela audience output",
  `app.wait_exit(timeout)` (fails if a window outlives the process),
  `app.running()`, `app.log()`, `app.descendants()`; module level
  `alive(pid)`, `wait_dead(pid, timeout)`, `children(pid)`.
- `app.controls(window)` → `{author_id: Control}` and
  `app.control(id).center`: AccessKit ids (the same ids UIA exposes on
  Windows, e.g. `live-next`, `mask-black`, `song-0`) with window-relative
  rectangles, read over AT-SPI with `gdbus`. Positions hold only for the
  layout at the time of the call. `Control.role` is the AT-SPI role name
  ("dialog", "push button", "list item", ...): AccessKit's adapter answers
  GetRoleName with an empty string, so the harness maps GetRole's number
  (scrutiny-r2; before that every role read as ""). Controls are looked up
  under the window's current title, which GPUI hands to AT-SPI when a frame
  begins.
- `app.file_dialog()`, `app.drive_file_dialog(path, cancel=False,
  owner=window)`: the portal Save/Open dialog; the path is typed into the
  location entry (Ctrl+L) in one burst because GTK's inline completion
  rewrites a slowly typed path.
- `nx.seed_library(path, *flags, seed=...)` (`examples/seed_library`),
  `nx.solid_png(path, w, h, rgb)`, `nx.write_png`, `nx.default_binary()`,
  `nx.default_seed()`, `nx.build()`.

Input (every call verifies the X input focus first; window-relative client
coordinates):

- `activate(window)` (bounded, verified), `ensure_focus(window, what,
  strict=False)`: a focus theft is printed and listed in `nx.THIEVES` (and in
  `summary.json`), then the window is re-activated, or the run fails when
  `strict`.
- `press(window, *chords)` with Windows-script names (`ctrl+q`, `pagedown`,
  `shift+tab`, `ctrl+delete`, `enter`) or xdotool keysyms;
  `type_text(window, text)`; `click`, `double_click`, `right_click`,
  `click_with(window, "ctrl", x, y)`, `mouse_move`, `mouse_down`,
  `mouse_up`, `drag(window, start, end)`, `scroll(window, x, y, clicks)`;
  `wm_close(window)` (Openbox Alt+F4, i.e. WM_DELETE_WINDOW, the X11 analogue
  of WM_CLOSE); `resize`, `move`, `geometry`, `focused()`,
  `window_name`, `window_exists`, `visible_windows(pid)`.

Waiting and captures:

- `wait_until(probe, timeout, what)`: bounded polling; returns the probe's
  truthy value or fails naming `what`.
- `capture(window)` → `Image` (root grab cropped to the client area; direct
  window grabs of GL/Vulkan windows are black or hang under Xvfb),
  `capture_stable(window)` (two equal consecutive grabs), `digest(window)`,
  `wait_for_change(window, before_digest, timeout, what)` → `(settled image,
  ms to first change)`.
- `Image`: `pixel`, `center`, `crop`, `digest`, `save`, `samples`,
  `count(rgb_or_predicate, tolerance, step, region)`, `box(...)`,
  `ink_box()` (near-white text box), `brightest()`, `colors()`,
  `difference(other)`; `near(a, b, tolerance)`.

Library and schedule files (read-only SQLite URIs, never through the app):
`song_payloads`, `section_ids`, `section_formats`, `backgrounds`,
`catalog_song_ids`, `read_schedule`, `sqlite_ro`.

Command line: `python3 scripts/native_x11.py doctor | serve [--display :N]
[--kind xvfb|dual] | openbox-config PATH [--kind] [--screen WxH]`.

## Conventions for feature scripts

1. **Name and shape.** `scripts/<feature>-x11.py`, executable, `#!/usr/bin/env
   python3`, standard library plus `native_x11` only. The module docstring is
   the `--help` text: usage, every check in order, and an "X11 differences"
   paragraph naming what differs from the Windows script or from Windows
   behavior. End with `sys.exit(nx.main(body, __doc__, "<feature>-x11"))`.
2. **Private everything.** Default to a private display (do not hard-code
   `:100`); start apps only through `session.app`, seed fixtures only into
   `session.temp()` folders with `nx.seed_library` and original synthetic
   content. Never search windows by title across the display, never touch
   the user's profile, session bus or display services.
3. **Input.** Use `press`/`click`/`type_text` (focus verified). Locate
   controls by AccessKit id (`app.control`) rather than counted Tabs when the
   path is not itself under test; pixel coordinates only where the Windows
   script also uses them (scale is 1.0 under Xvfb).
4. **Waits.** Every wait is bounded (`wait_until`, `wait_for_change`,
   `app.window`, `app.audience`); short fixed sleeps only to let a frame
   settle before a capture, as the Windows scripts do.
5. **Assertions.** Assert the same behavior as the Windows script where X11
   allows; document each difference in the docstring and in the work log.
   Prefer data assertions (SQLite, schedule files, process exit) over pixels;
   pixel checks sample colors or compare digests, never a stored golden PNG.
6. **Exit codes.** 0 PASS, 1 behavior failure, 2 environment problem (missing
   tool, binary or display; never reported as a pass), 130 interrupted. The
   last line is `PASS: <what was checked>`, `FAIL: <why>` or
   `ENVIRONMENT: <why>`.
7. **Artifacts.** `.amp/in/artifacts/<feature>-x11/` (ignored by git;
   `--out` to change): `summary.json` (binary path and SHA-256, git
   revision, display kind and monitors, measurements, focus thieves, error),
   named window captures, `failure-<app>-<n>.png` and each app's log. The
   folder's earlier PNG/log/summary files are deleted at start. Never keep a
   root capture: other runs' windows may be on it.
8. **Evidence.** Inspect the captures yourself before claiming a pass, and
   record the exact command, display kind, result and artifact path in the
   work log. A pass here never closes a Windows, hardware or reference item.

## Results (2026-10-22 onward; `:N` = `serve --display :N`)

The first six rows ran the HEAD f05e27a debug binary on 2026-10-22; each
later row names its stream and date and ran the debug binary of that
stream's tree (commits in the work log record of the same name).

| Script | Display | Result |
| --- | --- | --- |
| `live-output-x11.py --display :100` | xvfb, audience `window` 624x351 | PASS (58 s) |
| `schedule-x11.py --display :100` | xvfb | PASS (34 s) |
| `song-editor-x11.py --display :100` | xvfb, editor 1180x740 | PASS (64 s) |
| `live-output-x11.py --kind dual --tolerate-audience-focus` | private dual, audience `secondary` 1920x1080 | PASS, four tolerated focus thefts |
| `live-output-x11.py --kind dual` | private dual | FAIL as expected: the audience took the focus |
| `schedule-x11.py` + `song-editor-x11.py` in parallel | private xvfb `:0` and `:1` | PASS, PASS |
| `song-import-x11.py --display :112` (import-framework, 2026-10-22) | xvfb, audience `window` | PASS (27 s) |
| `autosave-x11.py --display :113` (2026-10-22, autosave-recovery; review round 1 adds the unanswered-close step) | xvfb `serve --display :113` | PASS (19 s; round 1: 20.5 s) |
| `songs-browser-x11.py --display :114` (songs-browser, 2026-10-22; re-run 2026-10-23 after review) | xvfb (`serve --display :114`), audience `window` 624x351 | PASS (Sort by keys mouse + keyboard, restart, label search, Options Song tab; since 2026-10-23 a song is Live and typing "bravo." in the search box, "CCLI b." in License Info and OK with changed options leave the audience frame unchanged; this catches leaked Black/Clear/Logo keys but not a re-cue on OK, which re-renders a pixel-identical slide here because the 351 px output draws no copyright strip: the GPUI test `typing_in_license_info_never_toggles_masks_or_cues_live` covers that) |
| all eight `*-x11.py` scripts `--display :131` (pr1-review, 2026-10-23, after the native_x11 timeout and AT-SPI probe fixes) | xvfb (`serve --display :131`), audience `window` 624x351 | PASS: schedule 35 s, song-import 31 s, autosave 24 s, songs-browser 47 s, live-output 60 s (underline run 514 px of a 514 px line; non-underlined slides at most 31 px), scenario-s01 47 s (chorus black samples 23684), scenario-s02 52 s, song-editor 66 s |

GTK portal folder chooser notes (song-import-x11.py): it opens on Recent,
where Enter in the location entry does nothing, so the script first clicks
the sidebar's Home row; a GTK tooltip under a resting pointer is a visible
portal window that `App.file_dialog()` may return, so the pointer is parked
away before the chooser opens; GPUI reads the X keymap at start and ignores
xdotool's on-the-fly mapping for characters outside the US layout, so "ñ" is
mapped to a spare keycode with `xmodmap` before the operator starts.

scrutiny-r2 (2026-10-22, `:111` = `serve --display :111`, HEAD of
`wave/scrutiny-r2`): `scenario-s01-x11.py` and `scenario-s02-x11.py` PASS,
and the three ported scripts above PASS again on the same binary (exact
commands and timings in the work-log record "scrutiny-r2"). Review round 1:
`native_x11.viewable(wid)` answers "exists and mapped" from one xwininfo
query (a window vanishing between two queries counts as gone), used by
`App.wait_exit`, the file dialog close wait and the scenario scripts; S01 now
also checks Options and the editor close prompt, S02 the Schedule item menu.
Review round 2: S02 step 2c drives New ▾ (show keys inert while open; a
press released 300 px away closes it and the first Enter reopens it) with
`nx.drag`; it FAILS on the binary without the SC2-FOCUS-03 fix.

Failure handling was checked with a nonexistent binary and an unusable
display (exit 2) and with a deliberate failure while the audience was live
(exit 1, operator and audience captured, all twelve tagged processes ended,
no temporary folder left).

## Known limits

- Software rendering (lavapipe/llvmpipe) on a virtual framebuffer: no GPU,
  driver, present pacing, vsync, scanout, hotplug, sleep/resume or device
  loss evidence; timings in `summary.json` are capture-polling latencies, not
  cue latency.
- Scale 1.0 at 96 dpi only; no mixed-DPI, fractional scaling or HiDPI.
- Openbox, not DWM: activation, focus-stealing and close semantics differ;
  Alt+F4 stands in for WM_CLOSE.
- The `xvfb` audience is a decorated development window, not the
  monitor-covering production window; use `--kind dual` for that path.
- AT-SPI ids are AccessKit's author ids, the same source as UIA ids, but this
  is not a Narrator/UIA or Orca qualification.
- The GTK portal file chooser is not the Windows common dialog; dialog
  wording, default folders and keyboard behavior differ.
- Fonts differ (no Arial; DejaVu fallback), so text metrics differ from
  Windows; no IME.
- No EasyWorship reference can run here; nothing in these runs is parity
  evidence.
