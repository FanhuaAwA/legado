# PERF-2026-06-15-JS-PREFETCH-CANCEL-COOPERATIVE

## Scope

Continue cancellation and performance maintenance for background bookshelf chapter prefetch. This gate covers propagating the prefetch task token into per-chapter JS content execution and making retry/throttle waits cancellation-aware.

Out of scope: changing prefetch progress payloads, content cache layout, HTTP timeout configuration, source rule semantics, or third-party/private source samples.

## Review Finding

Prefetch checked cancellation between chapters, but each chapter used plain `chapter_content()`. After a cancellation arrived during JS content execution, the error could be caught by retry handling and followed by a fixed retry backoff sleep. The inter-chapter throttle also used a fixed sleep.

## Changes

- Prefetch now calls `chapter_content_with_cancel()` with the active prefetch token.
- Added `cancellable_sleep()` for retry backoff and inter-chapter throttling.
- Prefetch returns cancellation immediately when content fetch fails after the token is set.
- Added a regression test for cancelling a JS-backed prefetch content fetch before the engine timeout.

## Verification

- `cargo fmt --all -- --check` - PASS
- `cargo test -p reader-core cancel_token_interrupts -- --nocapture` - PASS
- `cargo check -p reader-core` - PASS
- `cargo check -p legado-tauri` - PASS
- `cmd /c pnpm.cmd lint` - PASS
- `node scripts/ci/check-command-contract.mjs --json` - PASS
- `git diff --check` - PASS

## Residual Risk

- Already in-flight `reqwest::blocking` requests cannot be forcibly aborted by the cooperative token and still return on the configured request timeout.
- This gate covers the prefetch path after shelf chapters are already known; it does not change chapter discovery or source rule semantics.
