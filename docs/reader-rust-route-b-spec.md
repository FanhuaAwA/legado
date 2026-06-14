# Route B Spec: 将 reader-rust 核心能力迁移为 Tauri 原生架构

状态：Draft，已完成当前实现审计  
日期：2026-05-30  
适用仓库：`yuedu-tauri2`  
参考仓库：`reader-rust` at `82bd61da4d5df23f3f1ed5029e08a88ab563db80`

## 1. 背景

`reader-rust` 是一个完整的 Web 服务架构项目：Rust/Axum 后端监听 HTTP 端口，Vue 前端通过 `/reader3/...` API 调用后端，SQLite 和文件缓存位于服务端存储目录。它的核心价值在书源模型、Legado 规则兼容、解析器、爬虫、缓存、RSS、AI 资料和用户数据服务。

`yuedu-tauri2` 是 Tauri v2 + Vue 3 应用。前端已经通过 `src/composables/useTransport.ts` 和 `src/composables/useInvoke.ts` 抽象了统一命令层，在 Tauri 环境下走原生 IPC，在浏览器环境下可走 WebSocket 兼容层。现有业务已经大量调用命令，例如 `booksource_search`、`booksource_book_info`、`booksource_chapter_list`、`booksource_chapter_content`、`bookshelf_list`、`bookshelf_add` 等。

路线 B 的目标不是在 Tauri 中启动本地 HTTP 服务，而是把 `reader-rust` 的核心能力抽成 Rust 库，由 Tauri `#[tauri::command]` 直接调用。这样可以保留跨平台桌面和移动端的一致架构，避免 localhost 端口、sidecar、多进程生命周期、防火墙和移动端后台限制。

## 2. 目标

1. 将 `reader-rust` 可复用业务能力抽象为 `reader-core` Rust crate。
2. 在 `src-tauri` 中通过 Tauri commands 暴露阅读核心能力。
3. 前端继续通过 `invokeWithTimeout` 调用命令，尽量减少 UI 层改动。
4. 保持桌面端和移动端同一套调用模型：`Vue -> Tauri IPC -> Rust core`。
5. 支持渐进迁移，优先接入书源兼容和章节阅读主链路，再迁移高级能力。
6. 保留现有 JS 书源系统，同时引入 `reader-rust` 的 Legado JSON 书源兼容能力。

## 3. 非目标

1. 不在 Tauri 中长期运行 Axum HTTP 服务。
2. 不直接复用 `reader-rust/src/api` 和 HTTP handler 作为业务入口。
3. 不要求一次性迁移 `reader-rust` 的全部 Web 前端。
4. 不在第一阶段实现多用户服务端账号系统。
5. 不在第一阶段保证所有 Legado 规则 100% 兼容，先以测试覆盖和主流规则为验收边界。

## 4. 官方能力边界

本方案依赖 Tauri v2 的这些能力：

1. Commands：前端通过 `invoke` 调用 Rust 函数，Rust 侧通过 `#[tauri::command]` 暴露命令。官方文档：<https://v2.tauri.app/develop/calling-rust/>
2. Events / Channels：用于搜索进度、书源扫描批次、缓存下载进度等流式数据。官方同一文档说明 commands 可传入 channel 以流式发送数据。
3. State 管理：Tauri 应用可通过 `app.manage(...)` 保存共享运行时状态，再由 commands 读取。
4. 移动端：Tauri v2 支持桌面和移动目标，但移动端不能依赖桌面式 sidecar 进程模型。官方起步文档：<https://v2.tauri.app/start/>

## 5. 当前仓库观察

当前 `yuedu-tauri2` 具备良好的前端适配基础，并且工作树中已经开始实现 Route B：

1. `src/composables/useInvoke.ts` 已经是统一命令入口。
2. `src/composables/useTransport.ts` 已经抽象 Tauri IPC / Harmony bridge / WebSocket 三种传输。
3. `src/stores/scriptBridge.ts` 已定义阅读核心命令：
   - `booksource_search`
   - `booksource_book_info`
   - `booksource_chapter_list`
   - `booksource_chapter_content`
   - `booksource_purchase_chapter`
   - `booksource_call_fn`
   - `booksource_explore`
   - `booksource_cancel`
4. `src/composables/useBookshelf.ts` 和 `src/stores/bookshelf.ts` 已定义书架命令：
   - `bookshelf_list`
   - `bookshelf_add`
   - `bookshelf_get`
   - `bookshelf_update_progress`
   - `bookshelf_save_chapters`
   - `bookshelf_get_chapters`
   - `bookshelf_save_content`
   - `bookshelf_get_content`
   - `bookshelf_get_cached_indices`
