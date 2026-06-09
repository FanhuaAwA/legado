# Critical Remediation Plan

本文件记录 2026-06-09 本地审计发现的缺失、空壳和假实现问题。后续 AI 必须先处理本文件列出的 P0/P1/P2 问题，再继续 UI polish、新功能、书源扩展、同步、插件或发布工作。

## 当前结论

项目不是“已按规划完成”。当前状态是：

- 基础构建链路已明显推进：`pnpm build`、`cargo check -p legado-tauri`、`cargo test --workspace` 可通过。
- 当前工作树的 `pnpm lint` 不通过，阻断点是 3 个文档未过 `oxfmt --check`。
- 前后端 command 契约仍严重不完整：前端直接调用约 159 个 command，Tauri 注册约 80 个，静态扫描发现约 84 个前端调用未注册。
- 多个模块存在“有入口、无真实后端”或“返回假成功”的情况。
- 书源兼容测试存在 silent pass，不能作为真实全链路可用证明。

## 强制处理顺序

### P0：恢复真实门禁状态

问题：

- `docs/ai-task-status.md` 曾记录 `frontend.lint = passed`，但当前复测 `pnpm lint` 失败。
- 失败文件为：
  - `docs/ai-iteration-log.md`
  - `docs/ai-task-status.md`
  - `docs/source-compat-matrix.md`

解决方案：

1. 只做格式化基线，不混入业务改动。
2. 执行：

```powershell
pnpm exec oxfmt docs/ai-iteration-log.md docs/ai-task-status.md docs/source-compat-matrix.md docs/command-matrix.md docs/critical-remediation-plan.md
pnpm lint
```

3. 若 `pnpm lint` 仍失败，按阶段分类：
   - `oxfmt` 失败：继续修格式，不改业务。
   - `oxlint` warning/error：另开 lint 修复任务。
   - `vue-tsc` 失败：另开类型修复任务。

验收标准：

- `pnpm lint` 至少通过 `oxfmt --check`。
- `docs/ai-task-status.md` 必须记录当前真实结果，不得写假 PASS。

### P1：先修 command 契约，不得继续堆 UI

事实：

- `src/composables/useTransport.ts` 在 Tauri 环境直接调用原生 IPC，不会自动降级到 WebSocket。
- 未注册 command 在 Windows/Android 壳内会直接失败。
- 当前 `docs/command-matrix.md` 不是完整矩阵，不能作为唯一依据。

必须先新增一个自动化检查：

```text
scripts/ci/check-command-contract.mjs
```

检查逻辑：

1. 扫描 `src/**/*.ts`、`src/**/*.vue`、`src/**/*.js` 中的 `invokeWithTimeout("...")` 和 `invoke("...")`。
2. 扫描 `src-tauri/src/commands/mod.rs` 中 `generate_handler!` 注册项。
3. 输出：
   - 前端调用但未注册。
   - 后端注册但没有直接前端调用。
   - 可能是配置字段误判的项目。
4. 接入 `scripts/ci/quality-gate.mjs`，让命令差集不再靠人工维护。

验收标准：

- `node scripts/ci/check-command-contract.mjs` 可重复运行。
- `docs/command-matrix.md` 由扫描结果更新。
- 对每个缺失 command 明确选择一种处理：
  - 实现真实后端。
  - 隐藏/移除前端入口。
  - 返回结构化 `UNSUPPORTED`，并让前端显示或禁用入口。

### P1 缺失 command 分组

以下是 2026-06-09 静态扫描得到的高风险缺失分组。后续 AI 必须重新跑自动脚本确认数量后再改代码。

书源和书源工具：

```text
booksource_apply_update
booksource_check_update
booksource_delete_draft
booksource_http_proxy
booksource_open_in_vscode
booksource_resolve_path
explore_clear_cache
js_eval
repository_check_source_sync
repository_fetch
repository_install
repository_preview_source
```

书架、导出、封面、漫画缓存：

```text
bookshelf_export_book
bookshelf_export_book_data
bookshelf_reveal_export_file
comic_cache_clear
comic_cache_clear_chapter
comic_cache_size
comic_download_images
comic_get_cached_page
cover_cache_clear
cover_cache_size
cover_resolve_cache
```

浏览器探测和网页登录：

```text
browser_probe_clear_data
browser_probe_close
browser_probe_close_all
browser_probe_create
browser_probe_eval
browser_probe_get_cookies
browser_probe_hide
browser_probe_navigate
browser_probe_run
browser_probe_set_cookie
browser_probe_set_user_agent
browser_probe_show
```

备份、同步、Web 服务：

