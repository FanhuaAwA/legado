# PERF-2026-06-13-MODULEPRELOAD-PRUNE

## Scope

- Changed `vite.config.ts` only for build-time module preload policy.
- Updated AI status/iteration docs and this gate report.
- Did not change frontend business behavior, backend command contracts, reader-core logic, dependency versions, or release artifacts.

## Result

- `dist/index.html` modulepreload links: 20 -> 5.
- Kept eager HTML preloads for core entry dependencies: `rolldown-runtime`, `vendor-vue-naive`, `_plugin-vue_export-helper`, `useTransport`, `useInvoke`.
- JS dynamic import preload dependencies are no longer expanded into the full JS dependency chain; generated `__vite__mapDeps` now keeps async component CSS dependencies.
- Entry chunk after build: `index-BjH9Vjka.js` 65.83 kB, gzip 22.66 kB.

## Gates

- `cmd /c pnpm.cmd build`: PASS.
- `cmd /c pnpm.cmd lint`: PASS.
- `node scripts\ci\check-command-contract.mjs --json`: PASS, `frontendTotal=162`, `registeredTotal=161`, `bothCount=161`, `onlyFrontend=["js_eval"]`, `onlyBackend=[]`, `frontend_unsupported_stub_count=39`.
- `cargo fmt --all -- --check`: PASS.
- `cargo check -p reader-core`: PASS.
- `cargo check -p legado-tauri`: PASS.
- `cargo test -p reader-core`: PASS.
- `git diff --check`: PASS, with Windows LF/CRLF working-tree notice only.

## Remaining Risk

- This is a preload/network-fanout reduction, not a full bundle-size fix.
- `vendor-vue-naive` and `_plugin-vue_export-helper` remain the largest chunks and need separate dependency-chain work.
- Existing Vite warnings remain: `vconsole` direct eval, large chunks, and `useTransport` ineffective dynamic import.
