# AI Task Status

## 2026-06-13 CI-CARGO-FETCH-RETRY 状态更新

本轮状态：`closed-local`，待提交推送。

本轮处理用户报告的 GitHub Actions 失败：2026-06-13 01:00 左右，`cargo check -p reader-core` 在 crates.io 下载 `cipher` 依赖时连接 reset。该日志属于 registry 下载链路抖动，不是 reader-core 编译错误。

已修改 `quality-gate.yml`：

- 全局增加 `CARGO_NET_RETRY=10`、`CARGO_HTTP_TIMEOUT=120`、`CARGO_HTTP_MULTIPLEXING=false`、`CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`。
- 增加 `actions/cache@v4` 缓存 `~/.cargo/registry` 和 `~/.cargo/git`，降低重复下载 crates.io 依赖的概率。
- 在 Rust check/test 前增加 `cargo fetch --locked` 三次重试，失败间隔 20s/40s，把依赖下载问题提前隔离到 Fetch Cargo dependencies 步骤。

已通过 gate：

- `cmd /c node_modules\.bin\oxfmt.cmd --check .`
- `git diff --check`
- `node scripts/ci/check-command-contract.mjs --json`
- `cargo fetch --locked`
- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `cargo fmt --all -- --check`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cargo test -p reader-core`

推送后应观察 GitHub Actions 新一轮 `Quality Gate`，确认 `Fetch Cargo dependencies` 步骤生效。

## 2026-06-13 UI-SOURCE-AI-COMMENT-LAYOUT 状态更新

本轮状态：`closed-pushed`，提交 `ab514a1` 已推送到 `origin/master`。

实测命令契约基线：

```text
command_contract.frontendTotal = 162
command_contract.registeredTotal = 161
command_contract.bothCount = 161
command_contract.onlyFrontend = js_eval
command_contract.onlyBackend = none
command_contract.frontend_unsupported_stub_count = 39
command_contract.frontend_implemented_count = 122
```

本轮收口：

- `InstalledSourcesTab.vue`、`AiSourceTab.vue`、`AiTestPanel.vue`、`ReaderParagraphCommentsDrawer.vue` 已完成响应式布局加固，重点覆盖书源管理标题横排、搜索/统计/批量管理条、AI 写书源三栏与输入区、段评抽屉长文本。
- `src-headless/src/main.rs` 已补齐 `booksource_get_dir`、`booksource_get_dirs`、`booksource_list_streaming`，使 headless 预览可通过真实 WS 渲染书源卡片并验证流式列表事件。
- Chrome headless 实测 1000x800、768x800、390x800 三档均无横向溢出；书源管理标题 `writing-mode=horizontal-tb`，3 条测试书源卡片全部渲染；段评抽屉合成长文本检查 `tooWide=0`。

已通过 gate：

- `cmd /c node_modules\.bin\oxfmt.cmd --check .`
- `git diff --check`
- `node scripts/ci/check-command-contract.mjs --json`
- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cargo check -p legado-headless`
- `cargo test -p reader-core`

后续第一任务：`CI-2026-06-13-CARGO-FETCH-RETRY`。用户报告 GitHub Actions 在 2026-06-13 01:00 左右下载 crates.io 依赖 `cipher` 时连接 reset；该日志指向网络瞬断/registry 下载不稳定，不是本地代码编译失败。下一轮应加固 quality-gate 的 Cargo 缓存与 fetch/check/test 重试。

本文件记录当前 R 队列状态。事实数字只以当轮命令输出为准，不沿用历史表格。

最后实测：2026-06-12（AI-DEEPSEEK-MGZ 轮；stub 40→39）。下方基线仍以当轮 `check-command-contract.mjs --json` 输出为准。

实测命令：

```powershell
git status --short
node scripts/ci/check-command-contract.mjs --json
cargo fmt --all -- --check
cmd /c pnpm lint
cmd /c pnpm build
cargo check -p reader-core
cargo test -p reader-core
cargo check -p legado-tauri
cargo test -p legado-tauri
cargo test -p reader-core shuqi_source_full_chain -- --ignored --nocapture
cargo test -p reader-core qimao_source_full_chain -- --ignored --nocapture
cargo test -p reader-core fanqie_source_full_chain -- --ignored --nocapture
cmd /c pnpm build:windows:release
cmd /c pnpm build:android:release
node scripts/ci/check-command-contract.mjs --json
```

## 当前基线

```text
project.status = incomplete
command_contract.frontendTotal = 162
command_contract.registeredTotal = 161
command_contract.bothCount = 161
command_contract.onlyFrontend = js_eval
command_contract.onlyBackend = none
command_contract.frontend_unsupported_stub_count = 39
command_contract.frontend_implemented_count = 122
command_contract.classificationScope = frontend-facing registered commands
frontend.lint = passed_zero_warnings
form_b_headless_loopback = passed
prefetch_progress = passed
source.live_verification = 书旗/七猫/番茄 strict_pass
windows.release = passed
android.release_unsigned = passed
windows.smoke = process_window_ws_pass; computer_use_blocked
cleanup = target_dist_android_build_removed; artifacts_preserved
paragraph_comment = fixed 2026-06-12 PARA-COMMENT-VERIFY; js capabilities detect chapterParagraphCommentCounts/chapterParagraphComments/likeParagraphComment/replyParagraphComment; external sourceDir propagated for counts/details/like/reply; legacy Legado showCmt entry has offline facade regression
ai_booksource = fixed 2026-06-12 AI-DEEPSEEK-MGZ; DeepSeek provider presets added; backend ai_http_proxy_request implemented with POST/domain/path whitelist; yuedu://rsssource subscription import resolves MiaoGongZi pages into booksource package URLs
```

