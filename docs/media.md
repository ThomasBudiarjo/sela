# Media backend

Sela decodes video and audio by running a bundled LGPL FFmpeg as child
processes (`ffmpeg.exe` and `ffprobe.exe`), never by linking FFmpeg. This
note records the decisions for the FFmpeg base of M2-01/M2-02. The research
behind them is the mission note `research/ffmpeg-audio.md` (2026-10-07).

Status: the locator, the build check and the input argument rules are
implemented in `src/media/ffmpeg.rs` and unit-tested. Nothing in the app calls
them yet. M2-01 and M2-02 stay open.

## Pinned build

| | |
| --- | --- |
| Source | BtbN/FFmpeg-Builds, `win64-lgpl-shared`, release branch 8.1 |
| Tag | `autobuild-2026-09-30-13-08` (a month-end build, kept two years) |
| Asset | `ffmpeg-n8.1.3-9-g29e619e767-win64-lgpl-shared-8.1.zip` |
| Size | 80,733,362 bytes |
| SHA-256 | `3e47bda1607740550141e37c0e49d1e5182b34699f15adfd137ee266d346811a` |
| Version string | `n8.1.3-9-g29e619e767` |
| License | LGPL-3.0-or-later (`--enable-version3`, no `--enable-gpl`, no `--enable-nonfree`) |
| Dev folder | `.amp/in/ffmpeg/n8.1.3-lgpl/` (`PINNED_ID`) |

The fetch script that downloads, verifies and extracts this build into the
dev folder (VAL-MEDIA-004) is not written yet.

## Lookup order

`candidates` builds the list and `locate_with` walks it. The first candidate
that passes every check wins. A refused candidate is recorded and the next one
is tried.

| Order | Source | Directory | When |
| --- | --- | --- | --- |
| 1 | `Source::Env` | `SELA_FFMPEG_DIR` | set and not empty |
| 2 | `Source::Install` | `<exe dir>\ffmpeg\bin` | always |
| 3 | `Source::Dev` | `<checkout>\.amp\in\ffmpeg\n8.1.3-lgpl\bin` | debug builds only |

- PATH is never searched. A PATH copy could be a GPL build, or a different
  version than the one Sela ships and tests.
- The dev folder comes from `CARGO_MANIFEST_DIR` at build time, not from the
  executable's location, because `CARGO_TARGET_DIR` may be outside the
  checkout.
- A relative `SELA_FFMPEG_DIR` is refused (`Refusal::RelativeDir`) without
  looking inside it. Otherwise the executable that runs would depend on the
  directory Sela was started from.
- `Located` carries the earlier refusals, so a refused `SELA_FFMPEG_DIR` can
  still be reported when a later candidate is used. `NotFound` lists every
  candidate with its reason, in lookup order.

Checks per candidate, in order:

1. The directory is absolute.
2. `ffmpeg.exe` and `ffprobe.exe` are both files. `ffprobe` is only checked
   for presence: both come from the same build.
3. `ffmpeg -hide_banner -buildconf` and `ffmpeg -hide_banner -version` run and
   exit successfully.
4. The build configuration passes the license check below.
5. `-version` prints an `ffmpeg version <token>` line. The token is
   `Located::version`.

Each probe command has stdin and stderr set to null, `CREATE_NO_WINDOW` on
Windows, at most 64 KiB of stdout (`PROBE_OUTPUT_LIMIT`) and 5 s from spawn to
exit (`PROBE_TIMEOUT`). A child that overruns either bound is killed and
reaped. `locate()` can therefore block for up to about 10 s per candidate, so
callers must run it on a worker thread, never on the UI thread or the live
frame path.

## Build license check

`check_buildconf` reads the `-buildconf` output (the one-line `configuration:`
of `-version` output works too). Flags are matched as whole
whitespace-separated words.

