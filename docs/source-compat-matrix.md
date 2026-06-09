# Source Compatibility Matrix

书源兼容测试矩阵。记录各本地测试书源在 Tauri 项目中的兼容状态。

## 测试书源列表

| 书源      | 路径                                        | 体积    | 难度  | 验证顺序 |
| --------- | ------------------------------------------- | ------- | ----- | -------- |
| Mock 书源 | 内部                                        | -       | 低    | 1        |
| 书旗小说  | `E:\Book\书旗书源\sqxs260128_0ee680c1.json` | ~4.8 KB | 低-中 | 2        |
| 七猫小说  | `E:\Book\七猫书源\qmxs260128_432b9f7e.json` | ~7 KB   | 中    | 3        |
| 番茄小说  | `E:\Book\番茄书源\fqfix0529_45469384.json`  | ~315 KB | 高    | 4        |
| 番茄短剧  | `E:\Book\番茄短剧\fqdj0719_016377fa4.json`  | ?       | 特殊  | 5        |

## 兼容矩阵

### Mock 书源

| 能力     | 状态                   | 备注                              |
| -------- | ---------------------- | --------------------------------- |
| 导入     | PASS（route_b_facade） | 用 mock HTTP fixture 验证完整链路 |
| search   | PASS（route_b_facade） |                                   |
| bookInfo | PASS（route_b_facade） |                                   |
| toc      | PASS（route_b_facade） |                                   |
| content  | PASS（route_b_facade） |                                   |

### 书旗小说

| 能力     | 状态                        | 备注                                                   |
| -------- | --------------------------- | ------------------------------------------------------ |
| 导入     | PASS（2026-06-09）          | source_compat_import 测试通过                          |
| search   | PASS（2026-06-09 实网验证） | 搜索"系统"成功返回书籍列表                             |
| bookInfo | NOT_CONFIGURED              | ruleBookInfo 为空对象                                  |
| toc      | BLOCKED（JS API）           | @js: 规则依赖 java.base64Encode（已实现，待验证）      |
| content  | BLOCKED（JS API）           | @js: 规则依赖 java.hexDecodeToString（已实现，待验证） |

### 七猫小说

| 能力     | 状态                        | 备注                          |
| -------- | --------------------------- | ----------------------------- |
| 导入     | PASS（2026-06-09）          | source_compat_import 测试通过 |
| search   | PASS（2026-06-09 实网验证） | 搜索"测试"成功，JS API 链路通 |
| bookInfo | BLOCKED（JS API）           | 依赖 java.ajax                |
| toc      | BLOCKED（JS API）           | 依赖 java.ajax                |
| content  | BLOCKED（JS API）           | 依赖 java.ajax、svg/base64    |

### 番茄小说

| 能力     | 状态               | 备注                                    |
| -------- | ------------------ | --------------------------------------- |
| 导入     | PASS（2026-06-09） | source_compat_import 测试通过           |
| search   | BLOCKED（JS API）  | 依赖大量 java._、source._、cookie、变量 |
| bookInfo | BLOCKED（JS API）  | 同上                                    |
| toc      | BLOCKED（JS API）  | 同上                                    |
| content  | BLOCKED（JS API）  | 同上                                    |

### 番茄短剧

| 能力     | 状态   | 备注 |
| -------- | ------ | ---- |
| 导入     | 未验证 |      |
| 媒体解析 | 未验证 |      |

## JS API 依赖矩阵

各书源使用的 JS API 及当前实现状态：

| JS API                   | 书旗 | 七猫 | 番茄 | 实现状态         |
| ------------------------ | ---- | ---- | ---- | ---------------- |
| `java.ajax`              | Y    | Y    | Y    | OK（2026-06-09） |
| `java.ajaxAll`           | -    | -    | Y    | OK（2026-06-09） |
| `java.hexDecodeToString` | Y    | -    | -    | OK（2026-06-09） |
| `source.getLoginInfoMap` | Y    | Y    | ?    | OK（2026-06-09） |
| `source.getVariable`     | -    | -    | Y    | OK（2026-06-09） |
| `source.setVariable`     | -    | -    | Y    | OK（2026-06-09） |
| `cookie.getCookie`       | Y    | Y    | Y    | OK（2026-06-09） |
| `cache.getMemory`        | -    | Y    | -    | OK（2026-06-09） |
| `cache.putMemory`        | -    | Y    | -    | OK（2026-06-09） |
| `java.base64Encode`      | Y    | Y    | -    | OK（2026-06-09） |
| `java.startBrowser`      | -    | Y    | Y    | OK（2026-06-09） |
| `this.zdym()`            | Y    | -    | -    | OK（2026-06-09） |
| `jsmy()`                 | Y    | -    | -    | OK（2026-06-09） |

状态说明：Y = 书源使用, ? = 不确定, OK = 已实现, PARTIAL = 部分实现, MISSING = 未实现

---

最后更新：2026-06-09（Iteration 6：JS API shim 补全）
