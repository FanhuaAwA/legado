# AI Task Status

本文件记录当前 R 队列状态。事实数字只以当轮命令输出为准，不沿用历史表格。

最后实测：2026-06-11（NET-001/002 轮，网络设置配置接入；commit 82590e0）。下方基线已按当轮 `check-command-contract.mjs --json` 输出刷新。

实测命令：

```powershell
git status --short
node scripts/ci/check-command-contract.mjs --json
node scripts/ci/check-command-contract.mjs
pnpm exec oxfmt --check .
pnpm lint
pnpm build
cargo check -p reader-core
cargo check -p legado-tauri
cargo test -p reader-core
```

## 当前基线

```text
project.status = incomplete
command_contract.frontendTotal = 162
command_contract.registeredTotal = 161
command_contract.bothCount = 161
command_contract.onlyFrontend = js_eval
command_contract.onlyBackend = none
command_contract.frontend_unsupported_stub_count = 58
command_contract.frontend_implemented_count = 103
command_contract.classificationScope = frontend-facing registered commands
frontend.lint = passed_zero_warnings
```

口径变更（2026-06-11）：stub 数由 60 降至 58，差额来自上一轮 `UI-REMOVE-APP-UPDATE` 删除 `app_update_*`（2 个）。总纲/审计旧文档写「60」已过期，以本实测 58 为准。

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

### B. 60 个已隐藏/降级后端能力本体（§14 缺口，每项大特性，单独立项）

实现前必读审计文档第 4 节处置规则。**含可能不需要的能力，动手前先与用户确认取舍（见下「待用户决策」）。**

| ID             | 能力域                   | 数量 | 当前处置                                       | 实现要点（审计文档第 4 节）                                                                                    |
| -------------- | ------------------------ | ---- | ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| CAP-SYNC       | sync 云同步              | 16   | unsupported_hidden                             | WebDAV 至少做到凭据保存/连接测试/状态查询/手动同步；百度网盘/FTP 不做则保持隐藏                                |
| CAP-BROWSER    | browser_probe 浏览器探测 | 12   | unsupported_hidden                             | 真实 session/导航/JS 执行/cookie/UA；不保留则从入口/配置/文档全部移除                                          |
| CAP-TTS        | TTS 朗读                 | 6    | blocked_by_platform（已降级浏览器 Web Speech） | 开放需真实语音列表/播放/停止/状态/试听/错误回退                                                                |
| CAP-COMICCOVER | 漫画/封面缓存            | 9    | blocked_by_platform                            | 真实下载/缓存/清理/计量                                                                                        |
| CAP-REPO       | repository/source_update | 6    | unsupported_hidden                             | `booksource_check_update` 基于 `updateUrl` 真实比较；`apply_update` 真实下载/校验/写入/备份回滚                |
| CAP-MISC       | update/unlock/misc       | 7    | blocked/hidden                                 | AI/插件 HTTP 需方法白名单+域名/IP 限制+超时+大小限制（§20.2）；unlock challenge 需真实签名/校验/过期，否则下架 |
| CAP-VIDEO      | video 代理               | 2    | blocked_by_platform                            | 番茄短剧视频播放（Phase 7）                                                                                    |

（命令清单见本文件「当前前端可触达 UNSUPPORTED 模块」表。）

### C. 架构验收

| ID           | 任务                                                                                                    | 验收                                                                                                                  |
| ------------ | ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| FORMB-ACCEPT | 形态 B 浏览器闭环验收（§60.3）：纯浏览器前端连远端后端，走完「书源列表→搜索→加书架→目录→正文→进度保存」 | 闭环通过；途中 NOT_ROUTED 命令逐个评估加入 `router.rs` 白名单并补 `ws_router.rs` 测试；桌面独占入口被能力门禁正确隐藏 |

### 待用户决策（动手前确认，避免做不需要的功能）

- 是否实现百度网盘 / FTP 同步（用户已主动删除应用更新功能，类似的非必需能力可能也不需要）。
- browser_probe 是「真实实现」还是「彻底从前端/配置/文档移除」。
- unlock 完全体解锁是「真实实现签名校验」还是「下架」。

口径说明：

- R-P1-004 修正前端扫描器后，`onlyBackend = none`。旧的 3 个 onlyBackend 均为 `invokeWithTimeout<T>` 多行泛型调用漏扫。
- R-P0-001 的修正后 UI/调用层口径为 `frontend_unsupported_stub_count = 60`。新增计入的 `sync_baidu_start_auth`、`sync_baidu_token_status` 已在 `SectionSync.vue` 的 sync 能力门禁下隐藏/禁用，因此 R-P0-001 仍为 closed。
- `bookshelf_export_book_data` 是前端可触达且已实现的移动端导出路径，不是后台孤儿。

## R 队列状态