5. 当前工作树已经存在 Rust workspace：根 `Cargo.toml` 声明 `crates/reader-core` 和 `src-tauri` 两个成员。
6. `src-tauri/` 已经恢复 Rust 工程骨架，包含 `Cargo.toml`、`build.rs`、`src/lib.rs`、`src/main.rs`、`src/state.rs` 和 commands 目录。
7. `crates/reader-core/` 已经存在，包含 `model`、`parser`、`crawler`、`storage`、`service`、`source_runtime`、`facade`、`dto`、`error` 和测试。
8. 书源导入、双运行时列表、搜索/详情/目录/正文主链路、书架缓存、配置存储已经有可编译实现；当前完成度和缺口见第 26 节。
9. `reader-rust/` 当前作为本地参考仓库存在于工作树，但不属于 `yuedu-tauri2` 应提交源码，已在 `.gitignore` 中忽略。

## 6. 目标架构

```text
Vue UI
  |
  | invokeWithTimeout(command, args)
  v
Tauri command layer
  |
  | typed request/response DTO
  v
reader-core facade
  |
  +-- source catalog
  +-- source execution
  +-- parser / rule engine
  +-- crawler / HTTP client
  +-- bookshelf repository
  +-- chapter/content cache
  +-- config/document store
  +-- optional RSS / AI / sync adapters
  |
  v
App data directory
  +-- SQLite
  +-- source files
  +-- chapter cache
  +-- image cache
  +-- config JSON
```

推荐 Rust workspace：

```text
yuedu-tauri2/
  Cargo.toml                       # workspace，可选
  crates/
    reader-core/
      Cargo.toml
      src/
        lib.rs
        app_state.rs
        error.rs
        model/
        parser/
        crawler/
        source/
        bookshelf/
        cache/
        storage/
        facade/
  src-tauri/
    Cargo.toml
    src/
      lib.rs
      commands/
        mod.rs
        source.rs
        bookshelf.rs
        reader.rs
        cache.rs
        config.rs
```

如果不想立刻引入 workspace，也可以先把 `reader_core` 作为 `src-tauri/src/reader_core/` 内部模块，等接口稳定后再拆 crate。长期推荐独立 crate，便于测试和移动端编译控制。

## 7. 模块拆分

### 7.1 可直接迁入 reader-core 的模块

从 `reader-rust` 迁入并去 Web 化：

```text
reader-rust/src/model
reader-rust/src/parser
reader-rust/src/crawler
reader-rust/src/storage/cache
reader-rust/src/storage/fs
reader-rust/src/storage/db/migrations
reader-rust/src/storage/db/repo.rs
reader-rust/src/util
reader-rust/src/error
```

迁入后需要调整：

1. `crate::api` 依赖必须移除。
2. `AppError` 保留，但增加 Tauri 友好的 `SerializableError`。
3. 文件路径不能依赖当前工作目录，必须从 app data root 派生。
4. `tracing` 日志要接入 Tauri event，至少能发 `rust:log`。

### 7.2 需要改造后迁入的模块

```text
reader-rust/src/service/book_service.rs
reader-rust/src/service/book_source_service.rs
reader-rust/src/service/json_document_service.rs
reader-rust/src/service/book_group_service.rs
reader-rust/src/service/ai_book_service.rs
reader-rust/src/service/ai_model_service.rs
reader-rust/src/service/update_service.rs
```

改造方向：

1. 把 HTTP handler 参数转换逻辑移到 command layer。
2. service 只接收强类型 DTO 和领域模型。
3. 流式功能通过 callback/channel 抽象，不再返回 Axum SSE response。
4. `user_ns` 在本地模式默认为 `"local"`，后续同步/多配置再扩展。

### 7.3 不迁入或仅参考的模块

```text
reader-rust/src/api
reader-rust/src/app/bootstrap.rs
reader-rust/src/app/config.rs
reader-rust/frontend
Dockerfile*
```

这些属于 Web 服务壳、Web 前端和部署资产。路线 B 不复用 Axum 路由和 Docker 部署逻辑。

## 8. reader-core API 设计

核心通过一个 facade 暴露，不让 Tauri command 直接拼装底层组件。

```rust
pub struct ReaderCore {
    pub source_catalog: SourceCatalogService,
    pub source_runtime: SourceRuntimeService,
    pub bookshelf: BookshelfService,
    pub cache: CacheService,
    pub config: ConfigService,
}

impl ReaderCore {
    pub async fn new(options: ReaderCoreOptions) -> Result<Self, ReaderCoreError>;
}

pub struct ReaderCoreOptions {
    pub app_data_dir: PathBuf,
    pub request_timeout_secs: u64,
    pub user_agent: Option<String>,
    pub secure_mode: SecureMode,
}
```

内部依赖：

```text
HttpClient
RuleEngine
FileCache
Sqlite pool
JsonDocumentService
BookSourceService
BookService
```

`ReaderCore` 初始化步骤：

1. 创建目录。
2. 初始化 SQLite。
3. 执行 migrations。
4. 初始化 HTTP client。
5. 初始化 RuleEngine。
6. 初始化缓存目录。
7. 构造 service graph。
8. 返回可 clone 的 `Arc<ReaderCore>`。

