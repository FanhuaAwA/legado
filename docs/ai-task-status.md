# AI Task Status

Last updated: 2026-06-19

本文件只记录当前维护状态、可继续执行的队列和必须遵守的口径。旧的逐轮流水、未完成提示和过期统计已删除；需要考古时使用 git history 与 `reports/gates/*/summary.md`。

## 当前分支与发布

- Branch：`master`
- Master direct push：2026-06-15 已按用户要求直接推送到 `origin/master`，快进范围 `a91ff0f..3727a36`
- Delivered payload commit：`3727a36 docs: prune stale maintenance notes`
- Historical PR：`https://github.com/FanhuaAwA/legado/pull/2` 仅作为本轮历史 staging 记录；后续交付以 `master` 为准，不再依赖单独分支
- GitHub Quality Gate：PASS，run `27539725883`，约 13m47s
- Windows release artifact：
  - `target/x86_64-pc-windows-msvc/release/legado-tauri.exe`
  - `构建结果/windows/legado-tauri.exe`
- Current 2026-06-19 delivery gate：`reports/gates/2026-06-19-WINDOWS-STARTUP-SOURCE-STABILITY/summary.md`

## 当前契约基线

实测命令：

```powershell
node scripts\ci\check-command-contract.mjs --json
```

当前结果：

- `frontendTotal=163`
- `registeredTotal=162`
- `bothCount=162`
- `onlyFrontend=["js_eval"]`
- `onlyBackend=[]`
- `frontend_implemented_count=126`
- `registered_implemented_count=126`
- `frontend_unsupported_stub_count=36`
- `registered_unsupported_stub_count=36`

`js_eval` 是有意不注册的安全阻断项，不得作为缺失命令处理。新增、删除或改变命令实现状态时必须同步更新 `docs/command-matrix.md`、相关规格文档与对应 gate 报告。

## 已完成的当前性能线

- 大量书源列表加载：`booksource_list_streaming` 流式返回、前端增量 upsert、排序延后，避免首屏被全量书源阻塞。
- 书源刷新：前端 reload 合并和 token 保护，避免重复刷新互相覆盖。
- 搜索结果：聚合结果按批增量回写，分组渲染改为懒展开，降低大量书源搜索时的主线程压力。
- JS 搜索 / 章节 / 预取：加入协作取消，停止搜索或切换任务后旧结果不再继续污染 UI。
- 大量书源导入：新增 `booksource_import_legacy_json_texts` 批量文本导入；URL 包导入和本地多文件导入都走批量链路。
- 封面缓存：`cover_resolve_cache` / `cover_cache_size` / `cover_cache_clear` 已真实实现，支持同 URL 并发合并、8MB 流式大小上限、Tauri/headless 共用核心逻辑与 Web `/asset` 加载。
- 喵/猫公子书源实测：`packages=10 entries=1259 resolve_ms=1344 sequential_ms=3809 combined_ms=3831 local_sequential_ms=4146 local_combined_ms=3807 local_speedup=1.09x`。
- Headless WS cancel：浏览器/headless 模式已接入 `TaskRegistry` 支持 `booksource_cancel` 取消 search/chapter-list/chapter-content；Tauri/headless 取消后底层 JS 中断统一归一化为 `CANCELLED`。
- Full lint baseline：`.gitattributes` 固定源码/文档 LF 行尾，Windows Git 与 `oxfmt` 不再冲突，仓库级 `pnpm lint` 已通过。
- Headless repository：浏览器/headless 模式已暴露在线仓库和 `@updateUrl` 更新命令域，`capabilities_get.repository` 与 Tauri 对齐为 supported。
- Headless WebDAV sync：浏览器/headless 模式已暴露 `sync_set_credentials` / `sync_get_status` / `sync_now` / `sync_test_connection` / conflict / lifecycle / reader-session 等 WebDAV 同步命令，`capabilities_get.syncWebdav` 与 Tauri 对齐为 supported。
- Prefetch WS events：R-P2-012 已关闭；Tauri WS router 与 headless WS 均可执行 `bookshelf_prefetch_chapters` 并推送 `shelf:prefetch-progress` / `shelf:prefetch-done`。
- External open wrapper：业务组件已不再直接 import `@tauri-apps/plugin-opener`；外部链接打开统一走 `useExternalOpen.ts`，浏览器/headless 分支已修复 `noopener,noreferrer` 返回 `null` 导致的误报失败。
- Backup headless data：备份 inspect/create/peek/restore 载荷逻辑已下沉到 `reader-core`；Tauri 与 headless 复用同一实现，浏览器/headless 使用 data-transfer 下载、文件选择、预览与还原链路，headless path 型备份命令明确拒绝服务端路径读写。
- Windows startup/source stability：已修复 legacy SQLx migration-4 checksum 导致的 Windows 启动 panic/闪退；书源列表前端先完成事件监听再启动 streaming，并增加 80s final-batch timeout；后台 `@updateUrl` 检查降为单并发并间隔 1200ms，避免启动后对 CDN 源短时突发请求。实测 rebuilt Windows release：窗口 916ms 出现、2318ms UI ready、书源管理 2250ms 加载 1068 源、发现页 766ms 加载 957 个发现源。

## 当前验证命令

