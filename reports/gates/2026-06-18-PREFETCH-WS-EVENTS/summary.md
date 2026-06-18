# 2026-06-18 Prefetch WS Events Gate

## Scope

Close R-P2-012 for reader chapter prefetch progress in WebSocket deployments. The target behavior is that Tauri IPC, Tauri built-in WS, and `legado-headless` WS can all execute `bookshelf_prefetch_chapters` and deliver `shelf:prefetch-progress` plus `shelf:prefetch-done` events to the frontend event layer.

## Changes

- Extracted a Tauri prefetch helper that emits progress and done events, then reused it from both the IPC command and WS router.
- Made the Tauri WS router accept both `{ payload: ... }` and direct prefetch payloads for `bookshelf_prefetch_chapters`.
- Added `bookshelf_prefetch_chapters` to `legado-headless` dispatch.
- Headless prefetch now registers its `taskId` in the shared task registry, so `booksource_cancel` can cancel active prefetch work.
- Headless prefetch emits `shelf:prefetch-progress` and `shelf:prefetch-done` events over the same WS event protocol used by the frontend.
- Updated the frontend/backend separation document to mark R-P2-012 as fixed.

## Validation

```powershell
cargo fmt --all
cargo test -p legado-tauri --test ws_router bookshelf_prefetch_accepts_direct_payload_and_emits_done -- --nocapture
cargo test -p legado-headless bookshelf_prefetch -- --nocapture
cargo test -p legado-headless -- --nocapture
cargo test -p legado-tauri --test ws_router -- --nocapture
cargo check -p legado-tauri
cargo check -p legado-headless
node scripts\ci\check-command-contract.mjs --json
cmd /c pnpm.cmd lint
cargo build -p legado-headless
```

All commands passed. The Tauri test run still prints the known Windows linker stdout warning while passing.

## Browser Smoke

- Started `legado-headless` on `127.0.0.1:7795` with isolated data under `reader-data\codex-prefetch-smoke`.
- Opened `http://127.0.0.1:7795/?ws=ws://127.0.0.1:7795/ws` via Playwright CLI.
- From the page context, used the real WS protocol to:
  - save a JS source with `chapterContent`;
  - add a shelf book;
  - save one cached chapter entry;
  - invoke `bookshelf_prefetch_chapters`;
  - verify `bookshelf_get_content` returned the prefetched chapter body.
- Smoke result: `fetched=1`, cached content length `93`, received `shelf:prefetch-progress` before `shelf:prefetch-done`.
- Console result: `Errors: 0, Warnings: 0`.

## Remaining Risk

This gate verifies the protocol and local JS-source prefetch path. It does not claim CDN or remote source performance for every real-world book source; those remain covered by source-specific compatibility and freshness gates.
