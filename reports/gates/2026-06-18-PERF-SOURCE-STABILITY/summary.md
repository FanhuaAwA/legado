# 2026-06-18 PERF-SOURCE-STABILITY

Branch: `master`

## Scope

This iteration focused on source-heavy runtime stability:

- Reduce hot-path network logging overhead.
- Prevent timed-out frontend source searches from leaving backend tasks running.
- Keep inline reader chapter-list task cleanup scoped to the active task.
- Reduce frontend aggregate-search grouping cost under many sources/results.
- Re-check miaogongzi CDN source freshness for shuqi, qimao, and fanqie.

## Code Changes

- `crates/reader-core/src/crawler/fetcher.rs`
  - Replaced per-request `println!` debug output with `tracing::debug!`.
  - No longer prints request bodies; logs body length only.
  - Raised the `52dns.cc` fast-fail request timeout from 5s to 20s. This keeps a cap below the overall request timeout while avoiding false failures during source-provider jitter.
  - Added an 800ms global start interval for `52dns.cc` hosts. Multiple sources can share the same CDN/proxy host, so per-source throttling alone is not enough to avoid short-burst anti-DDoS / temporary blacklist behavior.

- `src/views/SearchView.vue`
  - Sends `booksource_cancel` when a per-source search rejects or times out, so the backend task registry is not left with work the UI has already abandoned.

- `src/composables/useInlineBookReader.ts`
  - Cancels the active chapter-list task on load failure.
  - Clears `chapterListTaskId` only if it still matches the active task.

- `src/utils/searchAggregation.ts`
  - Added a WeakMap-backed aggregation index.
  - Exact-name matches hit directly; fuzzy matches scan only groups sharing name bigrams, preserving the existing `isSameBook` behavior while reducing repeated full scans.

## Source Freshness Check

Checked current CDN payloads from:

- `https://cdn.miaogongzi.cc/shuyuan/sqxs260128_0ee680c1.json`
- `https://cdn.miaogongzi.cc/shuyuan/qmxs260128_432b9f7e.json`
- `https://cdn.miaogongzi.cc/shuyuan/fqfix0529_45469384.json`

Current CDN hashes differ from the local files for all three sources, so the old 2026-06-11 conclusion that CDN copies matched local backup copies is stale.

Observed status:

- shuqi local full chain: pass. Search/toc/content succeeded, `chapters=140`, `content_len=6656`.
- shuqi CDN/network import: import/search/toc pass, `chapters=140`; content still diagnosed as CDN `ruleContent` stale/upstream content timeout. Short-burst CDN protection is a likely contributor when tests are repeated back-to-back.
- qimao local full chain: pass. Search/toc/content succeeded, `chapters=2551`, `content_len=15132`.
- qimao CDN/network import: import/search/toc pass, `chapters=2551`; content remains `EMPTY`, diagnosed as CDN `ruleContent` stale.
- fanqie CDN/network import: import/list pass; full chain remains constrained by source-side `device_register` behavior.

Conclusion: engine compatibility is still healthy for the local refreshed shuqi/qimao rules. CDN versions are reachable and usable for search/toc, but shuqi/qimao CDN content rules still require upstream source update before network-imported copies can reliably read full text. The app now spaces shared `52dns.cc` requests to reduce the chance of triggering CDN anti-DDoS throttling during large source batches.

## Verification

Passed:

```powershell
cmd /c pnpm.cmd exec oxfmt --check src/utils/searchAggregation.ts src/views/SearchView.vue src/composables/useInlineBookReader.ts
cmd /c pnpm.cmd exec oxlint --type-aware --type-check .
cmd /c pnpm.cmd exec vue-tsc -p tsconfig.app.json --noEmit
cargo check -p reader-core
cargo check -p legado-tauri
cargo test -p reader-core --test route_b_facade import_legacy_json_text_reports_progress_batches -- --nocapture
cargo test -p reader-core --test source_compat_import imports_and_parses_fields -- --ignored --nocapture
cargo test -p reader-core --test source_compat_import shuqi_source_full_chain -- --ignored --nocapture --test-threads=1
cargo test -p reader-core --test source_compat_import qimao_source_full_chain -- --ignored --nocapture --test-threads=1
cargo test -p reader-core --test source_compat_import shuqi_network_import_full_chain -- --ignored --nocapture --test-threads=1
cargo test -p reader-core --test source_compat_import qimao_network_import_full_chain -- --ignored --nocapture --test-threads=1
cargo test -p reader-core --test source_compat_import fanqie_network_import -- --ignored --nocapture --test-threads=1
cmd /c pnpm.cmd build:windows:release
```

Known verification note:

```powershell
cmd /c pnpm.cmd lint
```

Still fails in the `oxfmt --check .` phase due to pre-existing format issues in 13 untouched files. The files changed in this iteration pass targeted `oxfmt --check`; `oxlint`, `vue-tsc`, and `reader-core` checks pass.

Desktop smoke note:

- The final release executable was built at `target\x86_64-pc-windows-msvc\release\legado-tauri.exe` and copied to `构建结果\windows\legado-tauri.exe`.
- A Windows desktop smoke test was attempted through the Codex Computer Use plugin, but the plugin failed during runtime bootstrap with `Package subpath './dist/project/cua/sky_js/src/targets/windows/internal/computer_use_client_base.js' is not defined by "exports" ... @oai/sky/package.json`. Because this is a desktop-control runtime failure outside the app, no unsafe foreground UI automation fallback was used.