口径变更（2026-06-11）：stub 数由 60 降至 58，差额来自上一轮 `UI-REMOVE-APP-UPDATE` 删除 `app_update_*`（2 个）。总纲/审计旧文档写「60」已过期。
口径变更（2026-06-12）：CAP-REPO 后 stub 58→52；CAP-SYNC WebDAV 后 WebDAV 12 命令转 implemented，stub 52→40；百度网盘 provider 4 命令按用户决策继续保留隐藏。
FORMB 口径（2026-06-12）：`src-headless` 不参与 Tauri command-contract 计数，本轮补齐的是独立 headless WS 分发契约；因此 stub 数维持 40，不应误判为无进展。
口径变更（2026-06-12）：AI-DEEPSEEK-MGZ 后 `ai_http_proxy_url` 旧 stub 移除，`ai_http_proxy_request` 转 implemented，stub 40→39。当前以本轮实测 39 为准。

## NET 任务（网络设置配置接入，审计第二类）

| ID        | 状态         | 证据 / 说明                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| --------- | ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- |
| NET-001   | closed       | 主 reqwest 客户端接入 `http_user_agent`/`http_follow_redirects`/`http_connect_timeout_secs`/`http_ignore_tls_errors`/`proxy_*`；`HttpClientConfig`+`from_config`，启动时构建；reqwest 加 `socks` feature；测试 `tests/http_client_config.rs` 7/7。见 `reports/gates/2026-06-11-NET-001-002-network-config/summary.md`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| NET-002   | closed       | `request_min_delay_ms` 接入 JS 桥（`AtomicU64`+`set_js_http_min_delay_ms`），启动下发 + `app_config_set` 实时更新                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| NET-003   | closed       | `engine_timeout_secs` 已接入：`parser/js.rs` 加 `JS_ENGINE_TIMEOUT_SECS` 原子 + thread-local deadline + `JsEvalDeadlineGuard`，`acquire_runtime` 装 `set_interrupt_handler`，`eval_js_inner_with_source` 单一 eval 收口处设 deadline。启动 + `app_config_set` 实时下发。测试 `tests/js_engine_timeout.rs`（死循环 1s 被中断、正常 eval 仍通过）。基准：js_compat 1.23s 与改前持平，handler 仅 thread-local 读取无热路径回归                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |     |
| NET-004   | closed       | `http_doh_server` 已接入：新模块 `crawler/doh.rs` 实现 `reqwest::dns::Resolve`（JSON DoH API，**无新依赖**），bootstrap 客户端用 `.resolve(host, 已知IP)` 钉死避免递归，**fail-open** 到系统 DNS（任何 DoH 失败不破坏解析）。主客户端经 `HttpClientConfig.doh_server` + `builder().dns_resolver(...)` 接入。**NET-004-LIVE（2026-06-12 实网验证）**：逐 provider 实测，修正 2 处缺陷——360dns 的 JSON 端点是 `/resolve`（旧填 `/dns-query` 仅收 RFC8484 wire-format，会静默 fail-open 假装走 DoH）；onedns 无公开 JSON DoH 端点（`doh.onedns.net/dns-query` 返回 HTTP 000，需账号），已从后端 provider 与前端 `DOH_OPTIONS` 移除。现存 5 provider（alidns/dnspod/360dns/cloudflare/google）live 均返回真实 Answer，新增 `#[ignore]` live 测试 `doh_live_each_provider_returns_real_answer`（`cargo test -p reader-core doh_live -- --ignored` 实测 5/5 通过，直接调用 `doh_query` 确保非 fail-open）。JS 桥（blocking 客户端）的 DoH 已由 NET-005 接入 |     |
| CLEAN-001 | closed       | 已移除死键 `ui_enable_aplus_tracking`（审计第四类）：`useAppConfig.ts`/`appConfig.ts` 类型+默认+computed+return、`facade.rs:default_app_config` 默认全部删除；lint 0/0、build PASS、契约不变                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| CLEAN-002 | 复核：非缺陷 | 审计第三类。复核：「解除限制」点击进 `FullModeUnlockDialog`，unlock capability 不支持时弹窗内报错（即审计第一类「弹窗内展示错误」），非静默 UNSUPPORTED。预先置灰为可选装饰性 polish，留待 unlock 能力真实化时处理                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| CLEAN-003 | 复核：非缺陷 | 审计第三类。复核：`SectionSync.vue` provider `n-select` 已 `:disabled="syncDisabled"`，sync 不支持时整段禁用，FTP/百度网盘不可实际选中。留待 sync 能力真实化时处理 provider 级 stub                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

⚠️ NET-001 行为变化：`http_ignore_tls_errors` 默认 `true`，接入后默认接受无效 TLS 证书（旧行为为始终校验）。属用户决策项，详见 gate 报告。

