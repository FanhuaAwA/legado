# PERF-2026-06-15-SOURCE-RELOAD-COALESCE

## Scope

Continue performance maintenance for large source-list import/reload flows. This gate covers the self-review finding that keep-alive BookSource/Search/Explore views can repeatedly force reload the same source list after one source change.

Out of scope: backend command contracts, source rule execution semantics, result DTO shapes, search concurrency/timeout settings, release signing material, or third-party/private source samples.

## Review Finding

Visited views remain mounted through the app keep-alive pattern, so BookSource, Search, and Explore event listeners can all respond to source reload events.

Before this change, BookSourceView handled a child reload by forcing its own source-list load and only then broadcasting `app:booksource-reload`. Search and Explore then marked sources stale and started another force load. InstalledSourcesTab also emitted reload and directly broadcast some app reload events, doubling those notifications.

`bookSourceStore.reloadSources()` also cleared `_loadInFlight`, which could bypass the store's existing single-flight guard.

## Changes

- Changed InstalledSourcesTab reload events to carry optional `scope`, `fileName`, and `sourceDir` payloads.
- Removed direct `app:booksource-reload` broadcasts from InstalledSourcesTab; BookSourceView now owns cross-view reload broadcast.
- BookSourceView now invalidates capability cache, marks sources stale, starts the force load, then emits `app:booksource-reload` with `refreshStarted: true`.
- SearchView and ExploreView detect `refreshStarted` and avoid starting a second force reload; they join the in-flight store load or return from a fresh cache.
- ExploreView still clears explore cache and bumps section refresh versions where needed.
- `reloadSources()` no longer clears `_loadInFlight`.
- Single-source reload paths prefer `sourceDir::fileName` capability invalidation when `sourceDir` is available.

## Verification

- `cmd /c pnpm.cmd lint` - PASS
- `cmd /c pnpm.cmd build` - PASS (existing vconsole eval, large chunk, and plugin timing warnings only)
- `git diff --check` - PASS

## Residual Risk

- Backend `booksource:changed` events are still handled by multiple mounted views; this change targets app-level reload broadcasts and store in-flight reuse, not a full event-bus debounce.
- Explore cache clearing still follows the existing backend API shape, which clears by fileName rather than sourceDir.
- Android real-device large-pack import/search stress testing is still required.
