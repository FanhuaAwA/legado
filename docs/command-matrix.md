# Command Matrix

本文件由 `scripts/ci/check-command-contract.mjs` 的实测结果半自动重建。旧的 2026-06-09 手工矩阵已删除，后续不得再手工沿用过期统计。

最后实测：2026-06-12 CAP-SYNC WebDAV 轮

实测命令：

```powershell
node scripts/ci/check-command-contract.mjs --json
node scripts/ci/check-command-contract.mjs
```

## 统计口径

| 指标                              | 数值 | 说明                                                             |
| --------------------------------- | ---: | ---------------------------------------------------------------- |
| frontendTotal                     |  162 | 前端 invoke 调用去重后数量                                       |
| registeredTotal                   |  161 | `generate_handler!` 注册命令数量                                 |
| bothCount                         |  161 | 前后端同名匹配数量                                               |
| onlyFrontend                      |    1 | `js_eval`，安全阻断，有意不注册                                  |
| onlyBackend                       |    0 | 无                                                               |
| registered_implemented_count      |  121 | 全部已注册命令中的实现数量                                       |
| registered_unsupported_stub_count |   40 | 全部已注册命令中的 UNSUPPORTED stub                              |
| frontend_implemented_count        |  121 | 前端可触达且已实现                                               |
| frontend_unsupported_stub_count   |   40 | 前端可触达但仅返回 UNSUPPORTED（CAP-REPO 后 52，CAP-SYNC 后 40） |

`classification` 数组的口径是 `frontend-facing registered commands`。需要全注册命令时使用 `registeredClassification`。

## Frontend Only

| Command   | 状态             | 处置                             |
| --------- | ---------------- | -------------------------------- |
| `js_eval` | security_blocked | 有意不注册，禁止作为缺失命令处理 |

## Backend Only

R-P1-004 修正前端扫描器后，当前无 backend-only 命令。旧表中的 3 个命令均为 `invokeWithTimeout<T>` 多行泛型调用漏扫：`bookshelf_export_book_data` 是移动端导出前端路径；`sync_baidu_start_auth` 与 `sync_baidu_token_status` 是同步设置页前端路径，并已由 sync 能力门禁隐藏。

## 争议命令裁决

| Command                       | 当前状态                            | 裁决                                                                      |
| ----------------------------- | ----------------------------------- | ------------------------------------------------------------------------- |
| `booksource_cancel`           | implemented_with_limit              | 真实接入 `TaskRegistry`，不是假取消；限制是不能抢占已经进入的单次网络请求 |
| `booksource_purchase_chapter` | implemented_or_explicit_unsupported | JS 源调用真实函数；Legado 规则源返回显式不支持，不再固定成功              |
| `booksource_call_fn`          | implemented_for_js_source           | JS 源调用真实函数；Legado 规则源返回明确错误                              |
| `booksource_run_tests`        | implemented                         | 支持 step filter、timeout 和真实链路执行                                  |
| `storage_debug_dump`          | implemented_summary                 | 读取真实 frontend namespace、app config、书架数量和路径摘要               |

## Frontend-Facing Unsupported Stubs

这些命令是 R-P0-001 的 UI 入口隐藏/禁用目标。逐条结果必须保持为 `implemented`、`unsupported_hidden` 或 `blocked_by_platform`；后端仍为 stub 的功能若未来实现，需要同步把对应项改为 `implemented`。

> 2026-06-12（CAP-REPO）：`repository/source_update` 6 命令已真实实现（`booksource_check_update`/`booksource_apply_update`/`repository_fetch`/`repository_install`/`repository_preview_source`/`repository_check_source_sync`），`repository` capability 置 `supported: true`，已从本表移出、计入「Implemented」。stub 数 58→52。
>
> 2026-06-12（CAP-SYNC WebDAV）：WebDAV 同步 12 命令已真实实现（凭据保存/只回是否已设置、连接测试、状态、push/pull/sync、冲突列表/解决、客户端状态推送、阅读进度同步、生命周期通知），新增 `syncWebdav` capability supported，旧 `sync` capability 仅保留百度/FTP provider 未实现命令。stub 数 52→40。

