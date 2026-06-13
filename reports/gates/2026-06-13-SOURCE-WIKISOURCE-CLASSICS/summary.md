# Gate Report: SOURCE-WIKISOURCE-CLASSICS

Task ID: `SOURCE-2026-06-13-WIKISOURCE-CLASSICS`

Date: 2026-06-13

Scope:

- Added public-domain Wikisource JS source fixture: `crates/reader-core/tests/fixtures/book_sources/wikisource_classics.js`.
- Added ignored live regression test in `crates/reader-core/tests/source_compat_import.rs`.
- Updated source compatibility and AI status documents.

Boundary:

- No user data or installed local source directory was modified.
- No paid, login-only, preview-only, captcha, device-bound, or access-controlled content bypass was implemented.
- The new source targets publicly accessible Chinese Wikisource pages and uses a clear User-Agent with an 800 ms source delay.

Live verification:

```powershell
cargo test -p reader-core wikisource_classics_public_domain_full_chain -- --ignored --nocapture
```

Result: PASS.

Evidence:

```text
Wikisource 三國演義 full chain: chapters=120, first_len=14153, latest_len=19775
```

The live test covers:

- `search("三国演义")`
- `bookInfo`
- `chapterList`
- first chapter content
- final/latest chapter content

Full gate status: PASS.

Full gate commands:

```powershell
cmd /c node_modules\.bin\oxfmt.cmd --check .
git diff --check
node scripts\ci\check-command-contract.mjs --json
cargo fmt --all -- --check
cmd /c pnpm.cmd lint
cmd /c pnpm.cmd build
cargo check -p reader-core
cargo check -p legado-tauri
cargo test -p reader-core
cargo test -p reader-core -- --nocapture
cargo test -p reader-core wikisource_classics_public_domain_full_chain -- --ignored --nocapture
```

Observed command contract:

```text
frontendTotal=162
registeredTotal=161
bothCount=161
onlyFrontend=js_eval
onlyBackend=0
frontend_unsupported_stub_count=39
frontend_implemented_count=122
```

Notes:

- `pnpm build` still reports the pre-existing Vite/Rolldown warnings for `vconsole` direct eval, large chunks, and ineffective dynamic import in `useTransport`; no new build warning was introduced by this task.
- `git diff --check` reports only Windows LF/CRLF normalization warnings.
