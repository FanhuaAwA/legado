# PERF-2026-06-14-IMPORT-BATCH-UPSERT Gate Summary

Date: 2026-06-14

Scope:

- Batch SQLite upserts for Legado JSON import without changing single-source save semantics.
- Keep per-source Legado JSON file writes and filenames unchanged.
- Reuse batch save behavior for default-source copy paths.
- Verify imported sources are still visible through `list_sources()`.

Commands:

- `cargo test -p reader-core import_legacy_json_text_reports_progress_batches -- --nocapture`：PASS
- `cmd /c pnpm.cmd lint`：PASS
- `cargo fmt --all -- --check`：PASS
- `node scripts/ci/check-command-contract.mjs --json`：PASS
- `cargo check -p reader-core`：PASS
- `cargo check -p legado-tauri`：PASS
- `git diff --check`：PASS

Notes:

- The focused import progress test now also asserts that all 30 imported sources are persisted and returned by `list_sources()`.
- This is not a formal benchmark, but the focused test completed faster locally after reducing per-source DB upserts.
- Android true-device import pressure testing is still pending.
