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
