# PERF-2026-06-15-JS-SEARCH-CANCEL-COOPERATIVE

## Scope

Continue performance maintenance for large source-dependent search flows. This gate covers cooperative cancellation propagation from the Tauri `booksource_search` task token into reader-core JS source execution.

Out of scope: search result shape, source compatibility migrations, command contract changes, HTTP timeout configuration, Android release signing, or third-party/private source samples.

## Review Finding

`booksource_search` could return `CANCELLED` to the frontend while JS source execution kept running in `spawn_blocking()`. Dropping the async future does not stop an already-started blocking worker, so runaway source JS or queued JS HTTP work could continue after the user stopped a large search.

## Changes

- Added `ReaderCore::search_with_cancel()` and kept `ReaderCore::search()` as the compatible no-token wrapper.
- Added `JsSourceRuntime::search_with_cancel()` and wrapped JS source search evaluation with a thread-local cancellation token.
- Extended the QuickJS interrupt handler to observe the active cancellation token in addition to the existing engine timeout deadline.
- Added cancellation checks before JS HTTP blocking sends and while waiting for same-host rate limiting.
- Updated Tauri `booksource_search` to pass the registered task token into reader-core while preserving the existing command signature and `CANCELLED` behavior.
- Added a regression test for cancelling a runaway JS search before the engine timeout.

## Verification

- `cargo fmt --all -- --check` - PASS
- `cargo test -p reader-core js_search_cancel_token_interrupts_runaway_source -- --nocapture` - PASS
- `cargo check -p reader-core` - PASS
- `cargo check -p legado-tauri` - PASS
- `cmd /c pnpm.cmd lint` - PASS
- `node scripts/ci/check-command-contract.mjs --json` - PASS
- `git diff --check` - PASS

## Residual Risk

- Already in-flight `reqwest::blocking` requests cannot be forcibly aborted by this cooperative token and still return on the configured request timeout.
- Cancellation is currently propagated for JS search execution. Other source-dependent commands with task IDs, such as chapter list/content, can adopt the same pattern in a later maintenance round.
