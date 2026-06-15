# AI Task Status

## 2026-06-15 PERF-SEARCH-GROUPED-LAZY-RENDER status update

Status: `local-gate-pass`; this entry will be added to the current performance PR after push. Remote Quality Gate remains governed by GitHub Actions.

Task ID: `PERF-2026-06-15-SEARCH-GROUPED-LAZY-RENDER`. This round continues the search-page performance review by reducing grouped-mode DOM work when thousands of source-dependent result groups exist.

Review findings:

- Aggregated search mode was already incremental, but grouped mode still rendered one `SourceSearchGroup` for every active source.
- With large source packs, switching to grouped mode after a search could create hundreds or thousands of group components, including empty groups, in one render pass.
- This frontend render burst can still feel like a search/loading stall even when backend source execution is incremental and cancellable.

Key changes:

- Grouped mode now renders sources in batches, starting with 48 groups.
- Sources with loading, errors, or results are prioritized before idle empty groups.
- A compact load-more control reveals additional groups without changing search execution or result data.

Passed local gate:

- `cmd /c pnpm.cmd lint`
- `git diff --check`

Gate report: `reports/gates/2026-06-15-PERF-SEARCH-GROUPED-LAZY-RENDER/summary.md`.

Residual risk: grouped-mode batching reduces DOM pressure but does not replace true viewport virtualization; if users need to inspect thousands of empty source groups frequently, a future round can add virtual scrolling.

## 2026-06-15 PERF-JS-PREFETCH-CANCEL-COOPERATIVE status update

Status: `local-gate-pass`; this entry will be added to the current performance PR after push. Remote Quality Gate remains governed by GitHub Actions.

Task ID: `PERF-2026-06-15-JS-PREFETCH-CANCEL-COOPERATIVE`. This round continues cancellation review for source-dependent background work by propagating bookshelf prefetch cancellation into per-chapter JS content execution and retry waits.

Review findings:

- `prefetch_chapters_inner()` checked cancellation between chapters, but called plain `chapter_content()` for each chapter.
- When cancelled during a JS-backed content fetch, the content error could be caught by the retry loop and followed by retry backoff sleep instead of returning promptly.
- The fixed chapter list/content cancellation tests did not cover the prefetch wrapper, so a regression could reintroduce delayed cancellation in background caching.

Key changes:

- Prefetch now calls `chapter_content_with_cancel()` with the active prefetch token.
- Retry backoff and inter-chapter throttling now use cancellable sleep slices instead of one long sleep.
- Added a prefetch regression test that starts a JS chapter content infinite loop and verifies cancellation interrupts the prefetch operation before the engine timeout.

Passed local gate:

- `cargo fmt --all -- --check`
- `cargo test -p reader-core cancel_token_interrupts -- --nocapture`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cmd /c pnpm.cmd lint`
- `node scripts/ci/check-command-contract.mjs --json`
- `git diff --check`

Gate report: `reports/gates/2026-06-15-PERF-JS-PREFETCH-CANCEL-COOPERATIVE/summary.md`.

Residual risk: already in-flight synchronous HTTP requests remain bounded by request timeout; this round makes prefetch retries and delay windows cooperative after cancellation.

## 2026-06-15 PERF-JS-CHAPTER-CANCEL-COOPERATIVE status update

Status: `local-gate-pass`; this entry will be added to the current performance PR after push. Remote Quality Gate remains governed by GitHub Actions.

Task ID: `PERF-2026-06-15-JS-CHAPTER-CANCEL-COOPERATIVE`. This round continues the source-dependent cancellation review by extending the JS cooperative cancellation path from search to chapter list and chapter content commands.

Review findings:

- `booksource_chapter_list` and `booksource_chapter_content` accepted `task_id`, but only checked it before starting the command.
- Unlike search, these commands did not race the active work against `wait_for_cancel()`, so UI cancellation could wait for the whole chapter list/content operation to finish.
- reader-core JS chapter list/content execution still called the no-token JS runtime methods, so runaway JS could continue on blocking workers after cancellation.

Key changes:

- Added `chapter_list_with_cancel()` and `chapter_content_with_cancel()` in reader-core facade and JS source runtime.
- Updated Tauri chapter list/content commands to pass the task token into reader-core and return `CANCELLED` promptly via `tokio::select!`.
- Reused the JS thread-local cancellation token and QuickJS interrupt path introduced for search.
- Refactored the runaway JS cancellation test helper and added coverage for search, chapter list, and chapter content.

Passed local gate:

- `cargo fmt --all -- --check`
- `cargo test -p reader-core cancel_token_interrupts_runaway_source -- --nocapture`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cmd /c pnpm.cmd lint`
- `node scripts/ci/check-command-contract.mjs --json`
- `git diff --check`

Gate report: `reports/gates/2026-06-15-PERF-JS-CHAPTER-CANCEL-COOPERATIVE/summary.md`.

Residual risk: already in-flight `reqwest::blocking` requests still complete on configured timeout; this round extends cooperative cancellation to more source-dependent JS commands but does not forcibly abort synchronous network requests mid-flight.

## 2026-06-15 PERF-JS-SEARCH-CANCEL-COOPERATIVE status update

Status: `local-gate-pass`; this entry will be added to the current performance PR after push. Remote Quality Gate remains governed by GitHub Actions.

Task ID: `PERF-2026-06-15-JS-SEARCH-CANCEL-COOPERATIVE`. This round continues the large-source search performance work by addressing a self-review finding in the cancellation path: `booksource_search` could return `CANCELLED` to the UI while the underlying JS source evaluation kept running on a blocking worker.

Review findings:

- `src-tauri/src/commands/source.rs::booksource_search` registered a task token and raced the search future against `wait_for_cancel()`, but the token was not passed into reader-core.
- `ReaderCore::search()` spawned JS source execution via `spawn_blocking()` without a cooperative cancellation channel, so cancelling a large multi-source search could leave runaway JS or queued JS HTTP calls consuming worker time after the UI had stopped waiting.
- The QuickJS interrupt handler only checked the engine timeout deadline; it could not observe user cancellation.

Key changes:

- reader-core now exposes `search_with_cancel()` and passes the optional task token into JS source runtime search.
- JS source runtime wraps source function evaluation in a thread-local cancel token so QuickJS interrupt polling can stop CPU-bound JS loops.
- JS HTTP bridge checks cancellation before starting blocking requests and while waiting for same-host rate limiting, reducing post-cancel queued work.
- Tauri `booksource_search` passes the same task token to reader-core while preserving the existing command signature and `CANCELLED` response behavior.
- Added a regression test that starts a runaway JS search, cancels it, and verifies that it returns promptly before the engine timeout.

Passed local gate:

- `cargo fmt --all -- --check`
- `cargo test -p reader-core js_search_cancel_token_interrupts_runaway_source -- --nocapture`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cmd /c pnpm.cmd lint`
- `node scripts/ci/check-command-contract.mjs --json`
- `git diff --check`

Gate report: `reports/gates/2026-06-15-PERF-JS-SEARCH-CANCEL-COOPERATIVE/summary.md`.

Residual risk: an HTTP request that has already entered `reqwest::blocking` still cannot be force-aborted by this cooperative token and will return on request timeout; this change prevents new/rate-wait HTTP work and interrupts active QuickJS execution.

## 2026-06-15 PERF-LEGACY-IMPORT-URL-PROGRESS 状态更新

本轮状态：`local-gate-pass`；将追加到当前性能优化草稿 PR，远端 Quality Gate 状态以 GitHub Actions 为准。

任务 ID：`PERF-2026-06-15-LEGACY-IMPORT-URL-PROGRESS`。本轮继续处理“大型开源阅读/Legado JSON 书源 URL 导入长时间无反馈”的导入链路；范围覆盖 reader-core URL 导入、Tauri IPC、Route B WS router 和前端命令封装，不改变导入结果结构、书源规则执行语义、文件命名或第三方书源样本。

Review 发现的问题：

- `ReaderCore::import_legacy_json_url()` 下载远程 JSON 后调用无进度的 `import_legacy_json_text()`，URL 导入链路没有复用已实现的分批 progress callback。
- Tauri `booksource_import_legacy_json_url` 命令没有可选 `requestId`，无法像 text 导入一样通过 `booksource:import-progress` 事件向 UI 回传进度。
- WS router 的 `booksource_import_legacy_json_text` / `booksource_import_legacy_json_url` 仍走无进度兼容路径，Route B 远端调用会长时间只等最终结果。

关键修改：

- reader-core 新增 `import_legacy_json_url_with_progress()`，URL 下载开始时先上报初始 progress，下载完成后复用 `import_legacy_json_text_with_progress()` 的分批导入进度。
- Tauri URL 导入命令新增可选 `requestId`，并复用同一个 `booksource:import-progress` 事件 payload。
- WS router 的 text/url 导入均解析可选 `requestId`，并通过 app event 发送 import progress，保持 Route B 命令契约一致。
- 前端 `importLegacyJsonUrl()` 支持可选 `requestId`；旧调用不传参数仍保持兼容。
- `docs/reader-rust-route-b-spec.md` 同步更新 URL 导入可选 `requestId` 和 `booksource:import-progress` 事件契约。

已通过本地 gate：

- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `cargo fmt --all -- --check`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cargo check -p legado-headless`
- `cargo test -p reader-core import_legacy_json_url_reports_progress -- --nocapture`
- `cargo test -p legado-tauri --test ws_router booksource_import_legacy_json_text_accepts_request_id_in_ws_router -- --nocapture`
- `node scripts/ci/check-command-contract.mjs --json`
- `git diff --check`

Gate 报告：`reports/gates/2026-06-15-PERF-LEGACY-IMPORT-URL-PROGRESS/summary.md`。

## 2026-06-15 PERF-SOURCE-RELOAD-COALESCE 状态更新

本轮状态：`local-gate-pass`；将追加到当前性能优化草稿 PR，远端 Quality Gate 状态以 GitHub Actions 为准。

任务 ID：`PERF-2026-06-15-SOURCE-RELOAD-COALESCE`。本轮继续处理“大量书源导入/刷新后，已访问的搜索/发现/书源管理视图重复触发书源列表扫描”的 keep-alive 链路；范围限定在前端事件广播、视图刷新协作和 store in-flight 复用，不改变后端命令契约、书源规则执行、搜索结果结构或第三方书源样本。

Review 发现的问题：

- `BookSourceView` 在子标签页 reload 后先执行本页 `loadSources({ force: true })`，完成后再广播 `app:booksource-reload`；已访问的 Search/Explore 因 keep-alive 未卸载，会收到广播后再各自 force load 一次。
- `InstalledSourcesTab` 的重载、单源重载和升级路径会在 `emits("reload")` 后自己再发 `app:booksource-reload`，和父视图广播叠加。
- `bookSourceStore.reloadSources()` 会把 `_loadInFlight` 清空，显式 reload 可能绕过 store 单飞保护并发开第二次列表扫描。
- single 刷新路径很多只传 `fileName`，在多目录同名书源场景下容易扩大能力缓存失效范围。

关键修改：

- `InstalledSourcesTab` 不再直接广播跨视图 reload，只向父视图发带 `scope/fileName/sourceDir` 的 reload payload。
- `BookSourceView` 统一负责跨视图广播；它会先失效缓存、标记 stale、启动本页 force load，再广播带 `refreshStarted: true` 的 `app:booksource-reload`，让 Search/Explore 复用同一轮 store in-flight 或命中新鲜缓存。
- Search/Explore 收到 `refreshStarted` 后不再 `markSourcesStale()` + force load，只做必要的本视图缓存失效/同步并调用普通 `loadSources()` join 当前加载。
- `bookSourceStore.reloadSources()` 不再清空 `_loadInFlight`，显式 reload 也复用正在进行的列表扫描。
- single reload 的能力失效尽量使用 `sourceDir::fileName`，没有目录时保留旧 fileName 兜底。

已通过本地 gate：

- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `git diff --check`

Gate 报告：`reports/gates/2026-06-15-PERF-SOURCE-RELOAD-COALESCE/summary.md`。

