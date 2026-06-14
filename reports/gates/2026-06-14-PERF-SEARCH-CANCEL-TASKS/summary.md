# Gate Summary

Task ID: `PERF-2026-06-14-SEARCH-CANCEL-TASKS`

Scope: add cancellable single-source search tasks for large-source aggregate search. This round keeps search result semantics, source execution rules, user concurrency settings, and timeout settings unchanged.

## Changes

- Added optional `taskId` to `booksource_search` in Tauri IPC and Route B WebSocket routing.
- Registered search tasks in the existing `TaskRegistry` and raced `ReaderCore::search` against cancellation.
- Updated the frontend script bridge to pass `taskId` for searches.
- Updated `SearchView.vue` to generate one task ID per active source search and cancel active tasks on stop.
- Added a WS router regression test for `booksource_search` with `taskId`.
- Updated Route B command contract docs.

## Gates

- `cargo fmt --all -- --check`：PASS
- `cmd /c pnpm.cmd lint`：PASS
- `cargo test -p legado-tauri booksource_search_accepts_task_id_in_ws_router -- --nocapture`：PASS
- `node scripts/ci/check-command-contract.mjs --json`：PASS
- `cmd /c pnpm.cmd build`：PASS
- `cargo check -p legado-headless`：PASS
- `cargo check -p reader-core`：PASS
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

- `pnpm build` still reports existing warnings for `vconsole` direct eval and large chunks.
- `cargo test -p legado-tauri` still prints the existing MSVC linker stdout warning.
- `git diff --check` only reports normal Windows LF/CRLF working-tree notices.

## Residual Risk

- Async Legado/network searches can return early when the command future is cancelled.
- JS searches currently run through `spawn_blocking`; cancellation returns the command/UI early, but it cannot preempt JS already executing in the blocking worker.
