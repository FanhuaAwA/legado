# 2026-06-13 PERF-FRONTEND-PLUGIN-BARREL-CUT

Task ID: `PERF-2026-06-13-FRONTEND-PLUGIN-BARREL-CUT`

## Scope

- Removed `useFrontendPluginsStore` and frontend-plugin type re-exports from `src/stores/index.ts`.
- Removed bookshelf feature store re-exports from `src/stores/index.ts` because `bookshelfUi` depends on the frontend plugin store.
- Replaced `@/stores` barrel imports in `BookshelfView`, bookshelf UI store, bookshelf actions, and `ExtensionsView` with direct store/type imports.
- Did not change plugin runtime behavior, plugin menu behavior, backend commands, dependency versions, or user data.

## Build Observations

- `stores-BWZGu8_n.js`: `16.63 kB`, gzip `5.30 kB`.
- During this iteration, before removing bookshelf feature store re-exports, `stores-Due8udog.js` was `21.49 kB`, gzip `7.16 kB`.
- `stores-*.js` no longer contains `frontendPlugins`, `useFrontendPlugins`, `plugin-action`, or `plugin-cover` strings.
- `frontendPlugins-DlRZ9-mS.js` is a tiny bridge chunk at `0.14 kB`, gzip `0.12 kB`.
- `useFrontendPlugins-BtFZi8GG.js` remains the large async runtime chunk at `1.17 MB`, gzip `509.87 kB`.
- `BookshelfView-D0k5JHUI.js` increased to `130.55 kB`, gzip `40.28 kB`, because bookshelf-only UI store code moved out of the shared stores chunk and into the bookshelf route boundary.
- `dist/index.html` still has 4 `modulepreload` links: `_plugin-vue_export-helper`, `rolldown-runtime`, `vendor-vue-naive`, and `useInvoke`.

## Gates

- `cmd /c pnpm.cmd lint`: PASS.
- `cmd /c pnpm.cmd build`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `node scripts\ci\check-command-contract.mjs --json`: PASS, `frontendTotal=162`, `registeredTotal=161`, `bothCount=161`, `onlyFrontend=["js_eval"]`, `onlyBackend=[]`, `frontend_unsupported_stub_count=39`, `frontend_implemented_count=122`.
- `git diff --check`: PASS.
- `cargo check -p reader-core`: PASS.
- `cargo test -p reader-core`: PASS, all non-ignored reader-core tests passed.
- `cargo check -p legado-tauri`: PASS.

## Remaining Risk

- This round narrows shared barrel boundaries; it does not shrink the frontend plugin runtime itself.
- Bookshelf still uses plugin actions and plugin cover generators, so the plugin bridge remains a bookshelf route dependency.
- Further reductions need a dedicated split inside `useFrontendPlugins` or a lazy menu plugin lookup strategy.
