# PERF-2026-06-15-SEARCH-GROUPED-LAZY-RENDER

## Scope

Continue search-page performance maintenance for large source libraries. This gate covers grouped-mode rendering in `SearchView.vue`.

Out of scope: backend search command behavior, search result DTOs, source execution semantics, aggregated mode ranking, or third-party/private source samples.

## Review Finding

Grouped mode rendered `SourceSearchGroup` for every active source. With large imported source packs, switching to grouped mode after a search could create hundreds or thousands of group components in one render pass, including empty groups. This can feel like a search stall even after backend streaming/cancellation improvements.

## Changes

- Added grouped-mode visible source batching with an initial limit of 48 groups.
- Added 48-group load-more increments for remaining grouped sources.
- Prioritized sources with loading state, errors, or results before idle empty groups.
- Reset the grouped visible limit when a new search starts.

## Verification

- `cmd /c pnpm.cmd lint` - PASS
- `git diff --check` - PASS

## Residual Risk

- This reduces initial DOM creation but is not full virtual scrolling.
- Extremely large grouped-mode inspection can still benefit from a future viewport virtualization pass.