## 2026-06-15 PERF-SEARCH-AGGREGATE-INCREMENTAL 状态更新

本轮状态：`local-gate-pass`；将追加到当前性能优化草稿 PR，远端 Quality Gate 状态以 GitHub Actions 为准。

任务 ID：`PERF-2026-06-15-SEARCH-AGGREGATE-INCREMENTAL`。本轮继续处理“大量书源搜索时，单源结果陆续返回导致页面持续卡顿”的前端公共链路；范围限定在搜索页聚合模式和聚合组件，不改变搜索命令、书源规则执行、结果结构、并发/超时配置或第三方书源样本。

Review 发现的问题：

- `SearchView.vue` 原先通过 `aggregatedTaggedResults` computed 在每次响应式更新时遍历当前全部 active sources 和全部结果，生成扁平 tagged 列表。
- `AggregatedSearchResults.vue` 收到扁平列表后，会重新做同书分组、相似度计算和排序；当 1000+ 书源逐个返回时，这部分会随结果流入不断重复扫描全量结果。
- 完成数、原始结果数、有结果书源数也通过多个 computed 反复遍历 active sources，会把“每个源返回一次”的更新放大成多次主线程全量计算。

关键修改：

- 新增 `src/utils/searchAggregation.ts`，复用原 Dice/bigram 相似度与作者兜底规则，并提供增量分组、排序和兼容的一次性聚合函数。
- `AggregatedSearchResults.vue` 新增可选 `groups` prop；搜索页可传入预聚合结果，旧的 `results` 调用仍走兼容聚合路径。
- `SearchView.vue` 改为每个源返回时增量合并到 `aggregatedGroupBuffer`，并用 `requestAnimationFrame` / 16ms timeout 合并发布 UI，避免每个源结果都触发全量扁平化和全量分组。
- 搜索完成数、原始结果总数、有结果书源数改为随源返回/停止搜索增量维护，减少 active source 列表上的重复 computed 扫描。
- 保留按当前限定书源判断 `hasSearched` 的交互语义：切换到未参与本轮搜索的单源时仍显示开始提示，而不是误报“暂无结果”。

已通过本地 gate：

- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `git diff --check`

Gate 报告：`reports/gates/2026-06-15-PERF-SEARCH-AGGREGATE-INCREMENTAL/summary.md`。

## 2026-06-15 PERF-SOURCE-STREAM-SORT-DEFER 状态更新

本轮状态：`local-gate-pass`；将追加到 `master`，远端 Quality Gate 和自动发布状态以 GitHub Actions 为准。

任务 ID：`PERF-2026-06-15-SOURCE-STREAM-SORT-DEFER`。本轮继续优化“大量书源加载/导入后依赖书源功能卡顿”的公共链路，范围覆盖 reader-core 的书源列表扫描和前端书源 store 的流式批次合并；不改变后端命令契约、书源规则执行语义、搜索结果结构、用户搜索并发/超时配置、签名材料或第三方/私有书源样本。

Review 发现的问题：

- 后端 `list_legado_sources()` / `stream_legado_sources()` 会先把 DB 中全部 Legado JSON 读出并完整反序列化为 `BookSource`，再生成列表元数据；大量书源时会把“逐步加载”前置成一次较重的全量解析。
- 前端 `src/stores/bookSource.ts` 的 `mergeSourcesBatch()` 每收到一个 `booksource:batch` 批次都会对完整 `sources.value` 排序，触发列表渲染、搜索页 `activeSources` watcher 和能力筛选反复重算。
- 前端能力缓存 `cap_*` 与用户搜索/发现禁用开关共用 `source.capabilities` 命名空间；后台逐源写能力缓存时，会触发禁用集合重载和相关搜索源过滤重算。
- 两处问题都不改变结果正确性，但会放大用户反馈的“加载书源后依赖书源的功能继续卡顿”。

关键修改：

- `BookSourceRepo` 新增基于 `(updated_at, book_source_url)` 的 keyset 游标分页读取，`BookSourceService` 暴露 `list_rows_page_after()`，reader-core 按页扫描 Legado 书源并在页间 `yield_now()`，降低单轮列表扫描峰值。
- 新增 `idx_book_sources_user_updated_url` 迁移，匹配列表分页排序条件，避免大库按页读取时退化成反复扫描。
- `BookSourceMeta::from_legado_row()` 使用轻量 seed 解析列表所需字段，并复用 `migrate_legacy_book_source_value()` / legacy 字段兜底保持旧书源字段兼容。
- 新增 `legado_list_meta_preserves_lightweight_fields` 回归测试，覆盖轻量列表元数据、能力推导、分组、类型、启用状态和版本字段。
- `mergeSourcesBatch()` 只负责合并新增/更新项和预热能力缓存，不再每批排序全量列表；流式 `done` 或旧命令 fallback 完成后统一排序一次。
- `source.capabilities` 仅保留 `cap_*` 能力缓存，用户搜索/发现禁用开关迁移到 `source.flags`；读取时保留旧命名空间兜底，避免已有用户设置丢失。

已通过本地 gate：

- `cmd /c pnpm.cmd lint`
- `cargo fmt --all`
- `cmd /c pnpm.cmd build`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cargo check -p legado-headless`
- `cargo test -p reader-core`
- `cargo test -p reader-core --test route_b_facade -- --nocapture`
- `node scripts/ci/check-command-contract.mjs --json`
- `git diff --check`

Gate 报告：`reports/gates/2026-06-15-PERF-SOURCE-STREAM-SORT-DEFER/summary.md`。

## 2026-06-14 PERF-LEGADO-SOURCE-OBJECT-CACHE 状态更新

本轮状态：`local-gate-pass`；将追加到 `master`，远端 Quality Gate 和自动发布状态以 GitHub Actions 为准。

任务 ID：`PERF-2026-06-14-LEGADO-SOURCE-OBJECT-CACHE`。本轮继续优化“大量书源导入/加载后，依赖书源的搜索、详情、目录、正文链路仍等待”的公共成本，范围限定在 reader-core 的 Legado 源对象缓存；不改变 Legado 规则执行语义、搜索结果结构、文件命名、DB upsert 语义、网络请求策略或第三方/私有书源样本。

关键修改：

- `ReaderCore` 新增 30 分钟 TTL 的 Legado 源对象缓存，缓存项保存已解析的 `BookSource`、文件 mtime、size 和加载时间。
- Legado 源写入路径在保存 `.legado.json` 后同步缓存已解析对象，大量导入后立即搜索/详情/目录/正文时可复用刚解析过的源对象，避免逐源重复读盘和 JSON 反序列化。
- `require_legado_source()` 路径会先用 `.legado.json` 当前 mtime/size 校验缓存，命中后直接返回 `BookSource`；文件被外部手动修改时自动重读并刷新缓存。
- 删除 Legado 源时同步移除对象缓存，避免同一运行期读到已删除源。
- 新增 `legado_source_cache_refreshes_after_external_file_change` 回归测试，覆盖“导入后命中缓存，再外部改 `.legado.json`，下一次搜索读取新规则”的正确性。

已通过本地 gate：

- `cargo test -p reader-core legado_source_cache_refreshes_after_external_file_change -- --nocapture`
- `cargo fmt --all -- --check`
- `cmd /c pnpm.cmd lint`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cargo test -p reader-core`
- `node scripts/ci/check-command-contract.mjs --json`
- `git diff --check`

Gate 报告：`reports/gates/2026-06-14-PERF-LEGADO-SOURCE-OBJECT-CACHE/summary.md`。

剩余风险：该轮减少的是 Legado 源对象重复读盘和反序列化开销；真实网络搜索仍受上游站点速度、规则复杂度、HTTP 并发/限速和设备性能影响。真实安卓设备大包导入/搜索压测仍未完成。

## 2026-06-14 PERF-JS-SOURCE-TEXT-CACHE 状态更新

本轮状态：`local-gate-pass`；将追加到 `master`，远端 Quality Gate 和自动发布状态以 GitHub Actions 为准。

任务 ID：`PERF-2026-06-14-JS-SOURCE-TEXT-CACHE`。本轮继续优化“大量书源加载后搜索仍卡顿/等待”的公共链路，范围限定在 reader-core 的 JS 书源文本读取缓存；不改变 JS 书源执行语义、搜索结果结构、书源规则、网络请求策略或第三方/私有书源样本。

关键修改：

- `ReaderCore` 新增 30 分钟 TTL 的源文本缓存，缓存项用文件路径、mtime 和 size 校验。
- JS 书源列表扫描与流式加载读取文件内容后会顺手写入缓存，后续搜索、详情、目录、正文和调试调用可复用刚读过的 JS 文本。
- `read_source()` 改为先校验缓存再读盘；外部手动修改文件时因 mtime/size 不匹配会自动重读。
- `save_js_source()`、`toggle_source()`、`delete_source()` 和外部书源目录变更会同步更新或清理缓存，避免读到旧书源。
- Legado JSON 写入只清理对应文本缓存，不保存导入过程中的整份 JSON，避免大包导入额外占用内存。
- 新增 `js_source_text_cache_refreshes_after_external_file_change` 回归测试，覆盖“列表扫描缓存后外部改文件，下一次搜索读到新内容”的正确性。

已通过本地 gate：

- `cargo test -p reader-core js_source_text_cache_refreshes_after_external_file_change -- --nocapture`
- `cargo fmt --all -- --check`
- `cmd /c pnpm.cmd lint`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `node scripts/ci/check-command-contract.mjs --json`
- `git diff --check`

Gate 报告：`reports/gates/2026-06-14-PERF-JS-SOURCE-TEXT-CACHE/summary.md`。

剩余风险：该轮减少的是 JS 书源文本重复磁盘读取；单源 JS 执行和网络请求本身仍受书源质量、上游接口速度、JS engine timeout、HTTP 并发/限速影响。真实安卓设备大包搜索/导入压测仍未完成。

## 2026-06-14 REL-ANDROID-CI-SYSROOT 状态更新

本轮状态：`remote-release-pass`；已追加到 `master`，Quality Gate 与 Master Release 均已通过。

任务 ID：`REL-2026-06-14-ANDROID-CI-SYSROOT`。本轮处理上一次 `Master Release` 远端运行在 Android 编译阶段失败的问题：Ubuntu runner 上 `rquickjs-sys` 的 bindgen/clang 没有使用 Android NDK sysroot，回退到宿主 `/usr/include` 后找不到目标平台头文件。

关键修改：

- `.github/workflows/master-release.yml` 的 Android 环境配置从已安装 NDK 推导 LLVM toolchain、sysroot 和 API level。
- 为 aarch64 Android build 设置 `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER`、`CC_aarch64_linux_android`、`CXX_aarch64_linux_android`、`AR_aarch64_linux_android`。
- 为 bindgen 设置 `LIBCLANG_PATH` 和 `BINDGEN_EXTRA_CLANG_ARGS`，显式传入 `--target`、`--sysroot` 和 Android include 目录。
- 增加 toolchain、apksigner、target include 目录存在性检查，让 CI 在环境缺失时更早失败。

已通过本地 gate：

- `cmd /c pnpm.cmd lint`
- `git diff --check`

远端观察：

- `Master Release` run `27493455571` 已确认失败在 Android `rquickjs-sys` bindgen 阶段；Windows build 已通过，publish 因 Android job 失败被跳过。
- 修复提交 `07a4f7d` 推送后，Quality Gate run `27493894613` 成功。
- 后续 `Master Release` run `27494084851` 成功；Android build、V1/V2/V3 签名、签名校验、Windows build 和 GitHub Release 发布均通过。
- Release 已创建：`master-v0.9.0-07a4f7d`。

剩余风险：本轮已由远端 Ubuntu Android build 验证；后续若调整 Android NDK/API level 或正式版发布策略，需要重新观察发布链路。

## 2026-06-14 REL-MASTER-SIGNED-RELEASE 状态更新

本轮状态：`local-gate-pass`；将追加到 `master`，GitHub Actions 自动发布状态以远端 run 为准。

