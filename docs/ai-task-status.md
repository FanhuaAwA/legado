# AI Task Status

Last updated: 2026-06-15

本文件只记录当前维护状态、可继续执行的队列和必须遵守的口径。旧的逐轮流水、未完成提示和过期统计已删除；需要考古时使用 git history 与 `reports/gates/*/summary.md`。

## 当前分支与发布

- Branch：`master`
- Master direct push：2026-06-15 已按用户要求直接推送到 `origin/master`，快进范围 `a91ff0f..3727a36`
- Delivered payload commit：`3727a36 docs: prune stale maintenance notes`
- Historical PR：`https://github.com/FanhuaAwA/legado/pull/2` 仅作为本轮历史 staging 记录；后续交付以 `master` 为准，不再依赖单独分支
- GitHub Quality Gate：PASS，run `27539725883`，约 13m47s
- Windows release artifact：
  - `target/x86_64-pc-windows-msvc/release/legado-tauri.exe`
  - `构建结果/windows/legado-tauri.exe`

## 当前契约基线

实测命令：

```powershell
node scripts\ci\check-command-contract.mjs --json
```

当前结果：

- `frontendTotal=163`
- `registeredTotal=162`
- `bothCount=162`
- `onlyFrontend=["js_eval"]`
- `onlyBackend=[]`
- `frontend_implemented_count=123`
- `registered_implemented_count=123`
- `frontend_unsupported_stub_count=39`
- `registered_unsupported_stub_count=39`

`js_eval` 是有意不注册的安全阻断项，不得作为缺失命令处理。新增、删除或改变命令实现状态时必须同步更新 `docs/command-matrix.md`、相关规格文档与对应 gate 报告。

## 已完成的当前性能线

- 大量书源列表加载：`booksource_list_streaming` 流式返回、前端增量 upsert、排序延后，避免首屏被全量书源阻塞。
- 书源刷新：前端 reload 合并和 token 保护，避免重复刷新互相覆盖。
- 搜索结果：聚合结果按批增量回写，分组渲染改为懒展开，降低大量书源搜索时的主线程压力。
- JS 搜索 / 章节 / 预取：加入协作取消，停止搜索或切换任务后旧结果不再继续污染 UI。
- 大量书源导入：新增 `booksource_import_legacy_json_texts` 批量文本导入；URL 包导入和本地多文件导入都走批量链路。
- 喵/猫公子书源实测：`packages=10 entries=1259 resolve_ms=1344 sequential_ms=3809 combined_ms=3831 local_sequential_ms=4146 local_combined_ms=3807 local_speedup=1.09x`。

## 当前验证命令

```powershell
cmd /c pnpm.cmd lint
cargo test -p reader-core import_legacy_json_text_reports_progress_batches -- --nocapture
cargo test -p reader-core import_legacy_json_texts_skips_bad_item_and_imports_valid_sources -- --nocapture
cargo test -p reader-core --test miaogongzi_import_perf miaogongzi_subscription_import_sequential_vs_combined -- --ignored --nocapture
cargo test -p legado-tauri task_ -- --nocapture
cargo test -p legado-tauri booksource_import_legacy_json_texts_accepts_request_id_in_ws_router -- --nocapture
node scripts\ci\check-command-contract.mjs --json
cargo check -p legado-tauri
cmd /c pnpm.cmd build:windows:release
```

以上命令在 `3727a36` 推送到 `master` 前均已本地通过。Tauri 测试/构建仍可能输出已知 Windows linker stdout warning；当前不作为失败。

## 当前未结工作

- 继续审计依赖书源加载的功能：首次导入后刷新、书源管理批量操作、搜索启用源过多时的进度、取消、按源超时和失败聚合。
- 继续优化本地书源导入：扩大样本，比较单文件、多文件、小文件密集和大文件包场景，避免只优化喵/猫公子一种形态。
- 继续按用户要求做代码 review：优先关注性能热路径、取消语义、任务 token、前后端契约和错误提示。
- 能力 backlog：`browser_probe`、TTS、漫画/封面缓存、video proxy、解锁挑战、百度/FTP provider。
- 形态 B / LAN 严格验收仍需外部设备或可访问局域网环境。
- 书源兼容：书旗/七猫 CDN 规则新鲜度与通用 `book.bookUrl` 绑定仍需按真实样本继续复查。

## 文档维护规则

- 本文件只保留当前状态，不再追加完整流水。
- 逐轮证据写入 `reports/gates/<date-topic>/summary.md`。
- 长期规格写入 `docs/reader-rust-route-b-spec.md`、`docs/frontend-backend-separation.md`、`docs/source-compat-matrix.md`。
- 命令契约以 `scripts/ci/check-command-contract.mjs --json` 实测为准，不沿用旧数字。
- 删除旧计划或旧审计文档后，不得在新文档中继续引用它们作为当前任务来源。
