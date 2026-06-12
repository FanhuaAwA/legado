# 2026-06-13 UI-READER-SETTINGS-LAYOUT

## Scope

- Fixed the reader full-screen modal lifecycle so a transparent closed modal cannot intercept bookshelf clicks.
- Removed reader menu enter/leave transitions that could remain at offscreen transform positions in background/headless verification.
- Stabilized narrow reader settings layout across the main panel and typography/spacing/page-padding/more subpages.
- Reduced the mobile Settings page list item radius to the 8px token.

## Browser Verification

- Viewport: 390x800 via in-app Browser against `legado-headless --port 7790 --bind 127.0.0.1 --dist dist`.
- Settings page: `.sv-mobile-list__item` computed `border-radius=8px`, `overflowX=0`.
- Bookshelf -> reader: after reload `readerModalCount=0`; after book click `readerModalCount=1`, `.reader-modal opacity=1`, `overflowX=0`.
- Reader menu: top bar `y=0`, bottom bar `y=682`, bottom bar `bottom=800`, settings button center `x=329,y=761`, `overflowX=0`.
- Reader settings panel: panel width `358px`, `overflowX=0`, flip buttons no text overflow, active swatch/background `transform=none`, measured `textOverflow=[]`.
- Known headless-only warning: `NOT_ROUTED: extension_get_dir` while loading the bookshelf, caused by the headless command whitelist and not by this UI change.

## Gates

- `cmd /c node_modules\.bin\oxfmt.cmd --check .`: PASS after formatting existing `docs/ai-iteration-log.md`.
- `cmd /c pnpm.cmd lint`: PASS.
- `cmd /c pnpm.cmd build`: PASS with existing Vite warnings for `vconsole` direct eval, large chunks, ineffective `useTransport` dynamic import, and plugin timings.
- `cargo check -p reader-core`: PASS.
- `cargo check -p legado-tauri`: PASS.
- `cargo test -p reader-core`: PASS.
- `node scripts\ci\check-command-contract.mjs --json`: PASS (`frontendTotal=162`, `registeredTotal=161`, `bothCount=161`, `onlyBackend=0`, `frontend_unsupported_stub_count=39`).
- `git diff --check`: PASS.