任务 ID：`REL-2026-06-14-MASTER-SIGNED-RELEASE`。本轮按用户要求新增 `master` 分支 Quality Gate 通过后的自动编译发布链路，并生成新的 Android release keystore，对 APK 执行 V1+V2+V3 签名。范围限定在 GitHub Actions 发布编排、本地/Secrets 签名材料配置和 APK 签名验证；不提交 keystore、密码、签名配置实值或 APK 构建产物。

关键修改：

- 新增 `.github/workflows/master-release.yml`，监听 `Quality Gate` 的 successful `workflow_run`，仅 `master` push 触发 prerelease 发布。
- Windows job 构建 x64 executable；Android job 构建 unsigned APK 后用 GitHub Secrets 解码 keystore 并执行 `apksigner` 签名。
- Android 签名显式启用 V1/V2/V3，并使用 `--min-sdk-version 21` 强制生成 V1/JAR 签名。
- Android 发布前校验 `Verified using v1/v2/v3 ... true`。
- `.gitignore` 新增 `.release-secrets/`，本地密钥元数据保持 ignored。
- 已生成新的本地 PKCS12 release keystore，并把对应签名材料写入 GitHub repository secrets。

已通过 gate：

- `cmd /c pnpm.cmd tauri android build --apk --target aarch64`
- `apksigner sign --min-sdk-version 21 --v1-signing-enabled true --v2-signing-enabled true --v3-signing-enabled true`
- `apksigner verify --verbose --print-certs --min-sdk-version 21`（V1/V2/V3 均为 true）
- `cmd /c pnpm.cmd lint`
- `git diff --check`

Gate 报告：`reports/gates/2026-06-14-REL-MASTER-SIGNED-RELEASE/summary.md`。

剩余风险：新增 workflow 仍需推送后由 GitHub Actions 真实解析和运行；本地无 actionlint/YAML parser。当前发布为 prerelease，tag 形如 `master-v{version}-{shortSha}`；正式版发布策略可后续独立设计。Windows 当前发布 executable，安装包需后续恢复 bundler。

## 2026-06-14 PERF-IMPORT-BATCH-UPSERT 状态更新

本轮状态：`local-gate-pass`；将追加到 `codex/perf-booksource-lazy-list` 分支，PR 状态以 GitHub 为准。

任务 ID：`PERF-2026-06-14-IMPORT-BATCH-UPSERT`。本轮继续优化“大量书源导入慢”的后端写入链路，范围限定在 Legado JSON 批量导入中的 SQLite upsert 批处理；不改变书源文件命名、JSON 内容、单源保存语义或第三方/私有书源样本。

关键修改：

- `BookSourceRepo` 新增事务内 `upsert_many()`，单条 `upsert()` 与批量写入共用同一 SQL。
- `BookSourceService::save_many()` 从循环单条保存改成一次序列化后批量提交。
- `ReaderCore::import_legacy_json_text_with_progress()` 在进度批次边界批量 flush DB，避免每个源一次 SQLite upsert。
- `copy_to()` 也改为事务内复制，减少默认书源复制时的写入开销。
- 扩展 reader-core 导入进度测试，导入后验证 30 个源都可通过 `list_sources()` 读取。

已通过 gate：

- `cargo test -p reader-core import_legacy_json_text_reports_progress_batches -- --nocapture`
- `cmd /c pnpm.cmd lint`
- `cargo fmt --all -- --check`
- `node scripts/ci/check-command-contract.mjs --json`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `git diff --check`

Gate 报告：`reports/gates/2026-06-14-PERF-IMPORT-BATCH-UPSERT/summary.md`。

剩余风险：本轮减少的是 DB upsert 事务和连接调度开销；Legado JSON 文件仍逐源写入以保持现有文件布局。导入进度批次当前仍是 25 项，后续可根据真实压测把 DB batch size 与 UI progress interval 分离。真实安卓设备大包导入压测仍未完成。

## 2026-06-14 PERF-IMPORT-PROGRESS-EVENTS 状态更新

本轮状态：`local-gate-pass`；将追加到 `codex/perf-booksource-lazy-list` 分支，PR 状态以 GitHub 为准。

任务 ID：`PERF-2026-06-14-IMPORT-PROGRESS-EVENTS`。本轮继续处理“大量书源导入长时间等待”的体验和调度问题，范围限定在开源阅读 Legado JSON 导入进度事件、批次让步和前端进度展示；不改变导入格式、文件命名、DB upsert 语义或第三方/私有书源样本。

关键修改：

- `ReaderCore::import_legacy_json_text_with_progress()` 新增批次进度回调，默认导入方法仍复用同一路径。
- 大 JSON 每处理 25 项或完成时上报进度并 `yield_now()`，避免长导入期间完全不可观测。
- Tauri IPC `booksource_import_legacy_json_text` 支持可选 `requestId`，通过 `booksource:import-progress` 事件向前端发送进度；Route B/Headless 仍走无进度兼容路径。
- `InstalledSourcesTab.vue` 在开源阅读书源 URL/文件导入时显示进度条和已处理/已导入/跳过/错误计数。
- 新增 reader-core 回归测试 `import_legacy_json_text_reports_progress_batches`。

已通过 gate：

- `cargo test -p reader-core import_legacy_json_text_reports_progress_batches -- --nocapture`
- `cmd /c pnpm.cmd lint`
- `cargo fmt --all -- --check`
- `node scripts/ci/check-command-contract.mjs --json`
- `cmd /c pnpm.cmd build`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `git diff --check`

Gate 报告：`reports/gates/2026-06-14-PERF-IMPORT-PROGRESS-EVENTS/summary.md`。

剩余风险：本轮让导入过程可见并在批次间让步，但仍不是 DB 批量事务写入；超大订阅包进一步提速可继续做 reader-core 批量 upsert/事务化和前端总包级进度聚合。真实安卓设备导入压测仍未完成。

## 2026-06-14 PERF-BACKGROUND-SOURCE-MAINTENANCE 状态更新

本轮状态：`local-gate-pass`；将追加到 `codex/perf-booksource-lazy-list` 分支，PR 状态以 GitHub 为准。

任务 ID：`PERF-2026-06-14-BACKGROUND-SOURCE-MAINTENANCE`。本轮继续处理“大量书源加载完成后仍卡顿”的后续链路，范围限定在前端书源 store 的自动能力检测和在线更新检查调度；不改变书源规则执行语义，不触碰第三方/私有书源样本，不写喵/猫公子等书源名特判。

关键修改：

- `loadSources()` 完成后不再同时触发能力检测和更新检查，改为延迟 250ms 启动可失效的后台维护流程。
- 后台维护先加载持久化能力缓存，再分批执行缺失能力检测；能力检测和更新检查每批之间让出 UI 线程，避免大列表加载后出现第二段尖峰。
- `checkUpdatesIfStale()` 增加 in-flight 去重；修正“上次检查 1 小时内但没有 pending 更新时仍重复扫全部 updateUrl”的问题。
- 更新检查/应用更新向后端透传 `sourceDir`，并在书源管理事件里保留目录上下文，降低多目录/同名书源下的歧义。

已通过 gate：

- `cmd /c pnpm.cmd lint`
- `cargo fmt --all -- --check`
- `cmd /c pnpm.cmd build`
- `node scripts/ci/check-command-contract.mjs --json`
- `git diff --check`
- `cargo check -p legado-tauri`
- `cargo check -p reader-core`

剩余风险：本轮降低的是加载完成后的后台维护尖峰，未做真实安卓设备大列表压测；JS 搜索 blocking worker 的不可抢占限制仍沿用上一轮说明，后续还需要继续做 JS 运行时可中断和搜索进度事件。

## 2026-06-14 PERF-SEARCH-CANCEL-TASKS 状态更新

本轮状态：`local-gate-pass`；将追加到 `codex/perf-booksource-lazy-list` 分支，PR 状态以 GitHub 为准。

任务 ID：`PERF-2026-06-14-SEARCH-CANCEL-TASKS`。本轮继续处理“大量书源搜索卡顿/等待”的搜索本体链路，范围限定在单源搜索命令取消与前端搜索队列取消；不改变搜索结果语义、书源规则执行逻辑、用户搜索并发和超时配置。

关键修改：

- `booksource_search` 新增可选 `taskId`，Tauri IPC 与 Route B WS router 共用同一命令实现并接入 `TaskRegistry`。
- Tauri 搜索命令会在 `taskId` 被 `booksource_cancel` 取消后提前返回 `CANCELLED`，避免用户停止搜索后继续等待已发出的单源搜索命令返回。
- `SearchView.vue` 为每个活跃单源搜索生成任务 ID；停止搜索时批量调用现有 `cancelTask()`，同时继续用 token 阻止旧结果回写 UI。
- `src-headless` 兼容接收 `taskId` 参数，但不引入独立任务注册表。
- Route B spec 更新 `booksource_search(fileName, keyword, page, taskId?, sourceDir?)`。

已通过 gate：

- `cargo fmt --all -- --check`
- `cmd /c pnpm.cmd lint`
- `cargo test -p legado-tauri booksource_search_accepts_task_id_in_ws_router -- --nocapture`
- `node scripts/ci/check-command-contract.mjs --json`
- `cmd /c pnpm.cmd build`
- `cargo check -p legado-headless`
- `cargo check -p reader-core`
- `git diff --check`

剩余风险：Legado/async 网络搜索在 future 被取消后可尽早释放等待；JS 搜索当前仍运行在 `spawn_blocking`，取消会让命令/UI 提前返回但不能抢占已经进入 blocking 线程的 JS 执行。下一轮如继续收口，应把 JS 搜索执行也纳入可中断/超时更细的任务模型。

## 2026-06-14 PERF-IMPORT-CACHE-INVALIDATION 状态更新

本轮状态：`local-gate-pass`；将追加到 `codex/perf-booksource-lazy-list` 分支，PR 状态以 GitHub 为准。

任务 ID：`PERF-2026-06-14-IMPORT-CACHE-INVALIDATION`。本轮继续处理“大量书源导入后卡顿”的公共链路，范围限定在 reader-core 批量 Legado JSON/article JSON 导入过程中的列表缓存失效策略；不改变导入格式、文件命名、DB 保存语义或第三方书源规则。

关键修改：

- `import_legacy_json_text()` 在解析出批量输入后先失效一次书源列表缓存。
- Legado 批量写入改走不重复失效缓存的内部保存 helper，避免每导入一个源都抢一次列表缓存锁。
- article JSON 导入同样不再逐项失效缓存。
- 单源保存、启停、删除等非批量路径仍由原 `persist_legado_source()` / 写入路径负责失效缓存。

已通过 gate：

