# Command Matrix

前端调用的 command 与后端 Rust 实现的对照矩阵。状态标记：

- **OK** = 已注册且基本实现
- **PARTIAL** = 已注册但部分实现或有已知限制
- **STUB** = 已注册但仅返回 UNSUPPORTED/空
- **MISSING** = 前端调用但后端未注册

## 书源管理 (booksource\_\*)

| Command                              | 前端调用                | 后端注册  | 状态 | 备注                                         |
| ------------------------------------ | ----------------------- | --------- | ---- | -------------------------------------------- |
| `booksource_get_dir`                 | useBookSource.ts        | source.rs | OK   |                                              |
| `booksource_get_dirs`                | useBookSource.ts        | source.rs | OK   |                                              |
| `booksource_add_dir`                 | useBookSource.ts        | source.rs | OK   |                                              |
| `booksource_remove_dir`              | useBookSource.ts        | source.rs | OK   |                                              |
| `booksource_pick_dir`                | useBookSource.ts        | source.rs | OK   | 桌面端保留，非桌面返回 UNSUPPORTED           |
| `booksource_list`                    | useBookSource.ts        | source.rs | OK   |                                              |
| `booksource_list_streaming`          | useBookSource.ts        | source.rs | OK   | 分批增量推送（每批 20），多次 emit           |
| `booksource_read`                    | useBookSource.ts        | source.rs | OK   |                                              |
| `booksource_save`                    | useBookSource.ts        | source.rs | OK   |                                              |
| `booksource_delete`                  | useBookSource.ts        | source.rs | OK   |                                              |
| `booksource_delete_batch`            | useBookSource.ts        | source.rs | OK   |                                              |
| `booksource_toggle`                  | useBookSource.ts        | source.rs | OK   |                                              |
| `booksource_import_legacy_json_text` | BookSourceInstallDialog | source.rs | OK   |                                              |
| `booksource_import_legacy_json_url`  | BookSourceInstallDialog | source.rs | OK   |                                              |
| `booksource_eval`                    | useBookSource.ts        | source.rs | OK   | 空 code→能力列表；非空→sandbox rquickjs 评估 |
| `booksource_save_draft`              | useAiAgent.ts:338       | source.rs | OK   | 保存到 drafts 目录                           |
| `booksource_run_tests`               | useAiAgent.ts:560       | source.rs | OK   | 运行 search/bookInfo/等测试步骤              |

## 书源执行 (booksource\_\*)

| Command                       | 前端调用                    | 后端注册  | 状态 | 备注                                 |
| ----------------------------- | --------------------------- | --------- | ---- | ------------------------------------ |
| `booksource_search`           | scriptBridge.ts:213         | source.rs | OK   | Legado + JS 双运行时                 |
| `booksource_book_info`        | scriptBridge.ts:227         | source.rs | OK   |                                      |
| `booksource_chapter_list`     | scriptBridge.ts:242         | source.rs | OK   |                                      |
| `booksource_chapter_content`  | scriptBridge.ts:270         | source.rs | OK   |                                      |
| `booksource_purchase_chapter` | scriptBridge.ts:293         | source.rs | STUB | 返回 `{ ok: true, purchased: true }` |
| `booksource_explore`          | scriptBridge.ts:395         | source.rs | OK   |                                      |
| `booksource_call_fn`          | scriptBridge.ts:313+        | source.rs | STUB | 返回 UNSUPPORTED                     |
| `booksource_cancel`           | scriptBridge.ts/prefetch.ts | source.rs | OK   | TaskRegistry + AtomicBool 取消       |

## 书架 (bookshelf\_\*)

| Command                           | 前端调用                | 后端注册     | 状态 | 备注                 |
| --------------------------------- | ----------------------- | ------------ | ---- | -------------------- |
| `bookshelf_list`                  | bookshelf.ts:68         | bookshelf.rs | OK   |                      |
| `bookshelf_add`                   | bookshelf.ts:97         | bookshelf.rs | OK   |                      |
| `bookshelf_remove`                | bookshelf.ts:107        | bookshelf.rs | OK   |                      |
| `bookshelf_get`                   | bookshelf.ts:220        | bookshelf.rs | OK   |                      |
| `bookshelf_update_progress`       | bookshelf.ts:236        | bookshelf.rs | OK   |                      |
| `bookshelf_set_private`           | bookshelf.ts:271        | bookshelf.rs | OK   |                      |
| `bookshelf_save_chapters`         | bookshelf.ts:163,277    | bookshelf.rs | OK   |                      |
| `bookshelf_get_chapters`          | bookshelf.ts:282        | bookshelf.rs | OK   |                      |
| `bookshelf_update_book`           | bookshelf.ts:183,320    | bookshelf.rs | OK   |                      |
| `bookshelf_restore_source_switch` | bookshelf.ts:372        | bookshelf.rs | OK   |                      |
| `bookshelf_save_content`          | bookshelf.ts:382        | bookshelf.rs | OK   |                      |
| `bookshelf_get_content`           | bookshelf.ts:387        | bookshelf.rs | OK   |                      |
| `bookshelf_delete_content`        | bookshelf.ts:392        | bookshelf.rs | OK   |                      |
| `bookshelf_get_cached_indices`    | bookshelf.ts:397        | bookshelf.rs | OK   |                      |
| `bookshelf_save_txt_chapters`     | bookshelf.ts:170        | bookshelf.rs | OK   |                      |
| `bookshelf_get_episode_progress`  | bookshelf.ts:404        | bookshelf.rs | OK   |                      |
| `bookshelf_save_episode_progress` | bookshelf.ts:418        | bookshelf.rs | OK   |                      |
| `bookshelf_prefetch_chapters`     | prefetch.ts:235,285     | bookshelf.rs | OK   | 后台逐章缓存正文     |
| `bookshelf_pick_save_path`        | exportFile.ts:112       | bookshelf.rs | OK   | 桌面端原生保存对话框 |
| `bookshelf_reveal_data_dir`       | bookshelfActions.ts:116 | bookshelf.rs | OK   | 打开阅读器数据目录   |

