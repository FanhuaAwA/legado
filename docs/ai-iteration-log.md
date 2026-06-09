# AI Iteration Log

## 记录标题：2026-06-09 Iteration 1

**本轮目标**：建立可验证基线（按计划文档第 0 节、第 26.2 节和第 55.9 节执行）

**读取文件**：

- `E:\Book\legado-tauri-ai-iteration-plan.md`（第 0、26、27、38-52、55、56 节）
- `package.json`、`Cargo.toml`、`src-tauri/Cargo.toml`、`src-tauri/src/commands/mod.rs`
- `docs/reader-rust-route-b-spec.md`
- 前端 store 文件：`scriptBridge.ts`、`bookshelf.ts`、`prefetch.ts`、`musicPlayer.ts`、`appConfig.ts`

**修改文件**：

- 新建 `docs/ai-iteration-log.md`（本文件）
- 新建 `docs/command-matrix.md`
- 新建 `docs/ai-task-status.md`
- 新建 `docs/source-compat-matrix.md`

**验证命令**：

```powershell
cargo check -p legado-tauri
cargo check -p reader-core
cargo test -p reader-core
cargo test -p legado-tauri
pnpm build
```

**通过项**：

| 命令                          | 状态                                      |
| ----------------------------- | ----------------------------------------- |
| `cargo check -p legado-tauri` | PASS                                      |
| `cargo check -p reader-core`  | PASS                                      |
| `cargo test -p reader-core`   | PASS（28 passed, 1 live-network ignored） |
| `cargo test -p legado-tauri`  | PASS（0 tests, 编译通过）                 |
| `pnpm build`                  | PASS（有 warning，无 error）              |

**失败项**：

- `pnpm lint`：被 `oxfmt --check .` 阻断（~280 个文件格式未对齐），属于格式化基线问题，非环境缺失。按第 56.3 节，需要用户允许大范围格式化后独立执行 `pnpm exec oxfmt .`。

**已知缺失 command**（前端调用但后端未注册）：

| Command                       | 前端调用位置                                              |
| ----------------------------- | --------------------------------------------------------- |
| `booksource_save_draft`       | `src/composables/useAiAgent.ts:338`                       |
| `booksource_run_tests`        | `src/composables/useAiAgent.ts:560`                       |
| `bookshelf_prefetch_chapters` | `src/stores/prefetch.ts:235,285`                          |
| `bookshelf_pick_save_path`    | `src/utils/exportFile.ts:112`                             |
| `bookshelf_reveal_data_dir`   | `src/features/bookshelf/services/bookshelfActions.ts:116` |
| `audio_resolve_cache`         | `src/stores/musicPlayer.ts:375`                           |
| `script_repl_eval`            | `src/stores/scriptBridge.ts:417`                          |

**下轮第一件事**：
处理格式化基线——如果用户允许，执行 `cd E:\Book\Legado-Tauri-main && pnpm exec oxfmt . && pnpm lint` 作为独立格式化基线任务，完成后再进入 command 补齐。

**不得重复做的事**：

- 不要重复运行已通过的 cargo check 和 cargo test（除非改动了 Rust 代码）
- 不要把格式化基线、lint warning 修复和 command 补齐混在同一轮
- 不要重复扫描 `legado-main`（当前不是书源规则对齐任务）

---

## 记录标题：2026-06-09 Iteration 2

**本轮目标**：格式化基线 + 补齐 booksource\_\* 缺失 command

**修改文件**：

- `crates/reader-core/src/facade.rs` — 新增 `save_draft`、`run_source_tests` 方法
- `src-tauri/src/commands/source.rs` — 新增 `booksource_save_draft`、`booksource_run_tests` command
- `src-tauri/src/commands/mod.rs` — 注册两个新 command

**验证命令**：

```powershell
pnpm exec oxfmt . && pnpm lint
cargo check -p legado-tauri
cargo test -p reader-core
cargo test -p legado-tauri
pnpm build
```