- `cargo fmt --all -- --check`
- `cargo test -p reader-core stream_sources_emits_incremental_batches_with_capabilities -- --nocapture`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`

剩余风险：本轮减少的是批量导入中的重复缓存失效开销；DB 仍是逐源 upsert、文件仍是逐源 pretty JSON 写入。若超大订阅包导入仍慢，下一步应评估 reader-core 的批量事务/批量写入接口和前端导入进度事件。

## 2026-06-14 PERF-SEARCH-STREAM-QUEUE 状态更新

本轮状态：`local-gate-pass`；已追加到 `codex/perf-booksource-lazy-list` 分支，PR 状态以 GitHub 为准。

任务 ID：`PERF-2026-06-14-SEARCH-STREAM-QUEUE`。本轮承接大量书源加载优化，处理搜索页在书源列表流式加载期间仍容易等待完整列表的问题。范围限定在前端搜索页调度；不改变后端 `booksource_search` 语义，不改变用户配置的搜索并发和超时，不触碰第三方/私有书源样本。

关键修改：

- `SearchView.vue` 将固定快照搜索改为动态队列：点击搜索后先搜索当前已到达的可搜索书源，后续流式批次进入 `activeSources` 时继续入队。
- 搜索队列继续遵循 `prefsStore.search.searchConcurrency` 并发上限；停止搜索通过 token 失效当前 run，旧请求返回后不再写入 UI。
- 当用户限定单一书源时，搜完该书源即可结束；只有“全部书源”搜索会在书源列表仍在加载时继续等待后续批次。
- 搜索页 loading 状态会覆盖“正在等待更多书源”的阶段，避免首批搜索完成但列表仍在加载时误显示搜索已结束。

已通过 gate：

- `cmd /c node_modules\.bin\oxfmt.cmd src\views\SearchView.vue`
- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `node scripts/ci/check-command-contract.mjs --json`
- `git diff --check`

剩余风险：本轮仍没有实现后端任务级取消或搜索进度事件；停止搜索只阻止旧结果回写 UI，已进入后端/网络的单源请求仍按原超时结束。下一轮应继续把搜索任务进度、取消和按源 timeout/失败聚合下沉到后端。

## 2026-06-14 PERF-BOOKSOURCE-LAZY-LIST 状态更新

本轮状态：`local-gate-pass`；提交、推送与远程 CI 状态以 git history 和 GitHub Actions 为准。

任务 ID：`PERF-2026-06-14-BOOKSOURCE-LAZY-LIST`。本轮按用户反馈优化大量书源（如喵/猫公子订阅导入后的大列表）在加载、搜索入口筛选、发现入口筛选等依赖书源列表功能上的长时间等待。范围限定在通用书源列表扫描、流式推送、前端逐批合并与能力元数据预热；不触碰第三方/私有书源样本，不写任何书源名特判，不改变搜索、详情、目录、正文规则语义。

关键修改：

- `ReaderCore::list_sources()` 增加 30 分钟内存缓存；新增 `stream_sources(batch_size, force, emit)`，从 Legado DB、JS 书源目录、article JSON 目录扫描阶段直接分批 emit，不再先全量扫描完再推送。
- `BookSourceMeta` 新增 `capabilities`，由 Legado rule 字段或 JS 顶层函数轻量扫描得出；前端用它预热 `fnsCache`，避免列表进入搜索/发现筛选时再逐源做能力检测。
- Tauri IPC、Route B WebSocket router、headless WS dispatcher 均接入同一 `ReaderCore::stream_sources`；`booksource_list_streaming` 新增可选 `force` 参数，`booksource:batch` 保持统一事件契约。
- `src/stores/bookSource.ts` 改为收到每个批次就合并、排序和渲染，最后 `done` 时只裁剪本轮未见的旧项；同时修复过期流监听器误清理新请求的竞态。
- 所有书源新增/删除/启停、外部目录变更、Legado 导入等会使列表变化的写入路径均失效缓存。
- `.cargo/config.toml` 仅因 `pnpm lint` 的 `oxfmt --check .` 基线要求做了格式化，无业务语义变化。

已通过 gate：

- `cargo fmt --all -- --check`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cargo check -p legado-headless`
- `cargo test -p reader-core stream_sources_emits_incremental_batches_with_capabilities -- --nocapture`
- `cargo test -p legado-tauri booksource_list_streaming_is_routed -- --nocapture`
- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `cargo test -p reader-core`
- `cargo test -p legado-tauri`
- `cargo test -p legado-headless`
- `node scripts/ci/check-command-contract.mjs --json`
- `git diff --check`

命令契约基线保持不变：`frontendTotal=162`、`registeredTotal=161`、`bothCount=161`、`onlyFrontend=["js_eval"]`、`onlyBackend=[]`、`frontend_unsupported_stub_count=39`、`frontend_implemented_count=122`。

剩余风险：本轮解决的是书源列表扫描和列表依赖筛选的首批可见与重复扫描问题；多源搜索本体仍会受启用书源数量、单源网络质量和规则执行耗时影响，后续应继续做搜索任务并发上限、进度事件、取消和按源超时的性能收口。Android 真机下的大批量导入/搜索体验仍需单独设备验证。

## 2026-06-13 PERF-EXTENSION-EXAMPLES-LAZY 状态更新

本轮状态：`local-gate-pass`；提交、推送与远程 CI 状态以 git history 和 GitHub Actions 为准。
本轮继续收口前端按需加载体积，范围限定在扩展管理页的内置示例库加载边界；不改变扩展安装、保存、启停命令，不改变插件运行时 API、后端命令契约或第三方私有书源样本。

关键修改：

- `src/data/extensionExamples.ts` 移除 23 个 `pluginExamples/*.js?raw` 静态导入，改为 `loadExampleScripts()` 缓存式动态导入。
- `src/views/ExtensionsView.vue` 仅在切到“示例库”页签时加载示例脚本；筛选、分类、预览安装都改为基于已加载的 `exampleScripts` 状态。
- 示例库加载期间使用 `n-spin` 表示等待，加载失败通过现有 `message.error` 报错，避免空状态误判为无示例。

构建观察：

- 上轮基线 `ExtensionsView-BgBpIuYE.js` 为 `137.93 kB`，gzip `34.64 kB`。
- 本轮 `ExtensionsView-BDO7O7EC.js` 降为 `33.35 kB`，gzip `10.42 kB`。
- 23 个示例脚本被拆为独立动态 chunk，例如 `reader-ad-cleaner-CKCGe1gc.js`、`tts-edge-read-aloud-CML0VQWv.js`。
- `dist/index.html` 首屏 `modulepreload` 仍为 4 条，未包含示例脚本 chunk。
- 上轮拆出的 `useFrontendPlugins` 与 `pluginChineseConverter` chunk 体积保持稳定。

已通过 gate：

- `cmd /c node_modules\.bin\oxfmt.cmd src\data\extensionExamples.ts src\views\ExtensionsView.vue src\components\extensions\ExampleCard.vue`
- `rg -n "EXAMPLE_SCRIPTS" src`
- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `node scripts\ci\check-command-contract.mjs --json`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo check -p reader-core`
- `cargo test -p reader-core`
- `cargo check -p legado-tauri`

剩余风险：示例库首次打开时会一次性并发加载 23 个较小 raw chunk，降低了首屏和扩展页入口体积，但没有减少示例总源码体积；后续如果更关注请求数，可再评估合并为单个按需示例包。

## 2026-06-13 PERF-FRONTEND-PLUGIN-RUNTIME-SPLIT 状态更新

本轮状态：`gate-pass`；提交与推送状态以 git history 和远程状态为准。

本轮继续收口前端插件运行时体积，目标是不改变插件 API 语义、不改变后端命令契约，只把 `useFrontendPlugins` 静态图里按能力才需要的重依赖移到按需块。

关键修改：

- `opencc-js` 从 `pluginTextUtils.ts` 拆到新的 `pluginChineseConverter.ts`，避免简繁转换词典随通用插件运行时一起进入 `useFrontendPlugins` chunk。
- `useFrontendPlugins.ts` 新增缓存动态导入与插件源码预判：插件源码包含 `convertChinese` 时，在插件求值前预加载转换器，从而保留 `api.text.convertChinese(text, mode) => string` 的同步 API。
- `builtinPlugins.ts` 将内置 MiMo TTS raw 源码改为 `loadBuiltinFrontendPlugins()` 动态加载，避免内置插件源码静态嵌入插件运行时 chunk。

构建观测：

- 上轮基线 `useFrontendPlugins-BtFZi8GG.js` 为 `1.17 MB`，gzip `509.87 kB`。
- 本轮 `useFrontendPlugins-BsTr7j8q.js` 降为 `36.12 kB`，gzip `10.36 kB`。
- `pluginChineseConverter-BFpjUyv2.js` 独立为按需 chunk：`1,122.13 kB`，gzip `494.29 kB`。
- `tts-xiaomi-mimo-v25-2bfbZ9OA.js` 独立为按需 chunk：`13.90 kB`，gzip `3.96 kB`。
- `dist/index.html` 首屏 `modulepreload` 仍为 4 条，未包含 `pluginChineseConverter`、`tts-xiaomi` 或 `useFrontendPlugins`。
- 入口 `index-BsSSkfpI.js` 保持 `57.05 kB`，gzip `20.14 kB`。

已通过 gate：

- `cmd /c node_modules\.bin\oxfmt.cmd src\data\builtinPlugins.ts src\composables\useFrontendPlugins.ts src\features\frontendPlugins\pluginChineseConverter.ts src\features\frontendPlugins\pluginTextUtils.ts`
- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `node scripts\ci\check-command-contract.mjs --json`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo check -p reader-core`
- `cargo test -p reader-core`
- `cargo check -p legado-tauri`

剩余风险：OpenCC 词典本身仍然很大，本轮只是把它移到能力触发的动态块；非常规插件若通过拼接字符串调用 `api.text["convert" + "Chinese"](...)`，第一次同步调用会走兜底返回原文并触发后台加载。后续可继续处理 `vendor-vue-naive` 首屏 preload 或 `ExtensionsView` 示例库体积。

## 2026-06-13 PERF-LAZY-FRONTEND-CHUNKS 状态更新

本轮状态：`gate-pass`；提交与推送状态以 git history 和远程状态为准。

本轮收口前端首屏性能与大 chunk 拆分：

- `BookSourceView.vue` 的已安装、在线、调试、测试、AI 写书源子页改为异步组件；AI 写书源标签页使用 `show:lazy`，未访问时不挂载 AI 工作台。
- `SectionSync.vue` / `useSync.ts` 将 `qrcode` 与 `@zxing/browser` 改为二维码生成、扫码动作触发时再加载。
- `useVConsole.ts` 将 `vconsole` 改为开发者开关启用时动态加载，并处理开关关闭时 import 仍在途的竞态。

构建观测：

- `vConsole` 懒加载前本轮中间构建入口 `index` chunk 为 `370.53 kB`，gzip `103.94 kB`。
- 最终构建入口 `index-BjH9Vjka.js` 为 `67.96 kB`，gzip `23.36 kB`。
- `vconsole.min-D3qedUWG.js` 独立为按需 chunk，`281.46 kB`，gzip `78.04 kB`。
- `AiSourceTab-CfbQgSAY.js` 保持独立异步 chunk，`463.36 kB`，gzip `118.58 kB`。

已通过 gate：

- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `git diff --check`
- `node scripts\ci\check-command-contract.mjs --json`
- `cargo fmt --all -- --check`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cargo test -p reader-core`

剩余风险：`vendor-vue-naive` 与 `_plugin-vue_export-helper` 仍超过 500 kB；`vconsole` 包内部 direct eval warning 仍会在构建扫描动态 chunk 时出现；`useTransport` ineffective dynamic import 仍需单独审计。

下一轮优先候选：继续前端性能收口，优先审计 Naive UI 全量注册与 `_plugin-vue_export-helper` 大 chunk；如范围过大，先登记拆分计划，再处理 `useTransport` ineffective dynamic import。

## 2026-06-13 SOURCE-WIKISOURCE-CLASSICS 状态更新

本轮状态：`gate-pass`，待提交并推送；提交推送状态以 git history 为准。

本轮新增一个可导入的 JS 书源 fixture：`crates/reader-core/tests/fixtures/book_sources/wikisource_classics.js`。该源面向中文维基文库公开页面，当前收录公有领域《三國演義》，用于补充一个不依赖第三方聚合站、不会触碰付费/登录/试看绕过的真实书源样本。

关键结果：

- `search("三国演义")` 返回《三國演義》，作者 `羅貫中`。
- `bookInfo` 返回 Wikisource 目录 URL 与 `完本` 状态。
- `chapterList` 实网解析 120 章目录。
- `chapterContent` 实网读取首章与最终章正文，2026-06-13 实测 `first_len=14153`、`latest_len=19775`。
- Wikimedia 缺少 `User-Agent` 时会返回 robot policy 提示；书源已统一通过 `fetchWiki()` 带可识别 `User-Agent` 与 `Accept` 头，并保留 `@minDelayMs 800`。

已通过专项实网验证：

```powershell
cargo test -p reader-core wikisource_classics_public_domain_full_chain -- --ignored --nocapture
```

边界声明：本轮不处理付费、登录、试看、验证码、设备绑定、加密绕过或访问控制规避；此源只抓取公开可访问的公有领域文本。

## 2026-06-13 CI-CARGO-FETCH-RETRY 状态更新

本轮状态：`closed-local`，待提交推送。

本轮处理用户报告的 GitHub Actions 失败：2026-06-13 01:00 左右，`cargo check -p reader-core` 在 crates.io 下载 `cipher` 依赖时连接 reset。该日志属于 registry 下载链路抖动，不是 reader-core 编译错误。

已修改 `quality-gate.yml`：

- 全局增加 `CARGO_NET_RETRY=10`、`CARGO_HTTP_TIMEOUT=120`、`CARGO_HTTP_MULTIPLEXING=false`、`CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`。
- 增加 `actions/cache@v4` 缓存 `~/.cargo/registry` 和 `~/.cargo/git`，降低重复下载 crates.io 依赖的概率。
- 在 Rust check/test 前增加 `cargo fetch --locked` 三次重试，失败间隔 20s/40s，把依赖下载问题提前隔离到 Fetch Cargo dependencies 步骤。

已通过 gate：

- `cmd /c node_modules\.bin\oxfmt.cmd --check .`
- `git diff --check`
- `node scripts/ci/check-command-contract.mjs --json`
- `cargo fetch --locked`
- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `cargo fmt --all -- --check`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cargo test -p reader-core`

