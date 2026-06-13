# 2026-06-13 PERF-APP-SHELL-SPLIT

Task ID: `PERF-2026-06-13-APP-SHELL-SPLIT`

## Scope

- Replaced App shell imports from the `stores/index.ts` barrel with direct store imports.
- Moved the App startup book-source limit check behind a dynamic `bookSource` store import.
- Loaded `BookSourceInstallDialog` only when the Legado deep-link dialog is shown.
- Converted global music player components to async components.
- Split TTS playback state into `useTtsState` so the App shell can observe playback without loading the full TTS/plugin runtime.
- Moved the `scriptBridge` debug-log fallback in `GlobalFeedbackMirror` behind a failure-path dynamic import.

## Build Observations

- Entry chunk: `index-Dm9WUU1T.js` is `56.55 kB`, gzip `19.82 kB`.
- Entry CSS: `index-bSHetTZa.css` is `59.71 kB`, gzip `12.43 kB`.
- `_plugin-vue_export-helper-CHAozXME.js` is now a tiny helper chunk at `0.08 kB`.
- `dist/index.html` keeps 4 `modulepreload` links: `_plugin-vue_export-helper`, `rolldown-runtime`, `vendor-vue-naive`, and `useInvoke`.
- `useFrontendPlugins`, `bookSource`, `useBookSource`, `BookSourceInstallDialog`, `musicPlayer`, and `scriptBridge` no longer appear as entry static imports or HTML preloads.
- `useFrontendPlugins-DD6nSmlQ.js` remains a large async chunk at `1.17 MB`, gzip `509.87 kB`, but is no longer pulled into the App shell entry graph.

## Gates

- `cmd /c node_modules\.bin\oxfmt.cmd src\App.vue src\composables\useTts.ts src\composables\useTtsState.ts`: PASS.
- `cmd /c pnpm.cmd lint`: PASS.
- `cmd /c pnpm.cmd build`: PASS.
- `node scripts\ci\check-command-contract.mjs --json`: PASS, `frontendTotal=162`, `registeredTotal=161`, `bothCount=161`, `onlyFrontend=["js_eval"]`, `onlyBackend=[]`, `frontend_unsupported_stub_count=39`, `frontend_implemented_count=122`.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.
- `cargo check -p reader-core`: PASS.
- `cargo test -p reader-core`: PASS, all non-ignored reader-core tests passed.
- `cargo check -p legado-tauri`: PASS.

## Remaining Risk

- Global music components are now async chunks; they can still load shortly after initial render when visible, but they are no longer entry static dependencies.
- The App startup book-source count check now pays one dynamic import after mount.
- TTS keep-awake now observes shared playback state through `useTtsState`; the full TTS composable still loads from reader controls.
- `vendor-vue-naive` remains the largest first-load dependency and needs a separate dependency-registration audit.
