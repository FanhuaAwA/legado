# REL-2026-06-14-MASTER-SIGNED-RELEASE Gate Summary

Date: 2026-06-14

Scope:

- Add a `master` post-Quality-Gate release workflow.
- Generate a new local Android release keystore.
- Sign Android APKs with V1, V2, and V3 APK signing schemes.
- Store signing material in GitHub repository secrets, not in git.

Local artifacts:

- Signed APK: `构建结果/android-signed/Legado-Tauri-0.9.0-android-aarch64-v1-v2-v3-signed.apk`
- Verification log: `构建结果/android-signed/apksigner-verify.txt`

Commands:

- `cmd /c pnpm.cmd tauri android build --apk --target aarch64`：PASS
- `apksigner sign --min-sdk-version 21 --v1-signing-enabled true --v2-signing-enabled true --v3-signing-enabled true --v4-signing-enabled false`：PASS
- `apksigner verify --verbose --print-certs --min-sdk-version 21`：PASS
- `cmd /c pnpm.cmd lint`：PASS
- `git diff --check`：PASS

Verification highlights:

- `Verified using v1 scheme (JAR signing): true`
- `Verified using v2 scheme (APK Signature Scheme v2): true`
- `Verified using v3 scheme (APK Signature Scheme v3): true`
- Signer certificate DN: `CN=Legado Tauri Release, O=FanhuaAwA, C=CN`
- Signer key algorithm: `RSA`
- Signer key size: `4096`

Secrets configured in GitHub:

- `APK_RELEASE_KEY_STORE`
- `APK_RELEASE_KEY_ALIAS`
- `APK_RELEASE_STORE_PASSWORD`
- `APK_RELEASE_KEY_PASSWORD`
- `APK_RELEASE_KEY_STORE_TYPE`

Notes:

- `release-signing.p12`, `keystore.properties`, `.release-secrets/`, and APK outputs are git ignored and must stay out of commits.
- V1 signing required `--min-sdk-version 21`; without it, apksigner skipped V1 because the app manifest minSdk is 24.
- Workflow syntax still needs remote GitHub Actions validation after push because no local actionlint/YAML parser is available in this environment.
