# 2026-06-18 Headless WebDAV Sync Gate

## Scope

Expose the existing `reader-core` WebDAV sync facade through `legado-headless` so browser/WebSocket mode has the same sync command surface as Tauri for credentials, status, manual sync, conflict handling, lifecycle events, client state, and reader-session reporting.

## Changes

- Added headless dispatch routes for `sync_set_credentials`, `sync_get_credentials`, `sync_clear_credentials`, `sync_get_status`, `sync_now`, `sync_test_connection`, `sync_list_conflicts`, `sync_resolve_conflict`, `sync_notify_lifecycle`, `sync_client_state_set`, `sync_report_reader_session`, and `sync_v2_sync_reading_progress`.
- Marked `capabilities_get.syncWebdav` as supported in headless.
- Emitted `sync:client-state` events from headless `sync_now` and `sync_resolve_conflict` when core returns client-state updates.
- Added headless tests for WebDAV sync capability and local command dispatch.
- Hardened headless test data directories with an atomic suffix to avoid parallel migration collisions on Windows.

## Validation

```powershell
cargo fmt --all
cargo test -p legado-headless sync_webdav -- --nocapture
cargo test -p legado-headless -- --nocapture
cargo check -p legado-headless
cargo check -p legado-tauri
node scripts\ci\check-command-contract.mjs --json
cmd /c pnpm.cmd lint
cargo build -p legado-headless
```

All commands passed.

## Browser Smoke

- Started `legado-headless` on `127.0.0.1:7793` with isolated data under `reader-data\codex-sync-smoke`.
- Opened `http://127.0.0.1:7793/?ws=ws://127.0.0.1:7793/ws` via Playwright CLI.
- Navigated to Settings -> Sync.
- Confirmed WebDAV fields and action buttons for connection test, immediate sync, pull-only, and push-only are enabled.
- Saved a temporary credential through the UI.
- Console result: `Errors: 0, Warnings: 0`.

## Remaining Risk

This gate verifies command routing, capability state, UI availability, and local credential/status paths. A real WebDAV server round-trip remains covered by `reader-core` tests and should be repeated through headless when a stable external or fixture-backed WebDAV endpoint is needed for full browser-mode sync acceptance.