推送后应观察 GitHub Actions 新一轮 `Quality Gate`，确认 `Fetch Cargo dependencies` 步骤生效。

## 2026-06-13 UI-SOURCE-AI-COMMENT-LAYOUT 状态更新

本轮状态：`closed-pushed`，提交 `ab514a1` 已推送到 `origin/master`。

实测命令契约基线：

```text
command_contract.frontendTotal = 162
command_contract.registeredTotal = 161
command_contract.bothCount = 161
command_contract.onlyFrontend = js_eval
command_contract.onlyBackend = none
command_contract.frontend_unsupported_stub_count = 39
command_contract.frontend_implemented_count = 122
```

本轮收口：

- `InstalledSourcesTab.vue`、`AiSourceTab.vue`、`AiTestPanel.vue`、`ReaderParagraphCommentsDrawer.vue` 已完成响应式布局加固，重点覆盖书源管理标题横排、搜索/统计/批量管理条、AI 写书源三栏与输入区、段评抽屉长文本。
- `src-headless/src/main.rs` 已补齐 `booksource_get_dir`、`booksource_get_dirs`、`booksource_list_streaming`，使 headless 预览可通过真实 WS 渲染书源卡片并验证流式列表事件。
- Chrome headless 实测 1000x800、768x800、390x800 三档均无横向溢出；书源管理标题 `writing-mode=horizontal-tb`，3 条测试书源卡片全部渲染；段评抽屉合成长文本检查 `tooWide=0`。

已通过 gate：

- `cmd /c node_modules\.bin\oxfmt.cmd --check .`
- `git diff --check`
- `node scripts/ci/check-command-contract.mjs --json`
- `cmd /c pnpm.cmd lint`
- `cmd /c pnpm.cmd build`
- `cargo check -p reader-core`
- `cargo check -p legado-tauri`
- `cargo check -p legado-headless`
- `cargo test -p reader-core`

后续第一任务：`CI-2026-06-13-CARGO-FETCH-RETRY`。用户报告 GitHub Actions 在 2026-06-13 01:00 左右下载 crates.io 依赖 `cipher` 时连接 reset；该日志指向网络瞬断/registry 下载不稳定，不是本地代码编译失败。下一轮应加固 quality-gate 的 Cargo 缓存与 fetch/check/test 重试。

本文件记录当前 R 队列状态。事实数字只以当轮命令输出为准，不沿用历史表格。

最后实测：2026-06-12（AI-DEEPSEEK-MGZ 轮；stub 40→39）。下方基线仍以当轮 `check-command-contract.mjs --json` 输出为准。

实测命令：

```powershell
git status --short
node scripts/ci/check-command-contract.mjs --json
cargo fmt --all -- --check
cmd /c pnpm lint
cmd /c pnpm build
cargo check -p reader-core
cargo test -p reader-core
cargo check -p legado-tauri
cargo test -p legado-tauri
cargo test -p reader-core shuqi_source_full_chain -- --ignored --nocapture
cargo test -p reader-core qimao_source_full_chain -- --ignored --nocapture
cargo test -p reader-core fanqie_source_full_chain -- --ignored --nocapture
cmd /c pnpm build:windows:release
cmd /c pnpm build:android:release
node scripts/ci/check-command-contract.mjs --json
```

## 当前基线

```text
project.status = incomplete
command_contract.frontendTotal = 162
command_contract.registeredTotal = 161
command_contract.bothCount = 161
command_contract.onlyFrontend = js_eval
command_contract.onlyBackend = none
command_contract.frontend_unsupported_stub_count = 39
command_contract.frontend_implemented_count = 122
command_contract.classificationScope = frontend-facing registered commands
frontend.lint = passed_zero_warnings
form_b_headless_loopback = passed
prefetch_progress = passed
source.live_verification = 书旗/七猫/番茄 strict_pass
windows.release = passed
android.release_unsigned = passed
windows.smoke = process_window_ws_pass; computer_use_blocked
cleanup = target_dist_android_build_removed; artifacts_preserved
paragraph_comment = fixed 2026-06-12 PARA-COMMENT-VERIFY; js capabilities detect chapterParagraphCommentCounts/chapterParagraphComments/likeParagraphComment/replyParagraphComment; external sourceDir propagated for counts/details/like/reply; legacy Legado showCmt entry has offline facade regression
ai_booksource = fixed 2026-06-12 AI-DEEPSEEK-MGZ; DeepSeek provider presets added; backend ai_http_proxy_request implemented with POST/domain/path whitelist; yuedu://rsssource subscription import resolves MiaoGongZi pages into booksource package URLs
```

口径变更（2026-06-11）：stub 数由 60 降至 58，差额来自上一轮 `UI-REMOVE-APP-UPDATE` 删除 `app_update_*`（2 个）。总纲/审计旧文档写「60」已过期。
口径变更（2026-06-12）：CAP-REPO 后 stub 58→52；CAP-SYNC WebDAV 后 WebDAV 12 命令转 implemented，stub 52→40；百度网盘 provider 4 命令按用户决策继续保留隐藏。
FORMB 口径（2026-06-12）：`src-headless` 不参与 Tauri command-contract 计数，本轮补齐的是独立 headless WS 分发契约；因此 stub 数维持 40，不应误判为无进展。
口径变更（2026-06-12）：AI-DEEPSEEK-MGZ 后 `ai_http_proxy_url` 旧 stub 移除，`ai_http_proxy_request` 转 implemented，stub 40→39。当前以本轮实测 39 为准。

## NET 任务（网络设置配置接入，审计第二类）

| ID        | 状态         | 证据 / 说明                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| --------- | ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- |
| NET-001   | closed       | 主 reqwest 客户端接入 `http_user_agent`/`http_follow_redirects`/`http_connect_timeout_secs`/`http_ignore_tls_errors`/`proxy_*`；`HttpClientConfig`+`from_config`，启动时构建；reqwest 加 `socks` feature；测试 `tests/http_client_config.rs` 7/7。见 `reports/gates/2026-06-11-NET-001-002-network-config/summary.md`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| NET-002   | closed       | `request_min_delay_ms` 接入 JS 桥（`AtomicU64`+`set_js_http_min_delay_ms`），启动下发 + `app_config_set` 实时更新                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| NET-003   | closed       | `engine_timeout_secs` 已接入：`parser/js.rs` 加 `JS_ENGINE_TIMEOUT_SECS` 原子 + thread-local deadline + `JsEvalDeadlineGuard`，`acquire_runtime` 装 `set_interrupt_handler`，`eval_js_inner_with_source` 单一 eval 收口处设 deadline。启动 + `app_config_set` 实时下发。测试 `tests/js_engine_timeout.rs`（死循环 1s 被中断、正常 eval 仍通过）。基准：js_compat 1.23s 与改前持平，handler 仅 thread-local 读取无热路径回归                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |     |
| NET-004   | closed       | `http_doh_server` 已接入：新模块 `crawler/doh.rs` 实现 `reqwest::dns::Resolve`（JSON DoH API，**无新依赖**），bootstrap 客户端用 `.resolve(host, 已知IP)` 钉死避免递归，**fail-open** 到系统 DNS（任何 DoH 失败不破坏解析）。主客户端经 `HttpClientConfig.doh_server` + `builder().dns_resolver(...)` 接入。**NET-004-LIVE（2026-06-12 实网验证）**：逐 provider 实测，修正 2 处缺陷——360dns 的 JSON 端点是 `/resolve`（旧填 `/dns-query` 仅收 RFC8484 wire-format，会静默 fail-open 假装走 DoH）；onedns 无公开 JSON DoH 端点（`doh.onedns.net/dns-query` 返回 HTTP 000，需账号），已从后端 provider 与前端 `DOH_OPTIONS` 移除。现存 5 provider（alidns/dnspod/360dns/cloudflare/google）live 均返回真实 Answer，新增 `#[ignore]` live 测试 `doh_live_each_provider_returns_real_answer`（`cargo test -p reader-core doh_live -- --ignored` 实测 5/5 通过，直接调用 `doh_query` 确保非 fail-open）。JS 桥（blocking 客户端）的 DoH 已由 NET-005 接入 |     |
| CLEAN-001 | closed       | 已移除死键 `ui_enable_aplus_tracking`（审计第四类）：`useAppConfig.ts`/`appConfig.ts` 类型+默认+computed+return、`facade.rs:default_app_config` 默认全部删除；lint 0/0、build PASS、契约不变                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| CLEAN-002 | 复核：非缺陷 | 审计第三类。复核：「解除限制」点击进 `FullModeUnlockDialog`，unlock capability 不支持时弹窗内报错（即审计第一类「弹窗内展示错误」），非静默 UNSUPPORTED。预先置灰为可选装饰性 polish，留待 unlock 能力真实化时处理                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| CLEAN-003 | 复核：非缺陷 | 审计第三类。复核：`SectionSync.vue` provider `n-select` 已 `:disabled="syncDisabled"`，sync 不支持时整段禁用，FTP/百度网盘不可实际选中。留待 sync 能力真实化时处理 provider 级 stub                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

⚠️ NET-001 行为变化：`http_ignore_tls_errors` 默认 `true`，接入后默认接受无效 TLS 证书（旧行为为始终校验）。属用户决策项，详见 gate 报告。

## 后续维护任务路线图（接手 AI 按优先级取用）

前后端接入审计（四类）已结清：第二类 8 键全部接入（NET-001~004），第四类死键已删（CLEAN-001），第三类经复核为既有 capability 门禁覆盖的非缺陷，第一类复核准确（58 stub）。以下是审计之外、面向第 14 节最终可用标准的剩余工作，按优先级登记，供后续 AI 维护更新。**实现任何已隐藏后端能力时，必须同步更新 `capabilities_get`、前端入口状态与 `docs/command-matrix.md`（审计文档第 4 节处置规则）。**

### A. 环境/网络阻塞项（有网或真机时优先验证）

| ID                  | 任务                                                                                                                                                                                                                                                                                                                                                                                                            | 验收                                                                  |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| ~~NET-004-LIVE~~    | **closed 2026-06-12**：实网逐 provider 验证完成。修正 360dns 路径（`/dns-query`→`/resolve`）、移除无 JSON 端点的 onedns；现存 5 provider live 5/5 通过。`#[ignore]` live 测试已入库（`cargo test -p reader-core doh_live -- --ignored`）。详见 NET-004 行                                                                                                                                                       | 已达成                                                                |
| ~~NET-005~~         | **closed 2026-06-12**：DoH 已接入 JS 桥 blocking 客户端。`parser/js.rs` 加 `JS_HTTP_DOH_SERVER: Mutex<String>` + `set_js_http_doh_server`，`JS_HTTP_CLIENT` Lazy 构建时 `builder.dns_resolver(...)`；`facade.rs` 启动用 `http_cfg.doh_server` 下发。异步 Resolver 跑在 blocking 客户端内部 runtime 的风险已 live 验证（`doh_live_blocking_client_resolves_and_fetches` 经 Cloudflare DoH 真实抓取 example.com） | 已达成（JS 桥 java.ajax 按 `http_doh_server` 走 DoH，fail-open 一致） |
| ~~SRC-FANQIE-LIVE~~ | **closed 2026-06-12**：番茄 search→bookInfo→toc(1928)→content(3135) 全链路 live_network_pass。bookInfo 字段完整性已验收（name/author/intro/kind/wordCount/coverUrl/tocUrl 均填充真实数据并加断言）。验收中发现并修复引擎两处通用字段管线缺陷（`@js:` 被 `##` 正则切分吞掉、单 `##` 删除被忽略），详见 `docs/source-compat-matrix.md`。书旗/七猫全链路无回归。剩 lastChapter=None（详情 JSON 无该字段，非缺陷）  | 已达成（含引擎保真修复）                                              |

