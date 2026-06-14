# Gate Summary

Task ID: `PERF-2026-06-14-SEARCH-STREAM-QUEUE`

Scope: continue large-source performance work on the frontend search page. This round changes only search-page scheduling; backend `booksource_search` behavior, source protocol semantics, user concurrency settings, and private source samples are unchanged.

## Changes

- Replaced the fixed search snapshot in `SearchView.vue` with a dynamic `SearchRun` queue.
- Search starts with currently streamed searchable sources and keeps enqueueing newly arrived `activeSources` while the source list is still loading.
- Existing user search concurrency settings continue to control worker count.
- Stop search invalidates the active run token so late responses do not write into the current UI.
- Limited single-source searches finish after that source completes instead of waiting for the whole source list to finish streaming.

## Gates

- `cmd /c node_modules\.bin\oxfmt.cmd src\views\SearchView.vue`：PASS
- `cmd /c pnpm.cmd lint`：PASS
- `cmd /c pnpm.cmd build`：PASS
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
- `git diff --check` only reports normal Windows LF/CRLF working-tree notices.

## Residual Risk

- Search cancellation is still UI-token based. Requests already running in the backend/network continue until their existing timeout.
- A backend search progress/cancel task model remains the next performance target.
