# PERF-2026-06-15-JS-CHAPTER-CANCEL-COOPERATIVE

## Scope

Continue performance maintenance for source-dependent operations after search. This gate covers cooperative cancellation propagation for Tauri `booksource_chapter_list` and `booksource_chapter_content` into reader-core JS source execution.

Out of scope: command contract changes, book detail cancellation, purchase flows, HTTP timeout configuration, source rule semantics, or third-party/private source samples.

## Review Finding

Chapter list and chapter content commands accepted `taskId`, but only checked the token before starting. If cancellation arrived while a JS-backed chapter list/content operation was running, the command did not return promptly and the blocking JS worker could continue until completion or engine timeout.

## Changes

- Added `ReaderCore::chapter_list_with_cancel()` and `ReaderCore::chapter_content_with_cancel()`.
- Kept `chapter_list()` and `chapter_content()` as compatible no-token wrappers.
- Added `JsSourceRuntime::chapter_list_with_cancel()` and `JsSourceRuntime::chapter_content_with_cancel()`.
- Updated Tauri chapter list/content commands to pass task tokens to reader-core and race active work against `wait_for_cancel()`.
- Reused the JS thread-local cancellation token, QuickJS interrupt polling, and JS HTTP preflight/rate-wait checks.
- Refactored runaway JS cancellation tests and added coverage for search, chapter list, and chapter content.

## Verification

- `cargo fmt --all -- --check` - PASS
- `cargo test -p reader-core cancel_token_interrupts_runaway_source -- --nocapture` - PASS
- `cargo check -p reader-core` - PASS
- `cargo check -p legado-tauri` - PASS
- `cmd /c pnpm.cmd lint` - PASS
- `node scripts/ci/check-command-contract.mjs --json` - PASS
- `git diff --check` - PASS

## Residual Risk

- Already in-flight `reqwest::blocking` requests cannot be forcibly aborted by the cooperative token and still return on the configured request timeout.
- `booksource_book_info` and purchase paths do not currently expose task IDs, so this gate focuses on the commands already wired for cancellation.