| ID                    | 状态                 | 当前证据                                                                                                                                                                                                                                                                                                                    |
| --------------------- | -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R-P1-003              | closed               | `SectionBackup.vue` zip 过滤已改为 json，见 `reports/gates/2026-06-10-1016-R-batch1/summary.md`                                                                                                                                                                                                                             |
| R-P1-001              | closed               | 契约脚本逐函数分类 + self-test；当前 `browser_probe_create` 判 stub，`backup_inspect` 判 implemented                                                                                                                                                                                                                        |
| R-P0-003              | closed for 书旗/七猫 | 书旗/七猫 full_chain 均 `live_network_pass`；番茄/番茄短剧转入 R-P2-003/004                                                                                                                                                                                                                                                 |
| R-P1-002              | closed               | `web_server_stop_releases_port_for_restart` 回归测试，见 R-batch2 提交                                                                                                                                                                                                                                                      |
| R-P0-002              | closed               | 本文件、`docs/command-matrix.md`、`docs/source-compat-matrix.md` 已按 2026-06-10 实测重写，旧冲突表已删除                                                                                                                                                                                                                   |
| R-P0-001              | closed               | 修正后 60/60 个前端可触达 UNSUPPORTED stub 已逐条归档为 `unsupported_hidden` 或 `blocked_by_platform`；R-P1-004 补扫出的 2 个 sync 命令已由既有 sync 能力门禁覆盖                                                                                                                                                           |
| R-P1-004              | closed               | 前端扫描器已支持 `invokeWithTimeout<T>` 多行泛型调用；`onlyBackend` 从 3 修正为 0，见 `reports/gates/2026-06-10-1818-R-P1-004-contract-scanner/summary.md`                                                                                                                                                                  |
| R-P2-001              | closed               | Android release signing 配置和文档已建立；`keystore.properties`/keystore 不入库；`:app:checkReleaseSigning` 在无密钥时按预期失败，`pnpm run build:android:release` 仍可产出 unsigned 验证包                                                                                                                                 |
| R-P2-002              | closed               | `pnpm lint` 已从 71 warnings / 0 errors 收敛到 0 warnings / 0 errors；动态执行边界均用局部 `oxlint-disable-next-line` 标注理由，见 `reports/gates/2026-06-10-1910-R-P2-002-lint-warnings/summary.md`                                                                                                                        |
| R-P2-003..007,009,010 | open                 | 番茄/短剧、缓存系统、Harmony 标注、`book` 对象绑定、QuickJS Runtime 复用、JS HTTP 桥线程池化；架构纪律见 `docs/frontend-backend-separation.md` 与总纲第 60 节，详见审计文档第 3 节                                                                                                                                          |
| R-P2-008              | in_progress          | 前后端分离 WS 服务端阶段 1+2 试点已落地：`commands/router.rs`（62 命令白名单路由，复用原命令函数）+ `ws_server.rs`（127.0.0.1:7688 `/ws` + 事件转发）；9 集成测试 + 真实 exe 实连冒烟全过，见 `reports/gates/2026-06-10-2051-R-P2-008-ws-pilot/summary.md`；剩余：浏览器闭环验收、LAN/token（阶段 3）、无头二进制（阶段 4） |
| R-P2-011              | closed               | 前端绕过传输层修复：prefetch.ts 已改环境分流（鸿蒙 → DOM、Tauri/WS → useEventBus），logger.ts 经评估保留直连并列入纪律文档第 4 节例外；见 `reports/gates/2026-06-10-2018-R-P2-011-transport-bypass/summary.md`                                                                                                              |
| R-P2-012              | open                 | 预取进度链路在所有传输方式下断裂：前端发 `{ payload }` 而后端参数名 `request`（按键取参必失败）；全仓库无任何代码 emit `shelf:prefetch-progress` / `shelf:prefetch-done`，监听是死路径。修法见审计文档 R-P2-012 行                                                                                                          |

## 5 个争议命令定真伪

