# 2026-06-19 Windows Startup And Source Stability Gate

## Scope

This iteration addresses the user-reported Windows startup crash and continues the source-heavy stability work:

- Repair the Windows desktop launch crash observed immediately after the app window appears.
- Stabilize frontend/backend source-list startup so large source libraries cannot miss the first streaming event or wait forever for the final batch.
- Reduce background source update checks so CDN-backed sources are not burst-requested after startup/list loading.
- Re-test the rebuilt Windows app with desktop control.
- Recheck source freshness and explicit source remarks/update hints with low-frequency network requests.

## Fixes

### Windows startup crash

Root cause:

- The installed user database had SQLx migration version 4 recorded with a legacy checksum.
- The current repository had the same migration version and description but a changed migration checksum.
- SQLx refused to start with `migration 4 was previously applied but has been modified`, and the Tauri setup hook panicked before the UI could remain open.

Changes:

- `crates/reader-core/src/storage/db/mod.rs` now runs a narrow legacy repair before `MIGRATOR.run()`.
- The repair only applies when `_sqlx_migrations` has version `4`, description `book source list index`, success `true`, and the stored checksum matches the known legacy checksum.
- It recreates `idx_book_sources_user_updated_url` idempotently and updates only that migration row to the current checksum.
- Any unknown checksum is left untouched so real migration drift is not hidden.

Regression:

- `crates/reader-core/tests/db_migrations.rs` seeds a temporary DB with the legacy checksum and proves `init_pool()` repairs it and preserves the source-list index.

### Source-list load stability

Changes:

- `src/stores/bookSource.ts` now awaits `eventListen()` registration before calling `listBookSourcesStreaming()`, preventing the frontend from missing the first or final streaming batch.
- The streaming path has a unified cleanup/settlement function and an 80s timeout, so a missing final `done` event no longer leaves the UI in an indefinite loading state.
- The unsupported-streaming fallback still loads through `listBookSources()` and performs the same merge/prune/sort cleanup.

### CDN-friendly update checks

Changes:

- Background `@updateUrl` checks now run with concurrency `1` and a 1200ms pause between checks.
- Capability detection keeps the existing lower-latency batching because it is local metadata/cache maintenance and is not the same CDN update path.

This matches the user's observation that short bursts against CDN/proxy endpoints can trigger anti-DDoS throttling or temporary blacklisting.

## Windows Desktop Smoke

Rebuilt release artifact:

```text
E:\Book\Legado-Tauri-main\构建结果\windows\legado-tauri.exe
```

Observed with Windows desktop control:

| Area | Result |
| --- | --- |
| Cold launch | Window appeared in 916ms; UI ready in 2318ms; stable after 5s; no immediate crash. |
| Main UI | Title `开源阅读`; evidence included bookshelf, discover, search, source management, frontend `v0.9.0`, Windows. |
| Source management | Ready in 2250ms; loaded `1068` sources; `1034` enabled; visible controls for edit/reload/debug. |
| Source filter | Real keyboard input filter for `QB` returned matching rows in 564ms without crashing. |
| Search page | Search view opened and showed `全部书源（988）`; app stayed stable. |
| Discover page | Opened in 766ms; showed `957 个发现源` and 23 visible source rows. |

Scroll note:

- Automated wheel/PageDown gestures did not produce a provable accessibility-tree change in the source/discover scroll containers.
- The pages loaded and remained stable, but this gate does not claim scroll-state verification passed. A future pass should add a UI-level scroll smoke that can read the actual container scroll offset or use a WebView-aware probe.

## Source Freshness And Remarks

Local metadata scan:

| Location | Files/items | Findings |
| --- | ---: | --- |
| `E:\Book\书旗书源` | 2 JSON files / 2 items | `lastUpdateTime` present; no explicit comments or update URLs. |
| `E:\Book\七猫书源` | 2 JSON files / 2 items | `lastUpdateTime` present; no explicit comments or update URLs. |
| `E:\Book\番茄书源` | 1 item | `lastUpdateTime` present; no explicit comments or update URLs. |
| `E:\Book\番茄短剧` | 1 item | Has comment; `lastUpdateTime=0`. |
| `E:\Book\猫公子书源` | 2 JSON files | Direct source parsing invalid because these are package/manifest style files, not single-source JSON files. |
| Installed `sources\legado-json` | 1076 items | 379 comments, 0 update URLs, 1073 `lastUpdateTime`, 1070 older than 180 days, 77 flagged remarks. |

Examples of user-visible remarks observed in the Windows app include sources marked as semi-broken, requiring login, server error, unavailable, or otherwise degraded. These are source-side freshness/availability signals and should remain visible to users rather than being treated as app crashes.

Low-frequency CDN checks, with pauses between requests:

| Source | URL status | SHA / match | Current conclusion |
| --- | --- | --- | --- |
| qimao | 200 | `902cd4f57...`; equals local `.backup.json`, not refreshed local `.json` | CDN is still stale backup-equivalent. |
| shuqi | 200 | `a46f80d86...`; equals local `.backup.json`, not refreshed local `.json` | CDN is still stale backup-equivalent. |
| fanqie | 200 | `59e47254a...`; equals local `.json` | CDN matches local. |
| fanqie short-drama | 404 | no local match | Network import entry is still broken. |

To avoid unnecessary CDN pressure, this pass did not run a broad live full-chain sweep across the installed 1000+ sources.

## Validation

```powershell
cmd /c pnpm.cmd exec vue-tsc -p tsconfig.app.json --noEmit
rustfmt --edition 2021 crates/reader-core/src/storage/db/mod.rs crates/reader-core/tests/db_migrations.rs
cargo test -p reader-core --test db_migrations -- --nocapture
cargo check -p reader-core
cargo check -p legado-tauri
cmd /c pnpm.cmd build:windows:release
cmd /c pnpm.cmd lint
git diff --check
```

All commands passed. The Windows release build still prints the known Windows linker stdout warning while succeeding.

## Residual Risk

- The legacy migration repair is intentionally narrow. Unknown checksum mismatches still fail migration startup and should be investigated instead of auto-repaired.
- Automated scroll proof remains inconclusive because the desktop accessibility tree did not expose a changed container state after wheel/PageDown gestures.
- Installed source freshness is mostly source-side: many installed sources are old, have no `updateUrl`, or carry comments indicating degraded availability.
- Large live source sweeps should stay rate-limited because multiple sources share CDN/proxy infrastructure.
