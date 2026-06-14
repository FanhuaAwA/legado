# Gate Summary

Task ID: `PERF-2026-06-14-BACKGROUND-SOURCE-MAINTENANCE`

Scope: reduce the background work spike after large book-source lists finish loading. This round keeps source execution semantics, import formats, search behavior, and third-party/private source samples unchanged.

## Changes

- Replaced the immediate post-load parallel maintenance triggers with a delayed, generation-checked background maintenance run.
- Preloaded persisted source capabilities before running missing capability detection.
- Kept existing capability/update concurrency limits but inserted a short pause between batches so the UI can render and handle input.
- Added in-flight de-duplication for update checks.
- Fixed stale update throttling so a fresh check is skipped even when the previous result found no pending updates.
- Passed `sourceDir` through update check/apply calls and book-source reload events to reduce ambiguity with multiple source directories.

## Gates

- `cmd /c pnpm.cmd lint`: PASS
- `cargo fmt --all -- --check`: PASS
- `cmd /c pnpm.cmd build`: PASS
- `node scripts/ci/check-command-contract.mjs --json`: PASS
- `git diff --check`: PASS
- `cargo check -p legado-tauri`: PASS
- `cargo check -p reader-core`: PASS

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
- `git diff --check` only reports normal Windows LF/CRLF working-tree notices.
- No private source sample directories were read or modified.

## Residual Risk

- This reduces post-load maintenance contention, but it is not a real Android device large-source stress test.
- JS searches running inside the blocking worker still cannot be preempted directly; they only benefit from command/UI-level cancellation added in the previous round.
- Further performance work should target JS runtime interruption, search/import progress events, and reader-core batch import transactions.
