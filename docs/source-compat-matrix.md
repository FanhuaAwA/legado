# Source Compatibility Matrix

记录各本地测试书源在 Tauri 项目中的兼容状态。

最后实测：2026-06-13（新增中文维基文库经典小说 JS 源；公有领域《三國演義》search → bookInfo → toc → first/final content 全链路实网复测）。

实测命令：

```powershell
# 本地导入（reader-core facade，对应 Tauri 命令 booksource_import_legacy_json_text；多文件批量对应 booksource_import_legacy_json_texts）
cargo test -p reader-core --test source_compat_import imports_and_parses_fields -- --ignored --nocapture
# 网络导入（reader-core facade，对应 Tauri 命令 booksource_import_legacy_json_url）
cargo test -p reader-core --test source_compat_import network_import -- --ignored --nocapture --test-threads=1
# 本地导入后全链路（search → bookInfo → toc → content）
cargo test -p reader-core --test source_compat_import full_chain -- --ignored --nocapture --test-threads=1
# 番茄搜索链路专项复测
cargo test -p reader-core --test source_compat_import fanqie_source_search_and_book_info -- --ignored --nocapture
# 公有领域 Wikisource JS 源全链路
cargo test -p reader-core wikisource_classics_public_domain_full_chain -- --ignored --nocapture
```

状态枚举：`strict_pass`（mock/fixture）/ `live_network_pass`（实网通过）/ `live_network_ignored`（默认跳过）/ `partial` / `blocked_by_source_rule` / `blocked_by_platform` / `blocked_by_js_api` / `not_verified`。

## 2026-06-13 中文维基文库经典小说 JS 源（SOURCE-WIKISOURCE-CLASSICS）

新增仓库内 JS 书源 fixture：`crates/reader-core/tests/fixtures/book_sources/wikisource_classics.js`。当前目录包含《三國演義》，目标站点为中文维基文库公开页面。该源只抓取公有领域文本，不用于绕过登录、付费、试看、验证码、设备绑定或访问控制。

关键边界：

1. Wikisource/Wikimedia 会拒绝缺少 `User-Agent` 的机器人请求。书源统一通过 `fetchWiki()` 带 `User-Agent` 与 `Accept` 请求头，并保留 `@minDelayMs 800`，避免高频抓取。
2. `chapterList` 解析 Wikisource 目录页的 `/第001回` 到 `/第120回` 链接，不依赖第三方中转站。
3. `chapterContent` 只提取 `mw-parser-output` 正文区域，并剔除表格、编辑链接、脚注标记等页面噪声。

实网验证：

```powershell
cargo test -p reader-core wikisource_classics_public_domain_full_chain -- --ignored --nocapture
```

结果：`live_network_pass`。2026-06-13 实测《三國演義》返回 `chapters=120`，首章正文 `first_len=14153`，最终章正文 `latest_len=19775`。测试同时断言首章包含 `話說天下大勢`，最终章可读取非空正文，并排除 `此页面目前没有内容` 与 `试看`。

## 2026-06-12 番茄 bookInfo 字段完整性 + 引擎字段管线两处修复（SRC-FANQIE-LIVE）

实网验收番茄 `bookInfo` 字段完整性时，发现 `kind` 字段输出为未处理的原始模板 `男生1女生\n连载0完结\n9.9分\n...`，其 `##正则` 清洗与尾部 `@js:` 后处理均未生效。定位为 `rule_engine.rs` 字段提取管线的两处通用缺陷（非番茄特例）：

1. **字段管线顺序错误**：`eval_field_json_with_ctx` 先 `split_legado_regex` 再 `extract_js`。对 `选择器##正则\n@js:...` 形式的规则，`##` 切分会把尾部 `@js:` 吞进正则替换串，导致正则与 JS 两段都不执行。Legado 的字段管线顺序是「取值 → `##`正则 → JS」。修复：先 `extract_js` 分离 JS 段，再对纯选择器部分 `split_legado_regex`，且正则在 JS **之前**应用（JS 看到的是已清洗的 `result`）。
2. **单 `##` 删除模式被忽略**：`apply_legado_regex` 的循环 `while i + 1 < parts.len()` 对「只有 `##pattern` 无 `##replacement`」的删除型规则不处理（Legado 中 `##正则` 即「替换为空 = 删除」）。修复：循环改为 `while i < parts.len()`，缺失的替换串按空串处理。

