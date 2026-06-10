# R-P2-002 lint warnings gate report

Date: 2026-06-10 19:10 +0800

## Result

PASS. R-P2-002 is closed.

## Scope

- Classified and fixed existing `oxlint --type-aware --type-check` warnings.
- Preserved intentional dynamic execution boundaries for book-source syntax validation, legacy plugin compatibility, and opt-in custom JS injection with local `oxlint-disable-next-line` comments and reasons.
- Replaced unsafe/default object stringification with explicit primitive/object formatting helpers.
- Marked fire-and-forget promises with `void` and rejection handling where they are intentionally non-blocking.
- Replaced string spread on text with `Array.from`.

## Gate Commands

| Command | Result |
| --- | --- |
| `pnpm exec oxfmt .` | PASS |
| `pnpm exec oxfmt --check .` | PASS |
| `pnpm lint` | PASS, 0 warnings / 0 errors |
| `node scripts/ci/check-command-contract.mjs` | PASS, frontend 164 / registered 163 / onlyBackend 0 |
| `node scripts/ci/check-command-contract.mjs --json` | PASS, `frontend_unsupported_stub_count=60`, `frontend_implemented_count=103` |
| `pnpm build` | PASS |
| `cargo check -p reader-core` | PASS |
| `cargo check -p legado-tauri` | PASS |
| `cargo test -p reader-core` | PASS, 31 passed / 9 ignored |
| `git diff --check` | PASS |

Build note: `pnpm build` still reports existing Vite/Rolldown warnings from `vconsole` direct eval, large chunks, and an ineffective dynamic import. They are not `pnpm lint` diagnostics and were not introduced by R-P2-002.

## Next Queue Item

R-P2-003: 番茄书源 JS API 缺口。Start by listing the concrete missing `java.*` / `source.*` / device-registration runtime APIs and deciding implement-vs-degrade per API.
