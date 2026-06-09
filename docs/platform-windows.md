# Windows Platform Notes

## 已验证

- **构建**: `pnpm run build:windows:release` PASS
- **产物**: `构建结果\windows\legado-tauri.exe` (~19MB)
- **编译器**: MSVC Build Tools
- **WebView2**: 系统自带或自动安装
- **数据目录**: `%APPDATA%\com.legado.tauri\reader\`

## 特性支持

| 功能           | 状态 | 说明                   |
| -------------- | ---- | ---------------------- |
| 文件导入       | ✅   | 通过 Tauri dialog      |
| 文件导出       | ✅   | 用户选择保存路径       |
| 系统文件管理器 | ✅   | `open_dir_in_explorer` |
| VS Code 打开   | ✅   | 失败降级系统打开       |
| 窗口控制       | ✅   | 关闭/最小化/最大化     |
| 中文路径       | ⚠️   | 需进一步验证           |
| 空格路径       | ⚠️   | 需进一步验证           |
| 长路径         | ⚠️   | 需进一步验证           |

## 构建命令

```powershell
cd E:\Book\Legado-Tauri-main
pnpm run build:windows:release
# 产物在 构建结果\windows\
```

## 已知限制

- 需要 MSVC Build Tools（Visual Studio 或 Build Tools）
- WebView2 运行时需要安装（Windows 10/11 通常已自带）
- 杀毒软件可能拦截未签名可执行文件
