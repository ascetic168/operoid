import io

BASE = r"C:\Users\charl\Documents\rust\projects\Operoid"

def patch(path, repls):
    raw = io.open(path, encoding="utf-8", newline="").read()
    nl = "\r\n" if "\r\n" in raw else "\n"
    for old_lines, new_lines, cnt in repls:
        old = nl.join(old_lines); new = nl.join(new_lines)
        found = raw.count(old)
        assert found == cnt, f"{path}: x{found} (expect {cnt}): {old_lines[0][:70]}"
        raw = raw.replace(old, new)
    io.open(path, "w", encoding="utf-8", newline="").write(raw)
    print(f"{path} OK ({nl!r})")

# ───────────────────────── README.md (EN) ─────────────────────────

EN_SECTION = [
    "## A knowledge boundary the server enforces",
    "",
    "The Action Registry draws the boundary for **actions** — what an Employee may",
    "*do*. The knowledge fabric draws it for **knowledge** — what an Employee may",
    "*know*. Both boundaries share the same constitutional shape: set by humans,",
    "enforced deterministically by the server, auditable after the fact.",
    "",
    "- **Authorization happens before retrieval.** Policy is evaluated in pure,",
    "  deterministic Rust — never by an LLM, which may assist understanding but",
    "  never grants access. Unauthorized knowledge never enters the candidate set",
    "  or an LLM's context; nothing is fetched-then-hidden.",
    "- **One fabric, partitioned into scopes.** Company-common, per-department,",
    "  per-project and restricted knowledge map onto GBrain *sources*; a query",
    "  runs against only the **authorized source set**, enforced at the SQL level",
    "  inside GBrain — zero modification to GBrain itself. Personal Brains stay",
    "  physically separate.",
    "- **Fail closed.** A missing or corrupt policy means *deny*, with an event on",
    "  record — and an explicit deny rule outranks everything, including temporary",
    "  grants.",
    "- **Identity is server-side.** Every principal (human operator or AI",
    "  Employee) authenticates with its own API token; identity is resolved from",
    "  the token chain, never from what a caller claims.",
    "- **Everything privileged leaves a receipt.** Every retrieval writes a",
    "  structured record — who, on which task, under which policy version, seeing",
    "  which scopes. Grants, revocations, policy changes and identity changes are",
    "  audited events. Temporary grants expire by TTL and vanish instantly on",
    "  revocation.",
    "- **Same brain + same query + different identity = different results.** That",
    "  one-liner is proven end-to-end against a real GBrain — the smallest proof",
    "  that the knowledge fabric actually works.",
    "",
    "Everything is managed under **Settings → Knowledge**: principals & API",
    "tokens, temporary grants, and the scope→source map. Two enterprise switches",
    "ship alongside (`ops_retrieval_enabled` retires retrieval ops from the",
    "console; `claude_code_handoff_enabled` retires the Claude Code handoff).",
    "",
    "## Core concepts",
]

EN_STATUS_BULLET = [
    "- **A knowledge boundary the server enforces** (landing in the next",
    "  release): permission-aware retrieval over the knowledge graph — scopes →",
    "  sources, pre-retrieval authorization, deterministic fail-closed policy,",
    "  per-principal tokens, TTL grants, retrieval receipts, task-focused",
    "  scoping, and a **Settings → Knowledge** admin page. See",
    "  [A knowledge boundary the server enforces](#a-knowledge-boundary-the-server-enforces).",
]

patch(BASE + r"\README.md", [
    (["## Core concepts"], EN_SECTION, 1),
    (["  way: [A delegation boundary humans can draw](#a-delegation-boundary-humans-can-draw)."],
     ["  way: [A delegation boundary humans can draw](#a-delegation-boundary-humans-can-draw)."] + EN_STATUS_BULLET, 1),
    (["| **Action Registry** | The delegation boundary. Which action categories may act without asking — and which may not. |"],
     ["| **Action Registry** | The delegation boundary. Which action categories may act without asking — and which may not. |",
      "| **Knowledge Policy** | The knowledge boundary. Who may retrieve which scope — evaluated before retrieval, fail-closed. |"], 1),
    (["                    domain · runtime · scheduler · event_bus · agents state",
      "                    gbrain capabilities (cli/brains/factories/converters) · llm"],
     ["                    domain · runtime · scheduler · event_bus · agents state",
      "                    knowledge (policy/service/planner/grants/receipts/identity)",
      "                    gbrain capabilities (cli/brains/factories/converters) · llm"], 1),
    (["                    agent-os read/write · GBrain domain · operations console",
      "                    event ingress /event · service install (Win/Linux/macOS)"],
     ["                    agent-os read/write · GBrain domain · operations console",
      "                    knowledge admin (principals/tokens/grants)",
      "                    event ingress /event · service install (Win/Linux/macOS)"], 1),
])

