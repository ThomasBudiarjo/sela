# M1-07a — song search index (schema 5)

State: **implemented-unqualified**. Backend slice of M1-07 only: the index,
query handling, worker commands and a database-latency benchmark. There is no
search box yet, and EasyWorship 8.0.49's search behavior (which fields it
searches, prefix/substring matching, ranking, live filtering) is **unobserved**.
Everything below is a provisional Sela choice until the W04 observation.

## Index

Schema 5 adds two tables (see [storage](storage.md#m1-07a--search-index--backed-up-schema-5)):

- `search_rows(row INTEGER PRIMARY KEY, song UNIQUE REFERENCES songs(id))`
  gives every indexed song a stable integer key. The implicit rowids of
  `songs` are not used because `VACUUM` may renumber them.
- `song_search`, an FTS5 table with columns `title`, `lyrics` and `metadata`.
  - `lyrics` is every section's lyrics joined by newlines; section labels are
    not indexed.
  - `metadata` is authors, copyright and license.

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

`ORDER BY bm25(song_search, 10.0, 1.0, 2.0), title COLLATE NOCASE, title,
song id`.

- Column weights are title 10, lyrics 1, metadata 2, so a title match ranks
  above a lyric-only match.
- Ties break deterministically by title ignoring ASCII case (SQLite `NOCASE`
  folds ASCII only), then exact title, then the stable song id. Duplicate
  titles therefore keep a fixed order.

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

## Open

- Search box UI in the operator Library: live filtering, keyboard focus
  ownership so typing never fires live shortcuts, empty, loading and error
  states, stale-reply rejection with `Generations`. This needs the W04 EW
  observation of search behavior.
- Searching on the field choice EW offers (title only, lyrics, all), once
  observed.
- End-to-end latency (keystroke to rendered list) once the UI exists.
- Short-prefix ranking cost (above).
- Linux and macOS runs, and a run on an older integrated-GPU laptop for the
  M1-15 rehearsal machine.