Tauri state：

```rust
pub struct AppState {
    pub core: Arc<ReaderCore>,
    pub cancel_tokens: TaskRegistry,
}
```

## 9. 存储规范

所有本地数据必须写到 Tauri app data 目录下，不能写仓库目录或当前工作目录。

建议目录：

```text
$APPDATA/yuedu-tauri2/
  reader/
    reader.db
    sources/
      legado-json/
      script-js/
    cache/
      chapters/
      images/
      http/
    assets/
    config/
    backups/
```

SQLite 建议统一放：

```text
sqlite:<app_data>/reader/reader.db?mode=rwc
```

兼容策略：

1. 现有 JS 书源文件仍保存在当前项目已有书源目录。
2. 新导入的 Legado JSON 书源可保存为 SQLite JSON 记录，也可转为本地 normalized source JSON 文件。
3. 书架、章节目录、正文缓存统一由 `reader-core` 管理。
4. 如果现有仓库已经有书架存储格式，先写迁移器，再切换命令实现。

## 10. 书源模型策略

当前 `yuedu-tauri2` 使用 JS 书源，文件头包含：

```text
@name
@version
@url
@type
@enabled
```

`reader-rust` 使用 Legado/阅读兼容 JSON 模型：

```text
bookSourceName
bookSourceUrl
searchUrl
ruleSearch
ruleBookInfo
ruleToc
ruleContent
```

目标是支持双运行时，而不是强行只保留一种：

```text
SourceRuntime
  +-- JsScriptRuntime       # 保留 yuedu-tauri2 当前书源系统
  +-- LegadoRuleRuntime     # 引入 reader-rust 规则引擎
```

统一前端返回模型仍使用现有 TypeScript 类型：

```ts
BookItem;
BookDetail;
ChapterItem;
ShelfBook;
CachedChapter;
```

Rust 侧增加统一 DTO：

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub source_id: String,
    pub file_name: Option<String>,
    pub source_dir: Option<String>,
    pub runtime: SourceRuntimeKind,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceRuntimeKind {
    JsScript,
    LegadoRule,
}
```

兼容规则：

1. `fileName` 继续作为前端主键，避免大改 UI。
2. Legado JSON 书源导入后生成稳定 `fileName`，例如 `legado-{hash}.json` 或 `{safeName}.legado.json`。
3. `BookSourceMeta.sourceType` 映射 Legado 默认 `novel`，RSS/漫画/音频后续扩展。
4. `BookSourceMeta.hasExplore` 由 `exploreUrl` 或 `ruleExplore` 推导。
5. `fnsCache` 中 Legado 书源能力映射：
   - 有 `searchUrl` + `ruleSearch` -> `search`
   - 有 `ruleBookInfo` -> `bookInfo`
   - 有 `ruleToc` -> `toc`
   - 有 `ruleContent` -> `content`
   - 有 `exploreUrl` -> `explore`
6. `BookSourceMeta.capabilities` 可携带后端轻量扫描结果，用于前端预热 `fnsCache`；复杂动态能力仍允许前端按需检测兜底。

## 11. Tauri Command 契约

命令命名优先复用当前前端已经调用的名字，降低迁移成本。

### 11.1 书源管理

```text
booksource_get_dir() -> string
booksource_get_dirs() -> string[]
booksource_add_dir(dirPath) -> void
booksource_remove_dir(dirPath) -> void
booksource_pick_dir() -> string
booksource_list() -> BookSourceMeta[]
booksource_list_streaming(requestId, force?) -> void
booksource_read(fileName, sourceDir?) -> string
booksource_save(fileName, content, sourceDir?) -> void
booksource_delete(fileName, sourceDir?) -> void
booksource_delete_batch(items) -> BookSourceBatchDeleteResult
booksource_toggle(fileName, enabled, sourceDir?) -> void
booksource_import_legacy_json_text(content, smartExploreSubCategories) -> LegacyJsonImportResult
booksource_import_legacy_json_url(url, smartExploreSubCategories) -> LegacyJsonImportResult
```

事件：

```text
booksource:batch
payload: { requestId, items, done, total?, error? }
```

### 11.2 书源执行

```text
booksource_search(fileName, keyword, page, sourceDir?) -> BookItem[]
booksource_book_info(fileName, bookUrl, sourceDir?) -> BookDetail
booksource_chapter_list(fileName, bookUrl, taskId?, sourceDir?) -> ChapterItem[]
booksource_chapter_content(fileName, chapterUrl, sourceDir?, categoryParams?) -> string | ContentPayload
booksource_purchase_chapter(fileName, chapterUrl, chapter?, sourceDir?) -> PurchaseChapterResult
booksource_explore(fileName, page, category, noCache?, sourceDir?) -> unknown
booksource_call_fn(fileName, fnName, args, sourceDir?) -> unknown
booksource_cancel(taskId) -> void
```

`ContentPayload` 为后续漫画/视频保留：

```ts
type ContentPayload =
  | string
  | {
      content?: string;
      imageUrls?: string[];
      mediaUrl?: string;
      headers?: Record<string, string>;
      type?: "text" | "comic" | "video" | "audio";
    };