## 音频 (audio\_\*)

| Command               | 前端调用           | 后端注册  | 状态 | 备注                   |
| --------------------- | ------------------ | --------- | ---- | ---------------------- |
| `audio_resolve_cache` | musicPlayer.ts:375 | system.rs | OK   | 代理下载音频缓存到本地 |

## 脚本 (script\_\*)

| Command                | 前端调用            | 后端注册  | 状态 | 备注               |
| ---------------------- | ------------------- | --------- | ---- | ------------------ |
| `script_dialog_result` | scriptBridge.ts:195 | system.rs | OK   |                    |
| `script_repl_eval`     | scriptBridge.ts:417 | system.rs | OK   | rquickjs REPL 评估 |

## 配置 (config*\* / app_config*_ / frontend*storage*_)

| Command                            | 前端调用           | 后端注册  | 状态 | 备注                        |
| ---------------------------------- | ------------------ | --------- | ---- | --------------------------- |
| `config_read`                      | useScriptConfig    | config.rs | OK   |                             |
| `config_write`                     | useScriptConfig    | config.rs | OK   |                             |
| `config_read_json`                 | useScriptConfig    | config.rs | OK   |                             |
| `config_write_json`                | useScriptConfig    | config.rs | OK   |                             |
| `config_delete_key`                | useScriptConfig    | config.rs | OK   |                             |
| `config_read_all`                  | useScriptConfig    | config.rs | OK   |                             |
| `config_clear`                     | useScriptConfig    | config.rs | OK   |                             |
| `config_read_bytes`                | useScriptConfig    | config.rs | OK   |                             |
| `config_write_bytes`               | useScriptConfig    | config.rs | OK   |                             |
| `config_list_scopes`               | -                  | config.rs | OK   | SQL DISTINCT namespace 查询 |
| `config_dump_scope`                | -                  | config.rs | OK   |                             |
| `app_config_get_all`               | appConfig.ts:107   | config.rs | OK   |                             |
| `app_config_set`                   | appConfig.ts:124   | config.rs | OK   |                             |
| `app_config_reset`                 | appConfig.ts:135   | config.rs | OK   |                             |
| `frontend_storage_list`            | useFrontendStorage | config.rs | OK   |                             |
| `frontend_storage_set`             | useFrontendStorage | config.rs | OK   |                             |
| `frontend_storage_remove`          | useFrontendStorage | config.rs | OK   |                             |
| `frontend_storage_list_namespaces` | useFrontendStorage | config.rs | OK   |                             |
| `storage_debug_dump`               | -                  | config.rs | OK   |                             |

## 扩展 (extension\_\*)

| Command                    | 前端调用       | 后端注册     | 状态 | 备注 |
| -------------------------- | -------------- | ------------ | ---- | ---- |
| `extension_get_dir`        | ExtensionsView | extension.rs | OK   |      |
| `extension_list`           | ExtensionsView | extension.rs | OK   |      |
| `extension_read`           | ExtensionsView | extension.rs | OK   |      |
| `extension_save`           | ExtensionsView | extension.rs | OK   |      |
| `extension_delete`         | ExtensionsView | extension.rs | OK   |      |
| `extension_toggle`         | ExtensionsView | extension.rs | OK   |      |
| `extension_open_in_vscode` | ExtensionsView | extension.rs | OK   |      |

## 系统 (system)

| Command                | 前端调用 | 后端注册  | 状态 | 备注 |
| ---------------------- | -------- | --------- | ---- | ---- |
| `frontend_log`         | 全局     | system.rs | OK   |      |
| `get_platform`         | useEnv   | system.rs | OK   |      |
| `open_dir_in_explorer` | 设置页   | system.rs | OK   |      |

## 统计

- **OK**: 58
- **PARTIAL**: 1
- **STUB**: 5
- **MISSING**: 7

总注册命令: 64 | 前端调用但缺失: 7
