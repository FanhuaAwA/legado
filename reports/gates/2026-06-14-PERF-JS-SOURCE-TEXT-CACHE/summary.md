# PERF-2026-06-14-JS-SOURCE-TEXT-CACHE Gate Summary

Date: 2026-06-14

Scope:

- Reduce repeated disk reads on JS source search/detail paths after large source-list scans.
- Keep JS source behavior, rule execution, and result shape unchanged.
- Avoid caching imported Legado JSON payloads during large import batches.

Changes:

- Added a `ReaderCore` source text cache keyed by resolved path and validated with file mtime, size, and TTL.
- Seeded the cache from JS source list scanning and streaming list loading.
- Updated `read_source()` to reuse valid cached text before reading from disk.
- Synchronized cache updates on JS source save/toggle/delete and cleared the cache on external source directory changes.
- Kept Legado JSON writes as cache invalidation only, so large Legado imports do not retain every JSON file in memory.
- Added a regression test proving a cached JS source is refreshed after an external file update.

Commands:

- `cargo test -p reader-core js_source_text_cache_refreshes_after_external_file_change -- --nocapture`: PASS
- `cargo fmt --all -- --check`: PASS
- `cmd /c pnpm.cmd lint`: PASS
- `cargo check -p reader-core`: PASS
- `cargo check -p legado-tauri`: PASS
- `node scripts/ci/check-command-contract.mjs --json`: PASS
- `git diff --check`: PASS

Notes:

- This improves the common sequence "load a large JS source list, then search" by reusing the source text already read during list metadata extraction.
- The cache is intentionally correctness-first: external file edits are detected by mtime/size before returning cached text.
- Remaining search latency can still come from JS execution, source network requests, upstream throttling, and timeout behavior.
