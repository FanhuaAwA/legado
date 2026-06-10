# R-P2-001 Android Signing Gate

时间：2026-06-10 18:46 +08:00

## 任务

关闭 R-P2-001：Android release 签名配置说明与发布前检查。

结论：closed for repository configuration。仓库已具备 release signing 配置入口、示例文件、发布前检查任务和文档；真实 keystore/密码必须只放在发布机器，不提交入库。

## 变更

- `src-tauri/gen/android/app/build.gradle.kts`
  - 读取本地 `keystore.properties`。
  - `storeFile/storePassword/keyAlias/keyPassword` 齐全时自动给 release buildType 挂载 signingConfig。
  - 新增 `checkReleaseSigning` 任务，缺少字段或 keystore 文件不存在时失败。
- `src-tauri/gen/android/app/keystore.properties.example`
  - 新增本地配置模板，示例使用 `PKCS12`。
- `.gitignore`
  - 忽略 `*.jks`、`*.keystore`、`*.p12`、`*.pfx`。
- `docs/platform-android.md`
  - 补 keytool 生成命令、配置示例、发布前检查命令和 unsigned 验证包说明。
- `docs/ai-task-status.md`、`docs/ai-iteration-log.md`、`E:\Book\legado-tauri-mandatory-completion-audit.md`
  - 同步 R-P2-001 closed，下一项为 R-P2-002。

## 验证

```text
.\gradlew.bat :app:tasks --all
  PASS：Gradle 配置可加载。

.\gradlew.bat :app:checkReleaseSigning
  EXPECTED FAIL：当前没有本地 keystore.properties，任务报告缺少 storeFile/storePassword/keyAlias/keyPassword。

pnpm run build:android:release
  PASS：明确 exit 0；复制 app-arm64-release-unsigned.apk 与 app-universal-release-unsigned.apk 到 构建结果\android。

pnpm exec oxfmt .
  PASS（371 files）

pnpm exec oxfmt --check .
  PASS（371 files）

pnpm lint
  PASS（71 warnings / 0 errors，既有 warning）

pnpm build
  PASS（既有 eval/chunk warnings）

cargo check -p reader-core
  PASS

cargo check -p legado-tauri
  PASS

node scripts/ci/check-command-contract.mjs --json
  PASS（frontendTotal=164 / registeredTotal=163 / onlyBackend=0）

git diff --check
  PASS（仅 CRLF 提示，无 whitespace error）
```

## 待发布操作

正式发布前，发布机器必须：

1. 使用 `keytool` 生成本地 keystore。
2. 从 `keystore.properties.example` 复制出 `keystore.properties` 并填写真实密码。
3. 运行 `.\gradlew.bat :app:checkReleaseSigning`，必须 PASS。
4. 再运行 `pnpm run build:android:release` 生成 signed release。

## 后续

下一项：R-P2-002 lint warnings 分类处理。`new Function`、书源脚本执行、插件执行等安全边界 warning 不得粗暴删除。