```

### 11.3 书架和缓存

```text
bookshelf_list() -> ShelfBook[]
bookshelf_add(book, fileName, sourceName) -> ShelfBook
bookshelf_remove(id) -> void
bookshelf_get(id) -> ShelfBook
bookshelf_update_progress(id, chapterIndex, chapterUrl, pageIndex?, scrollRatio?, playbackTime?, readerSettings?) -> void
bookshelf_set_private(id, isPrivate) -> void
bookshelf_save_chapters(id, chapters) -> void
bookshelf_get_chapters(id) -> CachedChapter[]
bookshelf_update_book(book, chapters?) -> ShelfBook
bookshelf_restore_source_switch(id) -> SourceSwitchRestoreResult
bookshelf_save_content(id, chapterIndex, content) -> void
bookshelf_get_content(id, chapterIndex) -> string | null
bookshelf_delete_content(id, chapterIndex) -> void
bookshelf_get_cached_indices(id) -> number[]
bookshelf_save_txt_chapters(id, chapters) -> void
bookshelf_get_episode_progress(id) -> Record<string, EpisodeProgress>
bookshelf_save_episode_progress(id, chapterUrl, time, duration) -> void
```

### 11.4 配置和脚本配置

保留现有命令：

```text
config_read(scope, key) -> string
config_write(scope, key, value) -> void
config_read_json(scope, key) -> JSON | null
config_write_json(scope, key, value) -> void
config_delete_key(scope, key) -> void
config_read_all(scope) -> string
config_clear(scope) -> void
config_read_bytes(scope, key) -> number[]
config_write_bytes(scope, key, value) -> void
```

这些可以映射到 `reader-rust` 的 `JsonDocumentService` 或现有本地存储实现。

## 12. 错误模型

Rust command 不应直接返回任意字符串错误。建议统一错误：

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
    pub retryable: bool,
}
```

常见 code：

```text
SOURCE_NOT_FOUND
SOURCE_DISABLED
SOURCE_PARSE_FAILED
SOURCE_RUNTIME_FAILED
NETWORK_FAILED
TIMEOUT
RULE_PARSE_FAILED
BOOK_NOT_FOUND
CHAPTER_NOT_FOUND
CACHE_MISS
CANCELLED
IO_ERROR
DB_ERROR
UNSUPPORTED
```

前端 `invokeWithTimeout` 可以先兼容字符串错误；第二阶段再识别结构化错误并优化 UI。

## 13. 流式和取消

路线 B 不使用 SSE。所有流式行为使用 Tauri event 或 channel。

推荐模型：

```text
command starts task
  -> returns immediately or returns taskId
  -> emits progress events
  -> emits done event
```

适用场景：

1. `booksource_list_streaming`
2. 多源聚合搜索
3. 缓存整本书
4. 检测无效书源
5. 批量导入书源

取消机制：

```rust
pub struct TaskRegistry {
    tasks: DashMap<String, CancellationToken>,
}
```

命令：

```text
booksource_cancel(taskId)
reader_task_cancel(taskId)
```

事件命名：

```text
booksource:batch
search:progress
cache:progress
reader:task
rust:log
script:log
```

## 14. HTTP 和安全策略

`reader-rust` 的爬虫能力可复用，但在 Tauri 中必须收紧：

1. 默认允许 `http` 和 `https` 请求。
2. 拒绝 `file://`、`ftp://`、`data:`、`javascript:` 等非网络协议。
3. 可选拦截内网地址访问，防止恶意书源 SSRF 到路由器、本机服务或云 metadata。
4. 对每个书源实现请求频控，复用 `concurrentRate`。
5. 支持书源级 cookie，但要隔离到 source/user namespace。
6. 支持全局 User-Agent 和 source header。
7. 移动端要支持系统 TLS 根证书，避免硬编码平台证书路径。

SSRF 策略建议分级：

```text
normal: 禁止 127.0.0.0/8、localhost、169.254.0.0/16、::1
developer: 允许本地地址，用于调试
unrestricted: 完全交给用户，需明显警告
```

## 15. JS 引擎策略

当前 `yuedu-tauri2` README 描述使用 Boa JS 引擎；`reader-rust` 使用 `rquickjs` 支持 Legado 规则中的 JS。路线 B 需要明确双引擎或统一引擎策略。

推荐阶段策略：

1. 阶段一保留现有 JS 书源运行时，不迁动 Boa 相关逻辑。
2. 引入 `reader-rust` 的 `rquickjs` 仅服务 Legado 规则兼容。
3. 将 `legado.http`、`legado.dom`、`legado.config` 等现有宿主 API 与 Legado rule JS API 做边界隔离。
4. 后续评估是否统一到一个 JS 引擎。