## 后续维护任务路线图（接手 AI 按优先级取用）

前后端接入审计（四类）已结清：第二类 8 键全部接入（NET-001~004），第四类死键已删（CLEAN-001），第三类经复核为既有 capability 门禁覆盖的非缺陷，第一类复核准确（58 stub）。以下是审计之外、面向第 14 节最终可用标准的剩余工作，按优先级登记，供后续 AI 维护更新。**实现任何已隐藏后端能力时，必须同步更新 `capabilities_get`、前端入口状态与 `docs/command-matrix.md`（审计文档第 4 节处置规则）。**

### A. 环境/网络阻塞项（有网或真机时优先验证）

| ID                  | 任务                                                                                                                                                                                                                                                                                                                                                                                                            | 验收                                                                  |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| ~~NET-004-LIVE~~    | **closed 2026-06-12**：实网逐 provider 验证完成。修正 360dns 路径（`/dns-query`→`/resolve`）、移除无 JSON 端点的 onedns；现存 5 provider live 5/5 通过。`#[ignore]` live 测试已入库（`cargo test -p reader-core doh_live -- --ignored`）。详见 NET-004 行                                                                                                                                                       | 已达成                                                                |
| ~~NET-005~~         | **closed 2026-06-12**：DoH 已接入 JS 桥 blocking 客户端。`parser/js.rs` 加 `JS_HTTP_DOH_SERVER: Mutex<String>` + `set_js_http_doh_server`，`JS_HTTP_CLIENT` Lazy 构建时 `builder.dns_resolver(...)`；`facade.rs` 启动用 `http_cfg.doh_server` 下发。异步 Resolver 跑在 blocking 客户端内部 runtime 的风险已 live 验证（`doh_live_blocking_client_resolves_and_fetches` 经 Cloudflare DoH 真实抓取 example.com） | 已达成（JS 桥 java.ajax 按 `http_doh_server` 走 DoH，fail-open 一致） |
| ~~SRC-FANQIE-LIVE~~ | **closed 2026-06-12**：番茄 search→bookInfo→toc(1928)→content(3135) 全链路 live_network_pass。bookInfo 字段完整性已验收（name/author/intro/kind/wordCount/coverUrl/tocUrl 均填充真实数据并加断言）。验收中发现并修复引擎两处通用字段管线缺陷（`@js:` 被 `##` 正则切分吞掉、单 `##` 删除被忽略），详见 `docs/source-compat-matrix.md`。书旗/七猫全链路无回归。剩 lastChapter=None（详情 JSON 无该字段，非缺陷）  | 已达成（含引擎保真修复）                                              |

### B. 已隐藏/降级后端能力本体（§14 缺口，每项大特性，单独立项）

实现前必读审计文档第 4 节处置规则。**含可能不需要的能力，动手前先与用户确认取舍（见下「待用户决策」）。** 用户决策（2026-06-12）：百度网盘/FTP 同步、browser_probe、完全体 unlock 三项「都先保留」，暂不移除。

| ID             | 能力域                   | 数量 | 当前处置                                        | 实现要点（审计文档第 4 节）                                                                                                                                                                              |
| -------------- | ------------------------ | ---- | ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ~~CAP-REPO~~   | repository/source_update | 6    | **closed 2026-06-12（已实现）**                 | `booksource_check_update` 基于 `@updateUrl` 真实版本比较；`apply_update` 真实下载/校验/写入并保留本地 @enabled；`repository_fetch/preview/install/check_source_sync` 全部接入。详见 NET/CAP 表与迭代日志 |
| ~~CAP-SYNC~~   | sync 云同步              | 12+4 | **WebDAV closed 2026-06-12；百度/FTP 保留隐藏** | WebDAV 12 命令已实现：凭据保存（不回传明文）、连接测试、状态、push/pull/sync、冲突列表/解决、客户端状态推送、阅读进度同步、生命周期通知；百度网盘 4 命令继续 `unsupported_hidden`（用户决策保留）        |
| CAP-BROWSER    | browser_probe 浏览器探测 | 12   | unsupported_hidden（用户决策保留）              | 真实 session/导航/JS 执行/cookie/UA                                                                                                                                                                      |
| CAP-TTS        | TTS 朗读                 | 6    | blocked_by_platform（已降级浏览器 Web Speech）  | 开放需真实语音列表/播放/停止/状态/试听/错误回退                                                                                                                                                          |
| CAP-COMICCOVER | 漫画/封面缓存            | 9    | blocked_by_platform                             | 真实下载/缓存/清理/计量                                                                                                                                                                                  |
| CAP-MISC       | update/unlock/misc       | 6    | blocked/hidden（unlock 用户决策保留）           | 插件 HTTP 需方法白名单+域名/IP 限制+超时+大小限制（§20.2）；unlock challenge 需真实签名/校验/过期。AI HTTP 已由 `ai_http_proxy_request` 按白名单实现。                                                   |
| CAP-VIDEO      | video 代理               | 2    | blocked_by_platform                             | 番茄短剧视频播放（Phase 7）                                                                                                                                                                              |

（命令清单见本文件「当前前端可触达 UNSUPPORTED 模块」表。CAP-REPO 已移出。）

### C. 架构验收