```text
backup_create
backup_create_data
backup_inspect
backup_peek
backup_peek_data
backup_restore
backup_restore_data
sync_baidu_poll_token
sync_baidu_revoke_auth
sync_baidu_start_auth
sync_baidu_token_status
sync_clear_credentials
sync_client_state_set
sync_get_credentials
sync_get_status
sync_list_conflicts
sync_notify_lifecycle
sync_now
sync_report_reader_session
sync_resolve_conflict
sync_set_credentials
sync_test_connection
sync_v2_sync_reading_progress
web_server_pick_dist_dir
web_server_start
web_server_status
web_server_stop
```

TTS、字体、视频代理、AI HTTP：

```text
ai_http_proxy_url
delete_user_font
get_local_ips
list_system_fonts
list_user_fonts
rename_user_font
start_video_proxy
stop_video_proxy
tts_get_voices
tts_is_initialized
tts_is_speaking
tts_preview_voice
tts_speak
tts_stop
upload_user_font
```

解锁挑战：

```text
issue_full_mode_challenge
issue_scoped_unlock_challenge
verify_full_mode_challenge
verify_scoped_unlock_challenge
```

处理建议：

- `booksource_*`、`bookshelf_*`、`comic_*`、`cover_*` 属于主阅读链路，应优先真实实现或隐藏入口。
- `backup_*`、`sync_*`、`web_server_*`、`browser_probe_*` 属于大模块，不允许只写空返回；若短期不做，应统一返回 `UNSUPPORTED` 并在 UI 禁用。
- `tts_*`、`start_video_proxy`、`stop_video_proxy` 与音频/视频相关；当前视频/音乐入口仍被屏蔽，不得在文档里标为完成。

### P2：修复假实现和空壳

#### `booksource_cancel`

问题：

- `TaskRegistry.register()` 和 `TaskRegistry.remove()` 当前未使用。
- `booksource_chapter_list` 接收 `_task_id` 但丢弃。
- `bookshelf_prefetch_chapters` 的 `task_id` 被标为 dead code。

解决方案：

1. 在 `booksource_chapter_list`、`booksource_chapter_content`、`bookshelf_prefetch_chapters`、`booksource_run_tests` 等长任务入口注册取消 token。
2. 将 token 传入 reader-core 的抓取、解析、预取循环。
3. 循环内定期检查 token，取消时返回结构化错误：

```text
code = "CANCELLED"
retryable = false
```

4. 任务完成或失败后调用 `TaskRegistry.remove()`。

验收标准：

- `cargo check -p legado-tauri` 不再提示 `register/remove` 未使用。
- 前端触发取消后，长任务能停止，而不是只返回 `TASK_NOT_FOUND`。
- 增加至少一个单元或集成测试覆盖取消状态。

#### `booksource_purchase_chapter`

问题：

- JS 书源会调用 `purchaseChapter`。
- Legado 规则书源仍直接返回 `{ ok: true, purchased: true }`，属于假成功。

解决方案：

1. 如果 Legado 规则源没有真实购买接口，返回结构化 `UNSUPPORTED`。
2. 如果规则里存在购买字段或 JS 回调，再设计真实执行路径。
3. 前端收到 `UNSUPPORTED` 时提示“当前书源不支持自动购买/需手动处理”。

验收标准：

- 不允许任何付费/购买相关命令无条件返回成功。
- 添加测试确认 Legado 规则源不会假成功。

#### `booksource_run_tests`

问题：

- 当前丢弃 `timeout_secs` 和 `step_filter`。
- Legado 源只返回能力是否配置，不真实运行 search/bookInfo/toc/content。

解决方案：

1. 支持 `step_filter`，只运行指定步骤。
2. 支持超时控制。
3. 对 Legado 源真实执行：
   - `search`
   - `bookInfo`
   - `toc`
   - `content`
   - `explore`
4. 每步输出输入、状态、错误码、预览、耗时。

验收标准：

- AI 书源测试面板看到的结果必须来自真实 reader-core 执行。
- 不允许只返回 `available/not_configured` 冒充测试通过。

#### `storage_debug_dump`

问题：

- 当前返回的 `frontend`、`scriptJson`、`scriptBytes`、`clientStates` 是空对象。

解决方案：

1. 从 reader-core 存储层读取真实 frontend namespaces、script config、bytes、client state。
2. 大字段只返回摘要，避免一次性 dump 超大内容。
3. 对不可读取字段返回结构化错误或 `unsupported` 字段，不得静默空对象。

验收标准：

- 设置页存储调试面板能看到真实数据。
- 空数据和未实现必须能区分。

#### Harmony 和 Node 书源脚本

问题：

