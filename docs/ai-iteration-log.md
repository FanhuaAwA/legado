# AI Iteration Log

Last updated: 2026-06-19

本文件是当前迭代索引，不再保存完整对话式流水。2026-06-15 之前的长历史、旧续办提示和过期命令统计已清理；需要追溯请查看 git history 与 `reports/gates/*/summary.md`。

## 2026-06-19 Windows Startup And Source Stability Iteration

- Gate report: `reports/gates/2026-06-19-WINDOWS-STARTUP-SOURCE-STABILITY/summary.md`
- Main fixes: repair the known legacy SQLx migration-4 checksum that made Windows launch panic on existing databases, await frontend source-stream listener registration before starting backend streaming, add an 80s source-list final-batch timeout, and throttle background `@updateUrl` checks to one request at a time with 1200ms spacing.
- Windows desktop smoke: rebuilt release stayed open; cold launch window appeared in 916ms, UI was ready in 2318ms, source management loaded 1068 sources / 1034 enabled in 2250ms, source filtering responded in 564ms, and discover loaded 957 discover sources in 766ms. Automated scroll-state proof remained inconclusive and is recorded as residual risk, not a pass.
- Source freshness: qimao/shuqi CDN files still equal local `.backup.json` and differ from refreshed local `.json`; fanqie CDN matches local; fanqie short-drama CDN URL still returns 404. Installed source scan found 77 explicit degraded/expired-style remarks and no `updateUrl` values in the installed Legado JSON set.

## 2026-06-19 Repository Request Pacing Iteration

- Gate report: `reports/gates/2026-06-19-REPOSITORY-REQUEST-PACING/summary.md`
- Main fixes: online repository installed-source sync checks now run one at a time with 1200ms spacing; bulk repository install/update downloads run one at a time with 1500ms spacing; successful repository installs/updates mark the source synced without immediately re-downloading the same file for comparison; bulk update now emits one parent reload instead of reloading after every source.
- Validation: `vue-tsc`, targeted `oxfmt`, `git diff --check`, full `pnpm lint`, and `build:windows:release` passed. Windows desktop smoke launched the rebuilt client, opened source management with 1068 sources / 1034 enabled, and opened the online source tab in 517ms without UI lockup.

## 2026-06-18 Source Stability Iteration

- Gate report: `reports/gates/2026-06-18-PERF-SOURCE-STABILITY/summary.md`
- Main fixes: remove stdout network debug logging, add shared `52dns.cc` host pacing for CDN anti-DDoS friendliness, keep source search timeout/error paths cancelling backend tasks, scope inline reader chapter-list task cleanup, and index aggregate search groups for large result sets.
- Source freshness: superseded by the later `SOURCE-FRESHNESS-RECHECK` pass below. Current result: shuqi/qimao CDN copies are stale backup-equivalent files, fanqie CDN matches local, and fanqie short-drama CDN URL is 404.

## 2026-06-18 Cover Cache Stability Iteration

- Gate report: `reports/gates/2026-06-18-COVER-CACHE-STABILITY/summary.md`
- Main fixes: implement HTTP/HTTPS book-cover disk cache for Tauri and headless, coalesce concurrent same-URL cover downloads, enforce an 8MB streamed body limit, route cached cover files through `useFileSrc`, and protect headless `/asset` with the same token when configured.
- Command contract: `cover_resolve_cache` / `cover_cache_size` / `cover_cache_clear` moved from unsupported stub to implemented; contract snapshot is now `frontend_implemented_count=126`, `frontend_unsupported_stub_count=36`.

## 2026-06-18 Source Freshness Recheck

- Gate report: `reports/gates/2026-06-18-SOURCE-FRESHNESS-RECHECK/summary.md`
- Main finding: shuqi/qimao CDN files still equal local `.backup.json` and differ from refreshed local `.json`, so network-imported copies still need upstream CDN rule update for content. Fanqie CDN matches local. Fanqie short-drama network import URL currently returns 404.
- Live checks: local shuqi/qimao full chains pass after retrying a transient shuqi TLS EOF; shuqi/qimao CDN imports pass search/toc but content remains `EMPTY`.

## 2026-06-18 Headless Cancel And Lint Baseline

