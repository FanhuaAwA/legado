# 2026-06-18 BACKUP-HEADLESS-DATA

## Scope

- Make backup/export/import usable from browser + `legado-headless`.
- Remove stale backup logic that read the old `config/` directory after app settings moved into `reader.db/json_documents`.
- Keep desktop Tauri and headless routes on the same backup payload implementation.

## Changes

- Added `crates/reader-core/src/backup.rs` as the shared backup implementation.
- Moved Tauri `backup_*` commands to a thin router over `ReaderCore`.
- Added headless WS support for `backup_inspect`, `backup_create_data`, `backup_peek_data`, and `backup_restore_data`.
- Explicitly reject path-based `backup_create`, `backup_peek`, and `backup_restore` in headless mode to avoid remote server path read/write over WS.
- Updated `SectionBackup.vue` so browser/headless uses data-transfer export/download and file input import/peek/restore, while desktop keeps native path dialogs.
- Fixed browser/headless transport readiness by checking the active transport instead of the static native-transport flag.

## Verification

```powershell
cmd /c pnpm.cmd lint
cargo test -p legado-headless
cargo check -p legado-tauri
node scripts\ci\check-command-contract.mjs --json
cmd /c pnpm.cmd build
cargo build -p legado-headless
```

Results:

- `pnpm lint` passed with 0 warnings/errors.
- `cargo test -p legado-headless` passed: 8 tests, including backup data roundtrip.
- `cargo check -p legado-tauri` passed.
- Command contract unchanged: `frontendTotal=163`, `registeredTotal=162`, `onlyFrontend=["js_eval"]`, `onlyBackend=[]`, implemented `126`, unsupported stubs `36`.
- `pnpm build` passed with existing vconsole direct-eval and chunk-size warnings only.

## Browser Smoke

- Served rebuilt `dist` through `target/debug/legado-headless.exe` on `127.0.0.1:7797`.
- Opened `http://127.0.0.1:7797/?ws=ws://127.0.0.1:7797/ws`.
- Opened `设置 -> 备份与还原`.
- Confirmed backup categories render in browser/headless, including app settings from DB and bookshelf data.
- Exported selected categories to `legado-backup-20260618-235817.json`.
- Uploaded that JSON through the browser file chooser; preview rendered 4 categories.
- Confirmed restore dialog and executed `继续还原`.
- Final Playwright console check: `Errors: 0`, `Warnings: 0`; headless stderr log stayed empty.

## Remaining Risk

- Backup still intentionally restores by category merge, not full destructive replacement.
- Some categories remain metadata-only or future work (`reader_settings`, `sync_state`) until their storage ownership is defined.
