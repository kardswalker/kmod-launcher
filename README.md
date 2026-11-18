# KMod Launcher

KMod Launcher 是 KMod Loader 的 Windows/Android 双端启动器与安装器。界面和安装流程参考 CeleMod，前端使用 React + Vite，桌面与 Android 后端使用 Tauri 2。

## 当前框架

- 从 GitHub Releases 读取 Windows 与 Android loader release
- 默认选择最新稳定 release
- 用户可以切换 `dev` 与 `release`
- Windows 下载 ZIP 并安全解压到 `KARDS\\Binaries\\Win64`
- Android 下载 APK 到应用缓存目录，准备交给系统安装器
- 下载文件计算 SHA-256，避免安装结果无法追踪

## 开发

需要 Node.js、pnpm、Rust 和 Tauri 2 环境。

```powershell
pnpm install
pnpm tauri dev
```

## Release 来源

- Windows: `kardswalker/KMod-Loader-Windows`
- Android: `CCB-TEAM/KMod-Loader-Android`

KMod Loader 与 KMod Launcher 均使用 MIT 许可证。KARDS 属于其所有者，本项目与其没有隶属关系。
