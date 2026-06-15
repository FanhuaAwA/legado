# PERF-2026-06-15-LEGACY-IMPORT-URL-PROGRESS

## Scope

Continue performance and UX maintenance for large source import flows. This gate covers URL-based open-reading/Legado JSON imports and Route B import progress propagation.

Out of scope: source rule execution semantics, import result DTO shape, file naming, source compatibility migrations, release signing material, or third-party/private source samples.

## Review Finding

Text imports already supported `requestId` and `booksource:import-progress`, but URL imports downloaded the remote JSON and then called the no-progress text import path. Route B WS routing for text/url import also used the no-progress compatibility path.

For large source packs, that left some callers waiting for a final result without stage or batch feedback.

## Changes

- Added `ReaderCore::import_legacy_json_url_with_progress()`.
- URL imports emit an initial progress event before download and then reuse `import_legacy_json_text_with_progress()` for batch import progress.
- Extracted `emit_legacy_import_progress()` in Tauri source commands so text/url IPC paths share the same event payload.
- Added optional `requestId` support to `booksource_import_legacy_json_url`.
- Updated WS router text/url import routes to parse optional `requestId` and emit `booksource:import-progress`.
- Updated frontend `importLegacyJsonUrl()` to accept optional `requestId`.
- Updated `docs/reader-rust-route-b-spec.md` with the URL `requestId?` parameter and import progress event contract.
- Added regression coverage for URL import progress and WS router requestId parsing.

## Verification

- `cmd /c pnpm.cmd lint` - PASS
- `cmd /c pnpm.cmd build` - PASS (existing vconsole eval, large chunk, and plugin timing warnings only)
- `cargo fmt --all -- --check` - PASS
- `cargo check -p reader-core` - PASS
- `cargo check -p legado-tauri` - PASS
- `cargo check -p legado-headless` - PASS
- `cargo test -p reader-core import_legacy_json_url_reports_progress -- --nocapture` - PASS
- `cargo test -p legado-tauri --test ws_router booksource_import_legacy_json_text_accepts_request_id_in_ws_router -- --nocapture` - PASS
- `node scripts/ci/check-command-contract.mjs --json` - PASS
- `git diff --check` - PASS

## Residual Risk

- The import progress DTO does not include a separate download byte counter; URL imports emit an initial progress event before download and detailed batch progress after JSON text is available.
- The existing InstalledSourcesTab URL flow resolves URL content on the frontend and already uses text import progress; the new URL command progress primarily benefits direct IPC/Route B callers and future UI paths.
- Android real-device large-pack import/search stress testing is still required.
