# 安装个人版

> 适用版本：v0.5.11。个人版＝桌面安装包，装在自己的电脑，无需构建。

## 下载安装包

到 [GitHub Releases](https://github.com/ascetic168/Operoid/releases) 页面，在最新版本的 **Assets** 找对应平台的文件：

| 平台 | 文件 | 说明 |
|---|---|---|
| Windows | `Operoid_*_x64-setup.exe` | **建议**——NSIS 安装向导；应用内自动更新即以此格式安装 |
| Windows | `Operoid_*_x64_en-US.msi` | MSI 安装包，适合企业软件分发 |
| macOS (Apple Silicon) | `Operoid_*_aarch64.dmg` | M 系列芯片 |
| macOS (Intel) | `Operoid_*_x64.dmg` | Intel 芯片 |
| Linux | `Operoid_*_amd64.deb`、`Operoid-*_x86_64.rpm`、`Operoid_*_amd64.AppImage` | 依发行版择一 |

其余资产不需下载：`.sig` 是应用内自动更新的签名文件，`latest.json` 是更新清单。

## 安装了什么

安装包内含完整组件，不需要另外准备：

- **Operoid 桌面应用**（Tauri v2）；
- **oserver** 本地服务（员工执行引擎）；
- **obridge** 外部通道桥接（Email／IM）。

## 各平台安装步骤

### Windows

1. 运行 `.exe`，照安装向导完成。
2. 若之前注册过「开机自动启动 Operoid 服务」，安装与卸载时向导会自动停止／恢复该服务（不需手动处理）。

### macOS

1. 打开 `.dmg`，将 **Operoid** 拖进「应用程序」。
2. 首次运行若出现 macOS 安全性提示，按「打开」确认。

### Linux

```bash
# Debian / Ubuntu
sudo dpkg -i Operoid_*_amd64.deb
# RPM 系
sudo rpm -i Operoid-*_x86_64.rpm
# AppImage
chmod +x Operoid_*_amd64.AppImage && ./Operoid_*_amd64.AppImage
```

Linux 需要先装好 `libwebkit2gtk-4.1` 等 Tauri v2 系统依赖，清单见 [Tauri 前置需求](https://v2.tauri.app/start/prerequisites/)。

## 首次启动

1. 打开 Operoid——它会自动带起本地服务（`127.0.0.1:7340`，token 自动生成；服务状态见「设置 → 服务」）。
2. 若缺少[前置组件](02-requirements.md)，会出现「缺少前置程序」对话框：列出 git／bun／gbrain 的缺项、用途与安装链接。安装后重开 Operoid。
3. 主窗口上方可能出现**能力警告横幅**（MinerU 未装、嵌入服务未启等）——缺项不阻断使用，但影响知识质量上限；对照[第 02 章](02-requirements.md)的表格决定是否处理。

## 更新

「设置 → 应用程序」的**版本与更新**区块：

| 开关 | 默认 | 行为 |
|---|---|---|
| **更新提示** | 开 | 启动时（及每 24 小时）自动检查新版；有新版显示横幅，安装需你按下确认 |
| **自动更新** | 关 | 检测到新版即自动下载安装，完成后提示重新启动 |

⚠️ 本项目**不保证版本间向上兼容**——开启「自动更新」前请评估；版本间的重大变更见 [CHANGELOG](../../CHANGELOG.md)。

更新流程：横幅显示「有新版 vX.Y.Z 可用」→「前往更新」→ 下载（显示进度）→「已更新至 vX.Y.Z，重新启动后生效」→ 按「重新启动」。也可以不经 App，直接到 [Releases 页](https://github.com/ascetic168/Operoid/releases)下载安装包覆盖安装（NSIS 向导会自动处理服务）。

## 卸载

- **Windows**：「设置 → 应用 → 安装的应用」卸载；卸载时会一并移除注册的服务。
- **macOS／Linux**：删除应用程序即可（Linux：包管理器移除）。

设置与数据（脑、员工、对话历史等）存于用户数据目录，卸载不会清除；位置清单见[第 17 章](17-config-reference.md)。

## 常见安装问题

安装后打不开、服务未启动、组件检测不到——见[第 18 章疑难解答](18-troubleshooting.md)。

---
[← 02 系统需求](02-requirements.md)｜[返回索引](README.md)｜[04 安装企业版 →](04-install-enterprise.md)