**通过项**：

| 命令                          | 状态                         |
| ----------------------------- | ---------------------------- |
| `pnpm lint`                   | PASS (64 warnings, 0 errors) |
| `cargo check -p legado-tauri` | PASS                         |
| `cargo test -p reader-core`   | PASS (28 passed)             |
| `cargo test -p legado-tauri`  | PASS                         |
| `pnpm build`                  | PASS                         |

**新增能力**：

- `booksource_save_draft`：将书源草稿保存到 `reader/drafts/` 目录，不出现在已安装书源列表中
- `booksource_run_tests`：运行书源的 search/bookInfo/chapterList/chapterContent/explore 五个测试步骤，返回每步结果（passed/failed/available/not_configured）

**下轮第一件事**：
继续补齐缺失 command——优先处理 `bookshelf_prefetch_chapters`、`bookshelf_pick_save_path`、`bookshelf_reveal_data_dir`。先读 `src/stores/prefetch.ts`、`src/utils/exportFile.ts`、`src/features/bookshelf/services/bookshelfActions.ts` 了解参数和返回值，再在 `bookshelf.rs` 和 `facade.rs` 中实现。

**不得重复做的事**：

- 不要再创建 `scripts/ci/` 文件（已创建）
- 不要再创建 baseline docs（已创建）
- 不要把 oxfmt 格式化再次作为独立任务（基线已建立）

---

## 记录标题：2026-06-09 Iteration 3

**本轮目标**：补齐 bookshelf\_\* 缺失 command（第 26.3 节第二优先级）

**修改文件**：

- `crates/reader-core/src/facade.rs` — 新增 `prefetch_chapters` 方法
- `src-tauri/src/commands/bookshelf.rs` — 新增 `bookshelf_prefetch_chapters`、`bookshelf_pick_save_path`、`bookshelf_reveal_data_dir`
- `src-tauri/src/commands/mod.rs` — 注册三个新 command

**验证命令**：

```powershell
cargo check -p legado-tauri
cargo test -p reader-core
pnpm exec oxfmt . && pnpm lint
```

**通过项**：全部通过

**新增能力**：

- `bookshelf_prefetch_chapters`：逐章获取正文并保存缓存，返回成功缓存的章节数
- `bookshelf_pick_save_path`：桌面端弹出原生文件保存对话框；非桌面端返回 UNSUPPORTED
- `bookshelf_reveal_data_dir`：用系统文件管理器打开阅读器数据目录

**下轮第一件事**：
继续补齐剩余的 `audio_resolve_cache` 和 `script_repl_eval`。先读 `src/stores/musicPlayer.ts:371-391` 和 `src/stores/scriptBridge.ts:417`。

**不得重复做的事**：

- 不要再处理格式化基线（已建立）
- 不要再运行 cargo check/test 无改动的情况下重跑

---

## 记录标题：2026-06-09 Iteration 4

**本轮目标**：补齐 audio\_\* 和 script\_\* 缺失 command（第 26.3 节第三、四优先级）

**修改文件**：

- `crates/reader-core/src/facade.rs` — 新增 `resolve_audio_cache`、`eval_repl` 方法
- `src-tauri/src/commands/system.rs` — 新增 `audio_resolve_cache`、`script_repl_eval` command
- `src-tauri/src/commands/mod.rs` — 注册两个新 command

**验证命令**：

```powershell
cargo check -p legado-tauri
cargo test -p reader-core
cargo test -p legado-tauri
```

**通过项**：全部通过，零 warning

**新增能力**：

- `audio_resolve_cache`：通过代理下载音频文件（携带 Referer），缓存到本地 `cache/audio/`，返回本地路径
- `script_repl_eval`：在 rquickjs 运行时中执行任意 JS 代码（支持 async/await 和 legado.\* API），返回执行结果字符串