回归保护（strict，无网）：`rule_engine` 新增 `test_json_field_applies_regex_before_trailing_js`、`test_json_field_single_hash_regex_deletes`。

实网验证：番茄 `kind` 修复后输出 `\n完结\n9.9分\n都市高武,都市,穿越`（连载0 删除、男生女生经 @js 处理），与上游规则意图一致。书旗（329 章 / 4657 字）、七猫（2551 章 / 15132 字）全链路无回归。该修复是通用引擎保真，不含任何书源特例硬编码（符合总纲 §39.1）。

## 2026-06-11 七猫正文编码回归修复

用户反馈：书籍打开后正文出现 `äº...` 乱码。实测确认乱码已在后端 `chapter_content` 返回值中出现，非前端渲染问题。

根因：七猫章节 URL 的 `{"type":"qimao"}` 使 `fetcher` 将响应 raw bytes hex 编码交给 `ruleContent`；规则再调用 `java.hexDecodeToString(result)`。旧实现用 `u8 as char` 把 UTF-8 字节按 Latin-1/单字节字符展开，导致 `e4 ba 8c` 被重新编码为 `c3 a4 c2 ba c2 8c`。

修复：`crates/reader-core/src/parser/js.rs` 的 `java.hexDecodeToString` 已改为 hex bytes → UTF-8 string，并补 `js_compat` 回归测试。复测 `diag_qimao_content_encoding` 首字节恢复为 `e4ba8c...`，`qimao_source_full_chain` 通过：search → toc(2551) → content(15132 字符)。

补充修复：用户实测《斗罗大陆》前两页正常、后续 `第一章 斗罗大陆，异界唐三（三）` 仍乱码。复查确认新抓取链路已经返回中文，残留问题来自旧版本写入的章节缓存。`book_service.get_content` 现在会在缓存命中时识别 UTF-8→Latin-1 旧乱码，可还原则回写修复后的缓存，不可还原则删除旧缓存并重新抓取；新抓取结果写缓存前也做同样兜底。`diag_qimao_douluo_first_chapters_encoding` 已覆盖《斗罗大陆》前 4 章并通过，第 4 章头部为正常中文。

## 2026-06-11 番茄搜索引擎兼容修复

用户反馈：番茄书源搜索小说无结果，日志显示搜索「我不是戏神」时 `device_register` 抛 `JS Exception: network error`。

根因：番茄 search 依赖一组 Legado/Rhino 兼容行为。旧引擎同时存在 `getVerificationCode` 伪实现、`base64DecodeToByteArray` 二进制 body 有损、OkHttp shim 请求规格不兼容、`with(JavaImporter)` 函数作用域不兼容、中文未声明全局变量/旧式 for 循环变量不兼容，以及 `<js>...</js>$[*]` 规则尾部 JSONPath 未继续执行的问题。

修复：

- `java.getVerificationCode`：按 Legado 真实语义确认其为交互式验证码读取；headless 环境明确降级为空并记录日志，不再伪造 MD5/salt。
- `java.base64DecodeToByteArray` + OkHttp shim + `java.ajax`：新增 byte-array marker/base64 通道，`bodyBase64` / `bodyBytesBase64` 会还原为原始字节 POST；`RequestBody.create` 兼容 `(content, mediaType)` 与 `(mediaType, content)`。
- `Packages.okhttp3.*`：改为生成 Legado 标准 `url,{options}` 规格，避免旧的自定义 `METHOD||url||headers||body` 规格不能被 `java.ajax` 识别。
- `JavaImporter` / Rhino 兼容：正确展开 `with(JavaImporter(...)) { ... }` 并保留顶层函数可见性；补中文未声明变量和旧式 for 循环变量声明。
- `rule_engine`：`search_books_js` 支持 JS 规则输出后继续套用尾部 JSONPath，再按字段规则提取书籍。

复测：`fanqie_source_search_and_book_info` 实网通过，搜索关键词「我不是戏神」返回 `番茄搜索: 我不是戏神`，书籍详情请求进入番茄 book detail API。

## 2026-06-11 番茄 toc/content 全链路修复

用户反馈：搜索和详情成功后，打开正文时后端先请求 `https://reading.snssdk.com/第一卷：戏中人0` 返回 404，随后正文中转接口收到乱码 `item_id`，最终 35s 超时且正文为空。