- Gate report: `reports/gates/2026-06-18-HEADLESS-CANCEL-LINT/summary.md`
- Main fixes: add headless `TaskRegistry` and route `booksource_cancel`, wire headless search/chapter-list/chapter-content through core cancellation tokens, normalize cancellation errors in headless/Tauri so user cancel returns `CANCELLED`, and add `.gitattributes` LF rules to resolve the Windows `oxfmt` baseline conflict.
- Validation: `cargo test -p legado-headless -- --nocapture`, Tauri task registry/router tests, `cargo check -p legado-headless`, `cargo check -p legado-tauri`, command contract, full `pnpm lint`, and `pnpm build` passed. Playwright headless smoke showed first-screen bookshelf, WS connected, and 0 console errors/warnings.

## 2026-06-18 Headless Repository Iteration

- Gate report: `reports/gates/2026-06-18-HEADLESS-REPOSITORY/summary.md`
- Main fixes: expose `repository_fetch` / `repository_install` / `repository_preview_source` / `repository_check_source_sync` / `booksource_check_update` / `booksource_apply_update` through `legado-headless`, and mark headless `repository` capability supported.
- Validation: local HTTP repository fixture covered manifest fetch, source preview/install, sync consistency, and `@updateUrl` check/apply. Playwright headless smoke opened `书源管理 -> 在线书源` and confirmed repository actions are enabled with 0 console errors/warnings.

## 2026-06-18 Headless WebDAV Sync Iteration

- Gate report: `reports/gates/2026-06-18-HEADLESS-WEBDAV-SYNC/summary.md`
- Main fixes: expose WebDAV sync commands through `legado-headless`, emit `sync:client-state` for headless `sync_now` / `sync_resolve_conflict`, mark headless `syncWebdav` capability supported, and harden headless test directories with an atomic suffix to avoid parallel migration collisions.
- Validation: headless WebDAV route tests covered capability, status, credentials, client state, reader session, lifecycle, conflict list, and invalid-args behavior. Playwright headless smoke opened `设置 -> 同步`, confirmed WebDAV actions are enabled, saved a temporary credential through the UI, and console stayed at 0 errors/warnings.

## 2026-06-18 Prefetch WS Events Iteration

- Gate report: `reports/gates/2026-06-18-PREFETCH-WS-EVENTS/summary.md`
- Main fixes: close R-P2-012 by emitting `shelf:prefetch-progress` / `shelf:prefetch-done` from Tauri WS router as well as IPC, accepting both wrapped and direct prefetch payloads, and adding the same prefetch command/event path to `legado-headless`.
- Validation: Tauri WS router tests covered direct payload parsing and done-event emission; headless tests covered real one-chapter prefetch, cached content, progress event, and done event. Playwright headless smoke drove the real browser WebSocket protocol and confirmed `fetched=1`, cached content, progress/done events, and 0 console errors/warnings.

## 2026-06-18 External Open Wrapper Iteration

- Gate report: `reports/gates/2026-06-18-EXTERNAL-OPEN-WRAPPER/summary.md`
- Main fixes: add `useExternalOpen.ts` as the sole `@tauri-apps/plugin-opener` wrapper, replace direct opener imports across book source, explore, reader, video, and service-mode UI, and fix browser fallback false negatives caused by `window.open(..., "noopener,noreferrer")` returning `null`.
- Validation: `rg` confirmed `@tauri-apps/plugin-opener` appears only in `useExternalOpen.ts`; `pnpm lint`, command contract, and `pnpm build` passed. Playwright headless smoke loaded installed/online source tabs and service mode, imported the built wrapper chunk, and confirmed `empty=false`, `opened=true`, with 0 console errors/warnings.

## 2026-06-18 Backup Headless Data Iteration

- Gate report: `reports/gates/2026-06-18-BACKUP-HEADLESS-DATA/summary.md`
- Main fixes: move backup inspect/create/peek/restore payload logic into `reader-core`, stop reading the stale `config/` directory for app settings, route Tauri backup commands through the shared core, and expose safe data-transfer backup commands in `legado-headless`.
- Validation: `pnpm lint`, `cargo test -p legado-headless`, `cargo check -p legado-tauri`, command contract, `pnpm build`, and `cargo build -p legado-headless` passed. Playwright headless smoke on `127.0.0.1:7797` exported a backup, uploaded it through the browser file chooser, rendered preview categories, confirmed restore, and ended with 0 console errors/warnings.

## 2026-06-15 Master Direct Push

- Branch：`master`
- Master direct push：2026-06-15 已按用户要求直接推送到 `origin/master`，快进范围 `a91ff0f..3727a36`
- Delivered payload commit：`3727a36 docs: prune stale maintenance notes`
- Historical PR：`https://github.com/FanhuaAwA/legado/pull/2` 仅作为本轮历史 staging 记录；后续交付以 `master` 为准，不再依赖单独分支
- GitHub Quality Gate：PASS，run `27539725883`