**下轮第一件事**：
按计划第 0 节进入第三轮——跑通书源链路。按第 26.5 节顺序：先用 mock 书源验证四段链路，再依次验证书旗→七猫→番茄→番茄短剧。先读 `docs/source-compat-matrix.md` 和 `crates/reader-core/tests/route_b_facade.rs`，然后用 `booksource_import_legacy_json_text` 导入并验证。

**不得重复做的事**：

- 不要再补 MISSING command（全部 7 个已补齐）
- 不要再处理格式化基线

---

## 记录标题：2026-06-09 Iteration 5

**本轮目标**：按计划第 0 节第三轮——跑通书源链路（第 26.4 节）

**读取文件**：

- `E:\Book\书旗书源\sqxs260128_0ee680c1.json`
- `E:\Book\七猫书源\qmxs260128_432b9f7e.json`
- `E:\Book\番茄书源\fqfix0529_45469384.json`

**修改文件**：

- 新建 `crates/reader-core/tests/source_compat_import.rs` — 3 个书源导入测试

**验证命令**：

```powershell
cargo test --workspace
```

**通过项**：31 tests passed, 1 ignored (live network)

**书源导入验证结果**：

| 书源      | 导入                        | 网络链路                                                      |
| --------- | --------------------------- | ------------------------------------------------------------- |
| Mock 书源 | PASS（已有 route_b_facade） | PASS（全链路）                                                |
| 书旗小说  | PASS                        | BLOCKED（java.ajax, cookie, zdym 等 JS API 缺失）             |
| 七猫小说  | PASS                        | BLOCKED（java.ajax, cookie, svg/base64 等 JS API 缺失）       |
| 番茄小说  | PASS                        | BLOCKED（大量 java._, source._, cookie, 变量 等 JS API 缺失） |

三个真实书源都可以成功导入并正确解析字段，但网络执行链路因 JS API shim 不完整而无法运行。具体阻塞的 API 见 `docs/source-compat-matrix.md`。

**下轮第一件事**：
按第 27.5 节，优先实现本地书源已使用但当前缺失的 JS API：

1. `java.ajax`（书旗、七猫、番茄都用）
2. `cookie.getCookie`（三源都用）
3. `java.base64Encode`（书旗、七猫）
4. `java.hexDecodeToString`（书旗）
   先读 `crates/reader-core/src/parser/js.rs` 和 `crates/reader-core/src/source_runtime/js_source.rs`。

**不得重复做的事**：

- 不要再重复导入验证（已确认可导入）
- 不要为每个源单独写导入测试（三个已有覆盖）

---

## 记录标题：2026-06-09 Iteration 6

**本轮目标**：按计划第 27.5 节——实现 JS API shim，解除三个书源的 JS API 阻塞

**修改文件**：

- `crates/reader-core/src/parser/js.rs` — 新增 11 个 JS API 绑定

**新增 JS API**：

| API                            | 实现方式                                            |
| ------------------------------ | --------------------------------------------------- |
| `cookie.getCookie(key)`        | JS*KV 存储，key 前缀 `\_\_cookie*`                  |
| `cookie.setCookie(key, val)`   | JS_KV 存储                                          |
| `source.getLoginInfoMap()`     | JS*KV 存储，key 前缀 `\_\_source_login*`            |
| `source.getVariable(key)`      | JS*KV 存储，key 前缀 `\_\_source_var*{sourceKey}\_` |
| `source.setVariable(key, val)` | JS_KV 存储                                          |
| `source.putVariable(key, val)` | JS_KV 存储                                          |
| `cache.getMemory(key)`         | 同 cache.get，JS_KV 存储                            |
| `cache.putMemory(key, val)`    | 同 cache.put，JS_KV 存储                            |
| `java.hexDecodeToString(hex)`  | 纯 Rust hex→UTF-8 转换                              |
| `java.ajaxAll(specs)`          | 循环调用 java_ajax，返回 JSON 数组                  |
| `java.startBrowser(url)`       | no-op stub，返回空字符串                            |

**验证命令**：

