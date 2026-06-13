# 2026-06-13 UI-SOURCE-AI-COMMENT-LAYOUT

任务 ID：`UI-2026-06-13-SOURCE-AI-COMMENT-LAYOUT`

## 范围

允许修改：
- `src/components/booksource/InstalledSourcesTab.vue`
- `src/components/booksource/AiSourceTab.vue`
- `src/components/booksource/AiTestPanel.vue`
- `src/components/reader/ReaderParagraphCommentsDrawer.vue`
- `src-headless/src/main.rs`
- `docs/ai-iteration-log.md`
- `docs/ai-task-status.md`
- `reports/gates/2026-06-13-UI-SOURCE-AI-COMMENT-LAYOUT/summary.md`

不触碰：书源解析规则、AI 生成业务逻辑、段评数据语义、用户数据目录、第三方书源样例、依赖版本、Windows/Android 发布产物。

## 变更

- 书源管理：搜索/统计/批量管理工具栏改为可换行布局；导入与目录管理弹窗宽度收敛到视口内；批量按钮在窄屏下平均分配宽度，避免新增按钮后撑坏排版。
- AI 写书源：工作台 topbar、三栏 grid、聊天输入区、prompt 按钮统一补齐 `min-width: 0`、`minmax(0, ...)` 和移动端断点收敛。
- AI 测试面板：标签条可横向滚动，手动测试输入和 footer hint 可换行。
- 段评抽屉：昵称、段落标识、评论正文、footer、回复行补齐 ellipsis/wrap/单列规则。
- Headless：补齐 `booksource_get_dir`、`booksource_get_dirs`、`booksource_list_streaming`，并通过 WebSocket `booksource:batch` 事件推送流式书源列表。

## UI 实测

运行方式：`legado-headless` 托管 `dist`，系统 Chrome headless 打开 `http://127.0.0.1:7791/?ws=ws://127.0.0.1:7791/ws`。

数据准备：通过真实 WS `booksource_save` 写入 3 条临时 JS 书源，再用 `booksource_list_streaming` 验证事件推送。

| 视口 | 结果 |
| --- | --- |
| 1000x800 | 书源管理标题 `writing-mode=horizontal-tb`，尺寸 `80x32`；显示 `共 3 个书源` 和 3 张卡片；页面/批量条/AI 工作台横向溢出均为 0 |
| 768x800 | 3 张书源卡片正常渲染；页面/批量条/AI 工作台横向溢出均为 0；AI grid 收敛为单列 `520px` |
| 390x800 | 标题仍为横排 `80x32`；批量按钮宽度约 83px；AI grid 收敛为 `366px`；横向溢出均为 0 |

段评抽屉：用编译后的 scoped CSS 属性 `data-v-cbd9482a` 注入长昵称、长 rangeKey、长评论、长回复内容，检查结果 `tooWide=0`，回复区 display 为 `grid`。

Headless 流式列表：`booksource_list_streaming` 推送 1 个 `booksource:batch` 事件，`items=3`、`done=true`、`total=3`。

## Gate

| 命令 | 结果 |
| --- | --- |
| `cmd /c node_modules\.bin\oxfmt.cmd --check .` | PASS |
| `git diff --check` | PASS，仅 Windows LF/CRLF 工作区提示 |
| `node scripts/ci/check-command-contract.mjs --json` | PASS，`162/161/161`，`onlyFrontend=["js_eval"]`，`onlyBackend=[]`，stub `39`，implemented `122` |
| `cmd /c pnpm.cmd lint` | PASS，0 warnings / 0 errors |
| `cmd /c pnpm.cmd build` | PASS，仅既有 Vite warning：`vconsole` direct eval、大 chunk、`useTransport` ineffective dynamic import |
| `cargo check -p reader-core` | PASS |
| `cargo check -p legado-tauri` | PASS |
| `cargo check -p legado-headless` | PASS |
| `cargo test -p reader-core` | PASS，全部非 ignored 测试通过 |

## 残余风险

- Headless 初始书架仍会提示 `NOT_ROUTED: extension_get_dir/list`，属于扩展模块 headless 白名单缺口，不影响本轮书源管理、AI 写书源、段评抽屉布局证据。
- GitHub Actions 在 2026-06-13 01:00 左右曾因 crates.io 下载 `cipher` 连接 reset 失败。该日志指向 CI 网络瞬断/registry 下载不稳定；下一轮任务为 `CI-2026-06-13-CARGO-FETCH-RETRY`，加固 Cargo 缓存与重试策略。
