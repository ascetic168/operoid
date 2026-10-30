# 疑难解答

> 适用版本：v0.5.11。原则：**先看横幅与状态，再查日志**。企业部署类问题另见 [DEPLOYMENT.md §9](../../DEPLOYMENT.md)。

## 诊断第一步

1. 主窗口上方的**警告横幅**说了什么？（能力缺项会在这里明说）
2. 「设置 → 服务」的**能力矩阵**——哪一项红？按「重新检查」。
3. 本地服务状态：同页「本地服务（oserver）」是否「运行中」。
4. 相关日志：`obridge.log`（Email 问题）、来源 repo 的 `conversion-report.json`（PDF 转换问题）、`oserver status`／`oserver version`（服务问题）。

## 个人版常见症状

| 症状 | 原因与处置 |
|---|---|
| 启动出现「缺少前置程序」 | git／bun／gbrain 缺项——按对话框链接安装后**重开 App**；路径不对时到「设置 → 应用程序」修正 `gbrain.exe` 路径 |
| 「Operoid 本地服务未启动」 | 重新开启 App（会自动带起服务）；或手动运行 `oserver`。port 被占用时可用 `OSERVER_PORT` 换 port |
| 横幅「未检测到 MinerU」 | 复杂 PDF 会降级为快速模式——安装：`uv tool install "mineru>4.0"`；或在「设置 → 服务」填 MinerU 安装目录（`mineru_dir`） |
| 横幅「嵌入模型未启用多模态」 | 图片仅以文字说明索引——llama-server 启动旗标加 `--mmproj`（投影器 GGUF 下载见 [DEPLOYMENT.md §2](../../DEPLOYMENT.md)） |
| 横幅「llama-server 缺 -b/-ub 8192 旗标」 | 长 chunk 被（默认 512）拒收、知识检索质量下降——启动命令补 `-b 8192 -ub 8192` 后重测。企业版可用 `GET /api/knowledge/health` 的 `embedding.long_input_ok` 核对 |
| PDF 转换警告「降级 pdf_extract」 | MinerU 取用阶梯全级落空——`conversion-report.json` 的 `ladder.attempts` 记录每级落空原因（路径／权限／版本） |
| 员工唤醒无反应 | 查三前置：git／bun／gbrain；模型 API key 是否已设（「设置 → 模型」的端点解析）；「事件」页看有没有被触发 |
| 员工收不到 email | 见[第 14 章](14-email-bridge.md)排解段落（routes 命中、IMAP 凭证、`obridge.log`） |
| 服务模式 LLM 突然失效 | 服务账户看不到用户环境变量——**换 API key 后要重启 App／重新注册服务**让快照更新 |
| 更新后配置／数据不见了 | 不会——数据在配置／数据目录（[第 17 章](17-config-reference.md)），更新只替换程序。若真异常，检查是否以不同用户账户登录系统 |

## 企业版常见症状

| 症状 | 原因与处置 |
|---|---|
| 启动失败「非 loopback 须 TLS」 | `operoid.toml` 补齐 `[tls]`（cert＋key **齐备**），或改 bind 127.0.0.1 |
| 页面加载但数据全空 | 看浏览器 Console（CSP／CORS／凭证）；核对 `/healthz` 的 `version`——常见是**旧版服务还在跑**，杀掉旧进程重启 |
| 登录回 429 | 连续 5 次密码错误锁定 15 分钟（重启服务即解除）；admin 可用「重设密码」 |
| obridge 行为像旧版 | 新旧二进制混用——`/api/obridge/status` 的 `exe_build_match=false` 即是；oserver 与 obridge 必须出自同一构建（同一部署包／`cargo dev-build`） |
| 图片嵌入失败 | 多模态需要 mm-bridge shim（8090→8080）——见 [DEPLOYMENT.md §2](../../DEPLOYMENT.md)「多模态」 |

## FAQ

**Q：忘记管理员密码？** 另一位 admin 可重设；或以 master token 经 API 重建账号（[DEPLOYMENT.md §6](../../DEPLOYMENT.md) 手动流程）。

**Q：可以两台电脑共用一颗脑吗？** 脑是本机目录；跨机请走企业版（一台服务器、大家连上去），不要直接共享文件夹。

**Q：怎么完全重来？** 删除脑（「脑」页移除来源可选永久删除）＋删除配置／数据目录（[第 17 章](17-config-reference.md)位置表）。先备份。

**Q：哪里反馈问题？** [GitHub Issues](https://github.com/ascetic168/Operoid/issues)——附上 `oserver version` 输出与相关横幅／日志摘录。

---
[← 17 配置参考](17-config-reference.md)｜[索引](README.md)｜[附录 A →](appendix-a-gbrain.md)
