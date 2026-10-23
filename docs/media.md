# Media backend

Sela decodes video and audio by running a bundled LGPL FFmpeg as child
processes (`ffmpeg.exe` and `ffprobe.exe`), never by linking FFmpeg. This
note records the decisions for the FFmpeg base of M2-01/M2-02. The research
behind them is the mission note `research/ffmpeg-audio.md` (2026-10-07).

Status (media-core, 2026-10-22): the locator, the build check, the input
argument rules, the fetch scripts, startup availability with its warning,
ffprobe metadata and the supervised decoder are implemented and tested on
Linux against the real pinned build (`src/media/`, `tests/media_decode.rs`,
`src/tests/media_backend.rs`, `scripts/media-backend-x11.py`). M2-01 and
M2-02 are **implemented-unqualified** for this slice (Windows, hardware and
EasyWorship checks open).

Status (video-playback, 2026-10-23): videos are shown. Profile videos,
video slide and song-master backgrounds, renderer playback with the
first-frame gate, the Live transport and the Media tab's Videos list are
described in [Video backgrounds and playback](#video-backgrounds-and-playback-m2-02-m2-03-m2-05-basic).
Implemented and tested on Linux (GPUI, unit, GPU and
`scripts/video-x11.py`); Windows, hardware, audio and EasyWorship checks
are open, so M2-02, M2-03 and M2-05 stay **implemented-unqualified**.

## Pinned build

| | Windows (installer and Windows development) | Linux (development) |
| --- | --- | --- |
| Source | BtbN/FFmpeg-Builds, `win64-lgpl-shared`, release branch 8.1 | same, `linux64-lgpl-shared` |
| Tag | `autobuild-2026-09-30-13-08` (a month-end build, kept two years) | same |
| Asset | `ffmpeg-n8.1.3-9-g29e619e767-win64-lgpl-shared-8.1.zip` | `ffmpeg-n8.1.3-9-g29e619e767-linux64-lgpl-shared-8.1.tar.xz` |
| Size | 80,733,362 bytes | 65,590,572 bytes |
| SHA-256 | `3e47bda1607740550141e37c0e49d1e5182b34699f15adfd137ee266d346811a` | `d4d7b6936f492c0b866b1cb4a29a1bbae09d6a064b4ec4eb90c95bed7969535a` |
| Version string | `n8.1.3-9-g29e619e767` | `n8.1.3-9-g29e619e767-20260930` (adds `--extra-version`) |
| License | LGPL-3.0-or-later (`--enable-version3`, no `--enable-gpl`, no `--enable-nonfree`) | same |
| Dev folder | `.amp/in/ffmpeg/n8.1.3-lgpl/` (`PINNED_ID`) | same |

Both hashes match the release's own `checksums.sha256` (read 2026-10-22).
The Linux build finds its shared libraries through `RPATH $ORIGIN/../lib`,
so the unpacked folder runs in place.

## Fetch scripts (VAL-MEDIA-004)

`scripts/fetch-ffmpeg` (bash; `--platform linux64|win64`, `--dest DIR`,
`--keep-archive`, `--force`) and `scripts/fetch-ffmpeg.ps1` (PowerShell;
`-Platform`, `-Dest`, `-KeepArchive`, `-Force`) share the pins above:

1. Download the asset with curl or `Invoke-WebRequest` into a temporary
   folder next to the destination. `SELA_FFMPEG_BASE_URL` replaces the
   download folder (a mirror or `file://` in tests; the PowerShell script
   copies a `file://` source itself, since PowerShell 7's `Invoke-WebRequest`
   refuses that scheme); the pins still apply. Usage mistakes (a flag without
   its value, an unknown flag or platform) exit 2 in both scripts; the
   PowerShell script parses its own arguments because a `param()` binding
   error exits 1.
2. Refuse unless the size and then the SHA-256 match; nothing is unpacked
   on a mismatch (exit 1).
3. Unpack, check the `<asset>/bin` layout, write `SELA-PIN.txt` (source
   URL, tag, asset, size, SHA-256, version, license, FFmpeg commit) and
   move the folder into place, replacing an older copy.
4. A destination whose `SELA-PIN.txt` already matches is left alone
   (exit 0), so the scripts are cheap to re-run.

The default destination is the development folder the locator reads in
debug builds. It is under `.amp/in/`, which git ignores: binaries are never
committed. For the Windows installer, fetch `win64` into a staging folder
and install its `bin` as `<install dir>\ffmpeg\bin` (with the notices
below, M1-16/M5).

Checked on Linux: a fresh fetch (6 s), the idempotent re-run, a 1,000-byte
download (size mismatch, exit 1, nothing unpacked), a right-sized zero file
(SHA-256 mismatch, exit 1, nothing unpacked) and an unknown platform
(exit 2). The PowerShell script has not run (no Windows host).

## Lookup order

`candidates` builds the list and `locate_with` walks it. The first candidate
that passes every check wins. A refused candidate is recorded and the next one
is tried.

| Order | Source | Directory | When |
| --- | --- | --- | --- |
| 1 | `Source::Env` | `SELA_FFMPEG_DIR` | set and not empty |
| 2 | `Source::Install` | `<exe dir>\ffmpeg\bin` | always |
| 3 | `Source::Dev` | `<checkout>\.amp\in\ffmpeg\n8.1.3-lgpl\bin` | debug builds only, unless `SELA_FFMPEG_NO_DEV_COPY=1` |

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

From the research note, for LGPLv3 and FFmpeg's legal checklist. They belong
to the installer (M1-16) and the third-party notices and are not done yet;
the fetch scripts only keep the build's own `LICENSE.txt` unmodified and
write `SELA-PIN.txt` (source URL, tag, SHA-256, version, FFmpeg commit), from
which item 2's README can be generated.

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

## Startup availability (VAL-MEDIA-001/002/003)

`src/operator/media_backend.rs` (operator) and `src/media/backend.rs`
(framework-free):

- The operator starts `ffmpeg::locate` once, on its own thread
  (`sela-media-locate`), when its window opens. The answer arrives over a
  channel that the 16 ms operator poll reads; nothing on the UI thread waits
  for it. `SettingsConfig::media_locator` injects it (`None` skips the check;
  tests inject fakes, `main` passes the real `locate`).
- The result is logged once on stderr: `sela: media: FFmpeg <version>
  accepted from the <source> (<dir>)`, plus every refused earlier candidate,
  or `sela: media: FFmpeg unavailable (<reason>): <candidates and reasons>`.
- When FFmpeg is unusable the Resources status row shows an amber dot and
  "Video and audio unavailable: …" (AccessKit role Status, id
  `media-unavailable`, its text as the accessible name; EN and ID strings).
  In a narrow header it shrinks and truncates (the output line too) so the
  header controls keep their size; the accessible name keeps the full text
  (GPUI test `the_longest_warning_fits_the_minimum_window_without_squeezing_controls`:
  Indonesian nonfree text at 720x440).
  The reason shown is the most actionable one, whatever the lookup order:
  nonfree build, then GPL build, then a copy that did not run, then nothing
  found (`backend::Unavailable::of`).
- Nothing else depends on it. Songs, images, Live output, Go Live and
  Black/Clear/Logo work the same while locating and without FFmpeg (GPUI test
  and the x11 run below).
- Sela-only behavior: EasyWorship has no separate decoder to report, so no
  EW8-OBS applies to the warning itself.
- `Operator::media_tools()` hands the accepted executables to the
  video-playback stream.

## Media metadata (ffprobe)

`media::probe::probe(ffprobe, path, timeout)` runs `ffprobe -hide_banner
-loglevel error -protocol_whitelist file -print_format json -show_format
-show_streams -i <path>` as a supervised child (10 s, 1 MiB of JSON, stderr
tail kept) and `probe::parse` turns the JSON into `MediaInfo`: container,
duration, the first video stream that is not cover art (codec, coded size,
display size after sample aspect and quarter-turn rotation, average rate,
a variable-rate hint) and the first audio stream. A video stream without a
codec name is `UnsupportedCodec` (an unknown tag such as `SELA`).

## Decoder (M2-02)

`media::decode::Decoder` is the video source handle the compositor or a
preparation worker consumes. It has no GPUI types.

```rust
let decoder = Decoder::start(&tools, path, DecodeOptions {
    fit: Fit::Exact(cue_extent),      // or Fit::Source / Fit::Within(max)
    queue_frames: 4,                  // frames held at once, 1..=16, lowered to fit frame_budget
    frame_budget: 128 << 20,          // bytes of frames held by the decoder (enforced)
    stall_timeout: Duration::from_secs(5),
    max_frames: None,                 // Some(1) for a poster frame
    content_id: None,                 // None: a fresh process-unique ContentVersion::id
    ..Default::default()
});
// UI thread / frame path: never blocks.
while let Some(event) = decoder.try_next() { /* Opened, Frame, End, Failed */ }
// A cue background without copying pixels:
let background: PreparedBackground = frame.background();
drop(decoder); // cancels: the ffmpeg tree is killed at once
```

`first_frame(&tools, path, extent, timeout)` is the blocking poster-frame
helper for workers (thumbnails, failed first-frame preparation). Its own
`timeout` passing first is `TimedOut(timeout)` (whatever was slow: probe,
start or decode); `Stalled` stays the decoder watchdog's verdict.

How it works:

1. `start` returns at once. A supervisor thread (`sela-decoder`) checks the
   path (`input_args` rules), runs ffprobe, picks the output size and sends
   `Opened { info, extent, queue_frames, content_id }` (`queue_frames` as
   applied after the budget).
2. It starts `ffmpeg -loglevel info -nostats -nostdin -hide_banner
   -protocol_whitelist file -i <path> -map 0:<video> -an -sn -dn -vf
   scale=W:H:flags=bicubic,setsar=1[,pad=...],format=rgba,showinfo=checksum=0
   -fps_mode passthrough -f rawvideo -pix_fmt rgba pipe:1` in its own kill
   domain (`media::process::Supervised`: a new process group killed with
   `SIGKILL` on Unix; a Job Object with `KILL_ON_JOB_CLOSE` and
   `CREATE_NO_WINDOW` on Windows). The Job Object also ends the tree when
   Sela itself dies, and on Linux the child gets `PR_SET_PDEATHSIG(SIGKILL)`
   (set in a `pre_exec` hook; it follows the spawning thread, which owns the
   decode or probe until the child is reaped). macOS has no parent-death
   signal, so there a child blocked on a slow read, or a hanging ffprobe,
   outlives a crashed Sela (open gap, backlog M2-02).
3. `showinfo`, last in the chain, prints each output frame's number,
   timestamp, duration and size on stderr before the frame's bytes reach
   stdout. A stderr thread parses those lines (bounded line length, last 32
   other lines kept for errors); the supervisor reads exactly `W×H×4` bytes
   per frame into a fresh `Arc<[u8]>` and pairs it with its line.
   `-fps_mode passthrough` keeps variable frame rate: no frame is duplicated
   or dropped, and each keeps its own time.
4. Timestamps are relative to the first frame and strictly increasing. A
   missing (`NOPTS`), repeated or backwards timestamp becomes the previous
   one plus the frame's duration (or 1/rate, or 1/30 s) with
   `VideoFrame::repaired` set.
5. The decoder holds at most `Opened::queue_frames` decoded frames at once,
   counting the one being filled and those queued for the consumer:
   `min(queue_frames clamped to 1..=16, frame_budget / frame bytes)`. A
   frame's buffer is allocated only after a slot is free, and a slot frees
   when the consumer takes the frame (`try_next`/`next_timeout`), so held
   bytes never exceed `frame_budget` (`Decoder::buffered_frames` reports the
   count). While no slot is free the supervisor stops reading, the pipe
   fills and FFmpeg stops (backpressure). Frames the consumer has taken are
   its own memory. A frame larger than `frame_budget` is refused
   (`TooLarge`); sides above 8192 are shrunk. Example: the defaults (4
   frames, four 4K frames of budget) on an 8K source hold one 132.7 MB frame.
   After the frame loop stops (end, limit, consumer gone, bad frame) the
   supervisor drops its frame-line receiver, so the stderr thread drains
   stderr to its end instead of waiting on a full line queue.
6. A watchdog thread kills FFmpeg when it produced nothing (no byte, no
   line) for `stall_timeout` while the supervisor was waiting on it. Time
   blocked on a full queue never counts.
7. Exactly one terminal event follows the frames: `End { frames, limited }`
   or `Failed(MediaError)`. Dropping the decoder kills the tree without
   waiting; its threads end on their own and send nothing more.

Failure handling (all in `tests/media_decode.rs` unless noted):

| Case | Result |
| --- | --- |
| Relative path, URL, UNC or device path | `Input(..)` before any child starts |
| File missing | `Missing` |
| Not media (random bytes) | `Corrupt("Invalid data found when processing input")` |
| Unknown codec tag (FFV1 renamed `SELA` in AVI) | `UnsupportedCodec`, no `Opened`, no frame |
| Audio only | `NoVideo` |
| Truncated file | the frames that decode, then `End` or `Failed`, never a hang |
| Header only | `Failed`, no frame (failed first-frame preparation) |
| ffprobe hangs | `ProbeTimedOut` at the probe timeout; `start` still returned at once |
| ffmpeg silent (fake that sleeps) | `Stalled` at the stall timeout; the child is gone |
| ffmpeg killed from outside | `Crashed("ffmpeg signal: 9 (SIGKILL)")` after the frames already read |
| Consumer stops reading for 4× the stall timeout | FFmpeg held (still running), then all 60 frames, no stall |
| `queue_frames: 8` with a budget of two frames, consumer not reading | `Opened::queue_frames == 2`, never more than 2 frames held, then all 60 frames |
| 1x1 frames, `max_frames: Some(2)`, hundreds of frame lines pending | `End { frames: 2, limited: true }`, no hang |
| `first_frame` deadline passes during a hanging probe | `TimedOut(timeout)` |
| Two decoders with default options | different `ContentVersion::id`s |
| Decoder dropped mid-stream | the ffmpeg process is gone within the bound; `drop` takes < 100 ms |
| Crafted local HLS playlist or ffconcat naming `http://127.0.0.1:<port>` (VAL-MEDIA-038) | `ProtocolRefused`, and a listener on that port saw no connection |

VAL-MEDIA-038 finding: FFmpeg 8.1's own `file` protocol already limits what
a local file may open to `file,crypto,data`, so the crafted playlists are
refused even without Sela's flag (checked by removing
`-protocol_whitelist file` from both argument builders: the runtime test
still passed). Sela's flag stays as defense in depth (it also refuses
`crypto:` and `data:`), and the unit test
`absolute_local_paths_get_the_local_only_flags_before_the_input` guards it.