```powershell
cmd /c pnpm.cmd exec oxfmt --check .
cmd /c pnpm.cmd lint
cmd /c pnpm.cmd exec vue-tsc -p tsconfig.app.json --noEmit
cargo test -p reader-core import_legacy_json_text_reports_progress_batches -- --nocapture
cargo test -p reader-core import_legacy_json_texts_skips_bad_item_and_imports_valid_sources -- --nocapture
cargo test -p reader-core --test miaogongzi_import_perf miaogongzi_subscription_import_sequential_vs_combined -- --ignored --nocapture
cargo test -p legado-tauri task_ -- --nocapture
cargo test -p legado-tauri booksource_import_legacy_json_texts_accepts_request_id_in_ws_router -- --nocapture
node scripts\ci\check-command-contract.mjs --json
cargo test -p legado-headless -- --nocapture
cargo test -p legado-headless repository_ -- --nocapture
cargo test -p legado-headless sync_webdav -- --nocapture
cargo test -p legado-headless bookshelf_prefetch -- --nocapture
cargo test -p legado-tauri --test ws_router bookshelf_prefetch_accepts_direct_payload_and_emits_done -- --nocapture
cargo test -p legado-tauri --test ws_router booksource_search_accepts_task_id_in_ws_router -- --nocapture
cargo test -p reader-core --test cover_cache -- --nocapture
cargo test -p legado-tauri --test ws_router cover_cache_commands_are_routed -- --nocapture
cargo test -p legado-tauri --test ws_router capabilities_get_returns_map -- --nocapture
cargo check -p legado-tauri
cargo check -p legado-headless
cargo build -p legado-headless
cmd /c pnpm.cmd build
cmd /c pnpm.cmd build:windows:release
cargo test -p reader-core --test db_migrations -- --nocapture
git diff --check
```

2026-06-18 封面缓存迭代已实测通过新增/受影响链路的格式、类型、Rust check、命令契约与路由/缓存测试；旧性能专项命令仍作为后续回归队列保留。Tauri 测试/构建仍可能输出已知 Windows linker stdout warning；当前不作为失败。

2026-06-18 headless cancel / lint baseline iteration verified that `cmd /c pnpm.cmd lint` now passes end-to-end (`oxfmt --check .`, `oxlint --type-aware --type-check .`, and `vue-tsc`). Playwright headless smoke on `127.0.0.1:7790` showed bookshelf first screen, WS connected, and 0 console errors/warnings.
2026-06-18 headless repository iteration verified local repository fixture import/update commands and Playwright headless `书源管理 -> 在线书源` smoke on `127.0.0.1:7791`; repository toolbar actions were enabled and console stayed at 0 errors/warnings.
2026-06-18 headless WebDAV sync iteration verified local WebDAV sync command dispatch and Playwright headless `设置 -> 同步` smoke on `127.0.0.1:7793`; WebDAV action buttons were enabled, temporary credential save succeeded through the UI, and console stayed at 0 errors/warnings.
2026-06-18 prefetch WS events iteration verified Tauri WS direct payload routing and Playwright headless raw WebSocket prefetch smoke on `127.0.0.1:7795`; one chapter was cached, progress arrived before done, and console stayed at 0 errors/warnings.
2026-06-18 external open wrapper iteration verified `@tauri-apps/plugin-opener` is isolated to `useExternalOpen.ts`, `cmd /c pnpm.cmd lint`, `cmd /c pnpm.cmd build`, command contract, and Playwright headless smoke on `127.0.0.1:7796`; installed/online source tabs and service mode loaded with 0 console errors/warnings, and the built wrapper returned `empty=false`, `opened=true`.
2026-06-18 backup headless data iteration verified shared `reader-core` backup payloads, headless `backup_*_data` dispatch, browser export/download/file-picker preview/restore on `127.0.0.1:7797`, `cmd /c pnpm.cmd lint`, `cargo test -p legado-headless`, `cargo check -p legado-tauri`, command contract, `cmd /c pnpm.cmd build`, and `cargo build -p legado-headless`.
2026-06-19 Windows startup/source stability iteration verified `cmd /c pnpm.cmd lint`, `git diff --check`, `cargo test -p reader-core --test db_migrations -- --nocapture`, `cargo check -p reader-core`, `cargo check -p legado-tauri`, `cmd /c pnpm.cmd build:windows:release`, and Windows desktop smoke against the rebuilt release. The app no longer exits immediately after launch on the repaired migration state.

## 当前未结工作

- 继续审计依赖书源加载的功能：首次导入后刷新、书源管理批量操作、搜索启用源过多时的进度、取消、按源超时和失败聚合。
- 继续优化本地书源导入：扩大样本，比较单文件、多文件、小文件密集和大文件包场景，避免只优化喵/猫公子一种形态。
- 继续按用户要求做代码 review：优先关注性能热路径、取消语义、任务 token、前后端契约和错误提示。
- 能力 backlog：`browser_probe`、TTS、漫画页缓存、video proxy、解锁挑战、百度/FTP provider。
- 形态 B / LAN 严格验收仍需外部设备或可访问局域网环境。
- 书源兼容：书旗/七猫 CDN 规则新鲜度与通用 `book.bookUrl` 绑定仍需按真实样本继续复查。
- 2026-06-18 复查结论：书旗/七猫 CDN 仍是旧 `.backup.json` 等价版本，网络导入 content 仍 `EMPTY`，需要上游 CDN 更新；番茄 CDN 与本地一致；番茄短剧网络导入 URL 当前 404。
- 2026-06-19 source freshness spot-check：qimao/shuqi CDN still match local `.backup.json`, fanqie CDN matches local `.json`, fanqie short-drama CDN URL still returns 404; installed Legado JSON scan found 77 explicit degraded/expired-style remarks and no `updateUrl` values.

## 文档维护规则

- 本文件只保留当前状态，不再追加完整流水。
- 逐轮证据写入 `reports/gates/<date-topic>/summary.md`。
- 长期规格写入 `docs/reader-rust-route-b-spec.md`、`docs/frontend-backend-separation.md`、`docs/source-compat-matrix.md`。
- 命令契约以 `scripts/ci/check-command-contract.mjs --json` 实测为准，不沿用旧数字。
- 删除旧计划或旧审计文档后，不得在新文档中继续引用它们作为当前任务来源。
