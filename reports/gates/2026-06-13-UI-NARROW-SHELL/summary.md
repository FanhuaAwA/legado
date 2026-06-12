# 2026-06-13 UI-NARROW-SHELL Gate Summary

## Scope

- Fix narrow desktop viewport shell selection.
- Preserve explicit layout mode overrides.
- No backend, source, user-data, or third-party source changes.

## Changed Files

- `src/composables/useEnv.ts`
- `docs/ai-iteration-log.md`
- `docs/ai-task-status.md`
- `reports/gates/2026-06-13-UI-NARROW-SHELL/summary.md`

## Browser Verification

Runtime:

```powershell
legado-headless --port 7788 --bind 127.0.0.1 --dist dist
```

390x800 bookshelf:

- `appClass`: `app-layout app-layout--mobile`
- `mainRect`: `390x744`
- `sideExists`: `false`
- `bottomVisible`: `true`
- `bodyOverflowX`: `0`

1000x800 bookshelf:

- `appClass`: `app-layout`
- `mainRect`: `800x716`
- `sideVisible`: `true`
- `bottomExists`: `false`
- `bodyOverflowX`: `0`

390x800 book source page:

- Bottom navigation `tab "书源管理"` was clickable.
- Visible `h1`: `书源管理`
- `h1` rect: `80x32`
- `writing-mode`: `horizontal-tb`
- `white-space`: `nowrap`
- Header/top button overlap pairs: none
- `bodyOverflowX`: `0`

Cleanup:

- Browser viewport reset.
- Temporary browser tab closed.
- Headless service on port `7788` stopped.

## Gates

| Command | Result |
| --- | --- |
| `cmd /c node_modules\.bin\vue-tsc.cmd -p tsconfig.app.json --noEmit` | PASS |
| `cmd /c node_modules\.bin\oxfmt.cmd --check src\composables\useEnv.ts` | PASS |
| `cmd /c pnpm.cmd lint` | PASS |
| `cmd /c pnpm.cmd build` | PASS |
| `cargo check -p reader-core` | PASS |
| `cargo check -p legado-tauri` | PASS |
| `cargo test -p reader-core` | PASS |
| `node scripts\ci\check-command-contract.mjs --json` | PASS |
| `git diff --check` | PASS |

## Known Warnings

- `pnpm build` still reports existing Vite warnings:
  - `vconsole` direct `eval`
  - chunks larger than 500 kB
  - `useTransport` ineffective dynamic import
- `git diff --check` emitted only the Windows LF/CRLF conversion warning for `src/composables/useEnv.ts`; no whitespace error was reported.