# ───────────────────────── README.zh-TW.md ─────────────────────────

TW_SECTION = [
    "## 伺服器執行的知識界線",
    "",
    "動作類別登記表畫的是**動作**的界線——Employee 可以*做*什麼；知識布料（Enterprise C′）",
    "畫的是**知識**的界線——Employee 可以*知道*什麼。兩條界線有同一種憲法形狀：",
    "由人類畫定、由伺服器確定性執行、事後可稽核。",
    "",
    "- **授權發生在檢索之前。** 政策以純 Rust 確定性評估——LLM 可以協助理解查詢，",
    "  但永不參與授權。未授權的知識不會進入候選集，更不會進 LLM 的上下文；",
    "  沒有任何「先取回再遮蔽」。",
    "- **一塊布料，按範圍分區。** 全公司／部門／專案／受限知識映射到 GBrain 的",
    "  *source*；每次查詢只對**授權 source 集合**發出，由 GBrain 在 SQL 層強制——",
    "  零修改 GBrain 本體。個人腦維持物理隔離。",
    "- **Fail closed。** 政策缺失或損壞＝拒絕並記錄事件；明示 DENY 規則高於一切，",
    "  連臨時授權都無法架空。",
    "- **身份由伺服器端裁定。** 每個 principal（人類操作者或 AI 員工）以自己的",
    "  API token 認證；身份出自 token 鏈，永不出自呼叫端的自稱。",
    "- **特權行為皆留收據。** 每次檢索寫下結構化紀錄——誰、在哪個任務、依哪版政策、",
    "  看到哪些範圍。grant 的發放／撤銷／屆期、政策變更、身份變更都是稽核事件。",
    "  臨時授權依 TTL 到期，撤銷即時失效。",
    "- **同一腦＋同一查詢＋不同身份＝不同結果。** 這句話已對真實 GBrain 做了",
    "  端到端實證——它是知識布料真的在運作的最小證明。",
    "",
    "一切在「**設定 → 知識授權**」管理：principals 與 API tokens、臨時授權、",
    "範圍→source 映射。隨附兩個企業面開關（`ops_retrieval_enabled` 自主控台退役檢索 op；",
    "`claude_code_handoff_enabled` 退役 Claude Code handoff）。",
    "",
    "## 核心概念",
]

TW_STATUS_BULLET = [
    "- **伺服器執行的知識界線**（隨下一版發布）：知識圖譜上的**授權檢索**——範圍分區、",
    "  檢索前授權、確定性 fail-closed 政策、per-principal token、臨時授權（TTL）、",
    "  檢索收據、任務聚焦，以及「設定 → 知識授權」管理頁。",
    "  見[伺服器執行的知識界線](#伺服器執行的知識界線)。",
]

patch(BASE + r"\README.zh-TW.md", [
    (["## 核心概念"], TW_SECTION, 1),
    (["  界線的「為什麼」與「形狀」：[人類畫得出的委任界線](#人類畫得出的委任界線)。"],
     ["  界線的「為什麼」與「形狀」：[人類畫得出的委任界線](#人類畫得出的委任界線)。"] + TW_STATUS_BULLET, 1),
    (["| **Action Registry 動作類別登記表** | 委任界線。哪些動作類別可免請示而行——由人類畫定、可稽核。 |"],
     ["| **Action Registry 動作類別登記表** | 委任界線。哪些動作類別可免請示而行——由人類畫定、可稽核。 |",
      "| **Knowledge Policy 知識政策** | 知識界線。誰可檢索哪個範圍——檢索前評估、fail closed。 |"], 1),
    (["                    GBrain 能力域（cli/brains/factories/converters）· llm"],
     ["                    知識授權（policy/service/planner/grants/receipts/identity）",
      "                    GBrain 能力域（cli/brains/factories/converters）· llm"], 1),
    (["                    agent-os 讀寫面 · GBrain 全域 · 操作主控台（ring buffer 輪詢）",
      "                    事件進氣口 /event · 服務註冊（Windows/Linux/macOS）"],
     ["                    agent-os 讀寫面 · GBrain 全域 · 操作主控台（ring buffer 輪詢）",
      "                    知識授權管理（principals/tokens/grants）",
      "                    事件進氣口 /event · 服務註冊（Windows/Linux/macOS）"], 1),
])

