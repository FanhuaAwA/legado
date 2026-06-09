# AI Task Status

本文件记录各任务模块的当前完成状态，用于后续 AI 快速了解已完成和待完成事项。不要将完成状态写入 README、用户文档或源码注释。

## 环境状态

```text
android.symlink = resolved
android.release = passed_unsigned
windows.release = passed
reader-core.tests = passed
tauri.cargo_check = passed
frontend.lint = passed
format.baseline = passed
  tool = oxfmt
  command = pnpm exec oxfmt . && pnpm lint
  scope = whole repository
  mixed_with_feature_changes = false
```

## 构建门禁状态（2026-06-09 17:20）

| 门禁                             | 状态 | 备注                              |
| -------------------------------- | ---- | --------------------------------- |
| `cargo check -p reader-core`     | PASS |                                   |
| `cargo check -p legado-tauri`    | PASS |                                   |
| `cargo test -p reader-core`      | PASS | 32 passed, 1 live-network ignored |
| `cargo test -p legado-tauri`     | PASS | 5 passed                          |
| `pnpm build`                     | PASS | eval/chunk-size warning           |
| `pnpm lint`                      | PASS | 73 warnings, 0 errors             |
| `pnpm run build:android:release` | PASS | unsigned APK                      |
| `pnpm run build:windows:release` | PASS |                                   |

## 缺失 Command（2026-06-09）

| Command | 状态 |
| ------- | ---- |

## STUB Command（需补真实实现）

| Command                       | 当前状态                |
| ----------------------------- | ----------------------- |
| `booksource_purchase_chapter` | 固定返回 `{ ok: true }` |
| `booksource_call_fn`          | 返回 UNSUPPORTED        |

## 审计问题追踪（来自 2026-06-09 轻量审计）

| ID        | 问题                                       | 状态                             |
| --------- | ------------------------------------------ | -------------------------------- |
| AUDIT-001 | 无 `.git`                                  | 已修复（2026-06-09 init + push） |
| AUDIT-002 | `scripts/copy-harmony-web.mjs` 缺失        | 已修复（Iteration 9）            |
| AUDIT-003 | `scripts/booksource-node-runtime.mjs` 缺失 | 已修复（Iteration 9）            |
| AUDIT-004 | `scripts/copy-build-result.mjs` 存在且正常 | 已确认                           |
| AUDIT-005 | `booksource_eval` 为 UNSUPPORTED           | 已修复（Iteration 11）           |
| AUDIT-006 | `config_list_scopes` 返回空                | 已修复（Iteration 9）            |
| AUDIT-007 | 前端裸 `console.log`                       | 已迁移完成（2026-06-09） — 剩余4处均为有意使用 |
| AUDIT-008 | 前端 TODO 和屏蔽逻辑                       | 已评估（9 处均为合法功能门禁）   |
| AUDIT-009 | reader-core 测试内 `unwrap()`              | 测试中可接受                     |

## 书源兼容状态

详见 `docs/source-compat-matrix.md`。

## 格式化基线状态

```text
format.baseline = passed
```