风险：

1. `rquickjs` 在移动端交叉编译可能需要额外验证。
2. iOS 对 JIT 有限制；QuickJS/Boa 是解释器，一般不依赖 JIT，但仍需真机编译和运行验证。
3. 宿主 API 必须有超时、取消和内存限制。

## 16. 前端适配策略

前端不应大面积重写。核心原则：

1. 保持 `invokeWithTimeout` 作为唯一业务调用入口。
2. 保持现有 `useScriptBridgeStore` 方法名。
3. 保持现有 `BookItem`、`BookDetail`、`ChapterItem`、`ShelfBook` 类型。
4. 只在 `useBookSource.ts`、`useBookshelf.ts`、`scriptBridge.ts` 层做少量兼容。

需要新增的前端能力：

1. 书源类型展示：区分 `JS` 与 `Legado JSON`。
2. 导入入口：支持粘贴 Legado JSON、远程 JSON URL。
3. 能力检测：Legado JSON 能力来自规则字段，不一定来自函数列表。
4. 错误 UI：显示规则解析错误、网络错误、源失效。
5. 调试面板：Legado 书源调试需要显示 URL 分析、请求 headers、解析结果。

## 17. 数据迁移

### 17.1 书架迁移

如果现有书架已经可用，迁移分两步：

1. 先实现 commands，保持返回格式完全一致。
2. 再编写一次性迁移器，把旧 JSON/文件数据导入 SQLite 或 reader-core 存储。

迁移必须幂等：

```text
old id -> new id
bookUrl + fileName unique
chapters by shelf book id
content cache by shelf book id + chapter index
progress fields preserved
```

### 17.2 书源迁移

现有 JS 书源不转换，继续由 JS runtime 执行。

Legado JSON 导入流程：

```text
read JSON
  -> book_source_from_value
  -> normalize legacy fields
  -> validate required fields
  -> assign sourceId/fileName
  -> save source record
  -> emit booksource:batch or refresh event
```

### 17.3 缓存迁移

章节正文缓存优先按现有命令语义兼容：

```text
bookshelf_save_content(id, chapterIndex, content)
bookshelf_get_content(id, chapterIndex)
```

底层可从文件缓存切换到 SQLite 或文件系统，但对前端透明。

## 18. 分阶段实施计划

### Phase 0: 工程骨架恢复

验收：

1. `src-tauri/Cargo.toml` 存在。
2. `src-tauri/src/lib.rs` 存在。
3. `pnpm run dev:desktop` 能启动空壳。
4. 最少注册 `get_platform`、`rust:log`、`open_dir_in_explorer` 等基础命令。

### Phase 1: reader-core crate

任务：

1. 新建 `crates/reader-core`。
2. 迁入 `model`、`parser`、`crawler`、`util`、`error`。
3. 迁入 `storage/cache`。
4. 让 `reader-rust` 的 parser/crawler 单元测试在 `reader-core` 中通过。

验收：

```bash
cargo test -p reader-core parser
cargo test -p reader-core crawler
```

### Phase 2: Legado 书源导入和列表

任务：

1. 实现 `SourceCatalogService`。
2. 实现 `booksource_import_legacy_json_text`。
3. 实现 `booksource_import_legacy_json_url`。
4. 实现 `booksource_list` 和 `booksource_list_streaming`。
5. 前端书源列表显示 Legado JSON 书源。

验收：

1. 可导入 `reader-rust` 测试用 JSON 书源。
2. 列表能显示名称、URL、启用状态、能力标记。
3. 禁用/启用能持久化。

### Phase 3: 搜索、详情、目录、正文主链路

任务：

1. 实现 `booksource_search`。
2. 实现 `booksource_book_info`。
3. 实现 `booksource_chapter_list`。
4. 实现 `booksource_chapter_content`。
5. 复用 `reader-rust` 的 `RuleEngine` 和 `BookService` 主流程。

验收：

1. 使用 Legado JSON 书源搜索书籍。
2. 打开详情并加载目录。
3. 打开章节正文。
4. 加入书架后可从书架继续阅读。
5. 缓存命令可读取已缓存正文。

### Phase 4: 书架和缓存统一

任务：

1. 将 `bookshelf_*` commands 接入 reader-core 存储。
2. 实现章节缓存、目录缓存、阅读进度。
3. 实现 source switch backup 或兼容现有 `bookshelf_restore_source_switch`。
4. 保持本地 TXT 功能可用。

验收：

1. 现有书架 UI 不需要大改。
2. 阅读进度、分页页码、滚动比例能保存和恢复。
3. 本地 TXT 导入仍可读。

### Phase 5: 高级功能

按价值排序迁移：

1. 发现页 `booksource_explore`
2. 书源测试 `booksource_run_tests`
3. 无效书源检测
4. 整本缓存进度
5. RSS
6. AI 资料
7. WebDAV/同步适配

