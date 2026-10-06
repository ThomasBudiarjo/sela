# Sela

An open-source, offline-first worship presentation platform built with Rust and
GPUI. Windows is the first-class deployment target; macOS and Linux are planned.

**Status: native technical preview with offline song authoring. Service schedules
and live presentation are not yet implemented or qualified for use in a service.**

## Build the technical preview

On Debian Linux, run `.agents/setup`, `cargo build --locked`, then
`cargo run --locked` in a graphical session. `target/debug/sela --version` also
works without a display. The preview opens a native GPUI window; Ctrl+Q or closing
the window closes it, with an unsaved-song guard. It opens no network listeners.
Choose **New → New Song** or **Songs → + New Song** to create original songs;
use **Library** in the editor to reopen saved songs and **Inspector** for metadata.
**OK** saves and closes only after the database acknowledges the commit. Or
use `target/debug/sela --library /absolute/path/to/library.sqlite` for an explicit
profile. Opening the library creates a local SQLite database. See the
[song editor guide](docs/song-library.md) for data locations, controls and limits.

Existing schema-1 profiles upgrade to schema 2 only after a verified sibling
`<profile>.schema1-backup` is created. Preserve that backup: a failed upgrade never
overwrites it, and retry/recovery may require a fresh profile destination. See the
[storage contract](docs/storage.md) before testing migration on valuable data.

See [bootstrap and Windows instructions](docs/gpui-bootstrap.md) for dependency
provenance, prerequisites, verification commands and qualification limits.
Windows build/native verification and the independent-audience feasibility gate
remain open; this is not an installable Sunday-ready release.

### Install the Linux developer preview locally

After the prerequisites above, install without administrator permissions:

```sh
cargo install --path . --locked --debug --bin sela --root "$PWD/.amp/install"
.amp/install/bin/sela --version
.amp/install/bin/sela
```

This installs an unoptimized technical preview, not a portable OS package.
It still needs the native libraries and fonts from setup. Add `--offline` to the
install command when dependencies are already cached; that path was tested in
the Debian orb. An initial dependency download still requires network access.
Repeat the install command after updating source. Song profiles live outside the
installation root; `cargo uninstall --root "$PWD/.amp/install" sela` removes the
executable, not those profiles. Production upgrade/recovery is not qualified.

In an Amp orb, `scripts/run-orb-preview.sh` runs the installed app in the foreground
using the verified software-GL backend. Amp Desktop supplies its graphical
environment; the script does not replace DISPLAY, Wayland or audio settings.
This override is orb-specific, not a recommended physical Windows/GPU setting.

The opt-in [audience spike](docs/output-spike.md) uses a separate winit/wgpu process
and tests UI stalls, delayed worker preparation and operator termination. It is
diagnostic geometry, not lyric presentation. [Native tests](docs/native-testing.md)
exercise the real GPUI window; the [reference ledger](docs/reference-observations.md)
records documentation evidence and the still-required Windows observations.

The opt-in [static composition diagnostic](docs/composition-spike.md) prepares
owned font/image resources and verifies actual GPU text-over-color/image readback.
Its composition code is also used by the separate native cue diagnostic, not yet
by the service UI.

The [native cue diagnostic](docs/delivery.md) now tests bounded cross-process
owned text/font/image commands, worker preparation/upload and native presentation
receipts, including stale/failed/expired cue retention and restart without replay.
The operator's Go Live and Black/Clear/Logo masks use the same renderer through
`sela --audience`.
[Verified backup/restore](docs/storage.md) is available through
the storage worker API, not yet through an operator recovery interface.

Section IDs and named arrangements persist with immutable song revisions and
survive editor save/load/duplicate/undo. Preview and Live follow a song's first
arrangement; Ctrl+Enter in the lyrics splits a section (undoable). Arrangement-
editing controls and audience pagination are not yet available; see the
[arrangement contract](docs/arrangement.md).

