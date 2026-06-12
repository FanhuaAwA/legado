# 2026-06-13 MAINT-IMPORT-UI-PERF Gate Summary

Task ID: `MAINT-2026-06-13-IMPORT-UI-PERF`

## Scope

- Stabilize book source management layout after added actions.
- Optimize MiaoGongZi / `yuedu://rsssource` import parsing by avoiding duplicate downloads and bounding concurrent page/package resolution.
- Keep AI DeepSeek proxy, yuedu deep links, paragraph comment sourceDir propagation, and window controls in the same reviewed stabilization batch.
- Fix the source limit warning dialog that stayed visible because it used uncontrolled `n-dialog`.

## Gates

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cmd /c pnpm.cmd lint` | PASS, 0 warnings / 0 errors |
| `cmd /c pnpm.cmd build` | PASS, existing Vite warnings only |
| `cargo check -p reader-core` | PASS |
| `cargo check -p legado-tauri` | PASS |
| `cargo test -p reader-core` | PASS |
| `cargo test -p legado-tauri` | PASS |
| `node scripts/ci/check-command-contract.mjs --json` | PASS, 162 / 161 / 161, onlyBackend=0, frontend stub=39 |
| `git diff --check` | PASS |

## Live Validation

Non-destructive MiaoGongZi subscription validation:

- URL: `http://yuedu.miaogongzi.net/shuyuan/miaogongziDY.json`
- Subscription items: 1
- Valid booksource packages resolved: 10
- All 10 packages downloaded and matched the legacy book source JSON shape.
- No Bilibili/profile/HTML-only entries were treated as importable JSON packages.

## UI Validation

Target: `legado-headless --port 7788 --bind 127.0.0.1 --dist dist`

- Desktop viewport `1000x800`: `书源管理` title was horizontal (`writingMode=horizontal-tb`, `whiteSpace=nowrap`, rect `80x32`); action buttons had no overlaps.
- Narrow viewport `390x800`: title remained horizontal (`80x32`); action buttons wrapped without overlap.
- Source limit warning dialog no longer blocks the page after production rebuild.

## Known Follow-Up

- Vite production build still reports existing warnings: `vconsole` direct eval, large chunks, and ineffective dynamic import for `useTransport`.
- Browser narrow-width simulation still leaves the desktop sidebar at 200px; Android/touch media behavior should be verified separately before changing shell layout.