本轮主线围绕用户反馈的“加载/导入大量书源后，搜索与依赖书源加载的功能长时间等待”展开，已完成：

1. 书源列表流式加载与排序延后。
2. 书源刷新 reload 合并，减少重复加载。
3. 搜索结果增量聚合与分组懒渲染。
4. JS 搜索、章节与预取链路协作取消。
5. URL 包导入进度优化。
6. 喵/猫公子订阅与本地多文件导入批量化。
7. Windows release build 与 GitHub quality-gate 复核。

## 当前 Gate 报告索引

- `reports/gates/2026-06-15-PERF-SOURCE-STREAM-SORT-DEFER/summary.md`
- `reports/gates/2026-06-15-PERF-SEARCH-AGGREGATE-INCREMENTAL/summary.md`
- `reports/gates/2026-06-15-PERF-SOURCE-RELOAD-COALESCE/summary.md`
- `reports/gates/2026-06-15-PERF-LEGACY-IMPORT-URL-PROGRESS/summary.md`
- `reports/gates/2026-06-15-PERF-JS-SEARCH-CANCEL-COOPERATIVE/summary.md`
- `reports/gates/2026-06-15-PERF-JS-CHAPTER-CANCEL-COOPERATIVE/summary.md`
- `reports/gates/2026-06-15-PERF-JS-PREFETCH-CANCEL-COOPERATIVE/summary.md`
- `reports/gates/2026-06-15-PERF-SEARCH-GROUPED-LAZY-RENDER/summary.md`
- `reports/gates/2026-06-15-PERF-MIAOGONGZI-LOCAL-IMPORT/summary.md`
- `reports/gates/2026-06-18-PERF-SOURCE-STABILITY/summary.md`
- `reports/gates/2026-06-18-COVER-CACHE-STABILITY/summary.md`
- `reports/gates/2026-06-18-SOURCE-FRESHNESS-RECHECK/summary.md`
- `reports/gates/2026-06-18-HEADLESS-CANCEL-LINT/summary.md`
- `reports/gates/2026-06-18-HEADLESS-REPOSITORY/summary.md`
- `reports/gates/2026-06-18-HEADLESS-WEBDAV-SYNC/summary.md`
- `reports/gates/2026-06-18-PREFETCH-WS-EVENTS/summary.md`
- `reports/gates/2026-06-18-EXTERNAL-OPEN-WRAPPER/summary.md`
- `reports/gates/2026-06-18-BACKUP-HEADLESS-DATA/summary.md`
- `reports/gates/2026-06-19-WINDOWS-STARTUP-SOURCE-STABILITY/summary.md`
- `reports/gates/2026-06-19-REPOSITORY-REQUEST-PACING/summary.md`

## 当前契约快照

```text
frontendTotal=163
registeredTotal=162
bothCount=162
onlyFrontend=["js_eval"]
onlyBackend=[]
frontend_implemented_count=126
frontend_unsupported_stub_count=36
```

封面缓存 3 个命令已在 2026-06-18 实现；完整命令列表与 stub 分类以 `docs/command-matrix.md` 为准。

## 当前性能实测摘要

喵/猫公子导入专项：

```text
packages=10 entries=1259 resolve_ms=1344 sequential_ms=3809 combined_ms=3831 local_sequential_ms=4146 local_combined_ms=3807 local_speedup=1.09x
```

同场景中，前端自行 JSON parse/stringify 合并方案会让本地导入变慢，已丢弃；保留方案为并发读取本地文件后交给后端批量文本导入。

## 下一步维护队列

- 继续清理文档：保留当前状态、规格与 gate 证据，删除旧流水和旧计划。
- 继续审计书源依赖功能的性能：导入后刷新、书源管理批量操作、搜索启用源过多时的进度/取消/失败聚合。
- 继续扩大本地导入测试样本：多小文件、大包文件、损坏文件混入、重复源覆盖。
- 继续代码 review 并修复发现的问题，优先保持前后端契约、任务取消与错误提示一致。

## 已删除的旧文档口径

- 2026-06-09 的一次性强制修复计划文档已删除；旧命令数量、旧 lint 状态和旧任务顺序均已过期。
- `docs/ai-task-status.md` 与本文件均已从“长历史流水”收敛为当前状态文档。不要再把 2026-06-13 及更早的命令契约数值作为当前基线。