## 19. 测试策略

### 19.1 Rust 单元测试

迁入并保留这些测试：

```text
reader-rust/tests/book_source_compat.rs
reader-rust/tests/book_source_validation.rs
reader-rust/tests/book_source_headers.rs
reader-rust/tests/js_compat.rs
reader-rust/tests/ai_proxy.rs
reader-rust/tests/version_update.rs
```

需要按 crate 路径调整 imports。

### 19.2 Command 集成测试

使用 Tauri command handler 或 core facade 直接测试：

```text
import legacy json
list sources
search
book info
chapter list
chapter content
save shelf
save progress
read cache
```

### 19.3 前端回归

最小回归：

1. 书源管理页导入、启用、禁用、删除。
2. 发现页/搜索页打开详情。
3. 目录加载。
4. 阅读器打开章节。
5. 阅读进度保存。
6. 关闭重开恢复。

### 19.4 移动端验证

必须真机验证：

1. Android 网络请求、TLS、文件存储。
2. Android WebView invoke 是否稳定。
3. iOS 编译 `reader-core`。
4. iOS QuickJS/Boa runtime 是否可运行。
5. App 前后台切换后任务取消/恢复行为。

## 20. 性能目标

1. 冷启动核心初始化 < 800ms，数据库迁移除外。
2. `booksource_list_streaming` 首批结果 < 300ms。
3. 单源搜索默认超时沿用前端设置，默认 35s。
4. 章节正文缓存命中读取 < 50ms。
5. 大量书源列表增量推送，不阻塞主线程。
6. 所有长任务可取消。

## 21. 安全目标

1. 不暴露 localhost HTTP 服务。
2. 不执行远程 native code。
3. JS 书源运行时默认无文件系统直接访问。
4. HTTP 请求有协议白名单和超时。
5. 用户敏感配置写入 app data，必要时走系统 keychain/credential 插件。
6. 日志不输出完整 token、cookie、Authorization。
7. 书源导入前做格式校验和明显提示。

## 22. 风险和缓解

| 风险                                    | 影响                  | 缓解                                                |
| --------------------------------------- | --------------------- | --------------------------------------------------- |
| `src-tauri` Rust 工程缺失               | 无法直接实现 commands | Phase 0 先恢复骨架                                  |
| `rquickjs` 移动端编译问题               | Legado JS 规则不可用  | 提前做 Android/iOS smoke test；必要时 feature gate  |
| 现有 JS 书源与 Legado JSON 书源模型不同 | UI 和缓存键混乱       | 引入 `SourceRuntimeKind`，前端继续用稳定 `fileName` |
| 规则兼容不完全                          | 部分书源不可用        | 迁入 reader-rust 兼容测试，逐步补 fixture           |
| 大量 command DTO 手写易漂移             | 前后端类型不一致      | 后续引入 ts-rs 或 specta 生成 TS 类型               |
| 网络请求被恶意书源滥用                  | SSRF/隐私风险         | 协议白名单、内网拦截、用户可见权限设置              |
| 一次性迁移过大                          | 难以稳定              | 严格按 Phase 实施，每阶段独立可验收                 |

## 23. 验收定义

路线 B 第一版完成的最低验收：

1. 不启动本地 HTTP 服务。
2. `booksource_import_legacy_json_text` 可导入 Legado JSON。
3. `booksource_list` 能返回 JS 和 Legado 两类书源。
4. `booksource_search` 可使用 Legado 书源搜索。
5. `booksource_book_info`、`booksource_chapter_list`、`booksource_chapter_content` 可完成一本书从详情到阅读。
6. `bookshelf_add`、`bookshelf_save_chapters`、`bookshelf_save_content`、`bookshelf_update_progress` 可持久化阅读状态。
7. 桌面端 `pnpm run dev:desktop` 主流程可用。
8. Android 至少完成编译和一次真机启动。
9. Rust 测试覆盖 reader-rust parser/crawler/book_source 兼容用例。

## 24. 推荐第一批 PR 拆分

1. `chore: restore tauri rust project skeleton`
2. `feat(reader-core): add model parser crawler modules`
3. `feat(reader-core): add source catalog and legado import`
4. `feat(tauri): expose booksource list/import commands`
5. `feat(reader-core): add legado search/info/toc/content facade`
6. `feat(tauri): wire reader commands to existing frontend`
7. `feat(reader-core): add bookshelf persistence facade`
8. `test: port reader-rust compatibility tests`

## 25. Open Questions

以下问题来自初版设计。当前工作树已经回答或部分回答：

