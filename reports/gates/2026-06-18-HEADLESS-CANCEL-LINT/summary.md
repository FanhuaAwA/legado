# 2026-06-18 HEADLESS-CANCEL-LINT

## Scope

- Close the browser/headless gap for `booksource_cancel`.
- Normalize user-initiated cancellation errors so cancelled JS work is not surfaced as a script exception.
- Resolve the repository-wide `oxfmt --check .` baseline failure on Windows.

## Changes

- Added a headless `TaskRegistry` and routed `booksource_cancel`.
- Wired headless `booksource_search`, `booksource_chapter_list`, and `booksource_chapter_content` through `*_with_cancel`.
- Added cancellation result normalization in headless dispatch.
- Added the same cancellation normalization for Tauri source commands and bookshelf prefetch.
- Added `.gitattributes` LF rules for source/document files so `oxfmt` and Windows Git line endings no longer conflict.

## Verification

```powershell
cargo fmt --all
cargo test -p legado-headless -- --nocapture
cargo test -p legado-tauri state::tests:: -- --nocapture
cargo test -p legado-tauri --test ws_router booksource_search_accepts_task_id_in_ws_router -- --nocapture
cargo test -p legado-tauri --test ws_router capabilities_get_returns_map -- --nocapture
cargo check -p legado-headless
cargo check -p legado-tauri
node scripts\ci\check-command-contract.mjs --json
cmd /c pnpm.cmd exec oxfmt --check .
cmd /c pnpm.cmd lint
cmd /c pnpm.cmd build
```

## Runtime Smoke

- Started `legado-headless` on `127.0.0.1:7790` with isolated data dir `reader-data/codex-cancel-smoke`.
- Opened `http://127.0.0.1:7790/?ws=ws://127.0.0.1:7790/ws` through Playwright CLI.
- Snapshot showed the bookshelf first screen in `headless` mode.
- Browser console reported `Errors: 0, Warnings: 0` and `WebSocket 已连接`.

## Result

- Headless browser mode can now cancel long source tasks using the same `booksource_cancel` command shape as Tauri.
- User cancellation is reported as `CANCELLED` instead of leaking `JS Exception: interrupted`.
- Full `pnpm lint` now passes; the prior `oxfmt --check .` repository baseline blocker is removed.

## Remaining Work

- `booksource_list_streaming` still has no explicit cancel token because it is short, batched metadata scanning.
- `booksource_run_tests` still runs under its timeout model and is not wired to `booksource_cancel`.
- Desktop dev-server/manual Tauri window testing remains separate from this headless browser smoke.