### B. 已隐藏/降级后端能力本体（§14 缺口，每项大特性，单独立项）

实现前必读审计文档第 4 节处置规则。**含可能不需要的能力，动手前先与用户确认取舍（见下「待用户决策」）。** 用户决策（2026-06-12）：百度网盘/FTP 同步、browser_probe、完全体 unlock 三项「都先保留」，暂不移除。

| ID             | 能力域                   | 数量 | 当前处置                                        | 实现要点（审计文档第 4 节）                                                                                                                                                                              |
| -------------- | ------------------------ | ---- | ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ~~CAP-REPO~~   | repository/source_update | 6    | **closed 2026-06-12（已实现）**                 | `booksource_check_update` 基于 `@updateUrl` 真实版本比较；`apply_update` 真实下载/校验/写入并保留本地 @enabled；`repository_fetch/preview/install/check_source_sync` 全部接入。详见 NET/CAP 表与迭代日志 |
| ~~CAP-SYNC~~   | sync 云同步              | 12+4 | **WebDAV closed 2026-06-12；百度/FTP 保留隐藏** | WebDAV 12 命令已实现：凭据保存（不回传明文）、连接测试、状态、push/pull/sync、冲突列表/解决、客户端状态推送、阅读进度同步、生命周期通知；百度网盘 4 命令继续 `unsupported_hidden`（用户决策保留）        |
| CAP-BROWSER    | browser_probe 浏览器探测 | 12   | unsupported_hidden（用户决策保留）              | 真实 session/导航/JS 执行/cookie/UA                                                                                                                                                                      |
| CAP-TTS        | TTS 朗读                 | 6    | blocked_by_platform（已降级浏览器 Web Speech）  | 开放需真实语音列表/播放/停止/状态/试听/错误回退                                                                                                                                                          |
| CAP-COMICCOVER | 漫画/封面缓存            | 9    | blocked_by_platform                             | 真实下载/缓存/清理/计量                                                                                                                                                                                  |
| CAP-MISC       | update/unlock/misc       | 6    | blocked/hidden（unlock 用户决策保留）           | 插件 HTTP 需方法白名单+域名/IP 限制+超时+大小限制（§20.2）；unlock challenge 需真实签名/校验/过期。AI HTTP 已由 `ai_http_proxy_request` 按白名单实现。                                                   |
| CAP-VIDEO      | video 代理               | 2    | blocked_by_platform                             | 番茄短剧视频播放（Phase 7）                                                                                                                                                                              |

（命令清单见本文件「当前前端可触达 UNSUPPORTED 模块」表。CAP-REPO 已移出。）

### C. 架构验收

| ID           | 任务                                                                                                    | 验收                                                                                                                                                                                                                                                                                                                     |
| ------------ | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| FORMB-ACCEPT | 形态 B 浏览器闭环验收（§60.3）：纯浏览器前端连远端后端，走完「书源列表→搜索→加书架→目录→正文→进度保存」 | **local headless loopback passed 2026-06-12**：`src-headless` 托管 `dist` + `/ws`，浏览器启动 0 error/0 warning，并跑通保存 fixture 书源→列表→搜索→详情→加书架→目录→正文→缓存正文→进度保存。自动回归：`cargo test -p legado-headless formb_accept_headless_dispatch_chain`。严格「另一台机器/LAN」实测仍需外部环境补跑。 |

### 待用户决策（2026-06-12 已确认：三项「都先保留」）

- 百度网盘 / FTP 同步：**保留隐藏**，暂不实现也不移除。
- browser_probe：**保留**（unsupported_hidden），暂不实现也不移除。
- unlock 完全体解锁：**保留**（unsupported_hidden），暂不实现也不移除。

口径说明：

- R-P1-004 修正前端扫描器后，`onlyBackend = none`。旧的 3 个 onlyBackend 均为 `invokeWithTimeout<T>` 多行泛型调用漏扫。
- R-P0-001 的修正后 UI/调用层口径原为 `frontend_unsupported_stub_count = 60`；CAP-REPO 后降至 52，CAP-SYNC WebDAV 后降至 40，AI-DEEPSEEK-MGZ 后降至 39。剩余 `sync_baidu_*` 4 命令由 provider 级门禁隐藏/禁用，因此 R-P0-001 仍为 closed。
- `bookshelf_export_book_data` 是前端可触达且已实现的移动端导出路径，不是后台孤儿。

## CAP-SYNC WebDAV 交接设计（下一个接手 AI 直接据此实现）

目标：把 `sync` 能力域的 **WebDAV** 部分真实化（凭据保存/连接测试/状态/手动同步/冲突）；**百度网盘/FTP 按用户决策保持隐藏**。这是 A 段全清、CAP-REPO 完成后建议优先取用的独立项（不在待用户决策之列）。

### 前端契约（已固定，后端必须照此实现，勿改前端）

配置键（app config，`SectionSync.vue` 经 `setConfig` 写入）：`sync_webdav_url`、`sync_webdav_username`、`sync_webdav_root_dir`（默认 `legado-sync`）、`sync_webdav_allow_http`（布尔，UI 存字符串，后端需同时接受 bool 与字符串，参照 NET-001 `config_bool`）。密码不走 app config，走 `sync_set_credentials`。

命令与类型（`src/composables/useSync.ts`）：

| 命令                                                                                                         | 入参                                                                               | 返回                                                                                                           |
| ------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `sync_get_status`                                                                                            | —                                                                                  | `SyncStatus{enabled,running,lastSuccessAt,lastFailedAt,lastError,dirtyDomains[],conflictCount,lastRunSummary}` |
| `sync_set_credentials`                                                                                       | `{password}`                                                                       | void                                                                                                           |
| `sync_get_credentials`                                                                                       | —                                                                                  | `SyncCredentials{password}`（建议只回 `{password:""}` 或是否已设置，勿回明文）                                 |
| `sync_clear_credentials`                                                                                     | —                                                                                  | void                                                                                                           |
| `sync_test_connection`                                                                                       | `{password?}`                                                                      | `{ok:bool,message}`                                                                                            |
| `sync_now`                                                                                                   | `{mode:"sync"\|"pull"\|"push", domains:null, conflictStrategy?:"local"\|"remote"}` | `SyncRunSummary{status,mode,domains[],uploadedDomains[],appliedDomains[],conflictCount,message}`               |
| `sync_list_conflicts`                                                                                        | —                                                                                  | `SyncConflict[]`                                                                                               |
| `sync_resolve_conflict`                                                                                      | `{conflictId,action}`                                                              | void                                                                                                           |
| `sync_client_state_set`/`sync_report_reader_session`/`sync_v2_sync_reading_progress`/`sync_notify_lifecycle` | 见 useSync.ts                                                                      | 多为 void，可先做最小实现/no-op 但需非 UNSUPPORTED                                                             |
| `sync_baidu_*`（4）/ FTP                                                                                     | —                                                                                  | **保持 UNSUPPORTED**（provider 级，见下「能力门禁」）                                                          |

### 后端实现要点

- 新建 `crates/reader-core/src/service/sync_webdav.rs`（或 `crawler/webdav.rs`）。WebDAV 用 reqwest 自定义方法：`PROPFIND`/`MKCOL`（`reqwest::Method::from_bytes`）、`PUT`、`GET`、`DELETE`，basic auth（`.basic_auth(user, Some(pass))`）。复用 `book_service.http_client()`（已带代理/超时/DoH/TLS）。
- `allow_http=false` 时拒绝非 https URL（与 `validate_network_url` 同风格新增 `validate_webdav_url`）。
- 连接测试：对 `{url}/{root_dir}/` 发 `PROPFIND Depth:0`，2xx/207 即 ok；404 则尝试 `MKCOL` 建目录再判。返回 `{ok,message}`，message 不得泄露完整凭据/敏感 header。
- 凭据：密码存何处需定（Windows 无 keychain 抽象时可存 app data 下受限文件或 `frontend_storage` 专用命名空间，**不要**明文回传给前端）。`sync_get_credentials` 只回「是否已设置」语义。
- 同步内容（`domains`/scopes，已排查清楚，共 8 个，由 `sync_scope_{scope}` 布尔开关启用，`useSync.ts:enabledScopes`）：

  | scope              | 数据来源（后端从哪取）                                                                                                                    |
  | ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
  | `bookshelf`        | 书架书目（facade 书架/`bookshelf_list` 同源 DB）                                                                                          |
  | `reading_progress` | 每本书阅读进度（facade 进度存储）                                                                                                         |
  | `booksources`      | 书源文件（`js_source_dir` 下 `.js` + legado JSON 源）                                                                                     |
  | `app_settings`     | app config（`app_config_get_all` 全量）                                                                                                   |
  | `reader_settings`  | **前端推送**：`sync_client_state_set{domain:"reader_settings",value}`，对应前端 namespace `dynamic-config.reader.defaults.lastEffective`  |
  | `source_flags`     | **前端推送**：`sync_client_state_set{domain:"source_flags",value:{exploreDisabled,searchDisabled}}`，对应 namespace `source.capabilities` |
  | `extensions`       | 待定（扩展/插件存储；grep `sync_scope_extensions` 确认，无现成推送则二期）                                                                |
  | `script_config`    | 待定（脚本 REPL/脚本配置；同上）                                                                                                          |

  关键架构点：`bookshelf`/`reading_progress`/`booksources`/`app_settings` 四个域后端**自有数据**直接读；`reader_settings`/`source_flags` 两个域是**前端在 `sync_now` 前经 `pushClientState()` → `sync_client_state_set` 推到后端内存**的——所以 `sync_client_state_set` 必须真实实现（存进进程内 `Mutex<HashMap<domain,Value>>`），`sync_now` 再把这些域 + 自有域一起序列化。`sync_now` 把每个启用域 JSON `PUT` 到 `{root}/legado/{domain}.json`，`pull` 时 `GET` 回按 `conflictStrategy` 合并；冲突入 `sync_list_conflicts`。先做 push/pull 全量覆盖（mode=push/pull），`mode=sync` 双向 + 冲突检测可二期。`SyncStatus.dirtyDomains` = 自上次成功同步后有改动的域集合（可先恒返回全部启用域，二期再做脏标记）。

- 状态：进程内 `Mutex<SyncStatus>`（last\*、running、conflictCount），`sync_get_status` 读它。

### 能力门禁（关键陷阱）

`sync` capability 是 **16 命令一个 key**。直接把 `system.rs` 的 `sync` 置 `supported:true` 会同时把 baidu/FTP 暴露出去，违反「保留隐藏」。两条路任选：

1. **拆 capability**：新增 `syncWebdav`（supported:true，含 webdav 相关命令）与保留 `sync`/`syncBaidu`（supported:false，含 baidu/ftp）。需同步改 `useCapabilities.ts` 的 `CapabilityKey` 联合类型、`fallbackCapabilities`、`SectionSync.vue` 的 provider 级门禁。
2. **provider 级门禁**（更小改动）：`sync` 置 supported:true，但 `sync_baidu_*` 命令体仍返回显式 `Err(provider_unsupported)`，且 `SectionSync.vue` 的 provider `n-select` 对 baidu/FTP 选项 `disabled`（CLEAN-003 已确认该 select 有 `:disabled`，需细化到 option 级）。契约脚本会把仍返回 UNSUPPORTED 的 baidu 命令计回 stub——需在 command-matrix 标注为 `provider_unsupported_hidden` 而非缺陷。

推荐路 1（拆 capability）更干净，契约 stub 数下降也更准确。

### 验收 / 测试