1. `src-tauri` Rust 源码已经创建，但截至本次审计仍是未跟踪文件，需要后续纳入提交。
2. 现有 JS 书源 runtime 已作为 `crates/reader-core/src/source_runtime/js_source.rs` 保留，底层通过 `parser::js` 调用 rquickjs 宿主 API。
3. 当前书架持久化在 app data 下的 JSON 文件和章节正文文件中，Legado 书源目录和配置使用 SQLite/JSON 文档；还没有实现旧书架格式到 SQLite 的一次性迁移器。
4. 移动端尚未完成本 spec 要求的 Android 真机、iOS 编译和 QuickJS/Boa 运行验证。
5. 当前实现已经采用 workspace 和独立 `crates/reader-core`。
6. 前端仍保留 WebSocket 浏览器模式抽象；当前 Rust Route B 没有实现可选 WS server adapter，Tauri IPC 是主路径。

## 26. 当前实现审计（2026-05-30）

本节基于当前工作树核查，用于说明“已经实现了本 spec 的哪些内容”。它不是新增目标，而是对当前实现状态的快照。

### 26.1 已实现或基本实现

| 范围                    | 当前证据                                                                                                                                                                                                                                                           | 状态     |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------- |
| Phase 0 工程骨架        | 根 `Cargo.toml` workspace；`src-tauri/Cargo.toml`；`src-tauri/src/lib.rs`、`main.rs`、`state.rs`；`commands::handler()` 注册命令                                                                                                                                   | 已实现   |
| 基础 Tauri commands     | `get_platform`、`open_dir_in_explorer`、`frontend_log`、`script_dialog_result` 已注册；初始化后 emit `rust:log`                                                                                                                                                    | 已实现   |
| `reader-core` crate     | `crates/reader-core` 已拆出 `model/parser/crawler/storage/service/source_runtime/facade/dto/error/util`                                                                                                                                                            | 基本实现 |
| App data 存储根         | `ReaderCore::new` 使用 `options.app_data_dir.join("reader")` 创建 `reader.db`、`sources/script-js`、`sources/legado-json`、`cache/chapters`、`config`                                                                                                              | 已实现   |
| SQLite migrations       | `storage/db/mod.rs` 使用 `sqlx::migrate!` 执行 `storage/db/migrations`；已有 `book_sources`、`json_documents` 等表                                                                                                                                                 | 已实现   |
| Legado JSON 导入        | `import_legacy_json_text` 支持对象/数组、字段校验、稳定 `.legado.json` 文件名、文件保存和 DB upsert；`import_legacy_json_url` 支持 HTTP/HTTPS URL 下载                                                                                                             | 已实现   |
| 书源列表和双运行时      | `list_sources` 合并 JS 书源目录和 Legado DB/文件；`BookSourceMeta.runtime` 区分 `JsScript`/`LegadoRule`；能力由 JS 函数或 Legado rule 字段推导                                                                                                                     | 基本实现 |
| 外部 JS 书源目录        | `booksource_get_dirs/add_dir/remove_dir` 通过配置持久化外部目录，并在列表时扫描                                                                                                                                                                                    | 已实现   |
| 书源启用/禁用和删除     | JS 书源修改 `@enabled`；Legado 书源更新 JSON/DB；删除支持文件和 DB 记录                                                                                                                                                                                            | 基本实现 |
| Legado 主阅读链路       | `search`、`book_info`、`chapter_list`、`chapter_content` 通过 `BookService`、`RuleEngine`、`HttpClient` 和 `FileCache` 执行                                                                                                                                        | 基本实现 |
| JS 书源主阅读链路       | `JsSourceRuntime` 支持 `search`、`bookInfo`、`chapterList/toc`、`chapterContent/content`、`explore`                                                                                                                                                                | 基本实现 |
| 书架和章节缓存 commands | `bookshelf_list/add/remove/get/update_progress/set_private/save_chapters/get_chapters/update_book/restore_source_switch/save_content/get_content/delete_content/get_cached_indices/save_txt_chapters/get_episode_progress/save_episode_progress` 已注册并接入 core | 基本实现 |
| 配置和前端存储 commands | `config_read/write/read_json/write_json/delete_key/read_all/clear/read_bytes/write_bytes`，以及 `frontend_storage_*`、`app_config_*` 已注册并接入 `JsonDocumentService`                                                                                            | 基本实现 |
| 结构化 command 错误     | `CommandError { code, message, detail, retryable }` 已实现，Tauri command 统一返回 `Result<T, CommandError>`                                                                                                                                                       | 基本实现 |
| 前端导入入口            | `BookSourceView.vue` / `InstalledSourcesTab.vue` 已有阅读源文件和 URL 导入入口；`useBookSource.ts` 调用原生命令                                                                                                                                                    | 基本实现 |

### 26.2 部分实现或仍有缺口

