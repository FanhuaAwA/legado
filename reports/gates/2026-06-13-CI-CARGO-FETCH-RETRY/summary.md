# 2026-06-13 CI-CARGO-FETCH-RETRY

任务 ID：`CI-2026-06-13-CARGO-FETCH-RETRY`

## 背景

用户报告 GitHub Actions 在 2026-06-13 01:00 左右失败：

```text
cargo check -p reader-core
failed to get `cipher` as a dependency of package `aes v0.8.4`
download of ci/ph/cipher failed
curl failed
[56] Failure when receiving data from the peer
```

结论：这是 crates.io registry 下载链路 reset，不是 reader-core 编译错误。

## 范围

允许修改：

- `.github/workflows/quality-gate.yml`
- `docs/ai-task-status.md`
- `docs/ai-iteration-log.md`
- `reports/gates/2026-06-13-CI-CARGO-FETCH-RETRY/summary.md`

不触碰：Rust 业务代码、前端业务代码、依赖版本、Cargo.lock、书源解析、Windows/Android 发布产物。

## 变更

- `quality-gate.yml` 增加 Cargo 网络环境变量：
  - `CARGO_NET_RETRY=10`
  - `CARGO_HTTP_TIMEOUT=120`
  - `CARGO_HTTP_MULTIPLEXING=false`
  - `CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`
- 增加 `actions/cache@v4`，缓存 `~/.cargo/registry` 和 `~/.cargo/git`。
- 增加 `Fetch Cargo dependencies` 步骤，在 Rust check/test 前执行 `cargo fetch --locked`，最多 3 次重试，失败间隔 20s/40s。

## Gate

| 命令                                                | 结果                                                                                                   |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `cmd /c node_modules\.bin\oxfmt.cmd --check .`      | PASS                                                                                                   |
| `git diff --check`                                  | PASS，仅 Windows LF/CRLF 工作区提示                                                                    |
| `node scripts/ci/check-command-contract.mjs --json` | PASS，`162/161/161`，`onlyFrontend=["js_eval"]`，`onlyBackend=[]`，stub `39`，implemented `122`        |
| `cargo fetch --locked`                              | PASS，按 lockfile 下载缺失依赖成功                                                                     |
| `cmd /c pnpm.cmd lint`                              | PASS，0 warnings / 0 errors                                                                            |
| `cmd /c pnpm.cmd build`                             | PASS，仅既有 Vite warning：`vconsole` direct eval、大 chunk、`useTransport` ineffective dynamic import |
| `cargo fmt --all -- --check`                        | PASS                                                                                                   |
| `cargo check -p reader-core`                        | PASS                                                                                                   |
| `cargo check -p legado-tauri`                       | PASS                                                                                                   |
| `cargo test -p reader-core`                         | PASS，全部非 ignored 测试通过                                                                          |

## 后续观察

推送后观察 GitHub Actions 新一轮 `Quality Gate`。若仍遇到 crates.io 下载 reset，再把 Rust check/test 步骤包进 retry wrapper，或评估 registry 镜像/缓存服务。
