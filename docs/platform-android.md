# Android Platform Notes

## 已验证

- **构建**: `pnpm run build:android:release` PASS (unsigned APK)
- **产物**: `构建结果\android\app-universal-release-unsigned.apk`
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

## 构建命令

```powershell
cd E:\Book\Legado-Tauri-main
pnpm run build:android:release
# 产物在 构建结果\android\
```

## 已知限制

- Windows 需要开启 Developer Mode 以支持 symlink（Tauri Android 构建所需）
- 生成的 APK 为 unsigned release，发布前需配置签名
- 部分桌面专属命令通过 `#[cfg(not(any(target_os = "windows", ...)))]` 返回 UNSUPPORTED
