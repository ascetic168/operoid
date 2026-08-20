import io, re
NL = chr(10)

def patch(path, texts):
    s = io.open(path, encoding='utf-8').read()
    for old, new in texts:
        assert old in s, f"{path}: {old[:40]}"
        s = s.replace(old, new, 1)
    io.open(path, 'w', encoding='utf-8', newline=NL).write(s)
    print(path, 'ok')

# ── zh-TW ──
patch('README.zh-TW.md', [
("目前版本（v0.2.x）實作了完整的 Agent-OS 技術棧：\n\n- 一個 **Tauri v2 桌面工作空間**（Vue 3 + TypeScript 前端、Rust 後端）。",
"""**v0.3.0 —— 常駐服務、多前端。** 後端如今以 **`oserver`** 常駐服務的形態運行
（127.0.0.1 的 HTTP API），由它擁有 Runtime：**無論視窗開關，Employee 持續工作**。
桌面 app 成為「諸多前端之一」——任何能說 HTTP 的程式都能驅動同一個後端。

目前版本（v0.3.0）實作完整的 Agent-OS 技術棧：

- **常駐服務架構**：`ocore`（純 Rust 核心：Runtime、排程器、事件匯流排、GBrain 能力域）
  ＋ `oserver`（axum HTTP API，token 認證）＋ **Tauri v2 桌面殼**（前端之一）。
- **服務生命週期雙語意**：安裝開機服務（Employee 從開機就運行；關閉 app 毫無影響）
  或不安裝（服務隨 app 啟停）。Windows 已實作並實機驗證；Linux（systemd）與
  macOS（launchd）已實作、尚未實機驗證。"""),
("一個以 [GBrain](https://github.com/garrytan/gbrain) 為基礎的**知識圖譜層** ——",
 "- 一個以 [GBrain](https://github.com/garrytan/gbrain) 為基礎的**知識圖譜層** ——"),
("- 第一個**代理入口**：在工作空間內啟動並監看 [Claude Code](https://claude.com/claude-code)。",
"""- **Email 進出**：內建的 [obridge](obridge/) 橋接器（收信經事件進氣口喚醒對應
  Employee；Employee 透過 send 工具回信）。IM 類通道走 WASM 外掛。
- 第一個**代理入口**：在工作空間內啟動並監看 [Claude Code](https://claude.com/claude-code)。"""),
])

# 技術棧與專案結構（zh-TW）——直接替換整段
s = io.open('README.zh-TW.md', encoding='utf-8').read()
m = re.search(r'## 技術棧\n.*?\n## ', s, re.S)
tech_tw = """## 技術棧

**前端：** Vue 3 · TypeScript · Vite · Tailwind CSS v4 · Pinia · Vue Router · vue-i18n · lucide-vue-next
**核心與服務：** Rust —— `ocore`（領域核心）· `oserver`（axum 服務）· `obridge`（Email/WASM 橋接）
**桌面殼：** Tauri v2（視窗＋桌面專屬能力；所有邏輯都在服務裡）

"""
if m:
    s = s.replace(m.group(0), tech_tw + '## ', 1)
m2 = re.search(r'## 專案結構\n\n```\n.*?\n```', s, re.S)
struct_tw = """## 專案結構

```
src/              Vue 3 前端（views、Pinia stores、i18n、HTTP 包裝）
                  —— Brains、Factories、Config、員工範本／實體、
                    員工對話、Operations（即時主控台）、收件匣
ocore/            Rust 領域核心（零 Tauri 依賴）
                    domain · runtime · scheduler · event_bus · agent 狀態
                    GBrain 能力域（cli/brains/factories/converters）· llm
oserver/          常駐服務 —— axum HTTP API（token 認證）
                    agent-os 讀寫面 · GBrain 全域 · 操作主控台（ring buffer 輪詢）
                    事件進氣口 /event · 服務註冊（Windows/Linux/macOS）
src-tauri/        桌面殼（Tauri v2）—— 視窗＋桌面專屬功能
                    （Claude Code、筆記預覽）、指令薄層、服務代管
obridge/          Email 橋接器＋WASM 外掛宿主（IMAP 收／SMTP 寄）
ocontract/        共享契約型別（Operoid ↔ obridge）
handbook/         架構手冊 —— 憲法（英文 + 中文）
```"""
if m2:
    s = s.replace(m2.group(0), struct_tw, 1)
