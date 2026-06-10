# AI Task Status

本文件记录当前 R 队列状态。事实数字只以当轮命令输出为准，不沿用历史表格。

最后实测：2026-06-10 11:43 +0800

实测命令：

```powershell
git status --short
node scripts/ci/check-command-contract.mjs --json
node scripts/ci/check-command-contract.mjs
```

## 当前基线

```text
project.status = incomplete
command_contract.frontendTotal = 160
command_contract.registeredTotal = 162
command_contract.bothCount = 159
command_contract.onlyFrontend = js_eval
command_contract.onlyBackend = bookshelf_export_book_data, sync_baidu_start_auth, sync_baidu_token_status
command_contract.registered_unsupported_stub_count = 60
command_contract.registered_implemented_count = 102
command_contract.frontend_unsupported_stub_count = 58
command_contract.frontend_implemented_count = 101
command_contract.classificationScope = frontend-facing registered commands
```

口径说明：

- R-P0-001 的 UI 入口处理以 `frontend_unsupported_stub_count = 58` 为准。
- `registered_unsupported_stub_count = 60` 额外包含两个后台孤儿 sync stub：`sync_baidu_start_auth`、`sync_baidu_token_status`。
- `bookshelf_export_book_data` 是后台孤儿但已实现，归入 R-P1-004 处置。

## R 队列状态

| ID            | 状态                 | 当前证据                                                                                                  |
| ------------- | -------------------- | --------------------------------------------------------------------------------------------------------- |
| R-P1-003      | closed               | `SectionBackup.vue` zip 过滤已改为 json，见 `reports/gates/2026-06-10-1016-R-batch1/summary.md`           |
| R-P1-001      | closed               | 契约脚本逐函数分类 + self-test；当前 `browser_probe_create` 判 stub，`backup_inspect` 判 implemented      |
| R-P0-003      | closed for 书旗/七猫 | 书旗/七猫 full_chain 均 `live_network_pass`；番茄/番茄短剧转入 R-P2-003/004                               |
| R-P1-002      | closed               | `web_server_stop_releases_port_for_restart` 回归测试，见 R-batch2 提交                                    |
| R-P0-002      | closed               | 本文件、`docs/command-matrix.md`、`docs/source-compat-matrix.md` 已按 2026-06-10 实测重写，旧冲突表已删除 |
| R-P0-001      | open                 | 58 个前端可触达 UNSUPPORTED stub 仍需要集中式能力声明 + UI 隐藏/禁用                                      |
| R-P1-004      | open                 | 3 个 onlyBackend 命令仍需逐个处置                                                                         |
| R-P2-001..007 | open                 | Android 签名、lint warnings、番茄/短剧、缓存系统、Harmony 标注、`book` 对象绑定等                         |

## 5 个争议命令定真伪

| Command                       | 当前状态                            | 证据                                                                                                                                                                                                                                                                                                        | 验证方式                                                                                                                            |
| ----------------------------- | ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `booksource_cancel`           | implemented_with_limit              | `src-tauri/src/commands/source.rs` 对 `booksource_chapter_list`、`booksource_chapter_content` 注册 `TaskRegistry` token；`src-tauri/src/commands/bookshelf.rs` 对 `bookshelf_prefetch_chapters` 注册 token；`crates/reader-core/src/facade.rs` 的预取循环检查 token。限制：不能抢占已经进入的单次网络请求。 | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；相关长任务源码检查                                          |
| `booksource_purchase_chapter` | implemented_or_explicit_unsupported | JS 书源路径调用 runtime `purchaseChapter(chapterUrl)`；Legado 规则源返回 `{ ok:false, purchased:false, unsupported:true }`，不再假成功。                                                                                                                                                                    | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::purchase_chapter`        |
| `booksource_call_fn`          | implemented_for_js_source           | JS 书源路径调用 runtime 命名函数；Legado 规则源返回明确错误 `不支持自定义 JS 函数调用`，不是静默成功。                                                                                                                                                                                                      | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::source_call_fn`          |
| `booksource_run_tests`        | implemented                         | 支持 `step_filter`、`timeout_secs`、逐 step timeout，并按 search -> bookInfo -> toc -> content -> explore 真实执行链路。                                                                                                                                                                                    | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::run_source_tests`        |
| `storage_debug_dump`          | implemented_summary                 | 读取 frontend namespaces、app config key 数、书架数量和真实路径摘要，不再返回固定空对象。                                                                                                                                                                                                                   | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`src-tauri/src/commands/config.rs` -> `facade.debug_dump()` |

## 当前前端可触达 UNSUPPORTED 模块

R-P0-001 下一步按模块处理以下 58 个命令。处理结果必须进入集中式能力声明，不允许在组件里零散复制判断。

| 模块                     | 数量 | 命令                                                                                                                                                                                                                                                                                                                                                |
| ------------------------ | ---: | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| sync                     |   14 | `sync_baidu_poll_token`, `sync_baidu_revoke_auth`, `sync_client_state_set`, `sync_get_status`, `sync_set_credentials`, `sync_clear_credentials`, `sync_get_credentials`, `sync_test_connection`, `sync_now`, `sync_list_conflicts`, `sync_resolve_conflict`, `sync_report_reader_session`, `sync_v2_sync_reading_progress`, `sync_notify_lifecycle` |
| tts                      |    6 | `tts_stop`, `tts_is_initialized`, `tts_is_speaking`, `tts_speak`, `tts_get_voices`, `tts_preview_voice`                                                                                                                                                                                                                                             |
| video                    |    2 | `start_video_proxy`, `stop_video_proxy`                                                                                                                                                                                                                                                                                                             |
| browser_probe            |   12 | `browser_probe_create`, `browser_probe_navigate`, `browser_probe_eval`, `browser_probe_run`, `browser_probe_get_cookies`, `browser_probe_set_cookie`, `browser_probe_set_user_agent`, `browser_probe_clear_data`, `browser_probe_show`, `browser_probe_hide`, `browser_probe_close`, `browser_probe_close_all`                                      |
| comic_cover              |    9 | `comic_download_images`, `comic_get_page_sizes`, `comic_get_cached_page`, `comic_cache_clear_chapter`, `comic_cache_clear`, `comic_cache_size`, `cover_resolve_cache`, `cover_cache_size`, `cover_cache_clear`                                                                                                                                      |
| repository/source_update |    6 | `booksource_check_update`, `booksource_apply_update`, `repository_fetch`, `repository_install`, `repository_preview_source`, `repository_check_source_sync`                                                                                                                                                                                         |
| update/unlock/misc       |    9 | `ai_http_proxy_url`, `app_update_download`, `app_update_install_downloaded_file`, `frontend_plugin_http_request`, `explore_clear_cache`, `issue_full_mode_challenge`, `verify_full_mode_challenge`, `issue_scoped_unlock_challenge`, `verify_scoped_unlock_challenge`                                                                               |

## 下轮第一件事

R-P0-001：设计集中式能力声明机制，先覆盖 sync/tts/video 这批前端入口，再逐模块推进 browser_probe、comic/cover、repository/source_update、update/unlock/misc。
