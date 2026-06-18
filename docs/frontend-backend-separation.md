# 前后端分离架构纪律（强制）

本文档是前后端分离的权威约束文件。所有后续 AI 在新增前端调用、新增后端命令、修改传输层或事件系统之前，必须先读本文档。与本文档冲突的实现一律视为缺陷。

建立日期：2026-06-10。事实基线以当日源码实测为准，状态变化只追加注记，不改写历史。

---

## 1. 目标部署形态

本项目必须同时支持两种部署形态，开发期以形态 A 为主，但任何代码都不得阻断形态 B：

```text
形态 A（开发期 / 桌面端，当前可用）：
  前端 (Vue dist) 与后端 (Rust) 打包在同一个 Tauri 壳内，通过 Tauri IPC 通信。

形态 B（远期目标，必须支持）：
  后端独立部署在服务器上（无头，无窗口、无系统对话框）。
  前端是纯静态资源（浏览器 / CDN / 任意壳），通过 WebSocket 连接远端后端。
  一个后端可服务多个前端客户端。
```

判断任何改动是否合规的唯一标准：**这段代码在形态 B 下还能工作或能正确降级吗？**

## 2. 当前事实基线（2026-06-10 实测）

已就绪：

- 前端三模传输层已完成：`src/composables/useTransport.ts` 在 Tauri IPC / Harmony 桥 / WebSocket 之间透明切换。浏览器模式自动探测 `ws://<host>:7688/ws`，支持 `?ws=` URL 参数指定自定义后端地址，invoke / listen 语义与 Tauri 一致，含超时（默认 35s）与指数退避重连。
- 统一调用入口已完成：`useInvoke.ts`（invokeWithTimeout）、`useEventBus.ts`（listen/emit）、`useFileSrc.ts`（本地文件 URL 转换）、`useExternalOpen.ts`（外部链接打开）、`useEnv.ts`（环境检测）。
- `crates/reader-core` 不依赖 Tauri，可被任意服务端二进制直接链接。
- 能力声明命令 `capabilities_get` 已注册，可作为按传输方式声明能力的载体。
- **2026-06-12 注记：FORMB-ACCEPT 本机 headless loopback 已通过。**`src-headless` 独立后端托管 `dist` + `/ws`，浏览器打开 `http://127.0.0.1:7788/?ws=ws://127.0.0.1:7788/ws` 后启动控制台 0 error / 0 warning，并用同一 WS 协议跑通「书源保存/列表 → 搜索 → 详情 → 加书架 → 目录 → 正文 → 进度保存」。证据见 `reports/gates/2026-06-12-FORMB-ACCEPT-headless-loopback/summary.md`；自动回归见 `src-headless/src/main.rs::tests::formb_accept_headless_dispatch_chain`。

缺失（详见审计文档 R-P2-008）：

- ~~`src-tauri` 没有任何 WebSocket 命令服务端~~ **2026-06-10 注记：阶段 1+2 试点已落地**——`src-tauri/src/commands/router.rs`（单一分发入口 `cmd + args(JSON) → result(JSON)`，复用原 `#[tauri::command]` 函数零复制，match 即白名单，62 命令入围）+ `src-tauri/src/ws_server.rs`（应用内 WS 服务端，127.0.0.1:7688 `/ws`，事件转发）。集成测试与真实 exe 实连冒烟证据：`reports/gates/2026-06-10-2051-R-P2-008-ws-pilot/summary.md`。
- 仍缺失：严格跨物理机器/LAN 的形态 B 实测（第 7 节「另一台机器」字面要求）。本机 loopback 已验证业务闭环；如需完成跨机实证，使用 `src-headless` 的 `--bind 0.0.0.0 --token <token>` 启动并从第二台设备以 `?ws=ws://<host>:<port>/ws?token=<token>` 连接复跑同一闭环。
- `src-tauri` 应用内 WS 服务端仍默认只绑定 `127.0.0.1:7688`；LAN 暴露应优先走 `src-headless`，桌面壳内对外暴露仍需单独立项。

## 3. WS 协议契约

协议定义以 `src/composables/useTransport.ts` 头部注释为唯一权威。摘要：

```json
客户端 → 服务器：{ "type": "invoke", "id": "uuid", "cmd": "booksource_list", "args": {} }
服务器 → 客户端：{ "type": "response", "id": "uuid", "data": ... }   // 失败时带 "error"
服务器 → 客户端：{ "type": "event", "event": "rust:log", "payload": ... }
```