io.open('README.zh-TW.md', 'w', encoding='utf-8', newline=NL).write(s)
print('README.zh-TW.md structure ok')

# ── zh-CN ──
patch('README.zh-CN.md', [
("当前版本（v0.2.x）实现了完整的 Agent-OS 技术栈：\n\n- 一个 **Tauri v2 桌面工作空间**（Vue 3 + TypeScript 前端、Rust 后端）。",
"""**v0.3.0 —— 常驻服务、多前端。** 后端如今以 **`oserver`** 常驻服务的形态运行
（127.0.0.1 的 HTTP API），由它拥有 Runtime：**无论窗口开关，Employee 持续工作**。
桌面 app 成为「诸多前端之一」——任何能说 HTTP 的程序都能驱动同一个后端。

当前版本（v0.3.0）实现完整的 Agent-OS 技术栈：

- **常驻服务架构**：`ocore`（纯 Rust 核心：Runtime、调度器、事件总线、GBrain 能力域）
  ＋ `oserver`（axum HTTP API，token 认证）＋ **Tauri v2 桌面壳**（前端之一）。
- **服务生命周期双语意**：安装开机服务（Employee 从开机就运行；关闭 app 毫无影响）
  或不安装（服务随 app 启停）。Windows 已实现并实机验证；Linux（systemd）与
  macOS（launchd）已实现、尚未实机验证。"""),
("- 第一个**代理入口**：在工作空间内启动并监看 [Claude Code](https://claude.com/claude-code)。",
"""- **Email 进出**：内置的 [obridge](obridge/) 桥接器（收信经事件进气口唤醒对应
  Employee；Employee 通过 send 工具回信）。IM 类通道走 WASM 插件。
- 第一个**代理入口**：在工作空间内启动并监看 [Claude Code](https://claude.com/claude-code)。"""),
])

s = io.open('README.zh-CN.md', encoding='utf-8').read()
m = re.search(r'## 技术栈\n.*?\n## ', s, re.S)
tech_cn = """## 技术栈

**前端：** Vue 3 · TypeScript · Vite · Tailwind CSS v4 · Pinia · Vue Router · vue-i18n · lucide-vue-next
**核心与服务：** Rust —— `ocore`（领域核心）· `oserver`（axum 服务）· `obridge`（Email/WASM 桥接）
**桌面壳：** Tauri v2（窗口＋桌面专属能力；所有逻辑都在服务里）

"""
if m:
    s = s.replace(m.group(0), tech_cn + '## ', 1)
m2 = re.search(r'## 项目结构\n\n```\n.*?\n```', s, re.S)
struct_cn = """## 项目结构

```
src/              Vue 3 前端（views、Pinia stores、i18n、HTTP 包装）
                  —— Brains、Factories、Config、员工模板／实例、
                    员工对话、Operations（实时控制台）、收件箱
ocore/            Rust 领域核心（零 Tauri 依赖）
                    domain · runtime · scheduler · event_bus · agent 状态
                    GBrain 能力域（cli/brains/factories/converters）· llm
oserver/          常驻服务 —— axum HTTP API（token 认证）
                    agent-os 读写面 · GBrain 全域 · 操作控制台（ring buffer 轮询）
                    事件进气口 /event · 服务注册（Windows/Linux/macOS）
src-tauri/        桌面壳（Tauri v2）—— 窗口＋桌面专属功能
                    （Claude Code、笔记预览）、指令薄层、服务托管
obridge/          Email 桥接器＋WASM 插件宿主（IMAP 收／SMTP 发）
ocontract/        共享契约类型（Operoid ↔ obridge）
handbook/         架构手册 —— 宪法（英文 + 中文）
```"""
if m2:
    s = s.replace(m2.group(0), struct_cn, 1)
io.open('README.zh-CN.md', 'w', encoding='utf-8', newline=NL).write(s)
print('README.zh-CN.md structure ok')
