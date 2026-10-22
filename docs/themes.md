# Themes and background resolution (M1-08)

Status: **implemented-unqualified** (2026-10-22, stream "themes"). Windows
native runs, EasyWorship observation of the Themes tab and screen-reader
checks remain open (see [parity pending](parity-pending.md#stream-themes)).

## What a theme is

`sela::theme::Theme` (framework-free, `src/theme.rs`):

| Field | Meaning | Reference |
| --- | --- | --- |
| `id`, `name` | 16-byte id (`ThemeId::BUILT_IN` is all zeros), name 1..128 bytes, no control characters | Sela |
| `format` | a full `SlideFormat` (font, size Auto/Fixed, color, B/I/U, H/V alignment, Outer outline, shadow) plus **margins** | EW8-OBS-028..030 |
| `background` | the theme's Master background (none/color/image with Maintain/Stretch/Zoom); `None` is black | EW8-OBS-037..043 |
| `show_copyright` | Theme Elements › copyright: draw the copyright strip | EW8-OBS-032, EW8-OBS-063 |

The built-in theme (`Theme::built_in()`, shown as "Default (built-in)") has
no format, no background and shows the copyright, i.e. exactly Sela's look
before themes, so an untouched library renders byte-identically. Margins
(`SlideFormat.margins`, codec field 10) are 1080-reference points per side
(0..=400), scaled to the output height like outline and shadow; without them
the renderer keeps its fixed 32 px inset. Copyright *position* is not a theme
setting: only "bottom, shown or hidden" is observed (EW8-OBS-032, -063).

## Inheritance (explicit, field by field)

1. **Which theme**: a schedule item's theme snapshot; else, for a library
   song, the song's library theme (`song_themes`, by song id); else the
   library default song theme (`theme_default`, EW8-OBS-019 "Set As Default
   Song Theme"); else the built-in theme. A song naming a deleted theme uses
   the default and the Preview pane says so ("This song's theme is missing").
2. **Text format**: each `SlideFormat` field of the slide wins; every unset
   field takes the theme's value (`SlideFormat::over`); unset in both keeps
   the built-in default. An explicit "off" on the slide (outline disabled)
   is never re-enabled by a theme.
3. **Background**: slide's own → song master (M1-05h) → theme background →
   black (`theme::apply` fills the master from the theme only when the song
   has none). This keeps the M1-05h resolution and adds the theme step.
4. **Copyright strip**: drawn when the song has copyright text **and** the
   theme shows the copyright element (`theme::copyright_strip`).

`theme::apply(song, theme)` returns the song as it renders (never changing
the stored song); the operator projects slides from it, so fonts, image
preparation, preflight-style substitution and cues use the existing paths.

## Snapshots and Live

- **Library songs** follow the library: Apply to Song, a theme edit or a new
  default restyle the *Preview* of a library song on its next load (the
  preview reloads when its theme key changes).
- **Schedule items snapshot** their theme (`schedule::ItemTheme`): the first
  time an item is shown (or saved) it captures its song's library theme;
  Apply to Item stores an *explicit* snapshot. Later library edits never
  reach a snapshot; "Item: Song Theme" drops it so the item captures the
  library theme again. Only explicit snapshots make the schedule dirty.
- **`.selaschedule` version 4** stores every item's snapshot (`theme`, hex
  of `Theme::encode`, and `theme_explicit`), so a file reopens with the same
  look on any library; versions 1-3 open without snapshots. A damaged
  snapshot makes the file corrupt (never silently restyled).
- **Live never changes on a theme action.** The Live pane keeps its own item
  copy; applying or editing a theme sends nothing; the new look reaches the
  audience only through Go Live (or Next/Previous of a re-cued item). The
  font-resolution key (`Projected`) includes the theme key, so a late font job
  for another look of the same revision never lands on the slides.

## Storage: schema 7

Behind the existing verified-backup migration (`.schema6-backup`, abort hook
`SELA_ABORT_MIGRATION_SCHEMA7`):

```sql
CREATE TABLE themes(id BLOB PRIMARY KEY CHECK(length(id)=16), revision INTEGER NOT NULL CHECK(revision>0), payload BLOB NOT NULL CHECK(length(payload)<=1024));
CREATE TABLE song_themes(song BLOB PRIMARY KEY REFERENCES songs(id), theme BLOB NOT NULL CHECK(length(theme)=16));
CREATE TABLE theme_default(slot INTEGER PRIMARY KEY CHECK(slot=1), theme BLOB NOT NULL CHECK(length(theme)=16));
```

At most 256 themes (`Error::Budget` past it); a save bumps `revision`;
deleting the default theme returns to the built-in default; the built-in
theme is never stored but may be named by a song (`song_themes` row of
zeros) so a library default never overrides it. Applying a theme to a song
does not create a song revision (`song_themes` is keyed by song id), so
pinned schedule revisions and search are untouched. Worker commands:
`ThemedSong`, `ThemedSongs` (schedule save), `Themes`, `SaveTheme`,
`DeleteTheme`, `SetDefaultTheme`, `SetSongTheme`.

## Rendering: margins on the wire

`PreparedCue` carries optional `Insets` (output pixels) set from the slide's
margins; `slides::lyrics_box` is the one box both the cue fit and the
audience raster use (the copyright strip keeps the fixed inset box's bottom,
and the lyrics end above it). The resource frame appends an optional trailing
block (tag 1 + four `u32`) after the copyright text; cues without margins are
byte-identical to before. A box that leaves no room is refused, never drawn
elsewhere.

## Operator: Themes tab

`src/operator/resources/themes.rs`. Rows: the built-in theme first, then
library themes by name; each row has a 160x90 thumbnail (sample lyrics over
the theme background, rendered off the UI thread by the editor's audience
raster twin, one at a time, cached by theme stamp and output size and pruned
to the listed themes) and badges (Default, In Preview) and warnings. The
accessible name of a row is "name · badges · warnings". Footer (one line,
stable positions): New Theme, Edit…, Duplicate, Delete (press twice:
Confirm Delete), Set as Default, Apply to Song (selected Songs rows or the
library preview), Apply to Item (selected schedule items), Item: Song Theme,
and a status line. The built-in theme cannot be edited or deleted. A double
click on a row opens the editor.

Warnings (`song_library::theme_preview`, also shown in the editor footer):

- font: a named family that is not installed ("Font “X” is not installed ·
  showing DejaVu Sans"); the operator Preview pane shows the same note for a
  themed or formatted slide whose face fell back;
- image: missing, changed, unreadable or too large (`Substitute`), rendered
  black as on the audience;
- contrast: WCAG 2 contrast below 3:1 between the text color and the mean
  background luminance (image backgrounds included), unless an enabled outline
  of enough contrast separates the text;
- aspect: an image whose aspect differs from the output (connected output, or
  1920x1080) by more than 2%: Stretch distortion %, Maintain bars, Zoom crop %.

## Theme editor

The Song Editor in theme mode (`src/song_library/theme_mode.rs`): the draft
is a one-slide sample song carrying the theme's format and background, so the
Format pane's Text and Slide tabs, undo/redo, the unsaved-changes prompt and
the off-thread preview are reused unchanged. The title field is the theme
name; the Slide tab adds Theme Elements › Copyright; the Text tab adds margins
(EW keeps them under Text › Layout, which is still inert, M1-05k); the footer
has Reset Look (one undo step back to the built-in look) and the warnings.
Song-only controls (New, Library, Inspector, Arrangement, +/−, Edit Slide
Layouts) are hidden. OK/Apply store the theme (`Command::SaveTheme`) and
notify the operator, which reloads its themes; Cancel with changes asks
Yes/No/Cancel. The sample lyrics can be edited to try long or short text and
are not saved. Window title: "Theme Editor - <name>".

## GPUI patterns used

No new GPUI pattern: list rows with `track_focus`/`ActivateControl` and
AccessKit `role`/`aria_label`/`accessibility_id` as the image list
(zed `a846890`, `crates/gpui/src/elements/div.rs`), thumbnails as
`img(Arc<RenderImage>)` released with `cx.drop_image`
(`crates/gpui/src/elements/img.rs`), off-thread work with
`cx.background_executor().spawn` (`crates/gpui/src/executor.rs`), all as the
song editor already does.

## Tests

- U: `theme::tests` (codec, inheritance order, copyright element, keys,
  resolution, WCAG contrast, aspect issues), `format::tests` (margins codec,
  `over`), `slides::tests` (margins box and cue insets; long/short text on
  1920x1080, 1280x720, 1024x768, 1920x1200 and 2560x1080 with Auto and Fixed
  themes), `schedule::tests` (captured vs explicit snapshots, dirty rules,
  duplicate, reopen), `schedule_file::tests` (v4 snapshots, v3 files,
  damaged snapshots), `transport::tests` (insets round trip and rejection),
  `audience::text::tests` (lyrics inside the margin box).
- I: `storage::tests` (schema 6→7 migration with verified backup, abort
  keeps schema 6, CRUD, default, song themes, bounds, corruption).
- GPUI: `tests::themes` (six tests: tab rows/thumbnails/warnings/default;
  apply to song restyles Preview while Live waits for Go Live; a theme edit
  while live reaches Live only when re-cued; item snapshots and schedule
  files; editor preview/undo/reset/cancel/OK; missing font and missing theme
  notes).
- N: `scripts/themes-x11.py` (PASS on Xvfb), `scripts/themes-windows.py`
  (not run: no Windows host).