- 默认端口 7688，路径 `/ws`，`?ws=` 参数可覆盖完整地址。
- 已知协议缺口：WS 模式下 `transportEmit` 仅在本地广播，协议尚无「客户端 → 服务器」事件通道。新增依赖前端 emit 到后端的功能前，必须先扩协议并同步改前后端与本文档。
- 修改协议字段、端口、路径中的任何一项，必须同轮更新：useTransport.ts、未来的 WS 服务端、本文档第 3 节。

## 4. 硬约束（违反即视为本轮失败）

1. **前端业务代码禁止直接 import `@tauri-apps/api`**。所有后端调用走 `invokeWithTimeout`（useInvoke.ts），所有后端事件走 `useEventBus` / `transportListen`，本地文件展示走 `useFileSrc`，外部链接打开走 `useExternalOpen`。允许的例外仅限：
   - 封装层自身（useTransport.ts、useEventBus.ts、useEnv.ts、useFileSrc.ts、useExternalOpen.ts）。
   - 窗口控制等桌面壳独占行为（如 TitleBar.vue、privacyMode.ts 的 `getCurrentWindow`），必须有 `isTauri` 守卫，且非 Tauri 环境下静默降级、不报错、不阻断功能。
   - `src/utils/logger.ts` 的 `frontend_log` 链路（评估结论见第 5 节，不得据此扩大例外范围）。
2. **后端业务逻辑必须写在 `crates/reader-core`**。`src-tauri` 的命令函数只做参数解析与转发。禁止在 `#[tauri::command]` 函数体里写只有 Tauri 壳能执行的业务逻辑（依赖 AppHandle / 窗口 / 系统对话框的命令除外，但它们必须可被能力门禁排除）。
3. **命令参数与返回值必须 JSON 可序列化**，不得携带本机句柄。后端返回的本机绝对路径只能用于「桌面壳内打开/定位」类场景，前端不得把它当作可直接加载的资源 URL（必须走 useFileSrc 或后端流式接口）。
4. **桌面独占能力必须经 `capabilities_get` 声明**。依赖系统对话框（pick_dir / pick_save_path）、打开资源管理器、VSCode、原生 TTS、窗口控制、本机 WebView（browser_probe）的功能，在 transport = websocket 时必须声明不可用，前端按能力门禁隐藏或禁用入口，不得让用户点击后报错。
5. **事件名与 payload 是跨进程契约**。后端事件 payload 必须 JSON 可序列化；改事件名或 payload 结构等同于改协议，必须全链路同步。
6. **远程部署安全底线**：WS 服务端默认只绑定 `127.0.0.1`；对外暴露必须显式开启，且必须有鉴权 token、命令白名单；`js_eval` 永久阻断（见 command-matrix.md）。公网部署必须走 wss/TLS（可由反向代理承担）。
7. **新增任何前后端交互**，必须自问第 1 节的判定标准；做不到形态 B 兼容的，必须在能力门禁中声明并在 `docs/ai-task-status.md` 登记原因。

## 5. 已知违规与缺口登记