根因：番茄目录 JS 会返回卷标题行和真实章节行。卷标题行 `isVolume=true` 且 `chapterUrl=""`，旧规则引擎把无 URL 的卷标题合成为 `标题+index` 伪 URL，前端无法区分卷标题与章节，点击第一项就会把 `第一卷：戏中人0` 当章节 URL。正文阶段还会丢失原始章节 data URI options 中的 `info=book_id#item_id`，使 `book.tocUrl` 为空。

修复：无真实 URL 的卷标题不再进入可读章节列表；`book_service.get_content` 保留原始 chapter URL 传给规则引擎；规则引擎从 `data:item_id;base64,...,{"info":"book_id#item_id"}` 还原 `book.tocUrl=data:book_id;base64,...`，供番茄 `ruleContent` 原规则继续执行。未修改 `E:\Book\番茄书源\*.json`。

复测：`fanqie_source_full_chain` 实网通过，搜索「我不是戏神」→ bookInfo → toc 1928 章，第一条为 `第1章 戏鬼回家` 且 URL 为 `data:item_id;base64,...`，content 返回 3135 字符正文。补充：同轮后续短时间重复运行时，外部设备注册链路多次返回 `JS Exception: network error at device_register`，测试未进入 toc/content；该失败归类为 `source_site = device_register_unreachable`，不是本轮修复的目录/正文回归。

## 2026-06-11 Windows 端本地导入 + 网络导入验证

验证目标：书旗 / 七猫 / 番茄三个书源在 Windows 端，本地文件导入与网络 URL 导入两条路径是否可用。结论按导入与使用两层分开记录（导入成功 ≠ 整链可用）。

| 书源 | 本地导入             | 网络导入             | search        | toc        | content               | 总体可用性                                                          |
| ---- | -------------------- | -------------------- | ------------- | ---------- | --------------------- | ------------------------------------------------------------------- |
| 书旗 | ✅ live_network_pass | ✅ live_network_pass | ✅            | ✅ 329 章  | ✅ 本地版 / ⚠️ CDN 版 | 本地导入完整可用；网络导入可搜索可读目录，正文受 CDN 规则新鲜度限制 |
| 七猫 | ✅ live_network_pass | ✅ live_network_pass | ✅            | ✅ 2551 章 | ✅ 本地版 / ⚠️ CDN 版 | 同上                                                                |
| 番茄 | ✅（导入+列表）      | ✅（导入+列表）      | ✅ 我不是戏神 | ✅ 1928 章 | ✅ 3135 字符          | search→bookInfo→toc→content 全链路已恢复；详情字段完整性仍待补验收  |

原则（2026-06-11 用户指令）：本地项目引擎必须兼容使用上游书源，不得反过来改书源去迁就引擎。本轮先证明引擎是否对上游忠实，再分清「引擎缺能力」与「书源规则过期」。

引擎忠实性核查：在 `crates/reader-core/src` 全量搜索 `书旗/七猫/番茄/shuqi/qimao/52dns/miaogongzi/qm_id/sq_id`，**无任何针对这三个源的特殊适配硬编码**（仅 `book_source.rs` 测试 fixture 用「番茄」作名字、`js.rs` 的通用 JS API）。引擎是按规则忠实执行的。

关键结论：

1. **三个书源的本地导入和网络导入本身全部成功**（下载 → 解析 → 入库 → 书源列表可见，errors 为空）。网络导入 URL 来自各目录 `网络导入.txt`，统一指向 `https://cdn.miaogongzi.cc/shuyuan/<file>.json`，CDN 可达。
2. **书旗 / 七猫本地导入完整可用**：全链路 search→toc→content 实网通过（书旗正文 8725 字符、七猫正文 22380 字符，2026-06-11 实测）。这证明**引擎本身对这两个源完全兼容**——当 `ruleContent` 与 API 匹配时正文正确提取。
3. **书旗 / 七猫网络导入正文受限 = 书源规则相对其自身 API 过期，不是引擎不兼容**。2026-06-11 决定性实测：直接抓 `jh.52dns.cc/shuqi/content.php` 原始响应，对 legado / okhttp / android / 无 UA 四种请求**一律返回直出 JSON** `{"data":{"content":...}}`（2043 字符真实正文），不存在按客户端返回 hex(URL) 的分支。而上游 CDN 版 `ruleContent` 仍是 `hexDecodeToString(result)`→期望 URL→二次请求 `java.ajax(url+"&key="+jsmy())`，结构上无法消费 JSON 输入——**任何 Legado 客户端用这份未改的上游源都会同样失败**（不是 Windows/本项目特有）。哈希比对：CDN 版 == 本地 `.backup.json`（原版）；本地 `.json` 是已更新版本。
   - 这不是「改书源迁就引擎」：更新一份相对自身 API 过期的 `ruleContent` 属 Legado 设计内的正常书源维护（`ruleContent` 就是 per-source 适配层），引擎对新旧规则都忠实执行、无特殊处理。引擎层无法、也不应替任意书源「猜测」其 hexDecode 应改成 JSON.parse（那才是污染通用逻辑）。
   - 实务建议：Windows 端完整阅读书旗/七猫用本地 `.json`（本地导入）即可；网络导入得到的是上游 CDN 当前（规则过期）版本，正文需待上游 CDN 更新其 `ruleContent`，或使用本地已更新版本。