# ───────────────────────── README.zh-CN.md ─────────────────────────

CN_SECTION = [
    "## 服务器执行的知识边界",
    "",
    "动作类别登记表画的是**动作**的边界——Employee 可以*做*什么；知识布料（Enterprise C′）",
    "画的是**知识**的边界——Employee 可以*知道*什么。两条边界有同一种宪法形状：",
    "由人类画定、由服务器确定性执行、事后可稽核。",
    "",
    "- **授权发生在检索之前。** 政策以纯 Rust 确定性评估——LLM 可以协助理解查询，",
    "  但永不参与授权。未授权的知识不会进入候选集，更不会进 LLM 的上下文；",
    "  没有任何「先取回再遮蔽」。",
    "- **一块布料，按范围分区。** 全公司／部门／项目／受限知识映射到 GBrain 的",
    "  *source*；每次查询只对**授权 source 集合**发出，由 GBrain 在 SQL 层强制——",
    "  零修改 GBrain 本体。个人脑维持物理隔离。",
    "- **Fail closed。** 政策缺失或损坏＝拒绝并记录事件；明示 DENY 规则高于一切，",
    "  连临时授权都无法架空。",
    "- **身份由服务器端裁定。** 每个 principal（人类操作者或 AI 员工）以自己的",
    "  API token 认证；身份出自 token 链，永不出自调用端的自称。",
    "- **特权行为皆留收据。** 每次检索写下结构化纪录——谁、在哪个任务、依哪版政策、",
    "  看到哪些范围。grant 的发放／撤销／届期、政策变更、身份变更都是稽核事件。",
    "  临时授权依 TTL 到期，撤销即时失效。",
    "- **同一脑＋同一查询＋不同身份＝不同结果。** 这句话已对真实 GBrain 做了",
    "  端到端实证——它是知识布料真的在运作的最小证明。",
    "",
    "一切在「**设置 → 知识授权**」管理：principals 与 API tokens、临时授权、",
    "范围→source 映射。随附两个企业面开关（`ops_retrieval_enabled` 自控制台退役检索 op；",
    "`claude_code_handoff_enabled` 退役 Claude Code handoff）。",
    "",
    "## 核心概念",
]

CN_STATUS_BULLET = [
    "- **服务器执行的知识边界**（随下一版发布）：知识图谱上的**授权检索**——范围分区、",
    "  检索前授权、确定性 fail-closed 政策、per-principal token、临时授权（TTL）、",
    "  检索收据、任务聚焦，以及「设置 → 知识授权」管理页。",
    "  见[服务器执行的知识边界](#服务器执行的知识边界)。",
]

patch(BASE + r"\README.zh-CN.md", [
    (["## 核心概念"], CN_SECTION, 1),
    (["  界线的「为什么」与「形状」：[人类画得出的委任界线](#人类画得出的委任界线)。"],
     ["  界线的「为什么」与「形状」：[人类画得出的委任界线](#人类画得出的委任界线)。"] + CN_STATUS_BULLET, 1),
    (["| **Action Registry 动作类别登记表** | 委任边界。哪些动作类别可免请示而行——由人类画定、可稽核。 |"],
     ["| **Action Registry 动作类别登记表** | 委任边界。哪些动作类别可免请示而行——由人类画定、可稽核。 |",
      "| **Knowledge Policy 知识政策** | 知识边界。谁可检索哪个范围——检索前评估、fail closed。 |"], 1),
    (["                    GBrain 能力域（cli/brains/factories/converters）· llm"],
     ["                    知识授权（policy/service/planner/grants/receipts/identity）",
      "                    GBrain 能力域（cli/brains/factories/converters）· llm"], 1),
    (["                    agent-os 读写面 · GBrain 全域 · 操作控制台（ring buffer 轮询）",
      "                    事件进气口 /event · 服务注册（Windows/Linux/macOS）"],
     ["                    agent-os 读写面 · GBrain 全域 · 操作控制台（ring buffer 轮询）",
      "                    知识授权管理（principals/tokens/grants）",
      "                    事件进气口 /event · 服务注册（Windows/Linux/macOS）"], 1),
])