| 模块               | Command                          | 当前处置            |
| ------------------ | -------------------------------- | ------------------- |
| sync provider      | `sync_baidu_start_auth`          | unsupported_hidden  |
| sync provider      | `sync_baidu_token_status`        | unsupported_hidden  |
| sync provider      | `sync_baidu_poll_token`          | unsupported_hidden  |
| sync provider      | `sync_baidu_revoke_auth`         | unsupported_hidden  |
| tts                | `tts_stop`                       | blocked_by_platform |
| tts                | `tts_is_initialized`             | blocked_by_platform |
| tts                | `tts_is_speaking`                | blocked_by_platform |
| tts                | `tts_speak`                      | blocked_by_platform |
| tts                | `tts_get_voices`                 | blocked_by_platform |
| tts                | `tts_preview_voice`              | blocked_by_platform |
| video              | `start_video_proxy`              | blocked_by_platform |
| video              | `stop_video_proxy`               | blocked_by_platform |
| browser_probe      | `browser_probe_create`           | unsupported_hidden  |
| browser_probe      | `browser_probe_navigate`         | unsupported_hidden  |
| browser_probe      | `browser_probe_eval`             | unsupported_hidden  |
| browser_probe      | `browser_probe_run`              | unsupported_hidden  |
| browser_probe      | `browser_probe_get_cookies`      | unsupported_hidden  |
| browser_probe      | `browser_probe_set_cookie`       | unsupported_hidden  |
| browser_probe      | `browser_probe_set_user_agent`   | unsupported_hidden  |
| browser_probe      | `browser_probe_clear_data`       | unsupported_hidden  |
| browser_probe      | `browser_probe_show`             | unsupported_hidden  |
| browser_probe      | `browser_probe_hide`             | unsupported_hidden  |
| browser_probe      | `browser_probe_close`            | unsupported_hidden  |
| browser_probe      | `browser_probe_close_all`        | unsupported_hidden  |
| comic_cover        | `comic_download_images`          | blocked_by_platform |
| comic_cover        | `comic_get_page_sizes`           | blocked_by_platform |
| comic_cover        | `comic_get_cached_page`          | blocked_by_platform |
| comic_cover        | `comic_cache_clear_chapter`      | blocked_by_platform |
| comic_cover        | `comic_cache_clear`              | blocked_by_platform |
| comic_cover        | `comic_cache_size`               | blocked_by_platform |
| comic_cover        | `cover_resolve_cache`            | blocked_by_platform |
| comic_cover        | `cover_cache_size`               | blocked_by_platform |
| comic_cover        | `cover_cache_clear`              | blocked_by_platform |
| update/unlock/misc | `ai_http_proxy_url`              | blocked_by_platform |
| update/unlock/misc | `frontend_plugin_http_request`   | unsupported_hidden  |
| update/unlock/misc | `explore_clear_cache`            | blocked_by_platform |
| update/unlock/misc | `issue_full_mode_challenge`      | unsupported_hidden  |
| update/unlock/misc | `verify_full_mode_challenge`     | unsupported_hidden  |
| update/unlock/misc | `issue_scoped_unlock_challenge`  | unsupported_hidden  |
| update/unlock/misc | `verify_scoped_unlock_challenge` | unsupported_hidden  |

## Implemented Frontend-Facing Commands

以下命令由契约脚本判定为前端可触达且非 UNSUPPORTED stub。业务深度不由本矩阵替代专项验收。

```text
app_config_get_all
app_config_reset
app_config_set
audio_resolve_cache
backup_create
backup_create_data
backup_inspect
backup_peek
backup_peek_data
backup_restore
backup_restore_data
bookshelf_add
bookshelf_delete_content
bookshelf_export_book
bookshelf_export_book_data
bookshelf_get
bookshelf_get_cached_indices
bookshelf_get_chapters
bookshelf_get_content
bookshelf_get_episode_progress
bookshelf_list
bookshelf_pick_save_path
bookshelf_prefetch_chapters
bookshelf_remove
bookshelf_restore_source_switch
bookshelf_reveal_data_dir
bookshelf_reveal_export_file
bookshelf_save_chapters
bookshelf_save_content
bookshelf_save_episode_progress
bookshelf_save_txt_chapters
bookshelf_set_private
bookshelf_update_book
bookshelf_update_progress
booksource_add_dir
booksource_apply_update
booksource_book_info
booksource_call_fn
booksource_cancel
booksource_chapter_content
booksource_chapter_list
booksource_check_update
booksource_delete
booksource_delete_batch
booksource_delete_draft
booksource_eval
booksource_explore
booksource_get_dir
booksource_get_dirs
booksource_http_proxy
booksource_import_legacy_json_text
booksource_import_legacy_json_url
booksource_list
booksource_list_streaming
booksource_open_in_vscode
booksource_pick_dir
booksource_purchase_chapter
booksource_read
booksource_remove_dir
booksource_resolve_path
booksource_run_tests
booksource_save
booksource_save_draft
booksource_search
booksource_toggle
capabilities_get
config_clear
config_delete_key
config_dump_scope
config_list_scopes
config_read
config_read_all
config_read_bytes
config_read_json
config_write
config_write_bytes
config_write_json
delete_user_font
export_save_file
extension_delete
extension_get_dir
extension_list
extension_open_in_vscode
extension_read
extension_save
extension_toggle
frontend_log
frontend_storage_list
frontend_storage_list_namespaces
frontend_storage_remove
frontend_storage_set
get_local_ips
get_platform
list_system_fonts
list_user_fonts
open_dir_in_explorer
rename_user_font
repository_check_source_sync
repository_fetch
repository_install
repository_preview_source
script_dialog_result
script_repl_eval
storage_debug_dump
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
upload_user_font
web_server_pick_dist_dir
web_server_start
web_server_status
web_server_stop
```

## 更新规则

1. 修改任何 Tauri command 或前端 invoke 后，必须重新运行 `node scripts/ci/check-command-contract.mjs --json`。
2. R-P0-001 每隐藏/实现一个模块，必须同步更新本文件的 `当前处置`。
3. 不得手工写入未经脚本验证的 frontend/registered/matched 数字。
