# 服务与更新

> 适用版本：v0.5.11。本章说明个人版的本地服务（oserver）生命周期与版本更新；企业版的服务部署见[第 04 章](04-install-enterprise.md)。

## 本地服务是怎么跑的

Operoid 桌面应用是外壳；真正运行员工的是**本地服务 oserver**（监听 `127.0.0.1:7340`，token 自动生成并保存）。两种运行模式，由「设置 → 服务」的**「开机自动启动 Operoid 服务」**开关决定：

| 模式 | 行为 |
|---|---|
| **未注册**（默认） | 开启 App 时启动服务，关闭 App 时一并停止——后台不常驻 |
| **已注册**（开机服务） | 开机自启、常驻后台；关闭 App 不影响运行——**员工持续工作**（定时任务、email 进来的任务照样处理） |

状态随时可在「设置 → 服务 → 本地服务（oserver）」查看（运行中／未运行、port）。

命令行等价操作（高级）：

```bash
oserver install     # 注册开机服务（Windows 需系统管理员；会走 UAC 提权）
oserver status      # {"installed": true, "running": true}
oserver uninstall   # 移除
oserver version     # 版本＋build hash＋build time
```

平台细节：Windows 为 SCM 服务「Operoid」（已实机验证）；Linux 为 systemd、macOS 为 launchd（以 `sudo oserver install` 安装，服务以安装者身份运行，见 [DEPLOYMENT.md §11.3](../../DEPLOYMENT.md)）。升级／卸载时，安装向导会自动停止／恢复服务。

## 服务模式下的 API key

开机服务以系统服务账户运行，**看不到你账户的环境变量**。Operoid 会在启动／注册服务时，把当时存在于你环境中的 provider API key 快照进配置文件——所以：**更换或新增 API key 后，重启 App（或重新注册服务）**才会生效。企业版则直接写在 `operoid.toml` 的 `[llm] env`。

## 版本与更新

「设置 → 应用程序 → 版本与更新」：

| 开关 | 默认 | 行为 |
|---|---|---|
| **更新提示** | 开 | 启动时（及每 24 小时）检查 [GitHub Releases](https://github.com/ascetic168/Operoid/releases)；有新版显示横幅，**安装一律需你按下确认** |
| **自动更新** | 关 | 侦测到新版即后台下载安装（minisign 签名验证），完成后提示重新启动 |

⚠️ 本项目**不保证版本间向上兼容**——「自动更新」默认关就是这个原因；开启前请评估，并习惯查阅 [CHANGELOG](../../CHANGELOG.md) 的 breaking 条目。

更新未就绪时（例如新版发布管线尚未备妥），页面会提示稍后再试或到 Releases 页手动下载。

### 企业版差异

企业版**不自动更新**：`/admin` 的「版本」卡片可检查新版（结果缓存 24 小时；气隔环境可在 `operoid.toml` 设 `[update] check_enabled = false` 关闭对外连接）。升级为手动覆盖安装（先备份数据目录），完整流程见 [DEPLOYMENT.md §12](../../DEPLOYMENT.md)。

---
[← 14 电子邮件桥接](14-email-bridge.md)｜[索引](README.md)｜[16 企业版三种角色界面 →](16-enterprise-consoles.md)
