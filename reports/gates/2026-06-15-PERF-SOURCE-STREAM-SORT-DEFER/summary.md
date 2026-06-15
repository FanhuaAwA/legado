# PERF-2026-06-15-SOURCE-STREAM-SORT-DEFER

## Scope

Continue performance maintenance for large source-list loading/import/search flows. This gate covers two linked self-review findings: reader-core still built Legado list metadata through full-object parsing, and the frontend book-source store sorted the full reactive source array after every streamed batch.

Out of scope: backend command contracts, reader-core rule execution semantics, search result structure, search concurrency/timeout settings, release signing material, or third-party/private source samples.

## Review Finding

`ReaderCore::list_legado_sources()` and `ReaderCore::stream_legado_sources()` still read every Legado JSON row and deserialized it into a full `BookSource` before producing list metadata. For large source packs, that pushes a heavy all-at-once parse into the front of an otherwise streaming list flow.

`src/stores/bookSource.ts::mergeSourcesBatch()` also called `sources.value.sort(...)` after every incoming `booksource:batch` event. For large source packs, this can repeatedly reorder the entire reactive array during the streaming phase, forcing list rendering, `activeSources` computed values, and search watchers to recompute more often than needed.

The final list only needs to be sorted once a load round has completed. During streaming, preserving incremental arrival order is acceptable and keeps the UI responsive.

## Changes

- Added keyset-paged Legado source-list row reads in `BookSourceRepo` / `BookSourceService`.
- Added `idx_book_sources_user_updated_url` to support the `updated_at DESC, book_source_url DESC` list order.
- Updated `stream_legado_sources()` and `list_legado_sources()` to page through DB rows with a cursor and yield between pages.
- Added lightweight `LegadoSourceMetaSeed` parsing for list metadata while still applying `migrate_legacy_book_source_value()` and legacy field fallbacks for old source formats.
- Added `legado_list_meta_preserves_lightweight_fields` to cover lightweight list metadata and capabilities.
- Removed per-batch full-array sorting from `mergeSourcesBatch()`.
- Added `sortSourcesByName()` and sort once after streaming `done` or non-streaming fallback completion.
- Split persisted source capability cache (`source.capabilities`) from user search/explore flags (`source.flags`) so background `cap_*` writes no longer trigger disabled-set reloads and related search-source recomputation. Existing `source.capabilities` flag values remain a read fallback for compatibility.

## Verification

- `cmd /c pnpm.cmd lint` - PASS
- `cargo fmt --all` - PASS
- `cmd /c pnpm.cmd build` - PASS (existing vconsole eval and large chunk warnings only)
- `cargo check -p reader-core` - PASS
- `cargo check -p legado-tauri` - PASS
- `cargo check -p legado-headless` - PASS
- `cargo test -p reader-core` - PASS
- `cargo test -p reader-core --test route_b_facade -- --nocapture` - PASS
- `node scripts/ci/check-command-contract.mjs --json` - PASS
- `git diff --check` - PASS

## Residual Risk

- During active streaming, the visible source list may briefly follow backend batch arrival order until the final `done` event sorts it.
- The lightweight Legado list parser still parses JSON and runs legacy field migration, but it avoids full typed rule-object construction for list display.
- This reduces list-load and frontend array churn, but real source searches still depend on upstream network latency, rule complexity, and configured source concurrency.
- Android real-device large-pack import/search stress testing is still required.