4. **番茄 search 链路已恢复，属于本轮已解除的引擎兼容缺口**。旧失败点确实在引擎侧：`java.getVerificationCode` 伪实现、`base64DecodeToByteArray` 二进制 body 有损、OkHttp shim 规格不兼容、`with(JavaImporter)` 作用域不兼容、Rhino 非 strict 全局变量兼容不足，以及 `<js>...</js>$[*]` 搜索列表尾部 JSONPath 未执行。本轮已逐项修复并实网复测通过：搜索「我不是戏神」返回书籍结果，详情请求进入番茄 book detail API。
   - **`java.getVerificationCode` 不再伪造**：Legado 真实语义是交互式验证码读取；当前 headless 环境明确降级为空并记录日志，不再编造 MD5/salt。
   - **二进制 body 通道已打通**：`base64DecodeToByteArray` 返回 byte-array marker，OkHttp shim 通过 `bodyBase64` / `bodyBytesBase64` 交给 `java.ajax`，Rust 侧按原始字节发送 reqwest body。
   - **已补验收范围**：番茄 toc/content 全链路已通过 `fanqie_source_full_chain` 实网复测。剩余未验收项是 bookInfo 字段完整性和真实交互验证码 UI，不应把这些未验收项反写成搜索或正文仍失败。

## 测试书源列表

| 书源      | 路径                                        | 体积    | 难度  | 验证顺序 |
| --------- | ------------------------------------------- | ------- | ----- | -------- |
| Mock 书源 | 内部 fixture                                | -       | 低    | 1        |
| 书旗小说  | `E:\Book\书旗书源\sqxs260128_0ee680c1.json` | ~4.8 KB | 低-中 | 2        |
| 七猫小说  | `E:\Book\七猫书源\qmxs260128_432b9f7e.json` | ~7 KB   | 中    | 3        |
| 番茄小说  | `E:\Book\番茄书源\fqfix0529_45469384.json`  | ~315 KB | 高    | 4        |
| 番茄短剧  | `E:\Book\番茄短剧\fqdj0719_016377fa4.json`  | ~33 KB  | 特殊  | 5        |

## 兼容矩阵

### Mock 书源

| 能力                                       | 状态        | 备注                                                        |
| ------------------------------------------ | ----------- | ----------------------------------------------------------- |
| import / search / bookInfo / toc / content | strict_pass | route_b_facade + book_source_compat，本地 mock HTTP，全链路 |

### 书旗小说（live_network_pass，2026-06-10）

| 能力     | 状态              | 备注                                                                                                                           |
| -------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| 导入     | strict_pass       | source_compat_import 通过                                                                                                      |
| search   | live_network_pass | 搜索"系统"返回书籍列表                                                                                                         |
| bookInfo | live_network_pass | ruleBookInfo={}，HTTP 成功、字段来自搜索（非 bug）                                                                             |
| toc      | live_network_pass | 4785 章；strict-mode 修复 + base64 URL 解码                                                                                    |
| content  | live_network_pass | **此前 PARTIAL → 已修复**。站点已改为直接返回 JSON 正文；ruleContent 改为三格式兼容（hex(JSON) / 纯 JSON / hex(URL)+key 回退） |

### 七猫小说（live_network_pass，2026-06-10）