| ID           | 任务                                                                                                    | 验收                                                                                                                                                                                                                                                                                                                     |
| ------------ | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| FORMB-ACCEPT | 形态 B 浏览器闭环验收（§60.3）：纯浏览器前端连远端后端，走完「书源列表→搜索→加书架→目录→正文→进度保存」 | **local headless loopback passed 2026-06-12**：`src-headless` 托管 `dist` + `/ws`，浏览器启动 0 error/0 warning，并跑通保存 fixture 书源→列表→搜索→详情→加书架→目录→正文→缓存正文→进度保存。自动回归：`cargo test -p legado-headless formb_accept_headless_dispatch_chain`。严格「另一台机器/LAN」实测仍需外部环境补跑。 |

### 待用户决策（2026-06-12 已确认：三项「都先保留」）

- 百度网盘 / FTP 同步：**保留隐藏**，暂不实现也不移除。
- browser_probe：**保留**（unsupported_hidden），暂不实现也不移除。
- unlock 完全体解锁：**保留**（unsupported_hidden），暂不实现也不移除。

口径说明：

- R-P1-004 修正前端扫描器后，`onlyBackend = none`。旧的 3 个 onlyBackend 均为 `invokeWithTimeout<T>` 多行泛型调用漏扫。
- R-P0-001 的修正后 UI/调用层口径原为 `frontend_unsupported_stub_count = 60`；CAP-REPO 后降至 52，CAP-SYNC WebDAV 后降至 40，AI-DEEPSEEK-MGZ 后降至 39。剩余 `sync_baidu_*` 4 命令由 provider 级门禁隐藏/禁用，因此 R-P0-001 仍为 closed。
- `bookshelf_export_book_data` 是前端可触达且已实现的移动端导出路径，不是后台孤儿。

## CAP-SYNC WebDAV 交接设计（下一个接手 AI 直接据此实现）

目标：把 `sync` 能力域的 **WebDAV** 部分真实化（凭据保存/连接测试/状态/手动同步/冲突）；**百度网盘/FTP 按用户决策保持隐藏**。这是 A 段全清、CAP-REPO 完成后建议优先取用的独立项（不在待用户决策之列）。

### 前端契约（已固定，后端必须照此实现，勿改前端）

配置键（app config，`SectionSync.vue` 经 `setConfig` 写入）：`sync_webdav_url`、`sync_webdav_username`、`sync_webdav_root_dir`（默认 `legado-sync`）、`sync_webdav_allow_http`（布尔，UI 存字符串，后端需同时接受 bool 与字符串，参照 NET-001 `config_bool`）。密码不走 app config，走 `sync_set_credentials`。

命令与类型（`src/composables/useSync.ts`）：

| 命令                                                                                                         | 入参                                                                               | 返回                                                                                                           |
| ------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `sync_get_status`                                                                                            | —                                                                                  | `SyncStatus{enabled,running,lastSuccessAt,lastFailedAt,lastError,dirtyDomains[],conflictCount,lastRunSummary}` |
| `sync_set_credentials`                                                                                       | `{password}`                                                                       | void                                                                                                           |
| `sync_get_credentials`                                                                                       | —                                                                                  | `SyncCredentials{password}`（建议只回 `{password:""}` 或是否已设置，勿回明文）                                 |
| `sync_clear_credentials`                                                                                     | —                                                                                  | void                                                                                                           |
| `sync_test_connection`                                                                                       | `{password?}`                                                                      | `{ok:bool,message}`                                                                                            |
| `sync_now`                                                                                                   | `{mode:"sync"\|"pull"\|"push", domains:null, conflictStrategy?:"local"\|"remote"}` | `SyncRunSummary{status,mode,domains[],uploadedDomains[],appliedDomains[],conflictCount,message}`               |
| `sync_list_conflicts`                                                                                        | —                                                                                  | `SyncConflict[]`                                                                                               |
| `sync_resolve_conflict`                                                                                      | `{conflictId,action}`                                                              | void                                                                                                           |
| `sync_client_state_set`/`sync_report_reader_session`/`sync_v2_sync_reading_progress`/`sync_notify_lifecycle` | 见 useSync.ts                                                                      | 多为 void，可先做最小实现/no-op 但需非 UNSUPPORTED                                                             |
| `sync_baidu_*`（4）/ FTP                                                                                     | —                                                                                  | **保持 UNSUPPORTED**（provider 级，见下「能力门禁」）                                                          |

### 后端实现要点