- `copy-harmony-web.mjs` 只是骨架，未配置真实目标目录。
- `booksource-node-runtime.mjs` 只做词法分析，不能执行书源。

解决方案：

- Harmony：若无真实 Harmony 工程，`build:harmony` 应明确失败或隐藏，不应打印成功式跳过。
- Node 书源运行器：要么接入真实 reader-core/Tauri 测试路径，要么在 package script 和文档中标为诊断工具，不得标为书源运行时。

验收标准：

- `pnpm run build:harmony` 的输出不能让用户误以为已生成 Harmony 产物。
- `pnpm run booksource:node:test` 的说明必须写明“非完整运行时”。

#### 视频、音乐、TTS

问题：

- `ReaderVideoSurface` 当前只显示锁定提示，`getCurrentTime()` 和 `getDuration()` 返回 0。
- `useInlineBookReader` 和 `bookshelfReaderLauncher` 中视频/音乐逻辑被注释屏蔽。
- 前端仍调用大量未注册 `tts_*` 和视频代理命令。

解决方案：

1. 短期：隐藏视频/音乐/TTS入口，或统一显示未支持状态。
2. 中期：实现 `start_video_proxy`、`stop_video_proxy`、`tts_*` 后再放开 UI。
3. 不得只做提示卡片后在状态文档里标为完成。

验收标准：

- 用户不能点击到必然 invoke 失败的入口。
- 若入口存在，后端必须有真实 command 或结构化 `UNSUPPORTED`。

## P2：修复测试门禁失真

问题：

- `shuqi_source_live_search` 遇到搜索错误只打印日志，不 fail。
- `shuqi_source_full_chain` 遇到 bookInfo/chapterList 错误会 `return`，测试仍 PASS。
- `content` 获取失败只打印日志，不 fail。

解决方案：

1. 真实门禁测试必须 fail，不得 silent pass。
2. 实网依赖不稳定的测试应加 `#[ignore = "live network"]`，并提供明确手动运行命令。
3. Mock/fixture 测试必须覆盖完整 search -> bookInfo -> toc -> content 链路。
4. `docs/source-compat-matrix.md` 只能把严格断言通过的结果标为 PASS。

验收标准：

```powershell
cargo test -p reader-core
cargo test shuqi_source_full_chain -- --nocapture
cargo test qimao_source_full_chain -- --ignored --nocapture
```

- 如果网络测试不可控，默认门禁不依赖它。
- 如果手动实网测试失败，矩阵必须写 BLOCKED，并写清楚证据。

## P3：完善 JS shim，但不得冒充完整 Legado

当前问题：

- `java.ajax`、`java.get/post/put`、`legado.http.*` 多处 `unwrap_or_default()` 吞掉错误。
- `java.startBrowser` 直接返回空字符串。
- `source/cookie/cache` 主要是进程内 KV，不等价于 Legado 持久变量、登录信息和 Cookie 管理。
- `ajaxAll` 当前串行执行，错误被吞掉。

解决方案：

1. JS shim 需要有能力矩阵和测试。
2. 网络失败应带错误上下文，不应全部变成空字符串。
3. Cookie、source variables、cache memory/cache disk 分层实现。
4. `java.startBrowser` 应映射到前端事件或明确返回 `UNSUPPORTED`。
5. 不支持 Android/Rhino 专属对象时，必须在兼容矩阵写明降级策略。

验收标准：

- 每新增一个 JS API，必须有单测或本地书源验证记录。
- 番茄源仍不可用时，不得把 `java.ajaxAll` 等单点 API 标为完整兼容。

## P4：授权文件

问题：

- README 声明 MIT 并链接 `LICENSE`，但仓库根目录没有 `LICENSE` 文件。

解决方案：

1. 如果坚持 MIT，补 MIT `LICENSE` 文件，并禁止复制 GPLv3 代码。
2. 如果接受 GPLv3 影响，明确改许可证声明。

验收标准：

- 根目录存在 `LICENSE`。
- README 的许可证链接有效。

## 后续 AI 禁止事项

- 禁止在上述 P0/P1/P2 未解决前继续做 UI polish。
- 禁止把未注册 command 当作“未来功能”忽略；入口存在就必须处理。
- 禁止用空对象、空数组、空字符串、固定成功结果冒充实现。
- 禁止把 silent pass 的测试结果写成真实 PASS。
- 禁止把 Harmony、Node 书源运行器、视频/TTS 这类空壳标为完成。

## 推荐下一轮第一件事

先做 command 契约自动检查：

```powershell
node scripts/ci/check-command-contract.mjs
```

如果脚本尚不存在，先创建它并接入 `quality-gate.mjs`，然后更新 `docs/command-matrix.md`。不要先修 UI。
