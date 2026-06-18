# 2026-06-18 COVER-CACHE-STABILITY

Branch: `master`

## Scope

This iteration focused on book-cover loading stability after large source imports/searches. The previous capability state exposed cover-cache commands to the frontend but left them as unsupported stubs, so cover images fell back to direct network loading and could trigger many repeated browser downloads.

## Code Changes

- `crates/reader-core/src/facade.rs`
  - Implemented cover cache size, clear, and HTTP/HTTPS resolve paths in the shared core.
  - Added per-URL in-flight coalescing so concurrent requests for the same cover produce one network download.
  - Replaced full unbounded body collection with streamed reading capped at 8MB.
  - Writes through a temporary `.part` file and atomically renames to the final hash+extension cache file.

- `src-tauri/src/commands/comic_cover.rs`, `router.rs`, `system.rs`
  - Routed `cover_resolve_cache`, `cover_cache_size`, and `cover_cache_clear` as implemented commands.
  - Set `coverCache` capability to supported while leaving comic page cache commands explicitly unsupported.

- `src-headless/src/main.rs`
  - Added the same cover cache commands to the WS dispatcher.
  - Added `/asset/:encoded` serving for files under `reader_dir`.
  - Enforced the configured headless token on `/asset` when token auth is enabled.

- `src/composables/useFileSrc.ts`, `src/components/BookCoverImg.vue`, `src/components/settings/SectionStorage.vue`
  - Cached cover paths are rendered through the existing local-file abstraction.
  - Browser/headless asset URLs derive from `?ws=` when present, including token propagation.
  - Storage settings can display and clear cover cache size through the capability gate.

## Command Contract

Passed:

```powershell
node scripts\ci\check-command-contract.mjs --json
```

Current snapshot:

```text
frontendTotal=163
registeredTotal=162
bothCount=162
onlyFrontend=["js_eval"]
onlyBackend=[]
frontend_implemented_count=126
registered_implemented_count=126
frontend_unsupported_stub_count=36
registered_unsupported_stub_count=36
```

The implemented set now includes:

```text
cover_cache_clear
cover_cache_size
cover_resolve_cache
```

## Verification

Passed:

```powershell
cargo fmt --all
cmd /c pnpm.cmd exec oxfmt --check src/composables/useFileSrc.ts src/components/BookCoverImg.vue src/components/settings/SectionStorage.vue
cargo test -p reader-core --test cover_cache -- --nocapture
node scripts\ci\check-command-contract.mjs --json
cargo check -p legado-headless
cargo check -p legado-tauri
cmd /c pnpm.cmd exec vue-tsc -p tsconfig.app.json --noEmit
cargo test -p legado-tauri --test ws_router cover_cache_commands_are_routed -- --nocapture
cargo test -p legado-tauri --test ws_router capabilities_get_returns_map -- --nocapture
cmd /c pnpm.cmd build
cmd /c pnpm.cmd build:windows:release
```

Runtime smoke passed:

```text
Headless server: http://127.0.0.1:7789/?ws=ws://127.0.0.1:7789/ws
Browser title: 开源阅读
Initial view: bookshelf rendered, footer shows headless transport
Settings -> Storage: cover cache row visible as supported, current size 0 B
Console: 0 errors, 0 warnings
/asset encoded Windows path smoke: HTTP 200, body asset-ok
```

Known verification note:

```powershell
cmd /c pnpm.cmd lint
```

Still fails at the repository-wide `oxfmt --check .` phase due to pre-existing format issues in 13 untouched files. The files changed in this iteration passed targeted formatting, and `vue-tsc`, Rust checks, command-contract validation, and focused route/cache tests passed.

`pnpm build` passes with existing Vite/Rolldown warnings from `vconsole` direct `eval`, large chunks, and plugin timing diagnostics; these warnings are not introduced by the cover-cache changes.

`build:windows:release` passes with the known Windows linker stdout warning and copies `target\x86_64-pc-windows-msvc\release\legado-tauri.exe` to `构建结果\windows\legado-tauri.exe`.
