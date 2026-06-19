# 2026-06-19 Repository SourceDir Stability Gate

## Scope

This iteration closes the residual online repository gap for multi-directory source installs:

- Carry `sourceDir` through repository sync/install commands.
- Prevent repository updates from reading a matching source in the wrong directory or writing an external-directory source into the default directory.
- Keep Tauri IPC, Tauri WS router, and headless dispatch contracts aligned.
- Verify the rebuilt Windows client still launches and handles the installed-source UI with 1000+ sources.

## Findings

Normal source read/search/update paths already accepted `sourceDir`, but repository commands did not:

- `repository_check_source_sync` always called `read_source(fileName, None)`.
- `repository_install` always saved to the default JS source directory.
- `OnlineSourcesTab.vue` found the correct local source, including `sourceDir`, but dropped that directory when checking or updating.
- `BookSourceInstallDialog.vue` could detect an already installed external source, then overwrite the default directory instead of the detected external path.

This is subtle in a single-source-directory setup, but becomes a real correctness issue once users import or manage many sources across external directories.

## Fixes

- Added optional `sourceDir` to frontend `installFromRepository()` and `checkRepositorySourceSync()`.
- Passed the matched local source directory from online repository sync checks and update actions.
- Passed the matched local source directory from the install confirmation dialog during diff checks and overwrite installs.
- Added optional `source_dir` to Tauri `repository_install` and `repository_check_source_sync` commands.
- Added the same optional `sourceDir` parsing to the Tauri WS router and headless dispatch.
- Updated `reader-core` repository install/sync methods to save/read through the provided source directory when present.

## Validation

Commands:

```powershell
cargo test -p reader-core --test repository -- --nocapture
cargo test -p legado-tauri --test ws_router repository_commands_are_routed -- --nocapture
cargo test -p legado-headless repository_ -- --nocapture
cargo check -p legado-tauri
cargo check -p legado-headless
cmd /c pnpm.cmd lint
git diff --check
cmd /c pnpm.cmd build:windows:release
```

All commands passed. Rust/Tauri builds emitted the known Windows linker stdout warning; the release build also emitted the known vconsole direct-eval, large chunk, plugin timing, and linker warnings while succeeding.

Test coverage added:

- `reader-core` repository test now creates an external source directory, saves an older repository source there, checks sync using `sourceDir`, overwrites that external file through repository install, and asserts no duplicate default-directory file was created.
- Tauri WS repository route test now sends `sourceDir` to `repository_install` and `repository_check_source_sync` and verifies the commands reach command logic rather than failing argument parsing.

Windows desktop smoke:

| Area | Result |
| --- | --- |
| Cold launch | Rebuilt `构建结果\windows\legado-tauri.exe` opened and stayed running; window/UI was ready in 3723ms. |
| Source management | Opened source management; installed-source UI showed `共 1068 个书源，已启用 1034 个`. |
| Filtering | Searching `Dragon` completed in 1229ms and narrowed the list to `共 2 个书源，已启用 2 个`. |
| Stability | No crash, flash exit, or visible error modal during the smoke. |

Source freshness note:

- A local-only scan of installed source files under the app data source directory checked 1076 files.
- A broad stale/error scan matched 140 files / 185 lines, but included noise such as browser user-agent versions.
- A focused degraded-remark scan (`半废|失效|不可用|不能看|需更新|需要更新|过期|校验超时|已废|废弃|无法使用|不能用`) matched 40 files / 50 lines.
- The Windows UI smoke also surfaced the `DragonQuestQB*` sources with remarks saying the source is partly degraded and newest chapters cannot be read.

No external CDN source refresh was triggered during the Windows UI smoke.

## Residual Risk

- Repository bulk install of brand-new sources still intentionally writes to the default JS source directory; this iteration only preserves `sourceDir` when updating or comparing an already matched local source.
- The freshness scan is a local text scan, not a live availability test. Live checks should remain paced and host-aware because the user reported CDN anti-DDoS / temporary blacklist behavior.
