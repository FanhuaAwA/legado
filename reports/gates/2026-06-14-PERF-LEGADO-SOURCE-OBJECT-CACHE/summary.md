# PERF-2026-06-14-LEGADO-SOURCE-OBJECT-CACHE

## Scope

Continue performance work for large imported/loaded book-source sets. This gate covers a reader-core optimization that caches parsed Legado `BookSource` objects after import/save and reuses them for source-dependent calls such as search, book info, TOC, and content.

Out of scope: changing Legado rule semantics, search result DTOs, file naming, DB upsert behavior, HTTP policy, release signing material, or third-party/private source samples.

## Changes

- Added `legado_source_cache` to `ReaderCore` with a 30 minute TTL.
- Cache entries store the parsed `BookSource`, file modified time, file size, and load time.
- `write_legado_source_file()` updates the parsed-source cache after writing `.legado.json`.
- `get_legado_source_by_file()` checks the cache before reading/parsing a source file, but only when current file metadata still matches the cached entry.
- `delete_source()` removes the corresponding Legado source cache entry.
- Added `legado_source_cache_refreshes_after_external_file_change` to verify external `.legado.json` edits refresh the cached source before the next search.

## Verification

- `cargo test -p reader-core legado_source_cache_refreshes_after_external_file_change -- --nocapture` - PASS
- `cargo fmt --all -- --check` - PASS
- `cmd /c pnpm.cmd lint` - PASS
- `cargo check -p reader-core` - PASS
- `cargo check -p legado-tauri` - PASS
- `cargo test -p reader-core` - PASS
- `node scripts/ci/check-command-contract.mjs --json` - PASS
- `git diff --check` - PASS

## Observed Command Contract

- `frontendTotal=162`
- `registeredTotal=161`
- `bothCount=161`
- `onlyBackendCount=0`
- `frontend_unsupported_stub_count=39`
- `frontend_implemented_count=122`

## Residual Risk

- This reduces repeated local file reads and JSON deserialization for Legado sources; it does not reduce upstream site latency or rule execution complexity.
- On a cold app start, the first call for a source still needs one metadata/read/parse pass.
- Real Android-device stress testing with large source packs is still required to measure import duration, first streamed list batch time, and multi-source search responsiveness.