- 仿 `crates/reader-core/tests/repository.rs`：用 axum mock 一个最小 WebDAV（响应 PROPFIND 207 / PUT 201 / GET 200），离线跑通 test_connection → set_credentials → sync_now(push) → sync_now(pull) 往返。
- Gate 同 §6；契约 stub 数应下降（webdav 命令转 implemented）；`docs/command-matrix.md`、本文件、`docs/ai-iteration-log.md` 同步。

### 实施记录（2026-06-12 CAP-SYNC WebDAV）

- 已按“拆 capability”方案落地：新增 `syncWebdav` capability 为 supported；旧 `sync` capability 只覆盖百度/FTP provider 未实现命令，保持 unsupported_hidden。
- 后端新增 `service/sync_webdav.rs`，用 reqwest 自定义 `PROPFIND`/`MKCOL`/`PUT`/`GET`，复用主 HTTP client；`allow_http=false` 时拒绝 HTTP。
- `ReaderCore` 已实现凭据保存（本机 app data；`sync_get_credentials` 只回 `passwordSet`，不回明文）、连接测试、状态查询、push/pull/sync、冲突列表/解决、`reader_settings`/`source_flags` 客户端状态、阅读进度同步入口与生命周期事件校验。
- 已支持域：`bookshelf`、`reading_progress`、`booksources`、`app_settings`、`reader_settings`、`source_flags`。`extensions`、`script_config` 仍为 deferred；若用户启用会明确报错，不上传空对象冒充成功。
- 验收证据：`crates/reader-core/tests/sync_webdav.rs::webdav_sync_push_pull_round_trip` 用本地 axum mock 跑通 credentials → test_connection → push → pull；`src-tauri/tests/ws_router.rs::webdav_sync_commands_are_routed` 确认形态 B 白名单已接线。

## R 队列状态

| ID                    | 状态                 | 当前证据                                                                                                                                                                                                                                                                                                                                                                                                            |
| --------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R-P1-003              | closed               | `SectionBackup.vue` zip 过滤已改为 json，见 `reports/gates/2026-06-10-1016-R-batch1/summary.md`                                                                                                                                                                                                                                                                                                                     |
| R-P1-001              | closed               | 契约脚本逐函数分类 + self-test；当前 `browser_probe_create` 判 stub，`backup_inspect` 判 implemented                                                                                                                                                                                                                                                                                                                |
| R-P0-003              | closed for 书旗/七猫 | 书旗/七猫 full_chain 均 `live_network_pass`；番茄/番茄短剧转入 R-P2-003/004                                                                                                                                                                                                                                                                                                                                         |
| R-P1-002              | closed               | `web_server_stop_releases_port_for_restart` 回归测试，见 R-batch2 提交                                                                                                                                                                                                                                                                                                                                              |
| R-P0-002              | closed               | 本文件、`docs/command-matrix.md`、`docs/source-compat-matrix.md` 已按 2026-06-10 实测重写，旧冲突表已删除                                                                                                                                                                                                                                                                                                           |
| R-P0-001              | closed               | 修正后 60/60 个前端可触达 UNSUPPORTED stub 已逐条归档为 `unsupported_hidden` 或 `blocked_by_platform`；R-P1-004 补扫出的 2 个 sync 命令已由既有 sync 能力门禁覆盖                                                                                                                                                                                                                                                   |
| R-P1-004              | closed               | 前端扫描器已支持 `invokeWithTimeout<T>` 多行泛型调用；`onlyBackend` 从 3 修正为 0，见 `reports/gates/2026-06-10-1818-R-P1-004-contract-scanner/summary.md`                                                                                                                                                                                                                                                          |
| R-P2-001              | closed               | Android release signing 配置和文档已建立；`keystore.properties`/keystore 不入库；`:app:checkReleaseSigning` 在无密钥时按预期失败，`pnpm run build:android:release` 仍可产出 unsigned 验证包                                                                                                                                                                                                                         |
| R-P2-002              | closed               | `pnpm lint` 已从 71 warnings / 0 errors 收敛到 0 warnings / 0 errors；动态执行边界均用局部 `oxlint-disable-next-line` 标注理由，见 `reports/gates/2026-06-10-1910-R-P2-002-lint-warnings/summary.md`                                                                                                                                                                                                                |
| R-P2-003..007,009,010 | open                 | 番茄/短剧、缓存系统、Harmony 标注、`book` 对象绑定、QuickJS Runtime 复用、JS HTTP 桥线程池化；架构纪律见 `docs/frontend-backend-separation.md` 与总纲第 60 节，详见审计文档第 3 节                                                                                                                                                                                                                                  |
| R-P2-008              | in_progress          | 前后端分离 WS 服务端阶段 1+2 试点已落地：`commands/router.rs` + `ws_server.rs`；阶段 4 `src-headless` 已存在。**2026-06-12 FORMB local loopback passed**：补齐 headless 分发契约后，纯浏览器连接独立 headless 后端跑通书源→阅读进度闭环，见 `reports/gates/2026-06-12-FORMB-ACCEPT-headless-loopback/summary.md`。剩余：严格跨物理机器/LAN 实测；桌面壳内 WS 对外暴露仍不建议，优先使用 headless `--bind/--token`。 |
| R-P2-011              | closed               | 前端绕过传输层修复：prefetch.ts 已改环境分流（鸿蒙 → DOM、Tauri/WS → useEventBus），logger.ts 经评估保留直连并列入纪律文档第 4 节例外；见 `reports/gates/2026-06-10-2018-R-P2-011-transport-bypass/summary.md`                                                                                                                                                                                                      |
| R-P2-012              | closed               | 2026-06-12 PREFETCH-LIVE-BUILD：`PrefetchPayload` 使用 `{ payload }` + camelCase，reader-core 预取支持 `startIndex`/`count`、同书任务取消、进度回调；Tauri IPC emit `shelf:prefetch-progress` / `shelf:prefetch-done`；WS 转发两类事件。新增 `prefetch_chapters_respects_range_and_emits_progress`，见 `reports/gates/2026-06-12-PREFETCH-LIVE-BUILD/summary.md`。                                                  |

## 5 个争议命令定真伪

| Command                       | 当前状态                            | 证据                                                                                                                                                                                                                                                                                                        | 验证方式                                                                                                                            |
| ----------------------------- | ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `booksource_cancel`           | implemented_with_limit              | `src-tauri/src/commands/source.rs` 对 `booksource_chapter_list`、`booksource_chapter_content` 注册 `TaskRegistry` token；`src-tauri/src/commands/bookshelf.rs` 对 `bookshelf_prefetch_chapters` 注册 token；`crates/reader-core/src/facade.rs` 的预取循环检查 token。限制：不能抢占已经进入的单次网络请求。 | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；相关长任务源码检查                                          |
| `booksource_purchase_chapter` | implemented_or_explicit_unsupported | JS 书源路径调用 runtime `purchaseChapter(chapterUrl)`；Legado 规则源返回 `{ ok:false, purchased:false, unsupported:true }`，不再假成功。                                                                                                                                                                    | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::purchase_chapter`        |
| `booksource_call_fn`          | implemented_for_js_source           | JS 书源路径调用 runtime 命名函数；Legado 规则源返回明确错误 `不支持自定义 JS 函数调用`，不是静默成功。                                                                                                                                                                                                      | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::source_call_fn`          |
| `booksource_run_tests`        | implemented                         | 支持 `step_filter`、`timeout_secs`、逐 step timeout，并按 search -> bookInfo -> toc -> content -> explore 真实执行链路。                                                                                                                                                                                    | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`crates/reader-core/src/facade.rs::run_source_tests`        |
| `storage_debug_dump`          | implemented_summary                 | 读取 frontend namespaces、app config key 数、书架数量和真实路径摘要，不再返回固定空对象。                                                                                                                                                                                                                   | `node scripts/ci/check-command-contract.mjs --json` 分类为 implemented；`src-tauri/src/commands/config.rs` -> `facade.debug_dump()` |

## 当前前端可触达 UNSUPPORTED 模块

R-P0-001 的契约口径经 R-P1-004 修正为 60 个前端可触达 stub；本轮已全部接入 `capabilities_get` + `useCapabilities`，并在 UI/调用层按模块禁用、隐藏、降级或 no-op。2026-06-12 CAP-REPO 真实实现 repository/source_update 6 命令后，stub 降至 52；CAP-SYNC WebDAV 真实实现 12 命令后，stub 降至 40；AI-DEEPSEEK-MGZ 实现 AI HTTP 代理后降至 39。仓库/更新、WebDAV 同步与 AI HTTP 代理已移出本表。注意：本表只关闭“点击后直撞 UNSUPPORTED”的入口裸露问题，不代表后端缓存、解锁等剩余能力已经实现。

| 模块               | 数量 | 当前处置            | 命令                                                                                                                                                                                                                                                                                                           |
| ------------------ | ---: | ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| sync provider      |    4 | unsupported_hidden  | `sync_baidu_start_auth`, `sync_baidu_token_status`, `sync_baidu_poll_token`, `sync_baidu_revoke_auth`                                                                                                                                                                                                          |
| tts                |    6 | blocked_by_platform | `tts_stop`, `tts_is_initialized`, `tts_is_speaking`, `tts_speak`, `tts_get_voices`, `tts_preview_voice`                                                                                                                                                                                                        |
| video              |    2 | blocked_by_platform | `start_video_proxy`, `stop_video_proxy`                                                                                                                                                                                                                                                                        |
| browser_probe      |   12 | unsupported_hidden  | `browser_probe_create`, `browser_probe_navigate`, `browser_probe_eval`, `browser_probe_run`, `browser_probe_get_cookies`, `browser_probe_set_cookie`, `browser_probe_set_user_agent`, `browser_probe_clear_data`, `browser_probe_show`, `browser_probe_hide`, `browser_probe_close`, `browser_probe_close_all` |
| comic_cover        |    9 | blocked_by_platform | `comic_download_images`, `comic_get_page_sizes`, `comic_get_cached_page`, `comic_cache_clear_chapter`, `comic_cache_clear`, `comic_cache_size`, `cover_resolve_cache`, `cover_cache_size`, `cover_cache_clear`                                                                                                 |
| update/unlock/misc |    6 | blocked/hidden      | `explore_clear_cache` 为 `blocked_by_platform` 降级；`frontend_plugin_http_request`、`issue_*unlock*`、`verify_*unlock*` 为 `unsupported_hidden`                                                                                                                                                               |

> ~~repository/source_update（6）~~ 已于 2026-06-12（CAP-REPO）真实实现并移出本表，`repository` capability 置 supported。
> ~~sync WebDAV（12）~~ 已于 2026-06-12（CAP-SYNC）真实实现并移出本表，新增 `syncWebdav` capability 置 supported；旧 `sync` capability 仅保留百度/FTP provider 未实现命令。
> ~~AI HTTP 代理（1）~~ 已于 2026-06-12（AI-DEEPSEEK-MGZ）真实实现并移出本表，新增 `ai_http_proxy_request`，`aiProxy` capability 置 supported。

## 下轮第一件事

前后端接入审计已结清，**路线图 A 段（环境/网络阻塞项）全部结清**，B 段 **CAP-REPO 与 CAP-SYNC WebDAV 已结清**，C 段 **FORMB-ACCEPT 本机 headless loopback 已通过**，本轮 `R-P2-012/PREFETCH-PROGRESS` 已结清：

- A 段：NET-004-LIVE（DoH 实测修 360dns/onedns）、NET-005（DoH 接入 JS 桥）、SRC-FANQIE-LIVE（番茄 bookInfo 字段验收 + 引擎字段管线两处保真修复）。
- B 段：CAP-REPO（书源仓库 + `@updateUrl` 在线更新，6 命令真实实现，stub 58→52）；CAP-SYNC WebDAV（12 命令真实实现，stub 52→40）；AI-DEEPSEEK-MGZ（AI HTTP 代理 1 命令真实实现，stub 40→39）。
- C 段：FORMB-ACCEPT（纯浏览器 + 独立 `legado-headless` + WS；本机 loopback 业务闭环通过，跨机器/LAN 实测待外部环境）。
- R-P2-012：预取进度链路已完成，Tauri IPC 有 progress/done 事件，WS 转发同名事件；见 `reports/gates/2026-06-12-PREFETCH-LIVE-BUILD/summary.md`。

