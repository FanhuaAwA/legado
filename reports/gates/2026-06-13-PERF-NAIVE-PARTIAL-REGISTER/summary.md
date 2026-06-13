# 2026-06-13 PERF-NAIVE-PARTIAL-REGISTER

Task ID: `PERF-2026-06-13-NAIVE-PARTIAL-REGISTER`

## Scope

- Removed the default `import naive from "naive-ui"` / `app.use(naive)` full plugin path from `src/main.ts`.
- Added `src/plugins/naiveComponents.ts` using Naive UI `create({ components })`.
- Registered the 39 globally used Naive components found in Vue templates.
- Did not add `unplugin-vue-components`, change dependency versions, or touch backend/runtime business logic.

## Build Observations

- `vendor-vue-naive-P8C4MHM6.js`: `697.89 kB`, gzip `197.04 kB`.
- Previous round `vendor-vue-naive` was about `1,396 kB`, gzip `378.84 kB`; this round removes roughly half of the Naive/Vue vendor payload.
- Entry chunk: `index-fjOpBFyM.js` is `57.02 kB`, gzip `20.12 kB`.
- Entry CSS remains `index-bSHetTZa.css` at `59.71 kB`, gzip `12.43 kB`.
- `dist/index.html` still has 4 `modulepreload` links: `_plugin-vue_export-helper`, `rolldown-runtime`, `vendor-vue-naive`, and `useInvoke`.
- Remaining large chunks include `useFrontendPlugins-CUUnhuCV.js` at `1.17 MB`, gzip `509.87 kB`, and `vendor-vue-naive` still above the large-chunk threshold.

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

- `naiveComponents.ts` is now a maintenance list and must stay in sync with globally used `<n-*>` template tags.
- This round used a script scan across Vue files and covered 39 unique Naive components, but future template additions can still require updating the list.
- No new auto-import dependency was introduced, so the build remains simpler but does not get automatic component registration.
- `vendor-vue-naive` remains preloaded and above 500 kB; further reductions likely require route-level Naive split strategy or reducing global App shell Naive providers.
