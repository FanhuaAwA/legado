# Gate Report - R-P0-001 capabilities complete

时间：2026-06-10 15:50 +0800

## 本轮目标

关闭 R-P0-001：58 个 frontend-facing UNSUPPORTED stub 的可见入口全部通过集中式能力声明禁用、隐藏、降级或 no-op。后端 stub 本身未实现，仍由后续 R-P2/R-P1 任务决定是否真实实现。

## 改动摘要

1. `capabilities_get` / `useCapabilities` 维持 12 个能力域，作为前端唯一能力事实源。
2. browser_probe 12 个命令：调试页与网络设置页禁用；`useBrowserProbe` 所有导出函数命令前 `requireCapability("browserProbe")`。
3. comic/cover 9 个命令：漫画页和封面图走网络直读；缓存大小/清理入口按能力禁用；低层函数在能力缺失时返回原始 URL、空尺寸或 0。
4. repository/source_update 6 个命令：在线仓库页、批量操作、远程安装弹窗、后台 updateUrl 检测全部接入 `repository` 能力。
5. update/unlock/misc 9 个命令：应用内更新退到发布页，AI 后端代理退到前端直连，插件 HTTP/解锁挑战命令前阻断，探索缓存清理无缓存时 no-op。

## 逐项处置

| 模块 | 数量 | 状态 |
| ---- | ---: | ---- |
| sync | 14 | unsupported_hidden |
| tts | 6 | blocked_by_platform |
| video | 2 | blocked_by_platform |
| browser_probe | 12 | unsupported_hidden |
| comic_cover | 9 | blocked_by_platform |
| repository/source_update | 6 | unsupported_hidden |
| update/unlock/misc | 9 | blocked_by_platform + unsupported_hidden |

## 验证结果

| 检查 | 结果 |
| ---- | ---- |
| `node scripts/ci/check-command-contract.mjs --json` | PASS：frontendTotal=161, registeredTotal=163, bothCount=160, frontend_unsupported_stub_count=58, registered_unsupported_stub_count=60；分类器 self-test 随脚本执行 |
| `pnpm exec oxfmt .` | PASS：371 files |
| `pnpm exec oxfmt --check .` | PASS：371 files |
| `pnpm lint` | PASS：71 warnings / 0 errors（既有 warning，无本轮新增 error） |
| `pnpm build` | PASS：Vite build complete；保留既有 eval/chunk 类 warning |
| `cargo check -p legado-tauri` | PASS |
| `cargo check -p reader-core` | PASS |
| `cargo test -p reader-core` | PASS：31 passed / 9 ignored |
| `cargo test -p legado-tauri` | PASS：1 passed；Windows linker stdout warning |

## 剩余队列

- R-P1-004：处理 `bookshelf_export_book_data`, `sync_baidu_start_auth`, `sync_baidu_token_status` 三个 onlyBackend 命令。
- R-P2：Android 签名、lint warnings、番茄/番茄短剧、真实缓存系统、Harmony 标注、规则引擎 book 对象绑定等仍在队列。