| Command                       | 当前状态                            | 证据                                                                                                                                                                                                                                                                                                        | 验证方式                                                                                                                            |
| ----------------------------- | ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `booksource_cancel`           | implemented_with_limit              | `src-tauri/src/commands/source.rs` 对 `booksource_chapter_list`、`booksource_chapter_content` 注册 `TaskRegistry` token；`src-tauri/src/commands/bookshelf.rs` 对 `bookshelf_prefetch_chapters` 注册 token；`crates/reader-core/src/facade.rs` 的预取循环检查 token。限制：不能抢占已经进入的单次网络请求。 | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；相关长任务源码检查                                          |
| `booksource_purchase_chapter` | implemented_or_explicit_unsupported | JS 书源路径调用 runtime `purchaseChapter(chapterUrl)`；Legado 规则源返回 `{ ok:false, purchased:false, unsupported:true }`，不再假成功。                                                                                                                                                                    | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::purchase_chapter`        |
| `booksource_call_fn`          | implemented_for_js_source           | JS 书源路径调用 runtime 命名函数；Legado 规则源返回明确错误 `不支持自定义 JS 函数调用`，不是静默成功。                                                                                                                                                                                                      | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::source_call_fn`          |
| `booksource_run_tests`        | implemented                         | 支持 `step_filter`、`timeout_secs`、逐 step timeout，并按 search -> bookInfo -> toc -> content -> explore 真实执行链路。                                                                                                                                                                                    | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::run_source_tests`        |
| `storage_debug_dump`          | implemented_summary                 | 读取 frontend namespaces、app config key 数、书架数量和真实路径摘要，不再返回固定空对象。                                                                                                                                                                                                                   | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`src-tauri/src/commands/config.rs` -> `facade.debug_dump()` |

## 当前前端可触达 UNSUPPORTED 模块

R-P0-001 的契约口径经 R-P1-004 修正为 60 个前端可触达 stub；本轮已全部接入 `capabilities_get` + `useCapabilities`，并在 UI/调用层按模块禁用、隐藏、降级或 no-op。注意：这只关闭“点击后直撞 UNSUPPORTED”的入口裸露问题，不代表后端缓存、仓库、更新、解锁等能力已经实现。

| 模块                     | 数量 | 当前处置            | 命令                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------ | ---: | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| sync                     |   16 | unsupported_hidden  | `sync_baidu_start_auth`, `sync_baidu_token_status`, `sync_baidu_poll_token`, `sync_baidu_revoke_auth`, `sync_client_state_set`, `sync_get_status`, `sync_set_credentials`, `sync_clear_credentials`, `sync_get_credentials`, `sync_test_connection`, `sync_now`, `sync_list_conflicts`, `sync_resolve_conflict`, `sync_report_reader_session`, `sync_v2_sync_reading_progress`, `sync_notify_lifecycle` |
| tts                      |    6 | blocked_by_platform | `tts_stop`, `tts_is_initialized`, `tts_is_speaking`, `tts_speak`, `tts_get_voices`, `tts_preview_voice`                                                                                                                                                                                                                                                                                                 |
| video                    |    2 | blocked_by_platform | `start_video_proxy`, `stop_video_proxy`                                                                                                                                                                                                                                                                                                                                                                 |
| browser_probe            |   12 | unsupported_hidden  | `browser_probe_create`, `browser_probe_navigate`, `browser_probe_eval`, `browser_probe_run`, `browser_probe_get_cookies`, `browser_probe_set_cookie`, `browser_probe_set_user_agent`, `browser_probe_clear_data`, `browser_probe_show`, `browser_probe_hide`, `browser_probe_close`, `browser_probe_close_all`                                                                                          |
| comic_cover              |    9 | blocked_by_platform | `comic_download_images`, `comic_get_page_sizes`, `comic_get_cached_page`, `comic_cache_clear_chapter`, `comic_cache_clear`, `comic_cache_size`, `cover_resolve_cache`, `cover_cache_size`, `cover_cache_clear`                                                                                                                                                                                          |
| repository/source_update |    6 | unsupported_hidden  | `booksource_check_update`, `booksource_apply_update`, `repository_fetch`, `repository_install`, `repository_preview_source`, `repository_check_source_sync`                                                                                                                                                                                                                                             |
| update/unlock/misc       |    7 | blocked/hidden      | `ai_http_proxy_url` / `explore_clear_cache` 为 `blocked_by_platform` 降级；`frontend_plugin_http_request`、`issue_*unlock*`、`verify_*unlock*` 为 `unsupported_hidden`                                                                                                                                                                                                                                  |

## 下轮第一件事

前后端接入审计已结清，**路线图 A 段（环境/网络阻塞项）也已全部结清**：NET-004-LIVE（DoH 实测修 360dns/onedns）、NET-005（DoH 接入 JS 桥）、SRC-FANQIE-LIVE（番茄 bookInfo 字段验收 + 引擎字段管线两处保真修复）均 closed 并推送（commit 82590e0~6245335 + 番茄轮）。

下一步只剩 B 段（隐藏后端能力真实化）与 C 段（FORMB-ACCEPT 形态 B 浏览器闭环）：

1. **B 段动手前必须先与用户确认「待用户决策」三项**（百度网盘/FTP 同步、browser_probe、unlock 取舍）——用户已主动删过应用更新功能，部分能力可能不需要。确认后按审计文档第 4 节处置规则实现，同步 `capabilities_get` + 前端入口 + `docs/command-matrix.md`。
2. C 段 FORMB-ACCEPT 不依赖新功能/网络，可独立推进。
3. B 段中 CAP-REPO（书源更新真实化）、CAP-SYNC WebDAV 不在「待用户决策」之列，相对独立，可优先取用。

历史项 R-P2-003（番茄 JS API 缺口）已并入并随 SRC-FANQIE-LIVE 结清。