| 能力     | 状态              | 备注                                                                                                                                                                       |
| -------- | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 导入     | strict_pass       | source_compat_import 通过                                                                                                                                                  |
| search   | live_network_pass | 搜索"凡人"返回书籍列表                                                                                                                                                     |
| bookInfo | live_network_pass | ruleBookInfo={}，HTTP 成功                                                                                                                                                 |
| toc      | live_network_pass | **此前 BLOCKED → 已解除**。2551 章；根因是 `let device... chapters=...` 在 strict-mode 下 redeclaration 失败，已修 `eval_script`                                           |
| content  | live_network_pass | **此前 BLOCKED → 已解除**。14648 字符；修了 reqwest blocking 在 tokio 上下文 panic + ruleContent 三格式兼容 + bid/cid 从 chapterId 派生（临时规避未绑定的 `book.bookUrl`） |

### 番茄小说（live_network_pass，2026-06-11）

| 能力     | 状态              | 备注                                                                                                                                                                                                                                                                                                                                                              |
| -------- | ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 导入     | strict_pass       | source_compat_import 通过                                                                                                                                                                                                                                                                                                                                         |
| search   | live_network_pass | 2026-06-11 实网复测，搜索「我不是戏神」返回书籍结果                                                                                                                                                                                                                                                                                                               |
| bookInfo | live_network_pass | **2026-06-12 字段完整性已验收**：name=我不是戏神、author=三九音域、intro=431 字、kind=`完结/9.9分/都市高武,都市,穿越`（修复后正确清洗）、wordCount=4003607、coverUrl=真实 https、tocUrl=`data:book_id;base64,...`。`fanqie_source_full_chain` 已对 author/intro/kind/coverUrl 加断言。剩 `lastChapter` 为 None（详情 JSON 未含 `last_chapter_title`，非引擎缺陷） |
| toc      | live_network_pass | 1928 章；无 URL 卷标题已过滤，第一条为真实章节 `第1章 戏鬼回家`                                                                                                                                                                                                                                                                                                   |
| content  | live_network_pass | 第一章正文 3135 字符；正文阶段可从 chapter data URI `info` 恢复 `book.tocUrl`                                                                                                                                                                                                                                                                                     |

#### 番茄书源 JS API 清单（共 49 个唯一 API）

表格状态：✅ = 已实现，🔧 = 本轮已修，⚠️ = 降级 stub，❌ = 依赖 Rhino/Android 无法等价实现

