# PERF-2026-06-13-EXTENSION-EXAMPLES-LAZY Gate Summary

## Scope

- Lazy-load built-in extension example scripts from `src/data/pluginExamples/*.js?raw`.
- Keep extension install/save/toggle semantics unchanged.
- Keep frontend plugin runtime APIs, backend command contracts, and private book source samples untouched.

## Implementation

- Replaced 23 static raw imports in `src/data/extensionExamples.ts` with dynamic import loaders.
- Added cached `loadExampleScripts()` so repeated visits reuse the same loaded example metadata and source strings.
- Updated `src/views/ExtensionsView.vue` so example categories, filtering, preview, and install-from-preview operate on loaded example state.
- Triggered example loading only when the examples tab becomes active, with loading and failure UI using existing Naive UI primitives.

## Build Observations

- Previous baseline: `ExtensionsView-BgBpIuYE.js` = `137.93 kB`, gzip `34.64 kB`.
- Current build: `ExtensionsView-BDO7O7EC.js` = `33.35 kB`, gzip `10.42 kB`.
- The 23 example scripts are emitted as independent dynamic chunks, including `reader-ad-cleaner-CKCGe1gc.js` and `tts-edge-read-aloud-CML0VQWv.js`.
- `dist/index.html` still has 4 first-load `modulepreload` links and does not preload example chunks.
- `useFrontendPlugins` and `pluginChineseConverter` chunk ownership remains unchanged from the previous iteration.

## Gates

- `cmd /c node_modules\.bin\oxfmt.cmd src\data\extensionExamples.ts src\views\ExtensionsView.vue src\components\extensions\ExampleCard.vue`: PASS
- `rg -n "EXAMPLE_SCRIPTS" src`: PASS, no remaining references
- `cmd /c pnpm.cmd lint`: PASS
- `cmd /c pnpm.cmd build`: PASS
- `node scripts\ci\check-command-contract.mjs --json`: PASS
- `git diff --check`: PASS, only LF/CRLF working-tree warnings
- `cargo fmt --all -- --check`: PASS
- `cargo check -p reader-core`: PASS
- `cargo test -p reader-core`: PASS
- `cargo check -p legado-tauri`: PASS

## Command Contract Snapshot

```json
{
  "frontendTotal": 162,
  "registeredTotal": 161,
  "bothCount": 161,
  "onlyFrontend": ["js_eval"],
  "onlyBackend": [],
  "frontend_unsupported_stub_count": 39,
  "frontend_implemented_count": 122
}
```

## Residual Risk

- The total example source payload is unchanged; this iteration only changes when that payload is loaded.
- Opening the examples tab performs many small dynamic imports. If request count becomes more important than page-entry chunk size, combine the examples into one lazy bundle in a later iteration.
