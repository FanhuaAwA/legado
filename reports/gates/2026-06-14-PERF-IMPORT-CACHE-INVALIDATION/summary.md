# Gate Summary

Task ID: `PERF-2026-06-14-IMPORT-CACHE-INVALIDATION`

Scope: reduce repeated source-list cache invalidation during bulk Legado/article JSON imports. This round does not change source parsing, file names, DB save semantics, or private source samples.

## Changes

- `import_legacy_json_text()` invalidates the source-list cache once before bulk writes.
- Added `persist_legado_source_without_cache_invalidation()` for batch import writes.
- Kept `persist_legado_source()` invalidating cache for normal single-source writes.
- Removed per-item cache invalidation from Legado and article JSON import loops.

## Gates

- `cargo fmt --all -- --check`：PASS
- `cargo test -p reader-core stream_sources_emits_incremental_batches_with_capabilities -- --nocapture`：PASS
- `cargo check -p reader-core`：PASS
- `cargo check -p legado-tauri`：PASS

## Residual Risk

- DB upsert and pretty JSON file writes are still per source. Very large subscription imports may still benefit from a future batch transaction/import progress pass.
