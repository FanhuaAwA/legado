# AI Iteration Log

Last updated: 2026-06-18

本文件是当前迭代索引，不再保存完整对话式流水。2026-06-15 之前的长历史、旧续办提示和过期命令统计已清理；需要追溯请查看 git history 与 `reports/gates/*/summary.md`。

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