| 位置                                                                    | 问题                                                                                                                                                                                             | 处置                                                                                                                                                                                                                                                                                                                                                                                       |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/stores/prefetch.ts`（setupManualListeners / setupSilentListeners） | 直接 `import("@tauri-apps/api/event")` 监听 `shelf:prefetch-*`，catch 回退仅覆盖 Harmony 的 DOM CustomEvent 路径；WS 模式下事件经传输层分发，不触发 DOM 事件，预取进度会静默丢失                 | **已修复（2026-06-10 R-P2-011）**：环境分流改为「鸿蒙 → DOM CustomEvent（Index.ets 推送路径不变）；Tauri / WS → useEventBus 统一事件层」，与 shellStatus.ts 既有用法一致                                                                                                                                                                                                                   |
| 预取进度链路（R-P2-012，非分离专属但在试点中发现）                      | 前端调用 `bookshelf_prefetch_chapters` 发顶层键 `payload`，后端参数名是 `request`，按键名取参必失败；且全仓库无任何代码 emit `shelf:prefetch-progress` / `shelf:prefetch-done`，前端监听是死路径 | **已修复（2026-06-18 PREFETCH-WS-EVENTS）**：Tauri IPC / Tauri WS / headless WS 均走同一预取进度事件契约；WS router 兼容 `{ payload: ... }` 与直接 payload；headless 预取注册可取消 token 并推送 `shelf:prefetch-progress` / `shelf:prefetch-done`。证据见 `reports/gates/2026-06-18-PREFETCH-WS-EVENTS/summary.md`。                                                                      |
| 业务组件外部链接打开                                                    | 多个业务组件直接 import 或动态 import `@tauri-apps/plugin-opener`，浏览器/headless 形态下外链打开语义分散；`noopener,noreferrer` 的 `window.open` 返回值还会导致“已打开但误判失败”               | **已修复（2026-06-18 EXTERNAL-OPEN-WRAPPER）**：新增 `useExternalOpen.ts` 作为唯一 opener 封装，业务组件只调用封装层；浏览器分支改用临时 `<a rel="noopener noreferrer" target="_blank">` 触发打开，避免 `window.open` 返回 `null` 的误报。证据见 `reports/gates/2026-06-18-EXTERNAL-OPEN-WRAPPER/summary.md`。                                                                             |
| `src/utils/logger.ts`（sendToRust）                                     | 直接 import invoke，非 Tauri 环境降级 console；WS 模式下前端日志不进后端日志文件                                                                                                                 | **评估后保留直连（2026-06-10 R-P2-011 结论，列入第 4 节例外）**：(1) 日志是传输层自身的底层依赖，改走 transportInvoke 会形成 log → transport 内部 log → frontend_log 的放大回路；(2) WS 多客户端把前端日志汇入同一服务器日志会互相污染，浏览器端日志去向应为 DevTools console 与 useRemoteDebug 通道；(3) Tauri 模式行为不变。若未来 R-P2-008 需要远端收集前端日志，单独立项并解决回路问题 |

## 6. 实施路线（与审计文档 R-P2-008 对齐，按序执行）

1. **命令路由收口**：把 `#[tauri::command]` 函数的业务体收口为可复用的命令分发入口（`cmd + args(JSON) → result(JSON)`），Tauri IPC 与 WS 共用同一函数体，禁止复制两套命令体。——**2026-06-10 试点完成**（`commands/router.rs`，扩白名单时在 match 中追加并补集成测试）。
2. **应用内 WS 服务端**：在既有 tokio 运行时上实现 `/ws`，按第 3 节协议分发命令与事件推送（事件复用现有事件名）。——**2026-06-10 试点完成**（`ws_server.rs`；注意：后端新增事件名必须同步追加到 `FORWARDED_EVENTS`，Tauri v2 无全量事件监听 API）。
3. **安全边界**：鉴权 token、绑定地址控制（默认 127.0.0.1，LAN 暴露需显式开启）、命令白名单、按 transport 的能力声明。——白名单已就位（router match）、默认仅绑定 127.0.0.1；token 与 LAN 开关未做。
4. **独立无头服务端二进制**：`src-headless` 已存在，提供 axum 静态 dist 托管 + `/ws` 命令服务，直接链接 `reader-core`。2026-06-12 已补齐浏览器启动、前端存储、书源、书架、章节、正文、进度保存所需命令并完成本机 headless loopback 闭环；2026-06-18 已补齐在线仓库、`@updateUrl` 更新命令域和 WebDAV 同步命令域。剩余为跨物理机器/LAN 实测与其他高级命令域持续补齐。

## 7. 验收标准

形态 B 视为达成，当且仅当：纯浏览器前端（非 Tauri 壳）通过 `?ws=` 连接部署在另一台机器上的后端，能完成「书源列表 → 搜索 → 加入书架 → 章节目录 → 正文阅读 → 进度保存」闭环，且桌面独占功能的入口被能力门禁正确隐藏，全程无未捕获报错。

2026-06-12 本机 headless loopback 验收已通过：纯浏览器连接独立 `legado-headless` 后端，业务闭环和页面启动均通过；严格「另一台机器」实测仍需外部设备或 LAN 环境补跑。

## 8. 相关文件索引

```text
src/composables/useTransport.ts   传输层 + WS 协议权威定义
src/composables/useInvoke.ts      统一命令调用入口
src/composables/useEventBus.ts    统一事件入口
src/composables/useFileSrc.ts     本地文件 URL 转换
src/composables/useExternalOpen.ts 外部链接打开封装
src/composables/useEnv.ts         isTauri / isHarmonyNative 环境检测
src/composables/useCapabilities.ts 能力门禁
src-tauri/src/commands/mod.rs     命令注册表（generate_handler!）
crates/reader-core/               业务核心（无 Tauri 依赖）
docs/command-matrix.md            命令契约实测矩阵
E:\Book\legado-tauri-ai-iteration-plan.md 第 60 节   手册级纪律入口
E:\Book\legado-tauri-mandatory-completion-audit.md R-P2-008 / R-P2-011   任务登记
```