The [operator shell](docs/operator-shell.md) provides separate panes, resizable
dividers, collapsible Resources and five resource tabs. Songs opens the working
editor; other resource libraries are not implemented. **Live ○ Off** starts the
audience output on the secondary monitor (`SELA_AUDIENCE_MONITOR` picks another
monitor index or `window`). Pick a song in Songs, a slide in Preview, then
**Go Live**; Live shows only what the renderer confirmed on screen, and
**‹ Previous / Next ›** step through the live song. **Logo / Black / Clear**
(Ctrl+L/B/C; Page Down is Go Live) light up once the audience confirms them;
Media → Images imports a picture and sets it as the logo. Plain white-on-black
text, resized to fit each slide (optionally the same size on every slide), no
themes or schedule items yet. `sela --operator-library PATH` opens the
operator on another library. The opt-in
[video diagnostic](docs/video-spike.md) checks original FFV1 frames through FFmpeg
and GPU composition; it is not a production video player.

## Product direction

- GPUI is the operator frontend, not the live presentation renderer.
- Match EasyWorship's operator layout and behavior, using original code and assets.
- Keep Schedule, Preview, Live, and Resources familiar to existing operators.
- Isolate live composition from editing, database work, imports, and decoding.
- Require no account or internet connection to run a service.
- Keep service files, songs, themes, and interchange formats portable and documented.

The initial compatibility reference is EasyWorship 8, build 8.0.49, listed on its
[official update page](https://www.easyworship.com/software/update) when checked
on October 4, 2026. Exact interactions still require observation and acceptance
tests; compatibility is a goal, not an implemented capability. Pin reference
builds deliberately rather than silently following upstream releases.

See [the engineering baseline](docs/plan.md) for milestones and unresolved gates.
Use [the implementation backlog](docs/backlog.md) for detailed tickets,
dependencies, checklists, test boundaries, and release gates. Start or resume work
from [the work log](docs/work-log.md); the first implementation ticket is M0-01.
Sela is an independent project, not affiliated with or endorsed by EasyWorship.

## Orb development environment

Run `.agents/setup` in a Debian 12 Amp orb. It installs missing native packages
and the Rust version/components in `rust-toolchain.toml`; repeated runs reuse
installed packages and toolchains. Rust is made available to subsequent login
shells. Setup needs network access and passwordless sudo for missing packages,
but no project secrets. It does not start services or authenticate accounts.
No resume script is needed while there are no services or credentials to repair.

The initial Rust 1.98.1 pin and Linux package selection follow Zed commit
[`a846890`](https://github.com/zed-industries/zed/commit/a84689073d296dfd39987bc7dd478e43ef76d83a),
specifically `rust-toolchain.toml`, `script/linux`, and the manifests under
`crates/gpui`, `crates/gpui_linux`, and `crates/gpui_wgpu`. Only standalone GPUI
prerequisites are included, not Zed's full application dependencies. ShellCheck,
DejaVu fonts, and Vulkan diagnostics support setup verification. No upstream
application UI code is copied.

X11/Wayland libraries and Mesa software Vulkan prepare the Linux environment;
they do not select Sela's audience renderer or qualify physical GPU performance.
Setup fetches the locked dependencies without compiling the application. Xvfb,
Openbox, xdotool and xwininfo support native Linux smoke checks, not Windows/physical
display qualification. External FFmpeg supports the opt-in decoder diagnostic;
it is not bundled with Sela. GPUI is pinned in `Cargo.toml`; transitive versions
are locked.

Check setup with `bash -n .agents/setup`, `shellcheck .agents/setup`, and two
consecutive `.agents/setup` runs. Changes only reach future project orbs after
merging/pushing them to the project's default branch (or explicitly configuring
a project pre-setup script).

## License

Copyright (C) 2026 Sela contributors.

Sela is free software: you can redistribute it and/or modify it under the terms
of the GNU Affero General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later version.

Sela is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE. See [LICENSE](LICENSE) for the full terms.

SPDX-License-Identifier: AGPL-3.0-or-later

Third-party code and content retain their own licenses. This license does not
grant rights to redistribute copyrighted lyrics, Bible translations, fonts, or media.