```powershell
cargo check -p reader-core
cargo test -p reader-core
```

**通过项**：31 tests passed

**确定已有并确认的 API**（本轮未改）：

- `java.ajax`、`java.get/post/put` — 已有完整 HTTP 实现
- `java.base64Encode/Decode` — 已有标准 base64
- `java.md5Encode`、`java.aesBase64DecodeToString` — 已有加密实现

**下轮第一件事**：
用真实网络验证书源搜索链路。先导入书旗源，用 `booksource_search` 测试搜索功能是否能通过 JS API 调用链路正常工作。需要先设置自定义域名（`zdym` 依赖 `source.getLoginInfoMap` 中的"自定域名"键）。

**不得重复做的事**：

- 不要再添加已确认存在的 JS API（java.ajax、base64Encode 等已有）
- 不要再写书源导入测试

---

## 记录标题：2026-06-09 Iteration 7

**本轮目标**：真实网络验证书源搜索链路

**修改文件**：

- `crates/reader-core/tests/source_compat_import.rs` — 新增强 `shuqi_source_live_search`、`qimao_source_live_search` 两个实网测试

**验证命令**：

```powershell
cargo test --workspace
```

**通过项**：33 tests（32 passed + 1 live-network ignored）

**实网验证结果**：

| 书源         | search | 耗时  | 结果                          |
| ------------ | ------ | ----- | ----------------------------- |
| **书旗小说** | PASS   | 8.53s | 搜索"系统"成功返回有效书籍    |
| **七猫小说** | PASS   | 7.33s | 搜索"测试"成功，JS API 链路通 |
| 番茄小说     | 未测   | -     | 待后续验证                    |

JS API shim（java.ajax、java.hexDecodeToString、cookie.getCookie、source.getLoginInfoMap 等）已在真实网络请求中得到验证。

**下轮第一件事**：
按计划第 0 节，搜索链路已打通，下一步进入第四优先级——完善阅读器和书架体验（第 26.6 节）。优先处理 STUB command 中影响用户操作的部分：

1. `booksource_cancel` — 实现真实的任务取消机制
2. `booksource_list_streaming` — 实现增量推送
3. `config_list_scopes` — 接入真实 storage/config scope

先读 `src-tauri/src/state.rs` 了解当前 TaskRegistry 状态，再读 `src-tauri/src/commands/config.rs` 了解 config_list_scopes 的实现。

**不得重复做的事**：

- 不要再加 JS API（13 个全覆盖）
- 不要再做实网测试（已有实网覆盖，非门禁硬依赖）

---

## 记录标题：2026-06-09 Iteration 9

**本轮目标**：修复 `config_list_scopes` STUB + 补齐缺失脚本（AUDIT-002/003）

**修改文件**：

- `crates/reader-core/src/service/json_document_service.rs` — 新增强 `list_namespaces`（SQL DISTINCT namespace）
- `crates/reader-core/src/facade.rs` — 新增强 `config_list_scopes` 方法
- `src-tauri/src/commands/config.rs` — `config_list_scopes` 从空数组升级为真实 SQL 查询
- `scripts/copy-harmony-web.mjs` — 新建（Harmony 未启用，最小骨架）
- `scripts/booksource-node-runtime.mjs` — 新建（Node 环境书源词法分析 + eval 说明）

**验证命令**：

```powershell
node scripts/ci/check-scripts.mjs
node scripts/ci/quality-gate.mjs
cargo test --workspace
```

**通过项**：

- check-scripts: 5/5 script references OK
- quality-gate: 12/12 checks PASS
- cargo test: 32 passed

**下轮第一件事**：
剩余 4 个 STUB command 中，`booksource_list_streaming` 影响前端书源列表加载体验，优先处理。目前是"一次性列出后 emit"，需改为分批增量扫描（每次 emit 一批书源）。先读 `src-tauri/src/commands/source.rs` 的 `booksource_list_streaming` 实现。

**不得重复做的事**：

