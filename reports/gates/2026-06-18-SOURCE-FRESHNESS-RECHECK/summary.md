# 2026-06-18 SOURCE-FRESHNESS-RECHECK

Branch: `master`

## Scope

This pass rechecked source freshness and explicit update hints for the local source samples under `E:\Book`, with emphasis on whether network-imported copies are stale relative to local refreshed JSON files.

## Metadata Findings

- `E:\Book\书旗书源\sqxs260128_0ee680c1.json`
  - `bookSourceName=书旗小说（严禁外传文件或直链）`
  - `lastUpdateTime=1769418918067` (`2026-01-26T09:15:18.067Z`)
  - No explicit `bookSourceComment` / `updateUrl`.

- `E:\Book\七猫书源\qmxs260128_432b9f7e.json`
  - `bookSourceName=七猫小说（严禁外传文件或直链）`
  - `lastUpdateTime=1769418879736` (`2026-01-26T09:14:39.736Z`)
  - No explicit `bookSourceComment` / `updateUrl`.

- `E:\Book\番茄书源\fqfix0529_45469384.json`
  - `bookSourceName=番茄（严禁外传文件或直链）`
  - `lastUpdateTime=1776180090270` (`2026-04-14T15:21:30.270Z`)
  - No explicit `bookSourceComment` / `updateUrl`.

- `E:\Book\番茄短剧\fqdj0719_016377fa4.json`
  - `sourceName=番茄短剧`, `articleStyle=2`, `lastUpdateTime=0`
  - `description=作者：明月照大江&Distance远方`

- `E:\Book\猫公子书源\猫公子书源.manifest.json`
  - `name=猫公子书源-搜索优先修复版`
  - `generatedAt=2026-06-13T14:45:00Z`
  - `totalItems=1092`, `replacedCount=3`
  - Replaced entries: 若初文学、阅友小说、飞卢小说, each marked search/explore/toc/content usable.

## CDN Freshness

| Source             | CDN URL                                                      | Status | Raw CDN hash                                                       | Local relation                                                    | Current judgment                        |
| ------------------ | ------------------------------------------------------------ | -----: | ------------------------------------------------------------------ | ----------------------------------------------------------------- | --------------------------------------- |
| shuqi              | `https://cdn.miaogongzi.cc/shuyuan/sqxs260128_0ee680c1.json` |    200 | `a46f80d86cb497c8e11cc476052e7ef587f3ba026b786bdf364bafb4da990141` | equals local `.backup.json`, differs from local refreshed `.json` | CDN is stale; needs upstream update     |
| qimao              | `https://cdn.miaogongzi.cc/shuyuan/qmxs260128_432b9f7e.json` |    200 | `902cd4f57b28ab9f6e6c91851d6f6cc94208766551933c8baab7d681374342ee` | equals local `.backup.json`, differs from local refreshed `.json` | CDN is stale; needs upstream update     |
| fanqie             | `https://cdn.miaogongzi.cc/shuyuan/fqfix0529_45469384.json`  |    200 | `59e47254a83a7b0ac4a5049f530108b2f0bc00efdba9450a6e74e374342764b3` | equals local `.json`                                              | CDN matches local                       |
| fanqie short drama | `https://cdn.miaogongzi.cc/shuyuan/fqdj0719_016377fa4.json`  |    404 | n/a                                                                | local file exists                                                 | network import URL is currently invalid |

## Live Verification

Passed:

```powershell
cargo test -p reader-core --test source_compat_import qimao_source_full_chain -- --ignored --nocapture --test-threads=1
cargo test -p reader-core --test source_compat_import shuqi_source_full_chain -- --ignored --nocapture --test-threads=1
cargo test -p reader-core --test source_compat_import shuqi_network_import_full_chain -- --ignored --nocapture --test-threads=1
cargo test -p reader-core --test source_compat_import qimao_network_import_full_chain -- --ignored --nocapture --test-threads=1
cargo test -p reader-core --test source_compat_import fanqie_network_import -- --ignored --nocapture --test-threads=1
```

Observed output:

- qimao local full chain: `chapters=2551`, `content_len=15132`.
- shuqi local full chain: first attempt failed with source-side TLS EOF from `jh.52dns.cc`; retry after 5 seconds passed with `chapters=140`, `content_len=6656`.
- shuqi CDN/network import: search/toc pass, `chapters=140`; content diagnostic remains `EMPTY（CDN 版 ruleContent 过期）`.
- qimao CDN/network import: search/toc pass, `chapters=2551`; content diagnostic remains `EMPTY（CDN 版 ruleContent 过期）`.
- fanqie CDN/network import: import and list pass; full chain remains constrained by source-side `device_register` behavior.

## Conclusion

The app engine remains compatible with the refreshed local shuqi/qimao rules. The user-visible update need is real for network-imported shuqi/qimao because the CDN still serves old backup-equivalent JSON. Fanqie CDN matches local, while the fanqie short-drama network import URL is currently broken with HTTP 404.
