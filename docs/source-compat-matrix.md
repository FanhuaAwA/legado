# Source Compatibility Matrix

记录各本地测试书源在 Tauri 项目中的兼容状态。

最后实测：2026-06-10（实网 live_network_pass，命令见文末）。

状态枚举：`strict_pass`（mock/fixture）/ `live_network_pass`（实网通过）/ `live_network_ignored`（默认跳过）/ `partial` / `blocked_by_source_rule` / `blocked_by_platform` / `blocked_by_js_api` / `not_verified`。

## 测试书源列表

| 书源 | 路径 | 体积 | 难度 | 验证顺序 |
| --- | --- | --- | --- | --- |
| Mock 书源 | 内部 fixture | - | 低 | 1 |
| 书旗小说 | `E:\Book\书旗书源\sqxs260128_0ee680c1.json` | ~4.8 KB | 低-中 | 2 |
| 七猫小说 | `E:\Book\七猫书源\qmxs260128_432b9f7e.json` | ~7 KB | 中 | 3 |
| 番茄小说 | `E:\Book\番茄书源\fqfix0529_45469384.json` | ~315 KB | 高 | 4 |
| 番茄短剧 | `E:\Book\番茄短剧\fqdj0719_016377fa4.json` | ~33 KB | 特殊 | 5 |

## 兼容矩阵

### Mock 书源

| 能力 | 状态 | 备注 |
| --- | --- | --- |
| import / search / bookInfo / toc / content | strict_pass | route_b_facade + book_source_compat，本地 mock HTTP，全链路 |

### 书旗小说（live_network_pass，2026-06-10）

| 能力 | 状态 | 备注 |
| --- | --- | --- |
| 导入 | strict_pass | source_compat_import 通过 |
| search | live_network_pass | 搜索"系统"返回书籍列表 |
| bookInfo | live_network_pass | ruleBookInfo={}，HTTP 成功、字段来自搜索（非 bug） |
| toc | live_network_pass | 4785 章；strict-mode 修复 + base64 URL 解码 |
| content | live_network_pass | **此前 PARTIAL → 已修复**。站点已改为直接返回 JSON 正文；ruleContent 改为三格式兼容（hex(JSON) / 纯 JSON / hex(URL)+key 回退） |

### 七猫小说（live_network_pass，2026-06-10）

| 能力 | 状态 | 备注 |
| --- | --- | --- |
| 导入 | strict_pass | source_compat_import 通过 |
| search | live_network_pass | 搜索"凡人"返回书籍列表 |
| bookInfo | live_network_pass | ruleBookInfo={}，HTTP 成功 |
| toc | live_network_pass | **此前 BLOCKED → 已解除**。2551 章；根因是 `let device... chapters=...` 在 strict-mode 下 redeclaration 失败，已修 `eval_script` |
| content | live_network_pass | **此前 BLOCKED → 已解除**。14648 字符；修了 reqwest blocking 在 tokio 上下文 panic + ruleContent 三格式兼容 + bid/cid 从 chapterId 派生（临时规避未绑定的 `book.bookUrl`） |

### 番茄小说

| 能力 | 状态 | 备注 |
| --- | --- | --- |
| 导入 | strict_pass | source_compat_import 通过 |
| search / bookInfo / toc / content | not_verified | 依赖大量 java.*/source.*/cookie/变量/设备注册；reqwest 线程桥已就绪，待逐项验证（审计 R-P2-003） |

### 番茄短剧

| 能力 | 状态 | 备注 |
| --- | --- | --- |
| 导入 / 媒体解析 | not_verified | 非标准小说源，按短剧/视频源单独建模（审计 R-P2-004） |

## JS API 实现状态

| JS API | 书旗 | 七猫 | 番茄 | 实现状态 |
| --- | --- | --- | --- | --- |
| `java.ajax` | Y | Y | Y | OK（2026-06-10 改走独立线程，修复 tokio 上下文 panic） |
| `java.ajaxAll` | - | - | Y | OK |
| `java.hexDecodeToString` | Y | Y | - | OK |
| `java.get` / `java.put` | - | Y | - | OK |
| `java.base64Encode` | Y | Y | - | OK |
| `java.deviceID` | - | Y | - | OK |
| `source.getLoginInfoMap` | Y | Y | ? | OK |
| `source.getVariable/setVariable` | - | - | Y | OK |
| `cookie.getCookie` | Y | Y | Y | OK |
| `cache.getFromMemory/putMemory` | - | Y | - | OK |
| `this.zdym()` / `jsmy()` | Y | Y | - | OK |
| `book.bookUrl`（规则引擎路径绑定 book 对象） | - | 需要 | ? | MISSING — 规则引擎 content 路径未绑定真实 book 上下文，当前由书源规则侧规避 |

状态说明：Y = 书源使用，? = 不确定，OK = 已实现，MISSING = 未实现。

## 已知项目能力缺口（区别于书源规则过期）

- 规则引擎（Legado JSON 源）的 content/toc JS 执行未绑定 `book` 对象（`book.bookUrl` 等为 undefined）。本轮七猫通过「从 chapterId 派生 bid/cid」在书源侧临时规避，并让段评增强失败时不阻断正文；通用修复需在 rule_engine content 路径绑定当前 book 上下文。JS 源运行时（非 Legado 规则）路径不受影响。
- TODO：完成 R-P2-007 后，复查七猫 `ruleContent`，优先改回从 `book.bookUrl` 获取 `bid`，仅保留 `chapterId` 派生作为兼容回退；回切后必须重跑 `qimao_source_full_chain`。

## 验证命令

```powershell
cargo test -p reader-core --test source_compat_import shuqi_source_full_chain -- --ignored --nocapture
cargo test -p reader-core --test source_compat_import qimao_source_full_chain -- --ignored --nocapture
```

说明：`source_compat_import` 测试依赖 `E:\Book\书旗书源`、`E:\Book\七猫书源`、`E:\Book\番茄书源`、`E:\Book\番茄短剧` 下的本机私有样本，GitHub Actions 默认跳过。手动验证时需指定具体测试名并加 `--ignored`。
