# PERF-2026-06-14-IMPORT-PROGRESS-EVENTS Gate Summary

Date: 2026-06-14

Scope:
- Add reader-core progress callbacks for Legado JSON import without changing existing import semantics.
- Emit Tauri IPC progress events for large open-source Reading/Legado source imports.
- Show per-request progress in the installed sources tab during URL/file imports.
- Keep Route B/headless import command compatible with the existing no-progress return contract.

Commands:
- `cargo test -p reader-core import_legacy_json_text_reports_progress_batches -- --nocapture`：PASS
- `cmd /c pnpm.cmd lint`：PASS
- `cargo fmt --all -- --check`：PASS
- `node scripts/ci/check-command-contract.mjs --json`：PASS
- `cmd /c pnpm.cmd build`：PASS
- `cargo check -p reader-core`：PASS
- `cargo check -p legado-tauri`：PASS
- `git diff --check`：PASS

Notes:
- `pnpm build` still reports the existing `vconsole` direct eval, large chunk, and plugin timing warnings.
- `git diff --check` only reports the normal Windows LF/CRLF working-tree warning.
- Android true-device import pressure testing is still pending.