Provisional format matrix (software decode, this build; not advertised to
operators until the video-playback stream and Windows/hardware runs qualify
it): containers MP4/MOV, Matroska/WebM, AVI; video H.264 (FFmpeg's native
decoder), HEVC, VP8, VP9 (libvpx and native), AV1 (dav1d), MPEG-4 Part 2,
MPEG-2, ProRes, MJPEG, FFV1. Only FFV1 (generated clips) has been decoded
here. Hardware decode, 4K/60 performance and memory per resolution are not
measured.

Not used: hardware decode, audio (M2-04 with `cpal`), seeking (M2-03), a
frame pool. Looping is `DecodeOptions::looping` (video-playback). Each frame allocates its buffer; pooling waits for a
profile showing it matters.

## Video backgrounds and playback (M2-02, M2-03, M2-05 basic)

video-playback stream, 2026-10-23. EasyWorship sources: EW8-OBS-032 (Slide
pane: Media Fill, Aspect Ratio, Repeating, Volume), EW8-OBS-038/039 (Media
Fill and Select Media… with Videos first), SRC-10 (Repeating loops "until you
stop it or until you go live to something else"). Everything else below is
an UNOBSERVED best guess listed in reference-observations.md ("Video
playback best guesses") and parity-pending.md (Stream: video-playback).

### Model and profile videos

- `background::Fill::Video { video: VideoRef, repeat }`: a Media Fill whose
  media is a profile video (codec tag 4 in the background codec: name, byte
  length u64, 32-byte fingerprint, repeat byte). The editor shows it as
  "Media Fill" like an image (EW8-OBS-038). New video fills repeat; the
  Slide pane's Repeating button turns it off (play once, hold the last
  frame). Aspect Ratio applies as for images: Zoom covers and crops
  (`decode::Fit::Cover`), Stretch distorts (`Fit::Stretch`), Maintain
  letterboxes (`Fit::Exact`), all to the cue's exact extent in FFmpeg's
  scaler.
- `src/videos.rs` (no GPUI): videos are copied (managed, like images) into
  `<profile>/Resources/Videos/`, names `*.mp4|m4v|mov|mkv|webm|avi|mpg|mpeg`
  (the provisional format matrix), at most 4,096 listed, 16 GiB per file.
  Import probes the chosen file with ffprobe first (no video stream, unknown
  codec or damage refuses it with a reason), copies it in 1 MiB chunks
  (cancellable, a dot-prefixed temporary, hard link to the final name so an
  existing video is never replaced), then checks the copy's fingerprint.
  Same name with other bytes gets "name (2).mp4"; identical bytes are
  reused.
- Pinning: `VideoRef` holds the name, the length and a SHA-256 over a tag,
  the length and the first and last MiB (`videos::fingerprint`). A video of
  gigabytes is not hashed whole; a change only in the middle is not noticed
  (documented limit). `videos::verify` reads at most 2 MiB.
- Schema: no SQLite schema change (backgrounds are codec blobs);
  `.selaschedule` version 6 is written only when an item holds a video
  background (`schedule_file::VIDEO_VERSION`), so an older Sela says
  "newer" instead of "damaged". A library opened by a Sela before video
  reports a song with a video background as damaged (no shipped release
  predates this; no schema marker was added).

### From Go Live to the audience

1. Operator (`src/operator/video.rs`): a video slide's pin is checked off the
   UI thread by `videos::Checker` (latest-wins list: a deferred cue, the live
   slide, the previewed slide and their neighbours). While a check or the
   FFmpeg lookup is still running, the cue is deferred ("Preparing
   background…") and the live scene stays. A missing, changed or unreadable
   file, or no usable FFmpeg, substitutes black behind the text with the
   warning "Background video ‘NAME’ is missing; showing black" (the M1-05h
   image rule; no Go Live dialog for videos yet).
2. Wire: background tag 2 in the version-2 command body: content version,
   aspect, flags (bit 0 repeat), the absolute UTF-8 path (at most 4,096
   bytes). `scene::VideoSource::is_valid` applies `ffmpeg::input_args` rules
   on both ends; this is the first path that crosses the pipe (decision: the
   renderer decodes the video itself, in its own process, rather than
   receiving frames over the pipe; it still never opens the file in-process,
   only its supervised FFmpeg child does).
3. Renderer (`src/audience/video.rs`): the preparation worker primes the
   video: a `Decoder` (Fit for the aspect, `-stream_loop -1` when repeating,
   at most 4 frames held) must deliver its first frame before the cue's
   deadline less 150 ms; the frame is uploaded into a texture the
   composition samples. Failure (no FFmpeg in the renderer, missing,
   damaged, unsupported, too slow) rejects the cue (RenderFailed) and the
   live scene stays. A primed video stands on its first frame until its
   scene is applied, then a `audience-video` thread presents frames at their
   timestamps (late frames are skipped to the newest one due), writes each
   into the texture (`queue.write_texture` plus an empty submit, so staging
   never piles up) and wakes the event loop; the redraw only encodes a pass
   over existing bindings. The renderer locates FFmpeg itself at start
   (`audience-media-locate`, same rules and environment as the operator;
   the stage renderer does not).
4. A later slide of the same song over the same video shares the running
   playback (`video::Registry`, keyed by path, aspect, repeat, extent and
   version), so Next keeps the video running; a slide with another
   background drops it, which kills its FFmpeg child without waiting.

### Transport, masks and reports

- `playback::MediaAction` Play, Pause, Restart travel as wire kind 8 MEDIA on
  the Safety lane (after masks: a mask wanted at the same time goes first),
  never waiting for a slide in preparation. The renderer applies them to the
  applied scene's video and acknowledges Applied; with no video applied it
  answers RenderFailed. Restart reopens the decoder on the playback thread
  (bounded 5 s) and plays from the first frame; Play after the end replays.
- `playback::Timeline` is the media clock (fake-clock unit tests). Masks
  never touch it: under Black and Logo the video runs on, Clear shows it
  without text, lifting the mask shows the current position (best guess,
  EW8-OBS-006 unobserved).
- Reports: the renderer offers wire kind 9 STATUS (scene version, state,
  presented frame time, duration) on a change of state and every 250 ms
  while playing, through a one-slot latest-wins status slot of the pipe
  writer, never in a receipt's queue slot. A repeating video reports its
  place in the clip. `output::Supervisor::media_status` returns it only for
  the confirmed live scene.
- Live pane: a transport row (`video-transport`: Play/Pause `video-play-pause`
  with its toggle state, Restart `video-restart`, status `video-position`
  "Playing · 0:03 / 0:08") shows while the acknowledged live slide plays a
  video; never for a substituted or refused one. Its text is the renderer's
  report, not the request. GPUI actions `ToggleVideoPlayback` and
  `RestartVideo` have no default keys (EW media shortcuts are unobserved).
  Preview never plays a video: its tiles show the first frame.

### Thumbnails and caches (M2-05 basic)

- `videos::Lookups<K, V>`: one worker thread per purpose with a latest-wins
  wanted list (scrolling or switching items never queues stale work), at
  most two finished replies ahead of the UI, and in-flight deduplication
  (a key being worked is never queued again by a newer list; the operator
  asks only for keys it has not cached).
  `Thumbnailer` decodes poster frames (the first frame, `Fit` by aspect) one
  FFmpeg child at a time; `Checker` verifies pins.
- `videos::Lru`: least recently used values with a byte budget and an entry
  cap. Posters: 16 MiB / 256 entries (`THUMB_BUDGET`, `THUMB_ENTRIES`);
  pin checks: 32 entries. Keys carry the identity (a listed file's name,
  length and mtime; a pin's fingerprint), so a changed file gets new keys;
  a rescan of the Videos folder drops posters of files that changed or left
  (and releases their GPUI atlas sprites) and clears the pin checks.
- Priority: the live video is decoded in the renderer process by its own
  child, so posters never delay it; the operator wants posters for the
  previewed and live items' tiles first, then at most the first 64 list
  rows. Next-cue preload, GPU budgets and the shared preparation scheduler
  of M2-05 are open.

### Media tab and editor

- Media tab sidebar: Videos and Images categories (`media-category-videos`,
  `media-category-images`, AccessKit tabs; Images stays the first view, as
  before). The Videos list (`video-N` rows) shows a poster and the name
  without its extension (EW8-OBS-039), Import video… with Cancel and a
  status line.
- Song Editor Slide pane: Select Media… has Videos and Images categories
  (`media-videos`, `media-images`); it opens on the chosen media's category,
  else Videos (EW8-OBS-039 lists Videos first). A video fill shows its
  poster and name, Aspect Ratio and Repeating (EW8-OBS-032). Editor
  previews and theme thumbnails show the poster frame.

### Measurements (this VM only)

Debug build, 4 vCPU cloud container, lavapipe software Vulkan, Xvfb,
`scripts/video-x11.py` on `:123`, 624x351 audience window, VP9 640x360
30 fps clip: Go Live to the first audience change 159-418 ms across runs
(capture polling included); while playing, the audience process used 48%
of one CPU and its FFmpeg child 7% (2 s sample), audience RSS 131 MiB.
Outside Sela, the pinned FFmpeg decodes a 1080p30 VP9 clip to RGBA at
about 140 frames/s on this VM. None of this is a frame pacing, GPU, 1080p60
or hardware claim (M2-08).

### Tests

- U: `background::tests` (video fill codec), `playback::tests` (clock,
  commands, clock text), `videos::tests` (names, list, fingerprint, verify,
  LRU, latest-wins worker; import, refusals, cancel and real posters with
  FFmpeg), `transport::tests` (video tag, MEDIA, STATUS, status slot),
  `media::decode::tests` (cover/stretch chains, `-stream_loop`),
  `schedule_file::tests` (version 6), fuzz strategies for video fills and the
  new frames.
- I: `tests/media_decode.rs` (`cover_and_stretch_fill_the_cue_extent_without_bars`,
  `looping_continues_past_the_end_with_increasing_timestamps`),
  `tests/output_process.rs` (`media_commands_need_a_scene_and_the_status_follows_the_live_scene`).
- R (real GPU adapter, lavapipe here; skipped without one unless
  `SELA_REQUIRE_GPU=1`): `audience::video::tests` (primed first frame, start,
  pause, play, restart, loop; shared playback; play once and end; failures
  before the first frame).
- GPUI: `tests::video_playback` (7) and
  `song_library::tests::select_media_videos_sets_a_repeating_video_fill_that_saves`.
- N: `scripts/video-x11.py` (PASS on `:123`); `scripts/video-windows.py`
  NOT RUN (no Windows host).

### Open

- Audio of a video background (M2-04), volume (EW8-OBS-032 Volume).
- Seek and a position slider; a start position; videos as logo (SRC-10 "Set
  as Logo Background"); videos as schedule items of their own; the Videos
  list context menu (delete, rename, relink) and Locate… for videos; videos
  in the schedule preflight report and the Go Live missing-file dialog.
- Resizing the audience restarts a background video (the new extent is a new
  playback).
- Next-cue preload, GPU memory budgets, hardware decode, 1080p60 and 4K
  pacing (M2-05, M2-08); the EasyWorship observations PP-VP-01..12.

## Native checks

- Linux: `python3 scripts/media-backend-x11.py --display :115` (attach to
  `python3 scripts/native_x11.py serve --display :115`): PASS. Run "with"
  (development copy accepted, no warning), "gpl" (`SELA_FFMPEG_DIR=/usr/bin`,
  Ubuntu's GPL build, plus `SELA_FFMPEG_NO_DEV_COPY=1`: warning, then Live
  output, Go Live and Ctrl+B on the real audience), "missing" and
  "narrow-id" (Indonesian GPL warning at 720x440: truncated inside the window,
  header buttons unchanged; hovering the warning and the output line shows
  each one's full text in a hint).
- Windows: `scripts/media-backend-windows.py` (adds the console-window check
  and runs `tests/media_decode.rs` on Windows, which exercises the Job
  Object): written and linted, **not run** (no Windows host).
  `scripts/fetch-ffmpeg.ps1` ran under PowerShell 7.4.6 **on Linux**
  (`-Platform linux64`: download, size and SHA-256 verified, unpack, then
  "already holds"; usage mistakes exit 2, covered by
  `fetch_ffmpeg_ps1_reports_usage_mistakes_as_exit_2` with `SELA_PWSH`),
  not under Windows PowerShell 5.1 or on Windows.
- Narrow window: at 720x440 the Live output line and the warning truncate;
  hovering either shows its full text in a hint (`StatusHint`; GPUI test
  `hovering_truncated_status_items_shows_their_full_text`, x11 run
  "narrow-id").

## Open

- Windows: fetch-ffmpeg.ps1, media-backend-windows.py, the Job Object kill
  and `CREATE_NO_WINDOW` (VAL-MEDIA-030/031) on a real Windows machine.
- Mapped network and `subst` drives look local (`GetDriveTypeW` needed).
- Decoder memory limit inside the Job Object (`JOB_OBJECT_LIMIT_PROCESS_MEMORY`)
  is not set: untested limits could break legitimate decodes.
- Preload, audio, the format matrix on hardware, notices in the installer
  and an About screen (playback, the video background and the Go Live gate
  are in "Video backgrounds and playback").
