# R-P1-004 Contract Scanner Gate

时间：2026-06-10 18:18 +08:00

## 任务

关闭 R-P1-004：处置旧表中 3 个 `onlyBackend` 命令。

结论：closed。`bookshelf_export_book_data`、`sync_baidu_start_auth`、`sync_baidu_token_status` 均不是后台孤儿；根因是契约脚本旧正则漏扫 `invokeWithTimeout<T>` 多行泛型调用。

## 变更

- `scripts/ci/check-command-contract.mjs`
  - 前端 invoke 扫描从单个正则改为轻量词法扫描。
  - 支持空白、注释、多行泛型、嵌套泛型和换行首参。
  - 新增 `selfTestFrontendScanner()`，覆盖 `bookshelf_export_book_data`、`sync_baidu_token_status`、`sync_baidu_start_auth`，并确认 `bridge.invoke(...)` 不会被误判为 Tauri command。
- `docs/command-matrix.md`
  - `frontendTotal=164`，`bothCount=163`，`onlyBackend=0`。
  - `bookshelf_export_book_data` 归入 implemented frontend-facing command。
  - `sync_baidu_start_auth`、`sync_baidu_token_status` 归入 `unsupported_hidden`。
- `docs/ai-task-status.md`、`docs/ai-iteration-log.md`、`E:\Book\legado-tauri-mandatory-completion-audit.md`
  - 同步 R-P1-004 closed。
  - R-P0-001 口径从旧 58/58 修正为 60/60；新增计入的两个 sync 命令已由既有 sync 能力门禁覆盖。

## 契约实测

```text
frontendTotal = 164
registeredTotal = 163
bothCount = 163
onlyFrontend = js_eval
onlyBackend = []
registered_unsupported_stub_count = 60
registered_implemented_count = 103
frontend_unsupported_stub_count = 60
frontend_implemented_count = 103
```

## 门禁结果

```text
node scripts/ci/check-command-contract.mjs --json    PASS
git diff --check                                    PASS（仅 CRLF 提示，无 whitespace error）
pnpm exec oxfmt .                                  PASS（371 files）
pnpm exec oxfmt --check .                          PASS（371 files）
pnpm lint                                          PASS（71 warnings / 0 errors，既有 warning）
pnpm build                                         PASS（既有 eval/chunk warnings）
cargo check -p reader-core                         PASS
cargo check -p legado-tauri                        PASS
cargo test -p reader-core                          PASS（31 passed / 9 ignored）
```

## 后续

R-P2 队列继续。下一项：R-P2-001 Android 签名配置说明与发布前检查；真实 keystore/密码不得入库，如需用户提供则登记 blocker 后继续后续 R-P2 项。
