# 2026-06-19 Repository Request Pacing Gate

## Scope

This iteration continues the large-source stability work on the online source repository path:

- Avoid burst-checking installed online repository sources after a manifest loads.
- Avoid burst-downloading many sources during bulk install/update.
- Avoid redundant remote downloads immediately after a successful repository install/update.
- Avoid repeated full source-list reloads during bulk update.
- Verify the rebuilt Windows client still launches and opens the source management / online source UI.

## Findings

`OnlineSourcesTab.vue` previously had three source-heavy request amplifiers:

- `runInstalledSyncChecks()` marked every installed repository source as `checking` and ran up to 4 remote consistency checks at once.
- `installAll()` used `Promise.allSettled()` over every install target, so many source downloads could start together.
- `updateAll()` used `Promise.allSettled()` over every update target. Each successful item also called `performRepositoryUpdate()`, which reloaded the parent source list and ran another remote sync check after already downloading and installing the same remote source.

These behaviors are risky for CDN/proxy-backed source packs because multiple sources can share the same upstream host. They also match the user's observation that short request bursts may trigger anti-DDoS or temporary blacklist behavior.

## Fixes

- Added a small repository queue helper with explicit concurrency and per-item pauses.
- Installed-source repository sync checks now run with concurrency `1` and a `1200ms` pause.
- Bulk repository install/update transfers now run with concurrency `1` and a `1500ms` pause.
- Sync checks initialize installed entries as `idle`; only the current source is marked `checking`, reducing UI churn on large repository lists.
- Successful install/update now marks that source `synced` locally instead of immediately downloading the same remote source again for comparison.
- Bulk update suppresses per-source reloads and emits one parent `reload` after the batch has at least one successful update.

## Validation

Commands:

```powershell
cmd /c pnpm.cmd exec vue-tsc -p tsconfig.app.json --noEmit
cmd /c pnpm.cmd exec oxfmt --check src/components/booksource/OnlineSourcesTab.vue
git diff --check
cmd /c pnpm.cmd lint
cmd /c pnpm.cmd build:windows:release
```

All commands passed. The release build emitted the known vconsole direct-eval, large chunk, and Windows linker stdout warnings while succeeding.

Build note:

- An earlier release build attempt continued compiling after the tool timeout, updated the target exe, but could not refresh the copied `构建结果\windows` artifact while an older client window was still running.
- The old window was closed through Windows desktop control and `cmd /c pnpm.cmd build:windows:release` then completed successfully, copying the rebuilt exe.

Windows desktop smoke:

| Area | Result |
| --- | --- |
| Cold launch | Window appeared in 1122ms; after 3s the main nav/accessibility tree showed bookshelf, discover, search, source management, settings, frontend `v0.9.0`, and Windows. |
| Source management | Opened and showed `书源管理`, installed/online/test tabs, source search, and `共 1068 个书源，已启用 1034 个`. |
| Online source tab | Opened in 517ms and showed repository controls including `添加仓库`, disabled search, and the current repository format warning. The UI stayed responsive. |

This smoke intentionally did not click bulk install/update or force a broad online sync run, because that would generate exactly the CDN pressure this iteration is designed to reduce.

## Residual Risk

- The queue is frontend-side pacing. Backend repository commands still execute one request per command and rely on callers not to fan them out aggressively.
- The current configured repository in the local app returns a format warning, so this UI smoke verifies page stability and controls but not a successful repository manifest with many installed matches.
- For future proof, a fixture-based UI or component test should exercise `runInstalledSyncChecks()` with many installed sources and assert request order/spacing without real network traffic.