| API                            | 类别         | 状态 | 说明                                                                                                     |
| ------------------------------ | ------------ | ---- | -------------------------------------------------------------------------------------------------------- |
| `java.ajax`                    | HTTP         | ✅   | 完整实现                                                                                                 |
| `java.ajaxAll`                 | HTTP         | ✅   | JS 层封装 `__ajaxAll` 返回 `[{body,string}]`                                                             |
| `java.md5Encode`               | crypto       | ✅   |                                                                                                          |
| `java.md5Encode16`             | crypto       | ✅   |                                                                                                          |
| `java.base64Encode`            | codec        | ✅   |                                                                                                          |
| `java.base64Decode`            | codec        | ✅   |                                                                                                          |
| `java.base64DecodeToByteArray` | codec        | 🔧   | 2026-06-11 修复：返回 byte-array marker，支持 OkHttp/`java.ajax` 二进制 body                             |
| `java.timeFormat`              | time         | ✅   |                                                                                                          |
| `java.timeFormatUTC`           | time         | ✅   |                                                                                                          |
| `java.toast`                   | ui           | ✅   | → tracing::info                                                                                          |
| `java.longToast`               | ui           | ✅   | → tracing::info                                                                                          |
| `java.log`                     | ui           | ✅   | → tracing::info                                                                                          |
| `java.get`                     | http/storage | ✅   | HTTP GET / KV fallback                                                                                   |
| `java.post`                    | http         | ✅   | HTTP POST                                                                                                |
| `java.put`                     | http/storage | ✅   | HTTP PUT / KV fallback                                                                                   |
| `java.getString`               | json         | ✅   | JSONPath extract from input                                                                              |
| `java.androidId`               | device       | ✅   | 生成 UUID（per-session）                                                                                 |
| `java.getVerificationCode`     | ui           | ⚠️   | 2026-06-11 修复：按 Legado 交互验证码语义降级为空并记录日志，不再伪造 MD5/salt                           |
| `java.hexDecodeToString`       | codec        | ✅   | 2026-06-11 修复：hex bytes 按 UTF-8 解码，避免中文正文 Latin-1 双重编码乱码                              |
| `java.encodeURIComponent`      | codec        | ✅   |                                                                                                          |
| `java.decodeURIComponent`      | codec        | ✅   |                                                                                                          |
| `java.encodeURI`               | codec        | ✅   |                                                                                                          |
| `java.decodeURI`               | codec        | ✅   |                                                                                                          |
| `java.now`                     | time         | ✅   |                                                                                                          |
| `java.uuid`                    | device       | ✅   |                                                                                                          |
| `java.startBrowser`            | ui           | ⚠️   | 降级空字符串（非关键，登录后可跳过）                                                                     |
| `java.startBrowserAwait`       | ui           | ⚠️   | 降级空字符串                                                                                             |
| `java.showBrowser`             | ui           | ⚠️   | 降级 false                                                                                               |
| `java.open`                    | ui           | ⚠️   | 降级 false                                                                                               |
| `java.refreshExplore`          | ui           | ⚠️   | 降级 false                                                                                               |
| `java.searchBook`              | ui           | ⚠️   | 降级 false                                                                                               |
| `java.reLoginView`             | ui           | ⚠️   | 降级 false                                                                                               |
| `java.connect`                 | http         | 🔧   | 本轮从 false stub → HTTP CONNECT（Rust 侧）                                                              |
| `java.upConfig`                | storage      | 🔧   | 本轮从 false stub → JS_KV 持久化（Rust 侧）                                                              |
| `java.upLoginData`             | storage      | 🔧   | 本轮从 false stub → JS_KV 持久化（Rust 侧）                                                              |
| `java.getCookie`               | cookie       | ✅   |                                                                                                          |
| `java.removeCookie`            | cookie       | ✅   |                                                                                                          |
| `java.getReadBookConfigMap`    | config       | ✅   | 返回 `{}`（暂无配置项）                                                                                  |
| `java.getThemeConfigMap`       | config       | ✅   | 返回 `{}`                                                                                                |
| `java.getThemeMode`            | config       | ✅   | 返回 0                                                                                                   |
| `java.aesBase64DecodeToString` | crypto       | ✅   | AES-128-CBC-PKCS7                                                                                        |
| `source.getKey`                | source       | ✅   |                                                                                                          |
| `source.bookSourceName`        | source       | ✅   |                                                                                                          |
| `source.loginUrl`              | source       | ✅   |                                                                                                          |
| `source.getVariable`           | source       | ✅   | JS_KV 读取                                                                                               |
| `source.setVariable`           | source       | ✅   | JS_KV 写入                                                                                               |
| `source.putVariable`           | source       | ✅   | JS_KV 写入（别名）                                                                                       |
| `source.getLoginInfo`          | source       | ✅   | Rust 侧检测存在性                                                                                        |
| `source.getLoginInfoMap`       | source       | ✅   | JS 侧封装 `__getLoginInfoJson`（含 get/set/save/toJSON）                                                 |
| `source.putLoginInfo`          | source       | ✅   | JS 侧封装 `__setLoginInfoValue`                                                                          |
| `source.removeLoginHeader`     | source       | 🔧   | 本轮从 false stub → `__clearLoginInfo`                                                                   |
| `source.refreshExplore`        | source       | ⚠️   | 降级 false                                                                                               |
| `cache.get`                    | cache        | ✅   | JS_KV 读取                                                                                               |
| `cache.put`                    | cache        | ✅   | JS_KV 写入                                                                                               |
| `cache.delete`                 | cache        | ✅   | JS_KV 删除                                                                                               |
| `cookie.getKey`                | cookie       | ✅   |                                                                                                          |
| `cookie.removeCookie`          | cookie       | ✅   |                                                                                                          |
| `Packages.okhttp3.*`           | http         | 🔧   | 2026-06-11 修复：RequestBody.create 参数顺序兼容，二进制 body 走 base64 marker，输出标准 `url,{options}` |
| `Packages.cn.hutool.*`         | util         | 🔧   | 本轮新增 DigestUtil/SecureUtil/StrUtil/HexUtil/Base64 shim → 现有 Rust API                               |
| `Packages.android.os.Build.*`  | device       | ✅   | 静态值（generic/LegadoTauri/35/15）                                                                      |
| `JavaImporter`                 | rhino        | 🔧   | 2026-06-11 修复：展开 `with(JavaImporter)` wrapper 并保留块内顶层函数可见性                              |

#### 已解除与剩余未验收点（2026-06-11 实网复测）

实网运行 `fanqie_source_search_and_book_info`（本地导入）已通过：导入、`loginUrl` 初始化、搜索请求、列表解析均跑通，搜索「我不是戏神」返回书籍结果。

