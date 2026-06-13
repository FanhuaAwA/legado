# PERF-2026-06-13-TRANSPORT-LAZY

## Scope

- Converted `useTransport` callers from static imports to dynamic imports at invoke/listen/mounted/action boundaries.
- Updated `useInvoke`, `useEventBus`, `useFrontendStorage`, app config/script bridge stores, bookshelf startup, settings panels, and WS connection dialog.
- Did not change backend command contracts, reader-core behavior, WS protocol payloads, dependency versions, or release artifacts.

## Result

- `useTransport` is now an async chunk: `useTransport-BKg5SsZx.js` 7.21 kB, gzip 2.75 kB.
- `dist/index.html` modulepreload links: 5 -> 4 after the previous preload-prune iteration.
- `useTransport` is no longer preloaded by the production HTML entry.
- Vite no longer reports `useTransport` ineffective dynamic import.
- Entry chunk after build: `index-C-PUgMzD.js` 65.87 kB, gzip 22.66 kB.

## Gates

- `cmd /c pnpm.cmd lint`: PASS.
- `cmd /c pnpm.cmd build`: PASS.
- `node scripts\ci\check-command-contract.mjs --json`: PASS, `frontendTotal=162`, `registeredTotal=161`, `bothCount=161`, `onlyFrontend=["js_eval"]`, `onlyBackend=[]`, `frontend_unsupported_stub_count=39`.
- `cargo fmt --all -- --check`: PASS.
- `cargo check -p reader-core`: PASS.
- `cargo check -p legado-tauri`: PASS.
- `cargo test -p reader-core`: PASS.
- `git diff --check`: PASS, with Windows LF/CRLF working-tree notice only.

## Remaining Risk

- First transport use now pays one async chunk load, intentionally trading eager startup dependency weight for demand loading.
- `vendor-vue-naive` and `_plugin-vue_export-helper` remain the dominant chunks.
- Existing Vite warnings remain for `vconsole` direct eval and large chunks.
