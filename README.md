# Sela

An open-source, offline-first worship presentation platform built with Rust and
GPUI. Windows is the first-class deployment target; macOS and Linux are planned.

**Status: planning and technical validation. There is no runnable application yet.**

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
The application/GPUI dependency revision, Windows build, and native launch remain
M0-01 work. There is no `Cargo.toml` yet, so setup does not fetch/build an app.
Revalidate packages and add locked dependency fetching when the manifest lands.

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
