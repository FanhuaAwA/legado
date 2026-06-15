# PERF-2026-06-15-SEARCH-AGGREGATE-INCREMENTAL

## Scope

Continue performance maintenance for large-source search flows. This gate covers the self-review finding that aggregated search repeatedly flattened, grouped, rescored, and sorted all results as each source returned.

Out of scope: backend command contracts, source rule execution semantics, result DTO shapes, search concurrency/timeout settings, release signing material, or third-party/private source samples.

## Review Finding

`SearchView.vue` previously built `aggregatedTaggedResults` as a computed value by scanning every active source and every current source result. `AggregatedSearchResults.vue` then grouped and sorted that full flattened array again.

During large searches, source results arrive incrementally. Recomputing the whole flattened result list and all aggregate groups for every returning source can produce visible main-thread churn, especially after importing large source packs.

Search progress counters also used separate computed scans over the active source list, adding more repeated work during the same updates.

## Changes

- Added `src/utils/searchAggregation.ts` for reusable Dice/bigram similarity, same-book matching, incremental group insertion, sorting, and one-shot compatibility aggregation.
- Updated `AggregatedSearchResults.vue` to accept optional precomputed `groups` while preserving the existing `results` fallback.
- Updated `SearchView.vue` to append each source's returned results into an incremental `aggregatedGroupBuffer`.
- Batched aggregate group publishing with `requestAnimationFrame` or a 16 ms timeout fallback.
- Replaced repeated computed scans for completed sources, raw result count, and sources-with-results count with per-source Set/Map counters.
- Preserved current-scope `hasSearched` behavior when users switch to a source that did not participate in the latest search run.

## Verification

- `cmd /c pnpm.cmd lint` - PASS
- `cmd /c pnpm.cmd build` - PASS (existing vconsole eval, large chunk, and plugin timing warnings only)
- `git diff --check` - PASS

## Residual Risk

- Incremental grouping still compares each new result with existing aggregate groups; this removes repeated full-history regrouping but is not yet an indexed grouping algorithm.
- The backend JS search cancellation limitation remains a separate hotspot for slow or stuck JS sources.
- Android real-device large-pack import/search stress testing is still required.
