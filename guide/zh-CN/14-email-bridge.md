# 电子邮件桥接（Obridge）

> 适用版本：员工 email 绑定自 v0.5.9 起。Obridge（Operoid Bridge）是员工与外部世界的双向桥：**Email 内置**（IMAP 收／SMTP 发），其他通道（企业 IM 等）走 WASM 插件——开发见[附录 B](appendix-b-development.md)。

## 它怎么运作

```
外部发件人 ──email──▶ [obridge: IMAP 轮询] ──POST /event──▶ Operoid（唤醒员工）
Operoid（员工回复）──POST /send──▶ [obridge: SMTP] ──email──▶ 原收件人（回同一 thread）
```

- **进**：obridge 定时轮询 IMAP 邮箱，比对 routes 把信投递给对应员工——员工被唤醒，整封信出现在它的对话里。
- **出**：员工的回复经 obridge 以对应的 From 身份寄出，**还原 In-Reply-To**——外部收信人看到的是正常的同一串往来。

## 让员工收发 email：四步

1. **绑定对外 email**：员工实体部署时填「对外 email」，或事后在实体上修改——这是唯一要记的员工端设置（单一真相存在员工数据上）。
2. **设置事件进出口**（`app-settings.json` 的 `app_config`，位置见[第 17 章](17-config-reference.md)）：
   ```json
   "event_ingress_port": 17341,
   "event_ingress_secret": "<密码A>",
   "event_outbound_url": "http://127.0.0.1:17401/send",
   "event_outbound_secret": "<密码B>"
   ```
3. **设置 obridge.toml**（模板见 `obridge/config.example.toml`）：
   - `[operoid]`：`ingress_url`（指到 Operoid 的 `/event`）＋`ingress_secret`（密码A）；
   - `[listen]`：`port`（如 17401）＋`secret`（密码B）；
   - `[[channels.email_imap]]`：IMAP／SMTP 账号、轮询间隔、routes（收件地址→员工）、senders（员工→From 地址）。
   - 员工绑定 email 后，**routes／senders 会自动同步**进 obridge.toml（自 v0.5.9 起），通常不用手改。
4. **启动 obridge**：最简单是交给 Operoid 代管——「设置 → 应用程序」设 **obridge.toml 路径**＋**obridge 可执行文件**＋勾选「启动时自动带起」；安装包自带 obridge.exe（与主程序同目录，路径可留空自动侦测）。之后 Operoid 启动时带起、退出时带走。

## 热重载

obridge 每 2 秒检查 `obridge.toml`——`[[channels]]` 与插件设置**改了即生效**；`[listen]`／`[operoid]` 变更需重启（代管模式下，设置页存档会自动重启子进程）。

「设置 → 应用程序」的 Obridge 区块也可直接**编辑 obridge.toml 原文**（Operoid 只当编辑器，通道账密属 obridge 职责）。

## 验证（全链走一遍）

1. 从外部邮箱发信到 route 命中的地址。
2. obridge 日志应见投递；员工被唤醒，对话页出现 In 消息（整封信序列化）。
3. 与员工对话；外部邮箱应收到**回原 thread** 的回信（发件人为 senders 对应的 From 身份）。

## 日志与排解

- 日志：数据目录下的 **`obridge.log`**（obridge 的 stderr 全部在这）；位置见[第 17 章](17-config-reference.md)。
- 员工没收到信：先看 routes 是否命中（地址拼写、大小写）、IMAP 账密、轮询间隔；再看有没有对应员工（已封存的员工不会被唤醒）。
- 收不到回信：看 `event_outbound_url` 是否已设（未设时员工的 send 会显示「外發未啟用」）、SMTP 凭据。
- 更多：[第 18 章](18-troubleshooting.md)。

> 治理提醒：email 是**对外通道**——让员工自动处理来信前，先用[登记表](13-governance.md)的围栏（例如「禁对外发送」或人裁决层）画好界线。

---
[← 返回索引](README.md)｜[15 服务与更新 →](15-service-and-updates.md)
