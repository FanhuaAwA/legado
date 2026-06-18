# 2026-06-18 HEADLESS-REPOSITORY

## Scope

- Expose the already implemented repository/source-update command domain through `legado-headless`.
- Keep browser/WS mode capability reporting aligned with Tauri and the frontend fallback capability table.
- Verify source freshness workflows can run in browser mode instead of being disabled by `capabilities_get`.

## Changes

- Routed the following headless commands to `reader-core`:
  - `repository_fetch`
  - `repository_install`
  - `repository_preview_source`
  - `repository_check_source_sync`
  - `booksource_check_update`
  - `booksource_apply_update`
- Changed headless `capabilities_get.repository` from unsupported to supported.
- Added headless unit tests using a local HTTP repository fixture:
  - manifest fetch
  - remote JS source preview
  - install
  - sync consistency check
  - `@updateUrl` update check
  - update apply while preserving local `@enabled`

## Verification

```powershell
cargo fmt --all
cargo test -p legado-headless repository_ -- --nocapture
cargo test -p legado-headless -- --nocapture
cargo check -p legado-headless
cargo check -p legado-tauri
node scripts\ci\check-command-contract.mjs --json
cmd /c pnpm.cmd exec oxfmt --check .
cmd /c pnpm.cmd lint
cargo build -p legado-headless
```

## Runtime Smoke

- Started `legado-headless` on `127.0.0.1:7791` with isolated data dir `reader-data/codex-repo-smoke`.
- Opened `http://127.0.0.1:7791/?ws=ws://127.0.0.1:7791/ws` through Playwright CLI.
- Opened `书源管理 -> 在线书源`.
- Verified the online repository toolbar showed enabled `获取列表`, `添加仓库`, `移除`, and `批量操作` entries instead of a repository unsupported warning.
- Browser console reported `Errors: 0, Warnings: 0` and `WebSocket 已连接`.

## Result

- Browser/headless mode can now use the same online source repository and JS-source update workflows as Tauri.
- Command contract count is unchanged because these commands were already registered and implemented in Tauri: `frontend_implemented_count=126`, `frontend_unsupported_stub_count=36`.

## Remaining Work

- `syncWebdav` still is not exposed by headless.
- Strict cross-device LAN validation remains separate from the local loopback smoke.
