# 2026-06-19 Online Source Sync Status Gate

## Scope

This iteration continues the online source repository stability pass:

- Restore the per-card sync status row for installed online sources.
- Fix unreadable repository error text such as `[object Object]`.
- Avoid false UUID mismatch errors when a repository manifest omits `uuid` but the downloaded JS source declares `@uuid`.
- Verify the rebuilt Windows client can launch, load 1000+ installed sources, open the online repository tab, and finish a local repository sync check without UI lockup.

## Findings

The online source card already had sync state helpers in `OnlineSourcesTab.vue`, but `OnlineSourceCard.vue` had the rendered status row commented out. This made installed online sources show only summary counts, with no per-source reason for `synced`, `update`, or `error`.

While testing with a local repository fixture, failed sync checks also surfaced as `[object Object]` because some catch paths converted command errors with `String(e)` instead of the repository-aware formatter.

A second issue appeared in the same fixture: `sourceUuid()` returns the repository identity, falling back to source name when the manifest has no explicit `uuid`. That identity is valid for local matching, but it was also being passed as `expectedUuid` to backend preview/install/sync commands. If the remote JS source declared `@uuid`, the backend correctly compared it to the expected value, but the expected value could be the source name, causing a false mismatch.

## Fixes

- Added explicit sync label/type/hint props to `OnlineSourceCard.vue`.
- Re-enabled the installed-source status row and wired it to the existing sync state helpers.
- Rendered error status text with the existing error emphasis class.
- Replaced remaining repository catch paths with `formatRepositoryError()`.
- Added `sourceExpectedUuid()` so backend UUID validation receives only an explicit manifest UUID.
- Kept `sourceUuid()` / `getBookSourceIdentity()` for local matching and sync keys, preserving name fallback for repositories that do not publish UUIDs.
- Updated install-success matching so no-UUID manifests can still mark a card synced by source name after the install dialog returns a downloaded source UUID.

## Validation

Commands:

```powershell
cmd /c pnpm.cmd exec vue-tsc -p tsconfig.app.json --noEmit
cmd /c pnpm.cmd exec oxfmt src/components/booksource/OnlineSourceCard.vue src/components/booksource/OnlineSourcesTab.vue
cmd /c pnpm.cmd lint
git diff --check
cmd /c pnpm.cmd build:windows:release
```

All commands passed. The release build emitted the known vconsole direct-eval, large chunk, and Windows linker stdout warnings while succeeding.

Windows desktop smoke with a local HTTP repository fixture:

| Area | Result |
| --- | --- |
| Cold launch | Rebuilt `构建结果\windows\legado-tauri.exe` opened and stayed running; window/UI was ready in 3716ms. |
| Large source load | Source management opened and showed `共 1069 个书源，已启用 1035 个` with the temporary fixture included. |
| Online repository | Local fixture repository loaded as `Codex Local Repository` and showed `已安装 1`. |
| Status row | The installed source card rendered `已同步` plus `本地与服务器内容一致，比较时已忽略 @enabled 行`. |
| Error formatting regression | No `[object Object]` text appeared after the formatter fix. The earlier false UUID mismatch disappeared once only explicit manifest UUIDs were sent for validation. |

The local repository service, temporary source file, and temporary app repository config were removed/restored after the smoke test. No external CDN repository requests were used for this UI smoke.

## Residual Risk

- The backend `repository_check_source_sync` command still reads the local source by `fileName` and default source directory only. If a future workflow allows an online repository source to match a local source installed in an extra `sourceDir`, the command should accept and pass `sourceDir`.
- This smoke covered one installed fixture source. A future automated component or browser fixture should cover many installed repository entries to assert status rendering plus request pacing together.