本轮已解除：

- `java.getVerificationCode` 伪实现：已改为按 Legado 交互验证码语义降级，不再伪造 MD5/salt。
- OkHttp 二进制 body 缺口：`base64DecodeToByteArray` → `RequestBody.create` → `java.ajax` → reqwest 已有字节级回归测试。
- OkHttp 请求规格缺口：shim 改为标准 Legado `url,{options}`。
- Rhino 兼容缺口：`with(JavaImporter)` 顶层函数可见性、中文未声明全局、旧式 for 循环变量已补回归测试。
- 搜索列表解析缺口：`<js>...</js>$[*]` 已支持 JS 输出后继续执行尾部 JSONPath。

剩余未验收：

- bookInfo 字段完整性；当前测试已确认 tocUrl 和阅读链路可用，但未逐项校验 intro/kind/wordCount 等展示字段。
- 真实交互验证码 UI；headless 场景只能明确降级。
- 外部中转服务仍是实网依赖，不应在最终报告中输出 auth token 或生成的敏感 header 值。

### 番茄短剧

| 能力            | 状态         | 备注                                                 |
| --------------- | ------------ | ---------------------------------------------------- |
| 导入 / 媒体解析 | not_verified | 非标准小说源，按短剧/视频源单独建模（审计 R-P2-004） |

## JS API 实现状态

| JS API                                                       | 书旗 | 七猫 | 番茄 | 实现状态                                                                                                     |
| ------------------------------------------------------------ | ---- | ---- | ---- | ------------------------------------------------------------------------------------------------------------ |
| `java.ajax`                                                  | Y    | Y    | Y    | OK（2026-06-10 改走独立线程，修复 tokio 上下文 panic）                                                       |
| `java.ajaxAll`                                               | -    | -    | Y    | OK                                                                                                           |
| `java.hexDecodeToString`                                     | Y    | Y    | -    | OK（2026-06-11 修复 UTF-8 hex 解码）                                                                         |
| `java.get` / `java.put`                                      | -    | Y    | -    | OK                                                                                                           |
| `java.base64Encode`                                          | Y    | Y    | -    | OK                                                                                                           |
| `java.base64DecodeToByteArray`                               | -    | -    | Y    | OK（2026-06-11 修复二进制 body 通道，避免经 String 损坏字节）                                                |
| `java.getVerificationCode`                                   | -    | -    | Y    | DEGRADED（Legado 交互验证码语义，headless 为空并记录日志，不再伪造 MD5）                                     |
| `java.deviceID`                                              | -    | Y    | -    | OK                                                                                                           |
| `source.getLoginInfoMap`                                     | Y    | Y    | ?    | OK                                                                                                           |
| `source.getVariable/setVariable`                             | -    | -    | Y    | OK                                                                                                           |
| `cookie.getCookie`                                           | Y    | Y    | Y    | OK                                                                                                           |
| `cache.getFromMemory/putMemory`                              | -    | Y    | -    | OK                                                                                                           |
| `this.zdym()` / `jsmy()`                                     | Y    | Y    | -    | OK                                                                                                           |
| `book.bookUrl` / `book.tocUrl`（规则引擎路径绑定 book 对象） | -    | 需要 | Y    | PARTIAL — 番茄 content 路径已从 chapter data URI `info` 恢复 `book.tocUrl`；真实 `book.bookUrl` 仍待通用绑定 |
| `Packages.okhttp3.*`                                         | -    | -    | Y    | OK（2026-06-11 修复 RequestBody 顺序、二进制 body、标准 `url,{options}`）                                    |
| `JavaImporter` / `with(...)`                                 | -    | -    | Y    | OK（2026-06-11 修复 wrapper 展开与函数可见性）                                                               |

状态说明：Y = 书源使用，? = 不确定，OK = 已实现，DEGRADED = 按平台能力明确降级，MISSING = 未实现。

## 已知项目能力缺口（区别于书源规则过期）

- 规则引擎（Legado JSON 源）的真实 `book.bookUrl` 仍未在所有 content/toc 路径完整绑定。番茄 content 已能从章节 data URI 的 `info` 恢复 `book.tocUrl`，七猫仍通过「从 chapterId 派生 bid/cid」规避；后续通用修复需在不改变命令契约的前提下保存/传递书籍上下文。JS 源运行时（非 Legado 规则）路径不受影响。
- TODO：完成 R-P2-007 后，复查七猫 `ruleContent`，优先改回从 `book.bookUrl` 获取 `bid`，仅保留 `chapterId` 派生作为兼容回退；回切后必须重跑 `qimao_source_full_chain`。

