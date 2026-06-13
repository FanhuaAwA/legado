# 2026-06-13 PERF-LAZY-FRONTEND-CHUNKS

任务 ID：`PERF-2026-06-13-LAZY-FRONTEND-CHUNKS`

## 范围

允许修改：

- `src/components/settings/SectionSync.vue`
- `src/composables/useSync.ts`
- `src/composables/useVConsole.ts`
- `src/views/BookSourceView.vue`
- `docs/ai-task-status.md`
- `docs/ai-iteration-log.md`
- `reports/gates/2026-06-13-PERF-LAZY-FRONTEND-CHUNKS/summary.md`

不触碰：后端命令契约、reader-core 业务逻辑、书源解析规则、用户数据目录、第三方书源样本、依赖版本、Windows/Android 发布产物。

## 变更

- `BookSourceView.vue` 将已安装、在线、调试、测试、AI 写书源五个子页改为 `defineAsyncComponent`，使书源管理入口先加载壳层，再按标签加载具体功能面板。
- AI 写书源标签页改为 `display-directive="show:lazy"`，避免未访问该标签时提前挂载 AI 工作台。
- `SectionSync.vue` 和 `useSync.ts` 将 `qrcode`、`@zxing/browser` 改为二维码生成/扫码动作触发时动态导入，避免同步设置页首屏静态加载二维码库。
- `useVConsole.ts` 将 `vconsole` 改为开发者开关启用时动态导入，并处理开关关闭时 import 仍在途的竞态；默认关闭时不再把 vConsole 打入主入口。

## 构建观测

本轮中间构建与最终构建对比：

- `vConsole` 懒加载前：`dist/assets/index-zbJ3OJFn.js` 为 `370.53 kB`，gzip `103.94 kB`。
- `vConsole` 懒加载后：`dist/assets/index-BjH9Vjka.js` 为 `67.96 kB`，gzip `23.36 kB`。
- `vconsole.min-D3qedUWG.js` 变为独立异步 chunk：`281.46 kB`，gzip `78.04 kB`，只在开发者调试开关启用时加载。
- `AiSourceTab-CfbQgSAY.js` 保持独立异步 chunk：`463.36 kB`，gzip `118.58 kB`，不再由书源管理入口同步导入。
- `useSync-D1IMfMM3.js` 保持轻量：`6.09 kB`，gzip `2.35 kB`；二维码与扫码库不在该 chunk 静态出现。

仍保留的构建警告：

- `vconsole` 包内部 direct eval 警告仍会在构建扫描动态 chunk 时出现，但已从主入口拆出；是否替换该依赖需单独立项。
- `vendor-vue-naive` 与 `_plugin-vue_export-helper` 仍超过 500 kB，根因包含 Naive UI 全量注册和共享依赖策略；本轮未改依赖注册方式。
- `useTransport` ineffective dynamic import 仍存在，因为该模块同时被若干设置组件静态导入；需要单独审计传输层调用入口，不能混入本轮。

## Gate

| 命令 | 结果 |
| --- | --- |
| `cmd /c pnpm.cmd lint` | PASS，0 warnings / 0 errors |
| `cmd /c pnpm.cmd build` | PASS；保留上述既有 Vite/Rolldown warning |
| `git diff --check` | PASS，仅 Windows LF/CRLF 工作区提示 |
| `node scripts\ci\check-command-contract.mjs --json` | PASS，`frontendTotal=162`、`registeredTotal=161`、`bothCount=161`、`onlyFrontend=["js_eval"]`、`onlyBackend=[]`、stub `39`、implemented `122` |
| `cargo fmt --all -- --check` | PASS |
| `cargo check -p reader-core` | PASS |
| `cargo check -p legado-tauri` | PASS |
| `cargo test -p reader-core` | PASS，reader-core 全部非 ignored 测试通过 |

## 审计结论

本轮只改变前端模块加载时机，不改变业务语义、命令名、事件契约或数据结构。新增动态导入均位于用户动作或开发者开关之后，且没有新增依赖。

下一轮第一件事：继续前端性能收口，优先审计 Naive UI 全量注册与 `_plugin-vue_export-helper` 大 chunk；如范围过大，则先登记拆分计划，再处理 `useTransport` ineffective dynamic import。