| 范围                          | 当前状态                                                                                                                         | 后续需要                                                                                   |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `pnpm run dev:desktop` 验收   | 本次未启动桌面 dev server，只验证了 Rust workspace 测试和前端 lint 入口                                                          | 后续需要启动桌面端做主流程手动或自动回归                                                   |
| `booksource_list_streaming`   | 2026-06-14 已改为 core 扫描阶段分批 emit，并支持 30 分钟列表缓存、`force` 刷新、Tauri IPC/WS/headless 同构路由；前端逐批合并渲染 | 仍需真实大书源库与 Android 真机验证首批耗时和事件吞吐                                      |
| 取消机制                      | `booksource_cancel` 当前空实现；没有 `TaskRegistry`/`CancellationToken`                                                          | 长任务、搜索、缓存和测试任务需要接入取消                                                   |
| `booksource_call_fn`          | 当前返回 `UNSUPPORTED`                                                                                                           | 若仍要兼容 JS 自定义函数调试，需要设计安全执行边界                                         |
| `booksource_purchase_chapter` | 当前返回 `{ ok: true, purchased: true }`                                                                                         | 付费章节真实购买/授权逻辑未实现                                                            |
| 错误 code 精细度              | `CommandError` 已有结构，但 `ReaderCoreError::Message` 等会粗略映射为 `IO_ERROR`                                                 | 需要补齐 `SOURCE_DISABLED`、`RULE_PARSE_FAILED`、`CACHE_MISS`、`CANCELLED` 等精细分类      |
| HTTP/安全策略                 | `HttpClient` 有超时、cookie store、默认 UA；导入 URL 限制 HTTP/HTTPS                                                             | 尚未实现 `SecureMode`、内网地址拦截、source/user cookie 隔离、日志脱敏和书源级频控完整策略 |
| `ReaderCoreOptions`           | 已定义 `user_agent`、`secure_mode`，但初始化时未实际使用这些选项                                                                 | 应接入 HTTP client 和安全策略                                                              |
| 书架统一存储                  | 当前书架主体是 app data JSON 文件，章节正文是文件缓存；未统一进 SQLite                                                           | Phase 4 若要求 SQLite 统一，需要迁移器和幂等迁移测试                                       |
| 前端 runtime 展示             | Rust DTO 返回 `runtime`，但当前 TypeScript `BookSourceMeta` 接口和列表 UI 主要展示 `sourceType`，未明确显示 `JS/Legado` runtime  | 需要补 UI 标识和类型定义                                                                   |
| 调试面板                      | `booksource_eval` 只在 entryCode 为空时返回能力；任意调试代码被拒绝                                                              | Legado URL 分析、headers、解析结果调试 UI 尚未完成                                         |
| 移动端验证                    | 未完成 Android 真机、iOS 编译、QuickJS/Boa runtime 验证                                                                          | 需要单独移动端 smoke test                                                                  |
| 高级功能                      | RSS、AI 资料、WebDAV/同步适配、整本缓存进度、无效书源批量检测尚未迁移到 Route B                                                  | 按 Phase 5 继续拆分                                                                        |

### 26.3 测试结果

本次审计运行了以下命令：

```bash
cargo fmt --check
cargo test --workspace
```

结果：通过。覆盖情况包括：

1. `cargo fmt --check` 通过。执行前发现 `src-tauri/src/commands/source.rs` 有一个函数签名格式差异，已用 `cargo fmt` 修正。
2. `reader-core` 单元测试：17 个 parser/crawler/service 测试通过。
3. `crates/reader-core/tests/book_source_compat.rs`：7 个 Legado 书源兼容、URL 分析、规则解析测试通过。
4. `crates/reader-core/tests/js_compat.rs`：3 个 JS/rquickjs 兼容和 JS 书源主链路测试通过。
5. `crates/reader-core/tests/route_b_facade.rs`：1 个 Route B facade 集成测试通过，覆盖 Legado 导入、列表、搜索、详情、目录、正文、加入书架、保存章节、保存正文和阅读进度。
6. `src-tauri` crate 编译并运行 0 个单测，证明 command 层当前可编译。

沙箱内第一次运行时，本地 HTTP fixture 测试因不能绑定 `127.0.0.1:0` 失败；在允许本地端口绑定的环境下重跑后全部通过。

```bash
pnpm run lint
pnpm exec oxlint --type-aware --type-check .
pnpm exec vue-tsc -p tsconfig.app.json --noEmit
```

结果：

1. `pnpm run lint` 未通过，当前阻塞点是 `oxfmt --check .`。更新 `.gitignore` 后它不再扫描 `reader-rust/`、`target/`、`dist/`、`src-tauri/gen/schemas/`，但仍认为现有 314 个源码文件不符合 oxfmt 默认格式基线。未在本次审计中批量改写这些既有前端文件。
2. `pnpm exec oxlint --type-aware --type-check .` 退出码为 0，有 64 个既有 lint warnings。
3. `pnpm exec vue-tsc -p tsconfig.app.json --noEmit` 通过。
4. Node 环境提示当前是 `v23.11.0`，而 `package.json` 要求 Node 24；pnpm 还尝试检查自身更新但因网络限制出现 registry fetch warning。这两个 warning 未阻止 `oxlint` 和 `vue-tsc` 完成。