- 新建 `crates/reader-core/src/service/sync_webdav.rs`（或 `crawler/webdav.rs`）。WebDAV 用 reqwest 自定义方法：`PROPFIND`/`MKCOL`（`reqwest::Method::from_bytes`）、`PUT`、`GET`、`DELETE`，basic auth（`.basic_auth(user, Some(pass))`）。复用 `book_service.http_client()`（已带代理/超时/DoH/TLS）。
- `allow_http=false` 时拒绝非 https URL（与 `validate_network_url` 同风格新增 `validate_webdav_url`）。
- 连接测试：对 `{url}/{root_dir}/` 发 `PROPFIND Depth:0`，2xx/207 即 ok；404 则尝试 `MKCOL` 建目录再判。返回 `{ok,message}`，message 不得泄露完整凭据/敏感 header。
- 凭据：密码存何处需定（Windows 无 keychain 抽象时可存 app data 下受限文件或 `frontend_storage` 专用命名空间，**不要**明文回传给前端）。`sync_get_credentials` 只回「是否已设置」语义。
- 同步内容（`domains`/scopes，已排查清楚，共 8 个，由 `sync_scope_{scope}` 布尔开关启用，`useSync.ts:enabledScopes`）：

  | scope              | 数据来源（后端从哪取）                                                                                                                    |
  | ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
  | `bookshelf`        | 书架书目（facade 书架/`bookshelf_list` 同源 DB）                                                                                          |
  | `reading_progress` | 每本书阅读进度（facade 进度存储）                                                                                                         |
  | `booksources`      | 书源文件（`js_source_dir` 下 `.js` + legado JSON 源）                                                                                     |
  | `app_settings`     | app config（`app_config_get_all` 全量）                                                                                                   |
  | `reader_settings`  | **前端推送**：`sync_client_state_set{domain:"reader_settings",value}`，对应前端 namespace `dynamic-config.reader.defaults.lastEffective`  |
  | `source_flags`     | **前端推送**：`sync_client_state_set{domain:"source_flags",value:{exploreDisabled,searchDisabled}}`，对应 namespace `source.capabilities` |
  | `extensions`       | 待定（扩展/插件存储；grep `sync_scope_extensions` 确认，无现成推送则二期）                                                                |
  | `script_config`    | 待定（脚本 REPL/脚本配置；同上）                                                                                                          |

  关键架构点：`bookshelf`/`reading_progress`/`booksources`/`app_settings` 四个域后端**自有数据**直接读；`reader_settings`/`source_flags` 两个域是**前端在 `sync_now` 前经 `pushClientState()` → `sync_client_state_set` 推到后端内存**的——所以 `sync_client_state_set` 必须真实实现（存进进程内 `Mutex<HashMap<domain,Value>>`），`sync_now` 再把这些域 + 自有域一起序列化。`sync_now` 把每个启用域 JSON `PUT` 到 `{root}/legado/{domain}.json`，`pull` 时 `GET` 回按 `conflictStrategy` 合并；冲突入 `sync_list_conflicts`。先做 push/pull 全量覆盖（mode=push/pull），`mode=sync` 双向 + 冲突检测可二期。`SyncStatus.dirtyDomains` = 自上次成功同步后有改动的域集合（可先恒返回全部启用域，二期再做脏标记）。

- 状态：进程内 `Mutex<SyncStatus>`（last\*、running、conflictCount），`sync_get_status` 读它。

### 能力门禁（关键陷阱）

`sync` capability 是 **16 命令一个 key**。直接把 `system.rs` 的 `sync` 置 `supported:true` 会同时把 baidu/FTP 暴露出去，违反「保留隐藏」。两条路任选：

1. **拆 capability**：新增 `syncWebdav`（supported:true，含 webdav 相关命令）与保留 `sync`/`syncBaidu`（supported:false，含 baidu/ftp）。需同步改 `useCapabilities.ts` 的 `CapabilityKey` 联合类型、`fallbackCapabilities`、`SectionSync.vue` 的 provider 级门禁。
2. **provider 级门禁**（更小改动）：`sync` 置 supported:true，但 `sync_baidu_*` 命令体仍返回显式 `Err(provider_unsupported)`，且 `SectionSync.vue` 的 provider `n-select` 对 baidu/FTP 选项 `disabled`（CLEAN-003 已确认该 select 有 `:disabled`，需细化到 option 级）。契约脚本会把仍返回 UNSUPPORTED 的 baidu 命令计回 stub——需在 command-matrix 标注为 `provider_unsupported_hidden` 而非缺陷。

推荐路 1（拆 capability）更干净，契约 stub 数下降也更准确。

### 验收 / 测试

- 仿 `crates/reader-core/tests/repository.rs`：用 axum mock 一个最小 WebDAV（响应 PROPFIND 207 / PUT 201 / GET 200），离线跑通 test_connection → set_credentials → sync_now(push) → sync_now(pull) 往返。
- Gate 同 §6；契约 stub 数应下降（webdav 命令转 implemented）；`docs/command-matrix.md`、本文件、`docs/ai-iteration-log.md` 同步。

### 实施记录（2026-06-12 CAP-SYNC WebDAV）

- 已按“拆 capability”方案落地：新增 `syncWebdav` capability 为 supported；旧 `sync` capability 只覆盖百度/FTP provider 未实现命令，保持 unsupported_hidden。
- 后端新增 `service/sync_webdav.rs`，用 reqwest 自定义 `PROPFIND`/`MKCOL`/`PUT`/`GET`，复用主 HTTP client；`allow_http=false` 时拒绝 HTTP。
- `ReaderCore` 已实现凭据保存（本机 app data；`sync_get_credentials` 只回 `passwordSet`，不回明文）、连接测试、状态查询、push/pull/sync、冲突列表/解决、`reader_settings`/`source_flags` 客户端状态、阅读进度同步入口与生命周期事件校验。
- 已支持域：`bookshelf`、`reading_progress`、`booksources`、`app_settings`、`reader_settings`、`source_flags`。`extensions`、`script_config` 仍为 deferred；若用户启用会明确报错，不上传空对象冒充成功。
- 验收证据：`crates/reader-core/tests/sync_webdav.rs::webdav_sync_push_pull_round_trip` 用本地 axum mock 跑通 credentials → test_connection → push → pull；`src-tauri/tests/ws_router.rs::webdav_sync_commands_are_routed` 确认形态 B 白名单已接线。