用户决策（2026-06-12）：百度网盘/FTP 同步、browser_probe、完全体 unlock 三项「都先保留」（不移除）。

下一轮第一件事：若有第二台设备或可访问 LAN，做 `FORMB-LAN-VERIFY`（headless `--bind 0.0.0.0 --token <token>` + 浏览器 `?ws=ws://<host>:<port>/ws?token=<token>` 复跑同一闭环）；若当前环境无外部设备，则转 B 段剩余能力本体 `CAP-BROWSER`（真实 session/导航/JS/cookie/UA）。

CAP-REPO 与 CAP-SYNC WebDAV 的形态 B WS 路由也已补齐（repository 6 命令 + WebDAV 12 命令入 `router.rs` 白名单；`ws_router.rs` 覆盖 repository 与 WebDAV 路由，11/11）。

下一步候选（B 段剩余 / C 段）：

1. C 段 **FORMB-LAN-VERIFY** 严格跨机器/LAN 形态 B 验收；需要第二台设备或可访问 LAN，命中用户/环境 blocker 时跳过。
2. B 段 **CAP-BROWSER**（真实 session/导航/JS/cookie/UA），以及 CAP-TTS/CAP-COMICCOVER/CAP-MISC/CAP-VIDEO。均为大特性，按审计文档第 4 节处置规则实现，同步 `capabilities_get` + 前端入口 + `docs/command-matrix.md`。

历史项 R-P2-003（番茄 JS API 缺口）已并入并随 SRC-FANQIE-LIVE 结清。

## 2026-06-13 MAINT-IMPORT-UI-PERF 状态更新

- 书源管理标题竖排问题已回归验证：1000x800 与 390x800 下标题均为横向 `80x32`，按钮区无重叠。
- 喵公子订阅导入链路已优化：`rsssource` 解析出的书源 JSON 内容会复用于导入，避免重复下载；订阅页解析使用 4 路受控并发。
- 喵公子订阅实网非破坏验证通过：当前订阅解析出 10 个有效书源包。
- 书源性能提示弹窗已从常驻 `n-dialog` 改为受控 `n-modal`，避免遮挡页面且按钮关闭无效。
- 本轮门禁通过；命令契约维持 frontendTotal=162、registeredTotal=161、bothCount=161、onlyBackend=0、frontend unsupported stub=39。

下一轮优先候选：继续 UI 收口，处理窄屏侧栏占宽与前端大 chunk 拆分；若需要做更大能力本体，则按文档转 `CAP-BROWSER`。

## 2026-06-13 UI-NARROW-SHELL 状态更新

- 已修复窄视口 shell 判定：自动布局现在同时考虑移动 UA/粗指针和 `(max-width: 640px)`，桌面浏览器缩到 390px 时会进入移动 shell。
- 用户显式布局模式覆盖保持不变：`desktop` 仍强制桌面，`mobile` 仍强制移动，`auto` 才使用新窄屏判定。
- 390x800 实测：`app-layout--mobile`，侧栏不存在，底部导航可见，主内容宽度 390px，横向溢出 0。
- 390x800 书源管理实测：`书源管理` 可见 `h1` 为 80x32，横向排版，按钮无重叠。
- 1000x800 实测：仍为桌面 shell，侧栏可见，主内容 800px，未回退。
- 本轮门禁通过：`pnpm lint`、`pnpm build`、`cargo check -p reader-core`、`cargo check -p legado-tauri`、`cargo test -p reader-core`、命令契约检查、`git diff --check`。

下一轮优先候选：继续 UI 回归设置页/阅读器设置/书源管理按钮行，随后处理前端大 chunk 和 `useTransport` 无效动态导入。

## 2026-06-13 UI-READER-SETTINGS-LAYOUT 状态更新

- 已修复阅读器关闭后透明 `n-modal` 残留拦截点击的问题：阅读器 modal 关闭时直接卸载，打开后不再停留在 `opacity=0`。
- 已修复阅读器菜单在后台/自动化环境中停在过渡初始位移的问题：顶部栏、底栏和遮罩改为按状态直接挂载，菜单按钮可正常点击。
- 已收口阅读器设置面板窄屏布局：颜色、背景、皮肤、字号、翻页按钮和更多设置入口在 390px 下无横向溢出、无文本裁切。
- 已将主设置页移动端入口圆角收敛为 8px，并验证 `overflowX=0`。
- 本轮门禁通过：`oxfmt --check .`、`pnpm lint`、`pnpm build`、`cargo check -p reader-core`、`cargo check -p legado-tauri`、`cargo test -p reader-core`、命令契约检查、`git diff --check`。

下一轮优先候选：继续 UI 回归书源管理批量导入/AI 写书源/段评抽屉；随后处理前端大 chunk 和 `useTransport` 无效动态导入。

## 2026-06-13 PERF-MODULEPRELOAD-PRUNE 状态更新

- 已在非 Harmony 构建中接入 Vite `modulePreload.resolveDependencies`，只调整预加载策略，不改变实际模块图、动态导入语义或业务代码。
- `dist/index.html` 的首屏 `modulepreload` 从 20 条收敛到 5 条，保留 `rolldown-runtime`、`vendor-vue-naive`、`_plugin-vue_export-helper`、`useTransport`、`useInvoke` 等核心入口依赖。
- JS 动态导入的预加载链不再提前注入整条 JS 依赖；构建产物中的 `__vite__mapDeps` 现在主要保留异步组件 CSS 依赖，减少首屏和切页时的网络扇出。
- `pnpm build` 后入口 `index-BjH9Vjka.js` 为 65.83 kB，gzip 22.66 kB；`vendor-vue-naive` 与 `_plugin-vue_export-helper` 仍超过 500 kB，后续需继续审计 Naive UI 全量注册和共享 chunk。
- 本轮门禁通过：`pnpm lint`、`pnpm build`、`cargo fmt --all -- --check`、`cargo check -p reader-core`、`cargo check -p legado-tauri`、`cargo test -p reader-core`、命令契约检查、`git diff --check`。

下一轮优先候选：继续前端性能收口，优先拆解 `vendor-vue-naive` / `_plugin-vue_export-helper` 的首屏来源；若范围过大，先处理 `useTransport` ineffective dynamic import 的静态导入链。

## 2026-06-13 PERF-TRANSPORT-LAZY 状态更新

- 已将 `useInvoke`、`useEventBus`、`useFrontendStorage`、appConfig/scriptBridge store、书架入口、设置页和 WS 连接弹窗里的 `useTransport` 静态引用改为调用期动态导入。
- `useTransport` 现在成为独立异步 chunk：`useTransport-BKg5SsZx.js` 为 7.21 kB，gzip 2.75 kB；不再进入 `dist/index.html` 首屏 `modulepreload`。
- `dist/index.html` 的首屏 `modulepreload` 从上一轮 5 条继续降到 4 条；`useTransport ineffective dynamic import` 构建告警已消失。
- 当前入口 `index-C-PUgMzD.js` 为 65.87 kB，gzip 22.66 kB；剩余大 chunk 仍是 `vendor-vue-naive` 与 `_plugin-vue_export-helper`。
- 本轮门禁通过：`pnpm lint`、`pnpm build`、`cargo fmt --all -- --check`、`cargo check -p reader-core`、`cargo check -p legado-tauri`、`cargo test -p reader-core`、命令契约检查、`git diff --check`。

下一轮优先候选：继续审计 `vendor-vue-naive` / `_plugin-vue_export-helper` 的首屏来源，重点评估 `stores/index.ts` 桶导出和 Naive UI 全量注册的拆分风险。

## 2026-06-13 PERF-APP-SHELL-SPLIT 状态更新

- 已从 App shell 移除 `stores/index.ts` 桶导入，改为直接导入首屏实际需要的 store，避免通过桶文件静态拉入 `bookSource`、`musicPlayer`、`scriptBridge` 等重模块。
- 已把启动期书源数量检查改为动态加载 `bookSource` store，把 Legado 深链安装弹窗里的 `BookSourceInstallDialog` 改为显示时异步加载。
- 已将全局音乐播放器条和覆盖层改为异步组件；新增 `useTtsState`，使 App 只观察 TTS 播放状态，不再静态加载完整 `useTts` 与前端插件运行时。
- `GlobalFeedbackMirror` 的 `scriptBridge` 调试日志兜底改为失败路径动态导入。
- 构建结果：入口 `index-Dm9WUU1T.js` 为 56.55 kB / gzip 19.82 kB，入口 CSS 为 59.71 kB / gzip 12.43 kB；`_plugin-vue_export-helper` 收敛为 0.08 kB 小 helper；`useFrontendPlugins` 仍是 1.17 MB 异步 chunk，但不再是首屏静态依赖。
- 本轮门禁通过：`pnpm lint`、`pnpm build`、命令契约检查、`cargo fmt --all -- --check`、`cargo check -p reader-core`、`cargo test -p reader-core`、`cargo check -p legado-tauri`、`git diff --check`。

下一轮优先候选：继续审计 `vendor-vue-naive` 首屏来源，重点看 Naive UI 全量注册、全局组件与入口 store 依赖是否还能继续拆分。

## 2026-06-13 PERF-NAIVE-PARTIAL-REGISTER 状态更新

- 已移除 `main.ts` 中的 Naive UI 全量默认插件注册，改为 `src/plugins/naiveComponents.ts` 中的显式 `create({ components })` 注册表。
- 本轮没有新增 unplugin 依赖，也没有修改依赖版本或 lockfile；用脚本扫描 Vue 模板后登记 39 个全局使用的 Naive 组件。
- 构建结果：`vendor-vue-naive` 从上一轮约 1,396 kB / gzip 378.84 kB 降到 697.89 kB / gzip 197.04 kB；入口 `index-fjOpBFyM.js` 为 57.02 kB / gzip 20.12 kB。
- 首屏 `modulepreload` 仍为 4 条，保留 `vendor-vue-naive`；`useFrontendPlugins` 仍是 1.17 MB 异步大 chunk。
- 本轮门禁通过：`pnpm lint`、`pnpm build`、命令契约检查、`cargo fmt --all -- --check`、`cargo check -p reader-core`、`cargo test -p reader-core`、`cargo check -p legado-tauri`、`git diff --check`。

剩余风险：`naiveComponents.ts` 是维护清单，后续新增全局 `<n-*>` 标签时必须同步登记；下一轮优先候选是继续拆 `useFrontendPlugins` 异步大 chunk，或评估 `vendor-vue-naive` 的路由级拆分空间。

## 2026-06-13 PERF-FRONTEND-PLUGIN-BARREL-CUT 状态更新

- 已从 `src/stores/index.ts` 移除 `useFrontendPluginsStore` 和插件类型透传，并移除书架 feature store 的通用桶出口，避免 `bookshelfUi` 通过桶文件间接把插件 store 带进通用 `stores` chunk。
- 已把 `BookshelfView`、书架 UI store、书架 action service、`ExtensionsView` 的相关导入改为具体 store/type 路径。
- 构建结果：`stores` chunk 从本轮中间态 21.49 kB / gzip 7.16 kB 降到 16.63 kB / gzip 5.30 kB；`stores-*.js` 中不再包含 `frontendPlugins` / `useFrontendPlugins` / `plugin-action` / `plugin-cover`。
- `frontendPlugins` store 桥接现在是 0.14 kB / gzip 0.12 kB 的独立小 chunk；完整 `useFrontendPlugins` 运行时仍是 1.17 MB / gzip 509.87 kB 的异步大 chunk。
- 本轮门禁通过：`pnpm lint`、`pnpm build`、命令契约检查、`cargo fmt --all -- --check`、`cargo check -p reader-core`、`cargo test -p reader-core`、`cargo check -p legado-tauri`、`git diff --check`。

剩余风险：本轮切断的是共享桶边界，不缩小插件运行时本体；下一步若继续性能收口，应拆 `useFrontendPlugins` 内部运行时，或给书架插件菜单做更细粒度懒查询。
