# Gate Summary

Task ID: `PERF-2026-06-14-BOOKSOURCE-LAZY-LIST`

Scope: optimize large book-source loading and list-dependent filtering without source-specific special cases. This round keeps book-source protocol semantics unchanged and does not touch private source samples.

## Changes

- Added cached and truly incremental `ReaderCore::stream_sources`, shared by Tauri IPC, Route B WebSocket router, and headless WebSocket dispatcher.
- Added `BookSourceMeta.capabilities` so the frontend can seed capability caches from backend metadata instead of re-detecting every source before search/explore filtering.
- Changed the book-source Pinia store to merge streamed batches immediately and prune stale entries only when the final `done` event arrives.
- Invalidated the source-list cache from write paths that can alter visible sources.
- Formatted `.cargo/config.toml` to satisfy the existing `pnpm lint` / `oxfmt --check .` baseline; no behavior change.

## Gates

- `cargo fmt --all -- --check`：PASS
- `cargo check -p reader-core`：PASS
- `cargo check -p legado-tauri`：PASS
- `cargo check -p legado-headless`：PASS
- `cargo test -p reader-core stream_sources_emits_incremental_batches_with_capabilities -- --nocapture`：PASS
- `cargo test -p legado-tauri booksource_list_streaming_is_routed -- --nocapture`：PASS
- `cmd /c pnpm.cmd lint`：PASS
- `cmd /c pnpm.cmd build`：PASS
- `cargo test -p reader-core`：PASS
- `cargo test -p legado-tauri`：PASS
- `cargo test -p legado-headless`：PASS
- `node scripts/ci/check-command-contract.mjs --json`：PASS
- `git diff --check`：PASS

Command contract:

```json
{
  "frontendTotal": 162,
  "registeredTotal": 161,
  "bothCount": 161,
  "onlyFrontend": ["js_eval"],
  "onlyBackend": [],
  "frontend_unsupported_stub_count": 39,
  "frontend_implemented_count": 122
}
```

## Notes

- `pnpm build` still reports existing warnings for `vconsole` direct eval, large chunks, and plugin timing.
- `cargo test -p legado-tauri` still prints the existing MSVC linker stdout warning.
- `git diff --check` only reports normal Windows LF/CRLF working-tree notices.

## Residual Risk

- This round optimizes source-list scan/render and capability filtering. Multi-source search execution still needs its own performance pass for progress events, cancellation, concurrency limits, and per-source timeout behavior.
- Android large-source import/search behavior still needs device validation.
