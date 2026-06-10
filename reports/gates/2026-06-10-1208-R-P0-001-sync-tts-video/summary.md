# R-P0-001 sync / TTS / video proxy gate summary

Time: 2026-06-10 12:08 +0800

## Scope

- Added backend `capabilities_get` as the centralized capability declaration source for sync, native TTS, and local video proxy.
- Added frontend `useCapabilities`.
- Routed sync settings and `useSync` command calls through the capability declaration.
- Routed native `tts_*` calls through the capability declaration and downgraded to browser speech when native TTS is unsupported.
- Routed `start_video_proxy` / `stop_video_proxy` through the capability declaration and surfaced the unsupported reason in the player.
- Updated status docs with R-P0-001 first-batch state: 22/58 frontend-facing unsupported stubs handled, 36 remaining.

## Gates

```text
pnpm exec oxfmt --check .                                      PASS (371 files)
node scripts/ci/check-command-contract.mjs --json              PASS
cargo check -p legado-tauri                                    PASS
pnpm lint                                                       PASS (71 warnings / 0 errors)
pnpm build                                                      PASS
```

Contract snapshot:

```text
frontendTotal = 161
registeredTotal = 163
bothCount = 160
onlyFrontend = js_eval
onlyBackend = bookshelf_export_book_data, sync_baidu_start_auth, sync_baidu_token_status
registered_unsupported_stub_count = 60
registered_implemented_count = 103
frontend_unsupported_stub_count = 58
frontend_implemented_count = 102
```

## Remaining R-P0-001 queue

- browser_probe: 12 pending_ui_hidden
- comic_cover: 9 pending_ui_hidden
- repository/source_update: 6 pending_ui_hidden
- update/unlock/misc: 9 pending_ui_hidden

R-P0-001 remains open until all 58 frontend-facing stub entries are implemented, `unsupported_hidden`, or `blocked_by_platform`.
