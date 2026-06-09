# AI Task Status

本文件记录各任务模块的当前完成状态，用于后续 AI 快速了解已完成和待完成事项。不要将完成状态写入 README、用户文档或源码注释。

## 环境状态

```text
android.symlink = resolved
android.release = passed_unsigned
windows.release = passed
reader-core.tests = passed（33 passed, 3 live-network ignored）
tauri.cargo_check = passed
frontend.lint = passed（75 warnings, 0 errors）
format.baseline = passed
command_contract = 159 frontend, 163 registered, 158 matched, 1 security-blocked (js_eval)
```

## 2026-06-09 强制优先修复项

P0/P1/P2 已基本清零（Iteration 18）：

- `pnpm lint` **PASS**
- Command 契约：158/159 匹配（仅 js_eval 因安全原因有意不注册）
- `booksource_cancel` **FIXED** — TaskRegistry 已接入 3 个长任务
- `booksource_purchase_chapter` **FIXED** — Legado 源返回 `{ok:false, unsupported:true}`
- `booksource_run_tests` **FIXED** — 支持 step_filter + 真实 Legado 源四段链路执行
- `storage_debug_dump` **FIXED** — 返回真实 frontend/shelf/app config 数据
- 77 个 UNSUPPORTED stubs 已注册（backup, sync, tts, video, web_server, fonts, unlock, comic, cover, repository, misc）

**剩余 P2 工作**：Harmony/Node 书源运行器、视频/音乐/TTS 空壳（plan 标注为中期任务）
**剩余 P3/P4**：JS shim 健壮性、LICENSE 文件

## 构建门禁状态（2026-06-09 Iteration 17 后）

| 门禁                             | 状态 | 备注                                                                                        |
| -------------------------------- | ---- | ------------------------------------------------------------------------------------------- |
| `cargo check -p reader-core`     | PASS |                                                                                             |
| `cargo check -p legado-tauri`    | PASS |                                                                                             |
| `cargo test -p reader-core`      | PASS | 33 passed (17 unit + 7 compat + 3 js + 1 route_b + 5 source_compat), 3 live-network ignored |
| `cargo test -p legado-tauri`     | PASS | 5 passed                                                                                    |
| `pnpm build`                     | PASS |                                                                                             |
| `pnpm lint`                      | PASS | 75 warnings, 0 errors                                                                       |
| `pnpm run build:android:release` | PASS | unsigned APK                                                                                |
| `pnpm run build:windows:release` | PASS |                                                                                             |

## 缺失 Command（2026-06-09 Iteration 17）

自动检查工具：`node scripts/ci/check-command-contract.mjs`

当前状态：159 frontend calls → 87 registered → 77 unregistered

| 模块              | 数量 | 处理方案                                  |
| ----------------- | ---- | ----------------------------------------- |
| P0 bookshelf\_\*  | 0    | 已全部实现（Iteration 3 + 17）            |
| P0 booksource\_\* | 0    | 已全部实现（Iteration 2 + 17）            |
| backup\_\*        | 8    | 待创建 UNSUPPORTED stubs 或移除 UI 入口   |
| browser*probe*\*  | 12   | 待创建 UNSUPPORTED stubs                  |
| sync\_\*          | 16   | 待创建 UNSUPPORTED stubs                  |
| tts\_\*           | 6    | 待创建 UNSUPPORTED stubs                  |
| video*proxy*\*    | 2    | 待创建 UNSUPPORTED stubs                  |
| web*server*\*     | 4    | 待创建 UNSUPPORTED stubs                  |
| font\_\*          | 5    | 待创建 UNSUPPORTED stubs                  |
| unlock\_\*        | 4    | 待创建 UNSUPPORTED stubs                  |
| misc              | 8    | jseval 安全阻塞，其余待 UNSUPPORTED stubs |
| comic*\*/cover*\* | 8    | 待创建 UNSUPPORTED stubs                  |
| repository\_\*    | 4    | 待创建 UNSUPPORTED stubs                  |

## STUB Command（需补真实实现）

| Command                       | 当前状态                                                                                  |
| ----------------------------- | ----------------------------------------------------------------------------------------- |
| `booksource_purchase_chapter` | PARTIAL/FAKE：JS 书源会调用 `purchaseChapter`；Legado 规则源仍固定返回 `{ok:true}`        |
| `booksource_call_fn`          | PARTIAL：JS 书源可调用命名函数；Legado 规则源返回错误，需前端正确处理                     |
| `booksource_cancel`           | FAKE-WIRED：有 `TaskRegistry`，但任务入口未注册 token，取消无法作用于真实长任务           |
| `booksource_run_tests`        | SHALLOW：丢弃 timeout/step_filter；Legado 源只返回能力配置，不真实运行链路                |
| `storage_debug_dump`          | SHALLOW：返回 `frontend/scriptJson/scriptBytes/clientStates` 空对象，不能代表真实存储状态 |

## 审计问题追踪（来自 2026-06-09 轻量审计）

| ID        | 问题                                       | 状态                                           |
| --------- | ------------------------------------------ | ---------------------------------------------- |
| AUDIT-001 | 无 `.git`                                  | 已修复（2026-06-09 init + push）               |
| AUDIT-002 | `scripts/copy-harmony-web.mjs` 缺失        | 已修复（Iteration 9）                          |
| AUDIT-003 | `scripts/booksource-node-runtime.mjs` 缺失 | 已修复（Iteration 9）                          |
| AUDIT-004 | `scripts/copy-build-result.mjs` 存在且正常 | 已确认                                         |
| AUDIT-005 | `booksource_eval` 为 UNSUPPORTED           | 已修复（Iteration 11）                         |
| AUDIT-006 | `config_list_scopes` 返回空                | 已修复（Iteration 9）                          |
| AUDIT-007 | 前端裸 `console.log`                       | 已迁移完成（2026-06-09） — 剩余4处均为有意使用 |
| AUDIT-008 | 前端 TODO 和屏蔽逻辑                       | 已评估（9 处均为合法功能门禁）                 |
| AUDIT-009 | reader-core 测试内 `unwrap()`              | 测试中可接受                                   |

## 书源兼容状态

详见 `docs/source-compat-matrix.md`。

## UI 体验 Polish（第 26.6 节）

| ID     | 问题                                        | 状态                   |
| ------ | ------------------------------------------- | ---------------------- |
| UX-001 | SearchView 无结果时仍显示翻页栏             | 已修复（Iteration 15） |
| UX-002 | AggregatedSearchResults 无过渡动画          | 已修复（Iteration 15） |
| UX-003 | BookshelfView 搜索弹窗结果无过渡动画        | 已修复（Iteration 15） |
| UX-004 | ShelfBookCard statusLabel 误判"已读完"      | 已修复（Iteration 15） |
| UX-005 | ReaderVideoSurface 锁定态 UX 差（强制关闭） | 已修复（Iteration 15） |
