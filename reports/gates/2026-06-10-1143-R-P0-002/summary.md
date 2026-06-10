# Change Gate Report - R-P0-002

日期：2026-06-10 11:43 +0800

项目路径：`E:\Book\Legado-Tauri-main`

任务：R-P0-002（三份状态文档同步、5 个争议命令定真伪）

## 变更摘要

- `check-command-contract.mjs --json` 增补双口径字段：
  - 全注册口径：`registered_unsupported_stub_count = 60`，`registered_implemented_count = 102`
  - 前端可触达口径：`frontend_unsupported_stub_count = 58`，`frontend_implemented_count = 101`
  - `classificationScope = frontend-facing registered commands`
  - `registeredClassification` 保留全注册分类清单
- `docs/ai-task-status.md` 重写为当前 R 队列状态，删除旧的自相矛盾 STUB 表。
- `docs/command-matrix.md` 按契约脚本结果半自动重建，列出 58 个 R-P0-001 前端可触达 stub。
- `docs/source-compat-matrix.md` 头部补实测命令。
- 审计文档 R-P0-002 标记 closed，R-P0-001 UI 验收口径修正为 58。
- `pnpm-workspace.yaml` 仅由 oxfmt 机械格式化：`'.'` -> `"."`。

## 5 个争议命令裁决

| Command | 结论 | 证据 |
| --- | --- | --- |
| `booksource_cancel` | implemented_with_limit | `src-tauri/src/commands/source.rs` 注册 token；`bookshelf_prefetch_chapters` 传入 cancel token；限制是不能抢占单次网络请求 |
| `booksource_purchase_chapter` | implemented_or_explicit_unsupported | JS 源调用 `purchaseChapter`；Legado 规则源返回 `{ ok:false, purchased:false, unsupported:true }` |
| `booksource_call_fn` | implemented_for_js_source | JS 源调用 runtime 命名函数；Legado 规则源返回明确错误 |
| `booksource_run_tests` | implemented | `facade.run_source_tests` 支持 step filter、timeout 和真实链路 |
| `storage_debug_dump` | implemented_summary | `facade.debug_dump` 读取 frontend namespace、app config、书架数量和路径摘要 |

## 门禁结果

| 命令 | 结果 |
| --- | --- |
| `pnpm exec oxfmt --check .` | PASS（370 files） |
| `pnpm lint` | PASS（71 warnings / 0 errors） |
| `pnpm build` | PASS |
| `cargo check -p reader-core` | PASS |
| `cargo check -p legado-tauri` | PASS |
| `cargo test -p reader-core` | PASS（31 passed / 9 ignored；8 个本机私有书源样本默认跳过） |
| `node scripts/ci/check-command-contract.mjs --json` | PASS（160 frontend / 162 registered / 159 matched / 58 frontend-facing stub / 60 registered stub） |

失败任务：无。

外部 blocker：无。

## 后续

下轮第一件事：R-P0-001。设计集中式能力声明机制，先覆盖 sync / tts / video 模块，统一隐藏或禁用前端可触达 UNSUPPORTED 入口，并同步更新 `docs/command-matrix.md` 的处置状态。
