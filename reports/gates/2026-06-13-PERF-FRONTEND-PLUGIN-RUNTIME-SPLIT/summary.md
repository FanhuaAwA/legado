# PERF-2026-06-13-FRONTEND-PLUGIN-RUNTIME-SPLIT

## Scope

This iteration reduces the static payload of the frontend plugin runtime chunk without changing backend command contracts or plugin execution semantics.

Touched areas:

- `src/composables/useFrontendPlugins.ts`
- `src/data/builtinPlugins.ts`
- `src/features/frontendPlugins/pluginTextUtils.ts`
- `src/features/frontendPlugins/pluginChineseConverter.ts`

Not touched:

- Backend command definitions and capability classification
- Reader-core parsing logic
- User data paths
- Third-party/private book-source samples
- Dependency versions or lockfile

## Key Changes

- Moved the `opencc-js` dependency out of `pluginTextUtils.ts` into `pluginChineseConverter.ts`.
- Added a cached dynamic import for the Chinese converter runtime.
- Preloads the converter before evaluating plugin sources that contain `convertChinese`, preserving the synchronous `api.text.convertChinese(text, mode) => string` plugin API for normal usage.
- Kept a defensive fallback for unusual dynamic plugin calls: it schedules the converter load and returns the original text if the source did not advertise `convertChinese`.
- Changed built-in frontend plugins from a static raw import to `loadBuiltinFrontendPlugins()`, so the MiMo TTS built-in source is loaded during plugin initialization instead of being embedded in the plugin runtime chunk.

## Build Observations

Previous baseline from `PERF-FRONTEND-PLUGIN-BARREL-CUT`:

- `useFrontendPlugins-BtFZi8GG.js`: `1.17 MB`, gzip `509.87 kB`
- `dist/index.html` modulepreload links: `4`

This iteration:

- `useFrontendPlugins-BsTr7j8q.js`: `36.12 kB`, gzip `10.36 kB`
- `pluginChineseConverter-BFpjUyv2.js`: `1,122.13 kB`, gzip `494.29 kB`, loaded only through dynamic import
- `tts-xiaomi-mimo-v25-2bfbZ9OA.js`: `13.90 kB`, gzip `3.96 kB`, loaded only through dynamic import
- `dist/index.html` modulepreload links remain `4`: `_plugin-vue_export-helper`, `rolldown-runtime`, `vendor-vue-naive`, `useInvoke`
- The entry chunk remains essentially unchanged: `index-BsSSkfpI.js` `57.05 kB`, gzip `20.14 kB`

Static verification:

- `dist/index.html` does not preload `pluginChineseConverter`, `tts-xiaomi`, or `useFrontendPlugins`.
- `useFrontendPlugins` contains only dynamic import references for the Chinese converter and built-in MiMo source, not the OpenCC dictionary payload.

## Gates

- `cmd /c node_modules\.bin\oxfmt.cmd src\data\builtinPlugins.ts src\composables\useFrontendPlugins.ts src\features\frontendPlugins\pluginChineseConverter.ts src\features\frontendPlugins\pluginTextUtils.ts`: PASS
- `cmd /c pnpm.cmd lint`: PASS, 0 warnings / 0 errors
- `cmd /c pnpm.cmd build`: PASS
- `node scripts\ci\check-command-contract.mjs --json`: PASS
  - `frontendTotal=162`
  - `registeredTotal=161`
  - `bothCount=161`
  - `onlyFrontend=["js_eval"]`
  - `onlyBackend=[]`
  - `frontend_unsupported_stub_count=39`
  - `frontend_implemented_count=122`
- `cargo fmt --all -- --check`: PASS
- `git diff --check`: PASS, only Windows LF/CRLF working-copy warnings
- `cargo check -p reader-core`: PASS
- `cargo test -p reader-core`: PASS, all non-ignored tests passed
- `cargo check -p legado-tauri`: PASS

## Remaining Risks

- The OpenCC payload is still large by nature; this iteration moves it behind a capability-driven dynamic import rather than shrinking the dictionary itself.
- Very unusual plugin code that calls `api.text["convert" + "Chinese"](...)` without containing the literal `convertChinese` can hit the defensive fallback on its first synchronous call. Normal plugin code and the bundled example use the literal and are preloaded before evaluation.
- `vendor-vue-naive` remains the largest first-viewport preload chunk.
- `ExtensionsView` still carries the extension example gallery payload; that is a plugin-management-page cost, not an app-shell preload.
