# 2026-06-18 EXTERNAL-OPEN-WRAPPER

## Scope

- Centralize external URL opening behind `src/composables/useExternalOpen.ts`.
- Remove direct `@tauri-apps/plugin-opener` imports from business UI components.
- Preserve browser/headless behavior for source URLs, book detail URLs, reader original-page links, video URL details, and service-mode local URLs.

## Changes

- Added `useExternalOpen.ts` with Tauri native opener support and browser fallback.
- Replaced opener calls in book source tabs, explore/detail UI, reader top bar, reader TOC/detail, legacy comment browser actions, video detail rows, and service-mode settings.
- Found and fixed a browser-specific false negative: `window.open(url, "_blank", "noopener,noreferrer")` can open a new tab while returning `null`. The browser fallback now uses a temporary anchor with `rel="noopener noreferrer"` and returns success after dispatching the click.

## Verification

```powershell
rg -n "@tauri-apps/plugin-opener|openUrl\(" src
cmd /c pnpm.cmd lint
node scripts\ci\check-command-contract.mjs --json
cmd /c pnpm.cmd build
```

Results:

- `@tauri-apps/plugin-opener` appears only in `src/composables/useExternalOpen.ts`.
- `pnpm lint` passed: formatting, oxlint type-aware checks, and `vue-tsc`.
- Command contract unchanged: `frontendTotal=163`, `registeredTotal=162`, `onlyFrontend=["js_eval"]`, `onlyBackend=[]`, implemented `126`, unsupported stubs `36`.
- `pnpm build` passed with existing vconsole direct-eval and chunk-size warnings only.

## Browser Smoke

- Served the rebuilt `dist` through `target/debug/legado-headless.exe` on `127.0.0.1:7796`.
- Opened `http://127.0.0.1:7796/?ws=ws://127.0.0.1:7796/ws`.
- Loaded installed source tab, online source tab, settings, and service-mode UI.
- Imported the built `useExternalOpen` chunk in the browser and verified:

```json
{ "empty": false, "opened": true }
```

- Final Playwright console check: `Errors: 0`, `Warnings: 0`.

## Remaining Risk

- This pass verifies browser/headless fallback behavior and build-time isolation. Native OS opener behavior still relies on Tauri's `@tauri-apps/plugin-opener`, now isolated to the wrapper.