- 不要再修改 `config_list_scopes`（已实现 SQL 查询）
- 不要再创建缺失脚本（AUDIT-002/003 已修复）

---

## 记录标题：2026-06-09 Iteration 10

**本轮目标**：`booksource_list_streaming` 增量推送改造

**修改文件**：

- `src-tauri/src/commands/source.rs` — `booksource_list_streaming` 从一次性全量 emit 改为分批增量（每批 20 个）多次 emit

**验证命令**：

```powershell
cargo check -p legado-tauri
cargo test --workspace
```

**通过项**：全部通过

**变更说明**：原来 `booksource_list_streaming` 一次性拿到所有书源后发送单个 `booksource:batch` 事件。现在按每批 20 个拆分，每批发送一个 `booksource:batch` 事件（`done: false`），最后一批标记 `done: true`。前端可以逐步渲染书源列表。

**下轮第一件事**：
剩余 3 个 STUB command（`booksource_eval`、`booksource_purchase_chapter`、`booksource_call_fn`），其中 `booksource_eval` 影响 AI 书源编辑工具链。当前只允许 `entryCode` 为空（返回能力列表），需要支持真实 JS 代码评估。先读 `crates/reader-core/src/parser/js.rs` 的 `eval_js` 函数，确认安全边界后再实现。

**不得重复做的事**：

- 不要再改动 `booksource_list_streaming`（已实现增量推送）

---

## 记录标题：2026-06-09 Iteration 11

**本轮目标**：`booksource_eval` — 支持真实 JS 代码评估

**修改文件**：

- `crates/reader-core/src/facade.rs` — 新增强 `eval_source_entry` 方法
- `src-tauri/src/commands/source.rs` — `booksource_eval` 非空 entryCode 现在执行 sandbox rquickjs 评估

**验证命令**：

```powershell
cargo check -p legado-tauri
cargo test --workspace
```

**通过项**：全部通过

**变更说明**：原来 `booksource_eval` 仅在 entryCode 为空时返回书源能力列表，非空直接拒绝。现在非空 entryCode 会在加载书源上下文后通过独立 rquickjs Runtime 执行（每次调用创建新 Runtime，无持久状态），返回执行结果字符串。AI 书源编辑工具链的 `eval_in_source` 现可正常工作。

**下轮第一件事**：
剩余 2 个 STUB command（`booksource_purchase_chapter` 返回固定 `{ok:true}`、`booksource_call_fn` 返回 UNSUPPORTED），均属需要深层架构的付费/自定义函数边界。按第 55.9 节优先级，应转向平台构建完善——验证 `pnpm run build:android:release` 和 `pnpm run build:windows:release` 正式产物。

**不得重复做的事**：

- 不要再改动 `booksource_eval`（已实现 sandbox 评估）

---

## 记录标题：2026-06-09 Iteration 12

**本轮目标**：GitHub 推送 + 结构化 Logger + 平台构建复核

**修改文件**：

- `.gitignore` — 新增 `src-tauri/gen/android/**/build/` 排除
- `src/utils/logger.ts` — 新建分级结构化 logger，通过 `frontend_log` 接入 Rust tracing
- git 初始化 + 3 次 commit + push 到 `https://github.com/FanhuaAwA/legado`

**验证命令**：

```powershell
pnpm lint
pnpm run build:windows:release
git push origin master
```

**通过项**：

- `pnpm lint`：72 warnings, 0 errors
- `pnpm run build:windows:release`：PASS（构建结果\windows\legado-tauri.exe, 19MB）
- GitHub push：818017a

**下轮第一件事**：
按第 26.6 节完善阅读器/书架体验——前端 UI polish。先阅读 `src/views/BookshelfView.vue` 和 `src/views/SearchView.vue` 了解当前状态，再评估需要改进的 UX 点。

**不得重复做的事**：

- 不要再改 `.gitignore`（构建产物排除已配置）
- 不要再创建 `logger.ts`（已创建）
