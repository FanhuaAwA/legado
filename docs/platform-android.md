# Android Platform Notes

## 已验证

- **构建**: `pnpm run build:android:release` PASS (unsigned APK)
- **产物**: `构建结果\android\app-universal-release-unsigned.apk`
- **签名配置**: Gradle release signingConfig 已接入；无本地 keystore 时 `:app:checkReleaseSigning` 按预期失败
- **工具链**: JDK 21, Android SDK 35/36, NDK 27.1.12297006
- **Rust target**: `aarch64-linux-android` (arm64)

## 环境配置

项目 `.env` 已固定为 JDK 21（不要改回 JDK 25）：

```text
JAVA_HOME=C:/Program Files/Eclipse Adoptium/jdk-21.0.11.10-hotspot
ANDROID_HOME=C:/Android/Sdk
ANDROID_NDK_HOME=C:/Android/Sdk/ndk/27.1.12297006
```

## 特性支持

| 功能             | 状态 | 说明                           |
| ---------------- | ---- | ------------------------------ |
| 网络权限         | ⚠️   | 需在真机验证                   |
| 文件导入         | ⚠️   | 需在真机验证                   |
| 文件导出         | ⚠️   | 需在真机验证                   |
| 深链导入         | ❌   | `legado://import/...` 未验证   |
| 返回键           | ❌   | 需实现                         |
| 软键盘遮挡       | ❌   | 需实现                         |
| 移动端阅读布局   | ❌   | 需适配小屏                     |
| 桌面专属功能隐藏 | 部分 | `booksource_pick_dir` 等已处理 |

## Release 签名

当前仓库只提交签名配置模板，不提交真实密钥或密码。发布机器需要自行生成 keystore，并把本地配置写入被 git 忽略的 `src-tauri/gen/android/app/keystore.properties`。

生成示例：

```powershell
cd E:\Book\Legado-Tauri-main
keytool -genkeypair -v `
  -storetype PKCS12 `
  -keystore src-tauri\gen\android\app\release-signing.p12 `
  -alias legado-tauri-release `
  -keyalg RSA `
  -keysize 4096 `
  -validity 10000
Copy-Item src-tauri\gen\android\app\keystore.properties.example src-tauri\gen\android\app\keystore.properties
```

编辑 `src-tauri/gen/android/app/keystore.properties`：

```properties
storeFile=release-signing.p12
storeType=PKCS12
storePassword=<本机密钥库密码>
keyAlias=legado-tauri-release
keyPassword=<本机 key 密码>
```

发布前检查：

```powershell
cd E:\Book\Legado-Tauri-main\src-tauri\gen\android
.\gradlew.bat :app:checkReleaseSigning
```

`checkReleaseSigning` 只检查发布签名材料是否齐备；没有本地 keystore 时应失败。普通 `pnpm run build:android:release` 仍允许产出 unsigned APK，便于 CI 和本地验证。

## 构建命令

```powershell
cd E:\Book\Legado-Tauri-main
pnpm run build:android:release
# 产物在 构建结果\android\
```

## 已知限制

- Windows 需要开启 Developer Mode 以支持 symlink（Tauri Android 构建所需）
- 未配置 `keystore.properties` 时生成 unsigned release；正式发布前必须先通过 `:app:checkReleaseSigning`
- 部分桌面专属命令通过 `#[cfg(not(any(target_os = "windows", ...)))]` 返回 UNSUPPORTED
