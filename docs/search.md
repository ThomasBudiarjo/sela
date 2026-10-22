# M1-07a — song search index (schema 5)

State: **implemented-unqualified**. M1-07a is the index, query handling,
worker commands and a database-latency benchmark; M1-07b (below, "Operator
search box") adds the Songs search box and the end-to-end measurement.
EasyWorship 8.0.49's behavior was observed in RUN-W07A (EW8-OBS-044..047):
case-insensitive word-prefix matching over title, labels and lyric words,
live filtering from one character, no ranking visible. Ranking and
normalization below are still Sela choices; the two-character minimum is an
owner decision that deliberately differs from EW.

## Index

Schema 5 adds two tables (see [storage](storage.md#m1-07a--search-index--backed-up-schema-5)):

- `search_rows(row INTEGER PRIMARY KEY, song UNIQUE REFERENCES songs(id))`
  gives every indexed song a stable integer key. The implicit rowids of
  `songs` are not used because `VACUUM` may renumber them.
- `song_search`, an FTS5 table with columns `title`, `lyrics`, `metadata`
  and (since schema 7) `labels`.
  - `lyrics` is every section's lyrics joined by newlines.
  - `metadata` is authors, copyright, license, administrator and song number (best guess, unobserved scope).
  - `labels` is each distinct section label once, in section order
    (EW8-OBS-046 matched label words; songs-browser, schema 7).
- Since schema 7 `search_rows` also holds the Sort by keys (`author`,
  `copyright`, `administrator`, `song_number`) that `Repository::library`
  orders by; see [storage](storage.md#songs-browser--schema-7-label-index-sort-by-keys).

The index holds exactly the nondeleted songs at their head revision. It is
derived data: it can always be rebuilt from `song_revisions` payloads, and a
rebuild never writes song tables.

Maintenance is transactional. `save_song` replaces the song's index entry and
`delete_song` removes it inside the same immediate transaction as the song
write, so a committed song and its index entry cannot disagree.

FTS5 is compiled into the bundled SQLite of `rusqlite =0.40.2`
(`fts5_is_compiled_into_the_bundled_sqlite`). No crate was added.

## Normalization

Tokenizer: `unicode61 remove_diacritics 2`.

- Case folding and diacritic folding: `cafe`, `CAFÉ` and `café` match both
  composed `Café` and decomposed `Cafe\u{301}`. Mode 2 is used because mode 1
  leaves some letters with several diacritics unfolded.
- Punctuation separates tokens. `kasih-Mu` is the two tokens `kasih mu`, and
  `s'lamanya` is `s lamanya`. So `kasih-mu`, `kasih mu` and `KASIH-MU` match,
  but the joined spellings `kasihmu` and `slamanya` do not. Whether EW matches
  joined spellings is unobserved.
- Display text is never changed. Hits carry the stored title.

## Query handling (`search::expression`)

- Every whitespace-separated term becomes one double-quoted FTS5 string with
  embedded quotes doubled. FTS5 syntax (`AND`, `OR`, `NEAR(`, `*`, `^`,
  `title:`, parentheses) is therefore always plain text and cannot raise an
  error or change the query.
- A term with no letter or digit is dropped. It would be an empty phrase,
  which matches nothing and would empty the whole query.
- Terms are ANDed. Only the **last** term is a prefix (`"amazing" "gr"*`),
  so results narrow as the operator types.
- An empty, whitespace-only or punctuation-only query returns no hits and
  does no database work.
- Queries longer than `MAX_QUERY_BYTES` (1024) are `Invalid`. They are
  refused at submit time and never reach the worker thread.
- At most `MAX_HITS` (200) hits are returned. `Results::truncated` reports
  that more matched.

## Ranking

`ORDER BY bm25(song_search, 10.0, 1.0, 2.0, 0.5), title COLLATE NOCASE, title,
song id`.

- Column weights are title 10, lyrics 1, metadata 2, labels 0.5, so a title
  match ranks above a lyric-only match and a label-only match ranks below
  both for the same word. bm25 scores each column separately and sums them,
  so a query that matches no label ranks exactly as before schema 7 (the
  existing ranking tests pass unchanged).
- Ties break deterministically by title ignoring ASCII case (SQLite `NOCASE`
  folds ASCII only), then exact title, then the stable song id. Duplicate
  titles therefore keep a fixed order. Since M1-07b the ranking and `LIMIT`
  run inside the FTS scan (a subquery that returns the kept rows' `rowid`)
  and only those rows are joined to `songs`; the cut at the 200-hit limit
  breaks a last tie by FTS rowid, the final order by song id.

## Worker API and stale results

- `Command::Search { query, generation }` replies
  `Reply::Search(Results { generation, hits, truncated })`. The generation
  comes back unchanged.
- `search::Generations` is the caller-side guard. `advance()` takes a new
  generation for every query, and `is_current(g)` accepts only the newest
  reply. This rejects a late reply from another worker and one whose
  cancellation lost the race on the same worker.
  - Generation 0 is never current.
- A search superseded while queued is canceled by the existing worker
  cancellation path (`Cancel::cancel`).
- `Command::RepairSearch` runs the full check and rebuilds only if it fails
  (`IndexState::Healthy` or `Rebuilt { songs }`). `Command::RebuildSearch`
  always rebuilds.

## Corrupt or missing index

On open:

- `PRAGMA quick_check` also runs the FTS5 integrity check. If the check fails
  at schema 5, Sela drops the derived index. If the file then passes, only
  the index was damaged: it is rebuilt from the song payloads and the library
  opens. If the file still fails, the drop rolls back and the library stays
  closed as `Corrupt`.
- A missing table, or rows that disagree structurally with the nondeleted
  songs, also trigger a rebuild on open.

Damage that only the full check finds (index text that differs from the head
revision, stale content) survives reopening. `repair_search` finds it and
rebuilds.

Tests: `damaged_or_missing_search_index_is_rebuilt_without_touching_songs`
(the song tables are compared byte for byte before and after) and
`damage_outside_the_search_index_keeps_the_library_closed`.

## Benchmark (`examples/search_bench.rs`)

Command (release build; it takes about 4 minutes, mostly building the
corpus):

```powershell
cargo run --locked --release -j 8 --example search_bench
```

Corpus and run:

- 20,000 synthetic songs: original generated words, 6,000-word vocabulary
  with rare accented vowels, seed `0x5e1a0007`, in a temporary directory.
  `--songs N` and `--keep NEW-PATH` are optional.
- Each query has warm-up runs, then 100 measured runs of
  `Repository::search` on the calling thread.
- It measures **database query latency only**. It includes no worker
  channel, UI or rendering, and claims no end-to-end latency.

Machine: Windows 11 Home, Intel i7-14650HX, 48 GB RAM, NVMe SSD, release
build, 2026-10-07.

Library-level results:

| Measurement | Result |
| --- | --- |
| Database size | 61.4 MiB |
| `save_song` with index | p50 7.8 ms, p95 12.9 ms, max 237 ms (188.6 s total) |
| `Repository::open` (quick_check incl. FTS5, consistency) | 451 ms |
| `check_search` (full) | 388 ms |
| `rebuild_search` (20,000 songs) | 1,457 ms |

Query results (ms):

| Query | Hits | Truncated | p50 | p95 | max |
| --- | ---: | --- | ---: | ---: | ---: |
| most common word | 200 | yes | 46.2 | 53.3 | 70.3 |
| common word | 200 | yes | 34.5 | 46.7 | 50.2 |
| mid-frequency word | 200 | yes | 6.9 | 10.9 | 12.9 |
| rare word | 181 | no | 3.3 | 5.0 | 5.4 |
| two common words | 200 | yes | 49.4 | 71.9 | 96.7 |
| exact title | 1 | no | 0.3 | 0.5 | 0.5 |
| title prefix | 200 | yes | 7.1 | 10.7 | 18.1 |
| one-letter prefix `k` | 200 | yes | 81.2 | **109.4** | 119.8 |
| two-letter prefix `ma` | 200 | yes | 44.5 | 61.2 | 70.9 |
| punctuation | 200 | yes | 22.4 | 39.1 | 41.8 |
| unaccented form of an accented word | 200 | yes | 32.5 | 47.1 | 53.7 |
| no match | 0 | no | 0.1 | 0.3 | 0.3 |
| FTS syntax as text | 0 | no | 0.1 | 0.2 | 0.2 |
| lyric line | 1 | no | 0.7 | 1.1 | 1.4 |
| **all 1,400 samples** | | | **18.1** | **75.9** | 119.8 |

Against the provisional budget (30 ms target, 100 ms p95 ceiling):

- The pooled p50 and p95 are within it.
- The one-letter prefix query misses the ceiling (p95 109 ms). Queries that
  match most of the library (most common word, two common words, two-letter
  prefix) have p50 above the 30 ms target.
- The cost is bm25 ranking over every match before `LIMIT`. Options to
  measure before choosing:
  - start the search only at two characters;
  - for very short prefixes, order by title instead of bm25;
  - add an FTS5 `prefix='1 2'` index.

Other costs to watch:

- Opening a 20k-song library spends about 0.45 s in the integrity and
  consistency checks. That runs on the storage worker, not the UI thread, but
  it delays the first search and counts against the startup budget.
- The `save_song` max of 237 ms is a single outlier during corpus build (WAL
  checkpoint suspected, not verified).
- No measurement without the index was taken, so the index's share of save
  time is unknown.

### Schema 7 re-run, this Linux VM only (songs-browser, 2026-10-22)

Not comparable to the Windows numbers above and not a qualification: a
shared 4-vCPU Intel Xeon 2.10 GHz cloud VM (15 GB RAM) with other builds
running, and no release build (disk budget). Both runs used the dev profile
with `opt-level=3` for `libsqlite3-sys`, `rusqlite` and `sela`
(`cargo build --locked -j 3 --example search_bench --config
profile.dev.package.libsqlite3-sys.opt-level=3 --config
profile.dev.package.rusqlite.opt-level=3 --config
profile.dev.package.sela.opt-level=3`, then `target/debug/examples/search_bench`),
same corpus and seed. "Base" is commit 8ba013e (schema 6) with the same two
label queries added; "schema 7" is the songs-browser tree.

| Measurement | Base (schema 6) | Schema 7 |
| --- | ---: | ---: |
| Database size | 62.0 MiB | 64.0 MiB |
| `save_song` p50 / p95 / max | 2.2 / 3.8 / 37.6 ms | 2.3 / 15.9 / 120.8 ms |
| `Repository::open` | 487 ms | 552 ms |
| `check_search` | 532 ms | 613 ms |
| `rebuild_search` | 1,268 ms | 1,436 ms |
| two-letter prefix `ma` p50 / p95 | 32.6 / 45.0 ms | 32.9 / 40.4 ms |
| one-letter prefix `k` p95 | 114.0 ms | 89.9 ms |
| most common word p50 / p95 | 34.0 / 39.6 ms | 37.0 / 49.3 ms |
| section label `verse` (every song) | 0 hits, 0.06 ms | 200 (truncated), p50 35.6 / p95 42.8 ms |
| section label and number `verse 3` | 0 hits | 200 (truncated), p50 37.6 / p95 46.9 ms |
| all samples p50 / p95 / max | 3.2 / 72.3 / 131.7 ms | 16.7 / 71.3 / 102.4 ms |

The title/lyric queries move within run-to-run noise on this shared VM; the
pooled p50 rises only because the two label queries now match all 20,000
songs (every fixture section is "Verse N"), like the most common word. Saves
cost one extra `UPDATE search_rows` and a larger FTS row; the higher p95 and
max of the schema 7 run coincide with concurrent builds on the VM and were
not investigated further.

Whole Songs list per Sort by order (`Repository::library`, 20,000 songs,
schema 7 run): Title p50 73 / p95 86 ms, Date modified 72 / 80 ms, Author
100 / 117 ms, Copyright 99 / 116 ms, Administrator 101 / 123 ms, Song Number
103 / 120 ms. The four key sorts sort a temporary B-tree on the storage
worker (no index: the empty-last expression comes first); the UI thread
only receives the result. Measuring on the reference laptop stays open
(parity-pending PP-SB-10).

## Operator search box (M1-07b)

Implemented-unqualified. See [operator-shell](operator-shell.md#m1-07b--songs-search-box-operator)
for the behavior list. Code: `src/operator/resources/song_search.rs` (pure
controller: trimmed query, `Generations`, one pending-submission slot, newest
accepted hits) and `songs.rs` (box, clear button, no-results row, Esc).

Differences from EasyWorship 8.0.49 (RUN-W07A):

| EW | Sela | Why |
| --- | --- | --- |
| Filters from one character | Filters from two; one character shows the full list and issues no query | owner decision (bounds the one-letter p95 of 109 ms measured in M1-07a) |
| Word-prefix over title, slide labels (section text labels), lyric words (EW8-OBS-045) | FTS over title, lyrics, metadata and (schema 7) section labels, last term a prefix | labels indexed by songs-browser ([backlog](backlog.md#m1-07-search-label-matching)); ranking is Sela's (labels weigh least) |
| All matches | At most 200 results; when the cap is hit a "More songs match than are shown. Type more to narrow the search." row sits under the list (`song-search-more`) | bounds the list and the query; EW shows no hint (not observed) |
| Empty list for no results | Empty list with a "No songs found" line | clearer for the operator |
| Title echoed in the box when the pane first opens | Plain empty field | EW8-OBS-044 best guess |

The two-character minimum is an owner-approved deviation from EW8-OBS-045 (EW filters from one character); it is recorded in [reference-observations](reference-observations.md).

Not observed in EW and set by best guess (EW8-OBS-047): Ctrl+B/L/C and
plain B/C/L typed in the box never toggle masks. Clearing the box with Ctrl+A
and Backspace, the clear button and Esc all restore the list.

### End-to-end latency (VAL-SEARCH-010)

Machine: Windows 11 Home, Intel i7-14650HX, 48 GB RAM, NVMe SSD, 2026-10-13,
release build, audience on the secondary monitor. Library: the 20,000-song
`search_bench --keep` corpus (seed `0x5e1a0007`, 61.5 MiB).

Method (`scripts/search-windows.py --latency-library PATH --binary
target\release\sela.exe`): the real operator is driven with SendInput. Each
of 30 distinct two-letter prefixes is typed: first letter, wait 0.4 s, then
the clock starts at the second letter's SendInput and stops at the first screen
capture of the Songs list (400x200 px patch, small-region BitBlt, about 1 ms a
poll) that differs from the unfiltered list. This is keystroke to painted
(composited) list, and it includes the app's text-input handling, the storage
worker query, the reply pickup, the 200-row layout and the present.

Results (three runs of 30 queries, `.amp/in/artifacts/search-latency-run1..3`):

| Run | p50 | p95 | max |
| --- | ---: | ---: | ---: |
| 1 | 66.5 ms | 81.5 ms | 84.8 ms |
| 2 | 66.4 ms | 83.8 ms | 84.7 ms |
| 3 | 65.7 ms | 82.4 ms | 89.2 ms |

All below the 100 ms p95 ceiling, above the provisional 30 ms target (the
database query itself is most of it: two-letter prefixes match most of the
library and bm25 ranks every match). Steps that moved it, same corpus and
method: first measurement with a full-window capture per poll 107 ms p50 /
136 ms p95 (the capture cost, about 20 ms a poll, dominated); the small patch
alone 80 / 96 ms; the 2 ms reply pickup 78 / 96 ms (single run); ranking and
`LIMIT` inside the FTS scan 66 / 82 ms.

Database-only re-run after the query change (`cargo run --locked --release
--example search_bench`, same corpus): pooled p50 11.8 ms, p95 61.9 ms, max
113 ms; two-letter prefix p50 50.2 / p95 58.3 ms; one-letter prefix p95 91.7 ms
(not queried by the UI any more).

Limits: one machine, one corpus, synthetic text; the harness samples the
screen so a frame the compositor delays shows as latency; not measured on the
older integrated-GPU laptop or with Live output driving the GPU. The Songs list is now virtualized
(see below), so clearing the box no longer rebuilds 20,000 rows.

## Songs list virtualization and scrolling (focus-scopes-resource-browser)

The Songs list is a GPUI `uniform_list` (fixed 28 px rows) over the whole
library (`Command::Library`, no payload decode) or the search hits, so only the
visible rows plus overdraw are built, for the unfiltered list and for results
alike. GPUI paints no scrollbar, so `songs.rs` draws a thumb and track
(drag or click maps the pointer to a scroll position). Ctrl+End/Home,
Page Up/Down and Shift variants move the selection and scroll the row into
view; the row is focused once built. When the focused row scrolls out (wheel
or scrollbar), keyboard focus moves to the list element (`Control::SongList`,
context `SelaControl SelaSongRow`), so the list keys keep working from the
selection and Page Down still pages instead of going live. Deleting the
selected song moves selection and focus to its neighbour row, or to the list
element (rendered even when empty) after the only song. Before
fix-sunday-focus-modality (SC-RES-01), focus stayed on the row that was no
longer rendered; GPUI then dispatches keys from its root view, which has no
key context, so every operator key was dead until the next click. Sort by
Title, Author, Copyright, Administrator, Song Number or Date modified is
persisted in `settings.txt` (`song_sort=title|author|copyright|administrator|song_number|modified`);
Date modified is the stored `songs.modified` stamp (schema 6). The four
observed keys (songs-browser, schema 7) put songs with an empty key last in
both directions, ignore ASCII case, compare Song Number as a number and
break ties by title (UNOBSERVED best guesses, reference-observations.md).
While a query is active the results keep their relevance order.

Frame measurement (release build, 20,000-song `search_bench --keep` corpus,
`scripts/resource-browser-windows.py --binary target\release\sela.exe
--library PATH`, operator frame log `SELA_FRAME_LOG`): numbers are in the work
log. The log holds render-to-render intervals while input is continuous and
the time spent building rows, so it bounds frame pacing but is not a GPU
present timestamp.

## Open

- The field choice, if EW exposes one (labels are searched since schema 7).
- Loading and error states of the box beyond the existing library error line.
- Linux and macOS runs, and a run on an older integrated-GPU laptop for the
  M1-15 rehearsal machine.