## R 队列状态

| ID                    | 状态                 | 当前证据                                                                                                                                                                                                                                                                                                                                                                                                            |
| --------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R-P1-003              | closed               | `SectionBackup.vue` zip 过滤已改为 json，见 `reports/gates/2026-06-10-1016-R-batch1/summary.md`                                                                                                                                                                                                                                                                                                                     |
| R-P1-001              | closed               | 契约脚本逐函数分类 + self-test；当前 `browser_probe_create` 判 stub，`backup_inspect` 判 implemented                                                                                                                                                                                                                                                                                                                |
| R-P0-003              | closed for 书旗/七猫 | 书旗/七猫 full_chain 均 `live_network_pass`；番茄/番茄短剧转入 R-P2-003/004                                                                                                                                                                                                                                                                                                                                         |
| R-P1-002              | closed               | `web_server_stop_releases_port_for_restart` 回归测试，见 R-batch2 提交                                                                                                                                                                                                                                                                                                                                              |
| R-P0-002              | closed               | 本文件、`docs/command-matrix.md`、`docs/source-compat-matrix.md` 已按 2026-06-10 实测重写，旧冲突表已删除                                                                                                                                                                                                                                                                                                           |
| R-P0-001              | closed               | 修正后 60/60 个前端可触达 UNSUPPORTED stub 已逐条归档为 `unsupported_hidden` 或 `blocked_by_platform`；R-P1-004 补扫出的 2 个 sync 命令已由既有 sync 能力门禁覆盖                                                                                                                                                                                                                                                   |
| R-P1-004              | closed               | 前端扫描器已支持 `invokeWithTimeout<T>` 多行泛型调用；`onlyBackend` 从 3 修正为 0，见 `reports/gates/2026-06-10-1818-R-P1-004-contract-scanner/summary.md`                                                                                                                                                                                                                                                          |
| R-P2-001              | closed               | Android release signing 配置和文档已建立；`keystore.properties`/keystore 不入库；`:app:checkReleaseSigning` 在无密钥时按预期失败，`pnpm run build:android:release` 仍可产出 unsigned 验证包                                                                                                                                                                                                                         |
| R-P2-002              | closed               | `pnpm lint` 已从 71 warnings / 0 errors 收敛到 0 warnings / 0 errors；动态执行边界均用局部 `oxlint-disable-next-line` 标注理由，见 `reports/gates/2026-06-10-1910-R-P2-002-lint-warnings/summary.md`                                                                                                                                                                                                                |
| R-P2-003..007,009,010 | open                 | 番茄/短剧、缓存系统、Harmony 标注、`book` 对象绑定、QuickJS Runtime 复用、JS HTTP 桥线程池化；架构纪律见 `docs/frontend-backend-separation.md` 与总纲第 60 节，详见审计文档第 3 节                                                                                                                                                                                                                                  |
| R-P2-008              | in_progress          | 前后端分离 WS 服务端阶段 1+2 试点已落地：`commands/router.rs` + `ws_server.rs`；阶段 4 `src-headless` 已存在。**2026-06-12 FORMB local loopback passed**：补齐 headless 分发契约后，纯浏览器连接独立 headless 后端跑通书源→阅读进度闭环，见 `reports/gates/2026-06-12-FORMB-ACCEPT-headless-loopback/summary.md`。剩余：严格跨物理机器/LAN 实测；桌面壳内 WS 对外暴露仍不建议，优先使用 headless `--bind/--token`。 |
| R-P2-011              | closed               | 前端绕过传输层修复：prefetch.ts 已改环境分流（鸿蒙 → DOM、Tauri/WS → useEventBus），logger.ts 经评估保留直连并列入纪律文档第 4 节例外；见 `reports/gates/2026-06-10-2018-R-P2-011-transport-bypass/summary.md`                                                                                                                                                                                                      |
| R-P2-012              | closed               | 2026-06-12 PREFETCH-LIVE-BUILD：`PrefetchPayload` 使用 `{ payload }` + camelCase，reader-core 预取支持 `startIndex`/`count`、同书任务取消、进度回调；Tauri IPC emit `shelf:prefetch-progress` / `shelf:prefetch-done`；WS 转发两类事件。新增 `prefetch_chapters_respects_range_and_emits_progress`，见 `reports/gates/2026-06-12-PREFETCH-LIVE-BUILD/summary.md`。                                                  |

## 5 个争议命令定真伪