## 验证命令

```powershell
# 本地文件导入（单文件走 booksource_import_legacy_json_text，多文件批量走 booksource_import_legacy_json_texts 同核心导入链路）
cargo test -p reader-core --test source_compat_import imports_and_parses_fields -- --ignored --nocapture
# 网络 URL 导入（booksource_import_legacy_json_url 同链路；下载来自各目录 网络导入.txt）
cargo test -p reader-core --test source_compat_import shuqi_network_import_full_chain -- --ignored --nocapture
cargo test -p reader-core --test source_compat_import qimao_network_import_full_chain -- --ignored --nocapture
cargo test -p reader-core --test source_compat_import fanqie_network_import -- --ignored --nocapture
# 本地导入后全链路（search → bookInfo → toc → content）
cargo test -p reader-core --test source_compat_import shuqi_source_full_chain -- --ignored --nocapture
cargo test -p reader-core --test source_compat_import qimao_source_full_chain -- --ignored --nocapture
# 番茄搜索专项
cargo test -p reader-core --test source_compat_import fanqie_source_search_and_book_info -- --ignored --nocapture
# 番茄全链路
cargo test -p reader-core --test source_compat_import fanqie_source_full_chain -- --ignored --nocapture --test-threads=1
```

说明：`source_compat_import` 测试依赖 `E:\Book\书旗书源`、`E:\Book\七猫书源`、`E:\Book\番茄书源`、`E:\Book\番茄短剧` 下的本机私有样本，GitHub Actions 默认跳过。手动验证时需指定具体测试名并加 `--ignored`。多个实网测试同跑时加 `--test-threads=1` 避免并发占用。

## 2026-06-11 交接：番茄引擎兼容（SRC-FANQIE-ENGINE）

背景：用户指令「让本地项目兼容使用上游书源，而不是让上游兼容你」。本轮未修改 `E:\Book\番茄书源\*.json`，只补引擎兼容能力。

本轮已完成：

1. `java.getVerificationCode` 伪实现已移除。对照 `E:\Book\legado-main` 后确认真实语义是交互式验证码读取；headless 场景降级为空并记录日志，不再编造 MD5/salt。
2. `java.base64DecodeToByteArray` 二进制 body 通道已打通。新增 byte-array marker/base64 传递，`java.ajax` 识别 `bodyBase64` / `bodyBytesBase64` 并按原始字节 POST。
3. OkHttp shim 已修：`RequestBody.create` 支持两种参数顺序，输出标准 Legado `url,{options}` 规格。
4. Rhino 兼容已修：`with(JavaImporter)` wrapper 展开后保留顶层函数可见性；中文未声明全局变量和旧式 for 循环变量已兼容。
5. 搜索列表规则已修：`<js>...</js>$[*]` 支持 JS 输出后继续执行尾部 JSONPath。
6. 实网复测已通过：`fanqie_source_search_and_book_info` 搜索「我不是戏神」返回书籍结果。
7. 后续实网复测已通过：`fanqie_source_full_chain` 返回 toc 1928 章，第一条为真实章节 `第1章 戏鬼回家`，content 返回 3135 字符。

当前未结事项：

1. 番茄 bookInfo 展示字段如继续扩展，需用真实样本逐项校验，不得只凭搜索/目录/正文通过就标记字段全量可用。
2. 若涉及真实交互验证码 UI，需单独设计前端交互与 headless 降级边界。

不得做的事：

- 不得改 `E:\Book\番茄书源\*.json` 去迁就引擎（番茄本地 .json 与 CDN 完全一致，必须保持）。
- 不得重新加入编造的 `getVerificationCode` / 校验码。
- 不得在报告或诊断输出中泄露 auth token、生成的设备 token、完整敏感 header 值。

书旗/七猫遗留说明（非引擎问题，无需引擎改动）：本地 `.json` 是相对自身 API 已更新的 `ruleContent`（正文可读），上游 CDN 版本仍过期（正文不可读）。若用户要求「网络导入也能读正文」，唯一正确路径是上游 CDN 更新其 `ruleContent`，或用户改用本地已更新版本——引擎层不得为特定书源猜测/重写其 hexDecode→JSON 逻辑（会污染通用规则引擎，违反总纲第 39.1）。
