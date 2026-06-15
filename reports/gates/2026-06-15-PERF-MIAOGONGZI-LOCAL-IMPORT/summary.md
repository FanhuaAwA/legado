# 2026-06-15 PERF-MIAOGONGZI-LOCAL-IMPORT

## Scope

- Optimized large Legado source imports that expand from the 喵/猫公子 subscription.
- Covered both URL subscription/package imports and local multi-file JSON imports.
- Kept the old single-text command for compatibility and added `booksource_import_legacy_json_texts` for batched multi-text imports.

## Changes

- URL subscription resolution concurrency increased from 4 to 8 for faster package expansion.
- Multi-package URL imports now call the batched text command once instead of issuing one import command per package.
- Local multi-file imports read selected files concurrently, then call the batched text command without parsing/stringifying JSON in the frontend.
- `reader-core` now supports `import_legacy_json_texts(_with_progress)` and skips only the bad text item while importing valid items.
- Legacy import file writes are bounded-concurrent per progress batch.
- Import progress/write batches increased from 25 to 100 entries to reduce transaction/progress overhead while keeping incremental UI progress.
- SQLite `save_many` now uses chunked multi-row upsert statements inside the transaction.

## Live Performance Check

Command:

```powershell
cargo test -p reader-core --test miaogongzi_import_perf miaogongzi_subscription_import_sequential_vs_combined -- --ignored --nocapture
```

Final output:

```text
packages=10 entries=1259 resolve_ms=1344 sequential_ms=3809 combined_ms=3831 local_sequential_ms=4146 local_combined_ms=3807 local_speedup=1.09x
```

Notes:

- Initial same-test import-only baseline before core import batching was `sequential_ms=6467`, `combined_ms=6199`.
- A frontend JSON-parse/stringify combine attempt was rejected after local-file testing because it slowed local imports (`local_sequential_ms=4199`, `local_combined_ms=4665`, `local_speedup=0.90x`).
- The retained local path uses concurrent file reads plus backend batched text import, giving `local_speedup=1.09x` on the same 喵/猫公子 packages.

## Verification

```powershell
cmd /c pnpm.cmd lint
cargo test -p reader-core import_legacy_json_text_reports_progress_batches -- --nocapture
cargo test -p reader-core import_legacy_json_texts_skips_bad_item_and_imports_valid_sources -- --nocapture
cargo test -p legado-tauri booksource_import_legacy_json_texts_accepts_request_id_in_ws_router -- --nocapture
node scripts\ci\check-command-contract.mjs --json
cargo check -p legado-tauri
cmd /c pnpm.cmd build:windows:release
```

All passed locally. The Tauri test/build still emit the known Windows linker stdout warning.

Windows release artifact:

```text
E:\Book\Legado-Tauri-main\target\x86_64-pc-windows-msvc\release\legado-tauri.exe
E:\Book\Legado-Tauri-main\构建结果\windows\legado-tauri.exe
```