| Command                       | 当前状态                            | 证据                                                                                                                                                                                                                                                                                                        | 验证方式                                                                                                                            |
| ----------------------------- | ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `booksource_cancel`           | implemented_with_limit              | `src-tauri/src/commands/source.rs` 对 `booksource_chapter_list`、`booksource_chapter_content` 注册 `TaskRegistry` token；`src-tauri/src/commands/bookshelf.rs` 对 `bookshelf_prefetch_chapters` 注册 token；`crates/reader-core/src/facade.rs` 的预取循环检查 token。限制：不能抢占已经进入的单次网络请求。 | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；相关长任务源码检查                                          |
| `booksource_purchase_chapter` | implemented_or_explicit_unsupported | JS 书源路径调用 runtime `purchaseChapter(chapterUrl)`；Legado 规则源返回 `{ ok:false, purchased:false, unsupported:true }`，不再假成功。                                                                                                                                                                    | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::purchase_chapter`        |
| `booksource_call_fn`          | implemented_for_js_source           | JS 书源路径调用 runtime 命名函数；Legado 规则源返回明确错误 `不支持自定义 JS 函数调用`，不是静默成功。                                                                                                                                                                                                      | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::source_call_fn`          |
| `booksource_run_tests`        | implemented                         | 支持 `step_filter`、`timeout_secs`、逐 step timeout，并按 search -> bookInfo -> toc -> content -> explore 真实执行链路。                                                                                                                                                                                    | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::run_source_tests`        |
| `storage_debug_dump`          | implemented_summary                 | 读取 frontend namespaces、app config key 数、书架数量和真实路径摘要，不再返回固定空对象。                                                                                                                                                                                                                   | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`src-tauri/src/commands/config.rs` -> `facade.debug_dump()` |

## 当前前端可触达 UNSUPPORTED 模块

R-P0-001 的契约口径经 R-P1-004 修正为 60 个前端可触达 stub；本轮已全部接入 `capabilities_get` + `useCapabilities`，并在 UI/调用层按模块禁用、隐藏、降级或 no-op。2026-06-12 CAP-REPO 真实实现 repository/source_update 6 命令后，stub 降至 52；CAP-SYNC WebDAV 真实实现 12 命令后，stub 降至 40；AI-DEEPSEEK-MGZ 实现 AI HTTP 代理后降至 39。仓库/更新、WebDAV 同步与 AI HTTP 代理已移出本表。注意：本表只关闭“点击后直撞 UNSUPPORTED”的入口裸露问题，不代表后端缓存、解锁等剩余能力已经实现。

| 模块               | 数量 | 当前处置            | 命令                                                                                                                                                                                                                                                                                                           |
| ------------------ | ---: | ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| sync provider      |    4 | unsupported_hidden  | `sync_baidu_start_auth`, `sync_baidu_token_status`, `sync_baidu_poll_token`, `sync_baidu_revoke_auth`                                                                                                                                                                                                          |
| tts                |    6 | blocked_by_platform | `tts_stop`, `tts_is_initialized`, `tts_is_speaking`, `tts_speak`, `tts_get_voices`, `tts_preview_voice`                                                                                                                                                                                                        |
| video              |    2 | blocked_by_platform | `start_video_proxy`, `stop_video_proxy`                                                                                                                                                                                                                                                                        |
| browser_probe      |   12 | unsupported_hidden  | `browser_probe_create`, `browser_probe_navigate`, `browser_probe_eval`, `browser_probe_run`, `browser_probe_get_cookies`, `browser_probe_set_cookie`, `browser_probe_set_user_agent`, `browser_probe_clear_data`, `browser_probe_show`, `browser_probe_hide`, `browser_probe_close`, `browser_probe_close_all` |
| comic_cover        |    9 | blocked_by_platform | `comic_download_images`, `comic_get_page_sizes`, `comic_get_cached_page`, `comic_cache_clear_chapter`, `comic_cache_clear`, `comic_cache_size`, `cover_resolve_cache`, `cover_cache_size`, `cover_cache_clear`                                                                                                 |
| update/unlock/misc |    6 | blocked/hidden      | `explore_clear_cache` 为 `blocked_by_platform` 降级；`frontend_plugin_http_request`、`issue_*unlock*`、`verify_*unlock*` 为 `unsupported_hidden`                                                                                                                                                               |

> ~~repository/source_update（6）~~ 已于 2026-06-12（CAP-REPO）真实实现并移出本表，`repository` capability 置 supported。
> ~~sync WebDAV（12）~~ 已于 2026-06-12（CAP-SYNC）真实实现并移出本表，新增 `syncWebdav` capability 置 supported；旧 `sync` capability 仅保留百度/FTP provider 未实现命令。
> ~~AI HTTP 代理（1）~~ 已于 2026-06-12（AI-DEEPSEEK-MGZ）真实实现并移出本表，新增 `ai_http_proxy_request`，`aiProxy` capability 置 supported。

## 下轮第一件事

前后端接入审计已结清，**路线图 A 段（环境/网络阻塞项）全部结清**，B 段 **CAP-REPO 与 CAP-SYNC WebDAV 已结清**，C 段 **FORMB-ACCEPT 本机 headless loopback 已通过**，本轮 `R-P2-012/PREFETCH-PROGRESS` 已结清：

- A 段：NET-004-LIVE（DoH 实测修 360dns/onedns）、NET-005（DoH 接入 JS 桥）、SRC-FANQIE-LIVE（番茄 bookInfo 字段验收 + 引擎字段管线两处保真修复）。
- B 段：CAP-REPO（书源仓库 + `@updateUrl` 在线更新，6 命令真实实现，stub 58→52）；CAP-SYNC WebDAV（12 命令真实实现，stub 52→40）；AI-DEEPSEEK-MGZ（AI HTTP 代理 1 命令真实实现，stub 40→39）。
- C 段：FORMB-ACCEPT（纯浏览器 + 独立 `legado-headless` + WS；本机 loopback 业务闭环通过，跨机器/LAN 实测待外部环境）。
- R-P2-012：预取进度链路已完成，Tauri IPC 有 progress/done 事件，WS 转发同名事件；见 `reports/gates/2026-06-12-PREFETCH-LIVE-BUILD/summary.md`。

用户决策（2026-06-12）：百度网盘/FTP 同步、browser_probe、完全体 unlock 三项「都先保留」（不移除）。

下一轮第一件事：若有第二台设备或可访问 LAN，做 `FORMB-LAN-VERIFY`（headless `--bind 0.0.0.0 --token <token>` + 浏览器 `?ws=ws://<host>:<port>/ws?token=<token>` 复跑同一闭环）；若当前环境无外部设备，则转 B 段剩余能力本体 `CAP-BROWSER`（真实 session/导航/JS/cookie/UA）。