| Output | Result |
| --- | --- |
| No `configuration:` section | `Refusal::Unrecognized` (not FFmpeg's output) |
| Contains `--enable-nonfree` | `Refusal::Nonfree` |
| Contains `--enable-gpl` | `Refusal::Gpl` |
| Neither flag (`--enable-version3` is fine) | accepted |

Nonfree is checked first because it also makes the binary
non-redistributable. Each refusal's message names the flag.

## Decoder input arguments

`input_args(path)` returns the leading `ffmpeg` arguments, and callers append
the output options:

```
-nostdin -hide_banner -protocol_whitelist file -i <path>
```

`-protocol_whitelist file` stops a playlist or concat file inside a local file
from opening a network URL or another protocol. These are `ffmpeg` arguments.
`ffprobe` has no `-nostdin` and needs its own builder when a caller needs one.

Only absolute local file paths are accepted:

| Input | Result |
| --- | --- |
| `C:\Media\clip.mp4` | accepted |
| `\\?\C:\Media\clip.mp4` | accepted |
| `http://...`, `file:...`, `concat:...`, `pipe:0`, any `scheme:` of two or more characters | `InputRefused::Url` |
| `clip.mp4`, `..\clip.mp4`, `C:clip.mp4`, `\Media\clip.mp4`, empty | `InputRefused::Relative` |
| `\\server\share\...`, `//server/share/...`, `\\?\UNC\...` | `InputRefused::Network` |
| `\\.\...`, `\\?\Volume{...}\...`, `\\?\GLOBALROOT\...` | `InputRefused::Device` |

The scheme test follows FFmpeg's own `url_find_protocol`. A single letter
before `:` is a drive, which FFmpeg also opens as a file.

`\\?\C:\` paths are accepted because `std::fs::canonicalize` returns them and
FFmpeg's file protocol opens them unchanged. `\\.\` paths are refused even
when they name a drive, since the same prefix also reaches pipes and devices.

Known gap: a mapped network drive or a `subst` drive looks like a local drive
letter and is accepted. Detecting it needs `GetDriveTypeW`, which is open.

## Redistribution obligations

From the research note, for LGPLv3 and FFmpeg's legal checklist. None of these
is done yet. They belong to the installer (M1-16) and the third-party notices.

1. Ship FFmpeg unmodified as separate executables and DLLs, so the user can
   replace them.
2. Install `LICENSE.txt` (LGPLv3), the GPLv3 text it incorporates, and a
   `README-FFmpeg.txt` with the version, the `-buildconf` output, the BtbN
   source and tag URL and the SHA-256. Show the same in an About or
   third-party notices screen.
3. Provide the corresponding source: FFmpeg commit `29e619e767` on
   `release/8.1` and BtbN's build scripts, alongside the download or through
   a written offer valid for 3 years. Mirroring the zip and the source tarball
   in a Sela-controlled location needs the owner's authorization.
4. Generate notices for the bundled dependency libraries (dav1d, libvpx,
   libaom, opus, mp3lame, openh264, harfbuzz, fontconfig, libxml2, zlib and
   others), which carry their own BSD, MIT, LGPL and ISC terms.
5. Do not present FFmpeg as Sela's own code, and do not use the FFmpeg logo or
   name to suggest endorsement.

## Patent caveat

H.264, HEVC, AAC, AC-3/E-AC-3, VC-1 and MPEG-2 may be patent-encumbered in
some jurisdictions. The LGPL grants no patent license, and FFmpeg's legal page
tells distributors to consult counsel. Record this as a known risk in the
release notes and the third-party notices. Prefer VP9, AV1 and Opus in any
sample media Sela provides, and do not ship encoding as a feature. A custom
build without those decoders, relying on Windows Media Foundation instead, has
not been evaluated.

## Not in this slice

- The fetch script and its size and hash checks (VAL-MEDIA-004).
- Wiring `locate()` into the app on a worker thread, logging the accepted
  version string, and the visible "media unavailable" warning with the app
  still working (VAL-MEDIA-001 threading clause, VAL-MEDIA-002 logging clause,
  VAL-MEDIA-003).
- The decoder supervisor: Job Object `KILL_ON_JOB_CLOSE`, stderr draining for
  long-running decoders, and no console flash (VAL-MEDIA-030, VAL-MEDIA-031).
- The VAL-MEDIA-038 crafted-playlist runtime test, which needs a real FFmpeg:
  a local playlist that references an `http://` URL must fail with a protocol
  error and make no network attempt.
- Any decoding, the format matrix, hardware decode and memory budgets (the
  rest of M2-01 and M2-02).