CAP-REPO 与 CAP-SYNC WebDAV 的形态 B WS 路由也已补齐（repository 6 命令 + WebDAV 12 命令入 `router.rs` 白名单；`ws_router.rs` 覆盖 repository 与 WebDAV 路由，11/11）。

下一步候选（B 段剩余 / C 段）：

1. C 段 **FORMB-LAN-VERIFY** 严格跨机器/LAN 形态 B 验收；需要第二台设备或可访问 LAN，命中用户/环境 blocker 时跳过。
2. B 段 **CAP-BROWSER**（真实 session/导航/JS/cookie/UA），以及 CAP-TTS/CAP-COMICCOVER/CAP-MISC/CAP-VIDEO。均为大特性，按审计文档第 4 节处置规则实现，同步 `capabilities_get` + 前端入口 + `docs/command-matrix.md`。

历史项 R-P2-003（番茄 JS API 缺口）已并入并随 SRC-FANQIE-LIVE 结清。

## 2026-06-13 MAINT-IMPORT-UI-PERF 状态更新

- 书源管理标题竖排问题已回归验证：1000x800 与 390x800 下标题均为横向 `80x32`，按钮区无重叠。
- 喵公子订阅导入链路已优化：`rsssource` 解析出的书源 JSON 内容会复用于导入，避免重复下载；订阅页解析使用 4 路受控并发。
- 喵公子订阅实网非破坏验证通过：当前订阅解析出 10 个有效书源包。
- 书源性能提示弹窗已从常驻 `n-dialog` 改为受控 `n-modal`，避免遮挡页面且按钮关闭无效。
- 本轮门禁通过；命令契约维持 frontendTotal=162、registeredTotal=161、bothCount=161、onlyBackend=0、frontend unsupported stub=39。

下一轮优先候选：继续 UI 收口，处理窄屏侧栏占宽与前端大 chunk 拆分；若需要做更大能力本体，则按文档转 `CAP-BROWSER`。

## 2026-06-13 UI-NARROW-SHELL 状态更新

- 已修复窄视口 shell 判定：自动布局现在同时考虑移动 UA/粗指针和 `(max-width: 640px)`，桌面浏览器缩到 390px 时会进入移动 shell。
- 用户显式布局模式覆盖保持不变：`desktop` 仍强制桌面，`mobile` 仍强制移动，`auto` 才使用新窄屏判定。
- 390x800 实测：`app-layout--mobile`，侧栏不存在，底部导航可见，主内容宽度 390px，横向溢出 0。
- 390x800 书源管理实测：`书源管理` 可见 `h1` 为 80x32，横向排版，按钮无重叠。
- 1000x800 实测：仍为桌面 shell，侧栏可见，主内容 800px，未回退。
- 本轮门禁通过：`pnpm lint`、`pnpm build`、`cargo check -p reader-core`、`cargo check -p legado-tauri`、`cargo test -p reader-core`、命令契约检查、`git diff --check`。

下一轮优先候选：继续 UI 回归设置页/阅读器设置/书源管理按钮行，随后处理前端大 chunk 和 `useTransport` 无效动态导入。

## 2026-06-13 UI-READER-SETTINGS-LAYOUT 状态更新

- 已修复阅读器关闭后透明 `n-modal` 残留拦截点击的问题：阅读器 modal 关闭时直接卸载，打开后不再停留在 `opacity=0`。
- 已修复阅读器菜单在后台/自动化环境中停在过渡初始位移的问题：顶部栏、底栏和遮罩改为按状态直接挂载，菜单按钮可正常点击。
- 已收口阅读器设置面板窄屏布局：颜色、背景、皮肤、字号、翻页按钮和更多设置入口在 390px 下无横向溢出、无文本裁切。
- 已将主设置页移动端入口圆角收敛为 8px，并验证 `overflowX=0`。
- 本轮门禁通过：`oxfmt --check .`、`pnpm lint`、`pnpm build`、`cargo check -p reader-core`、`cargo check -p legado-tauri`、`cargo test -p reader-core`、命令契约检查、`git diff --check`。

下一轮优先候选：继续 UI 回归书源管理批量导入/AI 写书源/段评抽屉；随后处理前端大 chunk 和 `useTransport` 无效动态导入。
