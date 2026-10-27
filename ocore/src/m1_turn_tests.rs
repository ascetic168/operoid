//! M1 native tool-loop 測試——自包含 fixtures（不依賴 `tests` 模組的私有 helpers）。

use super::*;
use crate::domain::tools::{ChatStepFuture, ToolFuture};
use crate::domain::JsonStore;
use crate::llm::{ChatMessage, LlmTurn, ToolCall, ToolDef};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

fn test_dir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("operoid-m1-turn-test-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 固定回覆的知識工具 stub（計呼叫次數）。
struct StubTool {
    spec: ToolSpec,
    canned: String,
    calls: AtomicU32,
}
impl StubTool {
    fn new(canned: impl Into<String>) -> Self {
        Self {
            spec: ToolSpec { id: "stub".into(), description: "stub".into() },
            canned: canned.into(),
            calls: AtomicU32::new(0),
        }
    }
    fn call_count(&self) -> u32 {
        self.calls.load(Ordering::SeqCst)
    }
}
impl Tool for StubTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn invoke<'a>(&'a self, _input: ToolInput, _ctx: &'a ToolCtx) -> ToolFuture<'a> {
        let text = self.canned.clone();
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(ToolOutput { text, meta: serde_json::json!({}), images: Vec::new() }) })
    }
}

fn outbound_disabled() -> crate::outbound::OutboundConfig {
    crate::outbound::OutboundConfig::default()
}

fn seed(store: &JsonStore) -> String {
    store
        .put_workspace(&Workspace {
            id: "ws".into(),
            name: "WS".into(),
            description: None,
            status: WorkspaceStatus::Active,
            created_at: "t".into(),
        })
        .unwrap();
    let emp_id = "emp".to_string();
    store
        .put_employee(&Employee {
            id: emp_id.clone(),
            workspace_id: "ws".into(),
            name: "E".into(),
            brain: BrainRef { brain_id: "__default__".into() },
            role: None,
            description: None,
            email: None,
            template_id: None,
            state: EmployeeState::Sleeping,
            archived: false,
            tools: None,
            created_at: "t".into(),
            owner_principal: None,
        })
        .unwrap();
    emp_id
}

fn seed_task(store: &JsonStore, emp_id: &str, input: &str) {
    store
        .put_task(&Task {
            id: "m1".into(),
            workspace_id: "ws".into(),
            owner_employee_id: emp_id.to_string(),
            objective: "Human message".into(),
            input: input.into(),
            status: TaskStatus::Assigned,
            output_artifact_id: None,
            commitment_id: None,
            project_id: None,
            external_reply_to: None,
            external_source: None,
            occurred_at: None,
            sender: None,
            created_at: "t".into(),
        })
        .unwrap();
}

fn ctx() -> ToolCtx {
    ToolCtx {
        gbrain_exe: String::new(),
        gbrain_home: None,
        chat_model: None,
        mcp: None,
        allowed_tools: Default::default(),
        employee_output_root: std::env::temp_dir(),
        registry: None,
        access: crate::knowledge::identity::test_default(),
        knowledge: None,
        turn_max_steps: 40,
        subagent_max_steps: 20,
        internal_mail_enabled: true,
        wake: None,
        tool_result_max_chars: 8_000,
        turn_deltas: None,
    }
}

/// native 協議 stub：`supports_native_tools()=true`，`chat_step` 依序回預錄 LlmTurn
/// （耗盡則回純文字 "done"）。
struct NativeStubReasoner {
    turns: std::sync::Mutex<std::collections::VecDeque<LlmTurn>>,
}
impl NativeStubReasoner {
    fn new(turns: Vec<LlmTurn>) -> Self {
        Self { turns: std::sync::Mutex::new(turns.into_iter().collect()) }
    }
    fn call(name: &str, args: serde_json::Value) -> LlmTurn {
        LlmTurn {
            content: None,
            tool_calls: vec![ToolCall { id: "call-1".into(), name: name.into(), arguments: args }],
            finish_reason: Some("tool_calls".into()),
            usage: None,
        }
    }
    fn text(t: &str) -> LlmTurn {
        LlmTurn {
            content: Some(t.into()),
            tool_calls: vec![],
            finish_reason: Some("stop".into()),
            usage: None,
        }
    }
}
impl Reasoner for NativeStubReasoner {
    fn reason<'a>(&'a self, _s: &'a str, _u: &'a str) -> ReasonerFuture<'a> {
        Box::pin(async { anyhow::bail!("native stub 不走 legacy") })
    }
    fn supports_native_tools(&self) -> bool {
        true
    }
    fn chat_step<'a>(&'a self, _m: &'a [ChatMessage], _t: &'a [ToolDef]) -> ChatStepFuture<'a> {
        let next = self
            .turns
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Self::text("done"));
        Box::pin(async move { Ok(next) })
    }
}

/// native 全流程：think（入 artifact）→ 純文字最終回覆（寫 Out Message）。
#[tokio::test]
async fn conversational_native_think_then_reply() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "蝕刻良率怎麼了？");
    let tool = StubTool::new("良率因 RF matching 接點氧化下降");
    let reasoner = NativeStubReasoner::new(vec![
        NativeStubReasoner::call("gbrain_think", serde_json::json!({"query": "蝕刻良率"})),
        NativeStubReasoner::text("主因是接點氧化，已換件。"),
    ]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    let msgs = store.list_messages_by_employee(&emp_id, 10).unwrap();
    assert_eq!(msgs.len(), 1, "一則最終回覆");
    assert_eq!(msgs[0].direction, MessageDirection::Out);
    assert!(msgs[0].text.contains("接點氧化"));
    assert_eq!(store.list_artifacts("ws").unwrap().len(), 1, "think 證據入庫");
    assert_eq!(tool.call_count(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

/// native：未知工具回錯誤 note 續跑（不中斷回合），模型改用正確工具後收斂。
#[tokio::test]
async fn conversational_native_unknown_tool_recovers() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "查一下出貨狀態");
    let tool = StubTool::new("出貨正常");
    let reasoner = NativeStubReasoner::new(vec![
        NativeStubReasoner::call("bash", serde_json::json!({"command": "ls"})), // 幻覺工具
        NativeStubReasoner::call("gbrain_search", serde_json::json!({"query": "出貨狀態"})),
        NativeStubReasoner::text("出貨正常，無異常。"),
    ]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    assert_eq!(tool.call_count(), 1, "幻覺工具不應打到底層");
    let msgs = store.list_messages_by_employee(&emp_id, 10).unwrap();
    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].text.contains("出貨正常"));
    std::fs::remove_dir_all(&dir).ok();
}

/// native：finish 工具不帶 text → silent（不回覆），task 照常完成。
#[tokio::test]
async fn conversational_native_finish_silent() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "純通知");
    let tool = StubTool::new("x");
    let reasoner = NativeStubReasoner::new(vec![NativeStubReasoner::call("finish", serde_json::json!({}))]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    assert!(store.list_messages_by_employee(&emp_id, 10).unwrap().is_empty(), "不回覆");
    let task = store.get_task("m1").unwrap().unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
    std::fs::remove_dir_all(&dir).ok();
}

/// native：同通道單一回覆保證（send 後純文字不再重複寫）。
#[tokio::test]
async fn conversational_native_send_then_text_no_duplicate() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "早安！");
    let tool = StubTool::new("x");
    let reasoner = NativeStubReasoner::new(vec![
        NativeStubReasoner::call("send_message", serde_json::json!({"text": "早安，有什麼需要？"})),
        NativeStubReasoner::text("早安，有什麼需要？"),
    ]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    let msgs = store.list_messages_by_employee(&emp_id, 10).unwrap();
    assert_eq!(msgs.len(), 1, "send 已回覆過，純文字不重複寫");
    assert_eq!(msgs[0].text, "早安，有什麼需要？");
    std::fs::remove_dir_all(&dir).ok();
}

/// auto 降級：native 首步遭 provider 拒（ToolsUnsupported）→ 當場降級 legacy，
/// 之後的 legacy JSON 動作照常執行、回合正常收斂。
#[tokio::test]
async fn conversational_downgrades_to_legacy_on_unsupported() {
    struct Downgrading;
    impl Reasoner for Downgrading {
        fn reason<'a>(&'a self, _s: &'a str, _u: &'a str) -> ReasonerFuture<'a> {
            Box::pin(async { Ok(serde_json::json!({"action": "finish", "text": "已降級回覆。"})) })
        }
        fn supports_native_tools(&self) -> bool {
            true
        }
        fn chat_step<'a>(&'a self, _m: &'a [ChatMessage], _t: &'a [ToolDef]) -> ChatStepFuture<'a> {
            Box::pin(async { Err(anyhow::Error::new(crate::llm::ToolsUnsupported).context("stub")) })
        }
    }
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "你好");
    let tool = StubTool::new("x");
    run_inbox(&emp_id, &tool, Some(&Downgrading), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    let msgs = store.list_messages_by_employee(&emp_id, 10).unwrap();
    assert_eq!(msgs.len(), 1, "降級後 legacy 路徑照常回覆");
    assert!(msgs[0].text.contains("降級回覆"));
    std::fs::remove_dir_all(&dir).ok();
}

/// native：update_todos 寫入 Memory（落庫、跨回合存活）＋結果 note 帶清單狀態。
#[tokio::test]
async fn conversational_native_update_todos_persists() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "幫我追蹤這件事");
    let tool = StubTool::new("x");
    let reasoner = NativeStubReasoner::new(vec![
        NativeStubReasoner::call(
            "update_todos",
            serde_json::json!({"todos": [
                {"content": "蒐集出貨紀錄", "status": "completed", "priority": "high"},
                {"content": "比對差異", "status": "in_progress"},
                {"content": "回報客戶", "status": "pending", "priority": "low"}
            ]}),
        ),
        NativeStubReasoner::text("已建立追蹤清單。"),
    ]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    let memory = store.get_memory(&emp_id).unwrap().unwrap();
    assert_eq!(memory.todos.len(), 3, "todos 應落庫");
    assert_eq!(memory.todos[0].content, "蒐集出貨紀錄");
    assert_eq!(memory.todos[0].status, "completed");
    assert_eq!(memory.todos[2].priority.as_deref(), Some("low"));
    // 回合仍正常收斂（有最終回覆）。
    let msgs = store.list_messages_by_employee(&emp_id, 10).unwrap();
    assert!(msgs.iter().any(|m| m.text.contains("追蹤清單")));
    std::fs::remove_dir_all(&dir).ok();
}

/// format_todo_state：狀態標記與空清單語意。
#[test]
fn todo_state_format_marks_status() {
    let todos = vec![
        crate::domain::TodoItem {
            content: "完成項".into(),
            status: "completed".into(),
            priority: None,
        },
        crate::domain::TodoItem {
            content: "進行中項".into(),
            status: "in_progress".into(),
            priority: Some("high".into()),
        },
        crate::domain::TodoItem {
            content: "待辦項".into(),
            status: "pending".into(),
            priority: None,
        },
    ];
    let s = format_todo_state(&todos);
    assert!(s.contains("- [x] 完成項"), "{s}");
    assert!(s.contains("- [~] 進行中項（高）"), "{s}");
    assert!(s.contains("- [ ] 待辦項"), "{s}");
    assert!(format_todo_state(&[]).contains("尚未建立"), "{}", format_todo_state(&[]));
}

/// 通道情境：email 事件的 task 注入來源／事件時間／回覆通道；人類訊息則為空。
#[test]
fn channel_context_line_covers_source_and_time() {
    let mk = |src: Option<&str>, occ: Option<&str>, reply: Option<&str>| Task {
        id: "t".into(),
        workspace_id: "ws".into(),
        owner_employee_id: "e".into(),
        objective: "Human message".into(),
        input: "x".into(),
        status: TaskStatus::Assigned,
        output_artifact_id: None,
        commitment_id: None,
        project_id: None,
        external_reply_to: reply.map(str::to_string),
        external_source: src.map(str::to_string),
        occurred_at: occ.map(str::to_string),
        sender: None,
        created_at: "t".into(),
    };
    let line = channel_context_line(&mk(Some("email"), Some("2026-10-05T09:00:00Z"), Some("msg-9")));
    assert!(line.contains("來源通道：email"), "{line}");
    assert!(line.contains("2026-10-05T09:00:00Z"), "{line}");
    assert!(line.contains("回覆將送回原通道"), "{line}");
    assert!(channel_context_line(&mk(None, None, None)).is_empty(), "人類訊息無通道行");
}

/// native 初始歷史：身分卡（含員工名稱）在最前、本則任務在最後；
/// 與任務輸入重複的喚醒 In message 不重複進歷史；近端對話 In→user／Out→assistant。
#[test]
fn native_history_identity_and_wake_dedup() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    let put = |id: &str, dirn: MessageDirection, text: &str, at: &str| {
        store
            .put_message(&Message {
                id: id.into(),
                workspace_id: "ws".into(),
                employee_id: emp_id.clone(),
                direction: dirn,
                text: text.into(),
                proposed_commitment_id: None,
                source_commitment_id: None,
                artifact_id: None,
                sender: None,
                created_at: at.into(),
            })
            .unwrap();
    };
    put("in-1", MessageDirection::In, "上次的報告呢？", "t1");
    put("out-1", MessageDirection::Out, "正在整理。", "t2");
    put("in-2", MessageDirection::In, "今天進度如何？", "t3"); // 與 task.input 相同 → 去重
    let emp = store.get_employee(&emp_id).unwrap().unwrap();
    let task = Task {
        id: "m1".into(),
        workspace_id: "ws".into(),
        owner_employee_id: emp_id.clone(),
        objective: "Human message".into(),
        input: "今天進度如何？".into(),
        status: TaskStatus::Assigned,
        output_artifact_id: None,
        commitment_id: None,
        project_id: None,
        external_reply_to: None,
        external_source: None,
        occurred_at: None,
        sender: None,
        created_at: "t".into(),
    };
    let history = build_native_turn_history(&emp, &task, &ctx(), "", &store).unwrap();
    assert!(matches!(history[0].role, crate::llm::ChatRole::System));
    assert!(history[0].content.contains("「E」"), "身分卡含員工名稱：{}", history[0].content);
    assert_eq!(history.len(), 4, "{history:?}");
    assert!(matches!(history[1].role, crate::llm::ChatRole::User));
    assert!(history[1].content.contains("上次的報告呢"));
    assert!(matches!(history[2].role, crate::llm::ChatRole::Assistant));
    let last = history.last().unwrap();
    assert!(last.content.contains("今天進度如何"), "{last:?}");
    assert!(matches!(last.role, crate::llm::ChatRole::User));
    std::fs::remove_dir_all(&dir).ok();
}

/// 對話回合過程可見性：native 步的每個工具呼叫落一筆 `tool_call` 事件
/// （detail JSON 契約 v1：v/step/tool/args/status/ms/note；超長 args/note 截斷）。
#[tokio::test]
async fn conversational_native_tool_call_events_recorded() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "蝕刻良率怎麼了？");
    let tool = StubTool::new("良率正常");
    // 超過 400 字元的 args（與其衍生的超長 note）→ 兩者都應被字元級截斷。
    let long_query = "查".repeat(600);
    let reasoner = NativeStubReasoner::new(vec![
        NativeStubReasoner::call("gbrain_think", serde_json::json!({ "query": long_query })),
        NativeStubReasoner::call("gbrain_search", serde_json::json!({"query": "蝕刻良率"})),
        NativeStubReasoner::text("良率正常。"),
    ]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    let mut calls: Vec<_> = store
        .list_events_by_employee(&emp_id, 100)
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == "tool_call")
        .collect();
    calls.reverse(); // rowid DESC → 時序
    assert_eq!(calls.len(), 2, "兩個工具呼叫各一筆（最終純文字非工具）：{:?}", calls);
    // 第 1 步 think：超長 args/note 均截斷（400/300 字＋…）。
    let d: serde_json::Value = serde_json::from_str(&calls[0].detail).unwrap();
    assert_eq!(d["v"], 1);
    assert_eq!(d["step"], 1);
    assert_eq!(d["tool"], "gbrain_think");
    assert_eq!(d["status"], "ok");
    assert!(d["ms"].is_u64(), "ms 應為數字：{d}");
    let args = d["args"].as_str().unwrap();
    assert_eq!(args.chars().count(), 401, "400 字＋…：{args}");
    assert!(args.ends_with('…'), "截斷以…標記");
    let note = d["note"].as_str().unwrap();
    assert_eq!(note.chars().count(), 301, "300 字＋…：{note}");
    assert!(note.starts_with("[think「"), "note 帶動作前綴：{note}");
    // 第 2 步 search：短查詢 → note 帶結果內容摘要。
    let d2: serde_json::Value = serde_json::from_str(&calls[1].detail).unwrap();
    assert_eq!(d2["step"], 2);
    assert_eq!(d2["tool"], "gbrain_search");
    assert!(d2["note"].as_str().unwrap().contains("良率正常"), "note 帶結果摘要：{d2}");
    std::fs::remove_dir_all(&dir).ok();
}

/// 未知工具的過程記錄：status=unknown（不中斷回合），後續正確工具為 ok。
#[tokio::test]
async fn conversational_native_unknown_tool_event_marked() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "查一下出貨狀態");
    let tool = StubTool::new("出貨正常");
    let reasoner = NativeStubReasoner::new(vec![
        NativeStubReasoner::call("bash", serde_json::json!({"command": "ls"})), // 幻覺工具
        NativeStubReasoner::call("gbrain_search", serde_json::json!({"query": "出貨狀態"})),
        NativeStubReasoner::text("出貨正常。"),
    ]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    let calls: Vec<_> = store
        .list_events_by_employee(&emp_id, 100)
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == "tool_call")
        .collect();
    let by_tool = |name: &str| {
        calls
            .iter()
            .find(|e| {
                serde_json::from_str::<serde_json::Value>(&e.detail)
                    .ok()
                    .and_then(|d| d["tool"].as_str().map(|t| t == name))
                    .unwrap_or(false)
            })
            .map(|e| serde_json::from_str::<serde_json::Value>(&e.detail).unwrap())
            .unwrap_or_else(|| panic!("找不到 {name} 的 tool_call 事件：{:?}", calls))
    };
    assert_eq!(by_tool("bash")["status"], "unknown", "幻覺工具標 unknown");
    assert_eq!(by_tool("gbrain_search")["status"], "ok");
    std::fs::remove_dir_all(&dir).ok();
}

// ───────────────── F1 串流：回合 delta 匯流排 ─────────────────

/// F1 串流 stub：文字片段經 `chat_step_streaming` 的 on_text 腳本化回呼。
struct StreamingStubReasoner {
    turns: std::sync::Mutex<std::collections::VecDeque<(LlmTurn, Vec<String>)>>,
    /// 每步的思考流增量（v0.5.8；佇列耗盡或未注入＝該步無 ThinkingDelta）。
    thinking: std::sync::Mutex<std::collections::VecDeque<Vec<String>>>,
}
impl StreamingStubReasoner {
    fn new(turns: Vec<(LlmTurn, Vec<String>)>) -> Self {
        Self {
            turns: std::sync::Mutex::new(turns.into_iter().collect()),
            thinking: std::sync::Mutex::new(std::collections::VecDeque::new()),
        }
    }
    fn with_thinking(turns: Vec<(LlmTurn, Vec<String>)>, thinking: Vec<Vec<String>>) -> Self {
        Self {
            turns: std::sync::Mutex::new(turns.into_iter().collect()),
            thinking: std::sync::Mutex::new(thinking.into_iter().collect()),
        }
    }
}
impl Reasoner for StreamingStubReasoner {
    fn reason<'a>(&'a self, _s: &'a str, _u: &'a str) -> ReasonerFuture<'a> {
        Box::pin(async { anyhow::bail!("streaming stub 不走 legacy") })
    }
    fn supports_native_tools(&self) -> bool {
        true
    }
    fn chat_step<'a>(&'a self, _m: &'a [ChatMessage], _t: &'a [ToolDef]) -> ChatStepFuture<'a> {
        let next = self
            .turns
            .lock()
            .unwrap()
            .pop_front()
            .map(|(t, _)| t)
            .unwrap_or_else(|| NativeStubReasoner::text("done"));
        Box::pin(async move { Ok(next) })
    }
    fn chat_step_streaming<'a>(
        &'a self,
        _m: &'a [ChatMessage],
        _t: &'a [ToolDef],
        on_text: &'a (dyn Fn(&str) + Send + Sync),
        on_thinking: &'a (dyn Fn(&str) + Send + Sync),
    ) -> ChatStepFuture<'a> {
        let next = self.turns.lock().unwrap().pop_front();
        let thinking = self
            .thinking
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_default();
        Box::pin(async move {
            let (turn, deltas) = next.unwrap_or_else(|| (NativeStubReasoner::text("done"), Vec::new()));
            for d in &thinking {
                on_thinking(d);
            }
            for d in &deltas {
                on_text(d);
            }
            Ok(turn)
        })
    }
}

/// 訂閱匯流排直到 TurnEnd（回完整 delta 序列）。
async fn drain_until_turn_end(
    rx: &mut tokio::sync::broadcast::Receiver<crate::agent_state::TurnDelta>,
) -> Vec<crate::agent_state::TurnDelta> {
    let mut out = Vec::new();
    loop {
        match rx.recv().await {
            Ok(ev) => {
                let end = matches!(ev, crate::agent_state::TurnDelta::TurnEnd { .. });
                out.push(ev);
                if end {
                    return out;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return out,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
        }
    }
}

/// F1：native 串流回合——StepStart/TextDelta/TextEnd/TurnEnd 全序列、順序正確、
/// 最終 Message 內容不變（DB 語意零改動）；tool 步的草稿標記收回（TextEnd{false}）。
#[tokio::test]
async fn conversational_native_streams_deltas_to_bus() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "蝕刻良率怎麼了？");
    let tool = StubTool::new("良率因 RF matching 接點氧化下降");
    let reasoner = StreamingStubReasoner::new(vec![
        (
            NativeStubReasoner::call("gbrain_think", serde_json::json!({"query": "蝕刻良率"})),
            vec!["我先查一下".to_string(), "良率資料".to_string()],
        ),
        (
            NativeStubReasoner::text("主因是接點氧化，已換件。"),
            vec!["主因是".to_string(), "接點氧化，已換件。".to_string()],
        ),
    ]);
    // 匯流排：AppState::new 內建（簽名不變）；訂閱先於回合。
    let (wtx, _wrx) = tokio::sync::mpsc::channel(8);
    let (etx, _erx) = tokio::sync::mpsc::channel(8);
    let state = crate::agent_state::AppState::new(wtx, etx, 4);
    let mut rx = state.subscribe_turn_deltas();
    let mut ctx = ctx();
    ctx.turn_deltas = Some(state.turn_delta_sender());
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx, &store, &outbound_disabled())
        .await
        .unwrap();
    let deltas = drain_until_turn_end(&mut rx).await;
    use crate::agent_state::TurnDelta as TD;
    let kinds: Vec<String> = deltas
        .iter()
        .map(|d| match d {
            TD::StepStart { .. } => "step_start".into(),
            TD::TextDelta { text, .. } => format!("text:{text}"),
            TD::TextEnd { final_reply, .. } => format!("text_end:{}", final_reply),
            TD::ThinkingDelta { .. } => "thinking".into(),
            TD::ToolCallStart { name, .. } => format!("tool_start:{name}"),
            TD::ToolCallEnd { name, status, .. } => format!("tool_end:{name}:{status}"),
            TD::TurnEnd { .. } => "turn_end".into(),
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "step_start",                     // 步 1（tool 步）
            "text:我先查一下",                 // 中間步文字照發（草稿）
            "text:良率資料",
            "text_end:false",                 // 轉 tool_calls → 收回
            "tool_start:gbrain_think",        // v0.5.8：工具開始/結束幀（即時）
            "tool_end:gbrain_think:ok",
            "step_start",                     // 步 2（終局文字）
            "text:主因是",
            "text:接點氧化，已換件。",
            "text_end:true",                  // 終局 → 等最終訊息替換
            "turn_end",
        ]
    );
    // DB 語意不變：最終 Message 一次寫入、內容＝串流文字全量。
    let msgs = store.list_messages_by_employee(&emp_id, 10).unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].text, "主因是接點氧化，已換件。");
    std::fs::remove_dir_all(&dir).ok();
}

/// v0.5.8 過程可見性：思考流（reasoning_content）增量 → ThinkingDelta 幀即時上匯流排；
/// 思考內容不入對話——最終 Message 仍為 content 全量（DB 語意零改動）。
#[tokio::test]
async fn conversational_native_streams_thinking_deltas_to_bus() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "蝕刻良率怎麼了？");
    let tool = StubTool::new("良率因 RF matching 接點氧化下降");
    let reasoner = StreamingStubReasoner::with_thinking(
        vec![
            (
                NativeStubReasoner::call("gbrain_think", serde_json::json!({"query": "蝕刻良率"})),
                vec![],
            ),
            (
                NativeStubReasoner::text("主因是接點氧化。"),
                vec!["主因是".to_string(), "接點氧化。".to_string()],
            ),
        ],
        vec![
            vec!["先確認機台".to_string(), "，再看良率趨勢".to_string()],
            vec!["綜合檢索結果".to_string()],
        ],
    );
    let (wtx, _wrx) = tokio::sync::mpsc::channel(8);
    let (etx, _erx) = tokio::sync::mpsc::channel(8);
    let state = crate::agent_state::AppState::new(wtx, etx, 4);
    let mut rx = state.subscribe_turn_deltas();
    let mut ctx = ctx();
    ctx.turn_deltas = Some(state.turn_delta_sender());
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx, &store, &outbound_disabled())
        .await
        .unwrap();
    let deltas = drain_until_turn_end(&mut rx).await;
    use crate::agent_state::TurnDelta as TD;
    let thinking: Vec<&str> = deltas
        .iter()
        .filter_map(|d| match d {
            TD::ThinkingDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        thinking,
        vec!["先確認機台", "，再看良率趨勢", "綜合檢索結果"],
        "思考流增量應全數上匯流排"
    );
    // 幀序：stub 先思考後文字——每步 ThinkingDelta 應在該步 TextDelta 之前。
    let first_think = deltas
        .iter()
        .position(|d| matches!(d, TD::ThinkingDelta { .. }))
        .unwrap();
    let first_text = deltas
        .iter()
        .position(|d| matches!(d, TD::TextDelta { .. }))
        .unwrap();
    assert!(first_think < first_text);
    let msgs = store.list_messages_by_employee(&emp_id, 10).unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].text, "主因是接點氧化。", "思考不得混入最終訊息");
    std::fs::remove_dir_all(&dir).ok();
}

/// F1：非串流 Reasoner（預設 chat_step_streaming 委派 chat_step）→ 僅生命週期幀
/// （StepStart×步數＋TurnEnd），無 TextDelta——DR-4 降級路徑。
#[tokio::test]
async fn conversational_non_streaming_reasoner_emits_lifecycle_only() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "查一下出貨狀態");
    let tool = StubTool::new("出貨正常");
    let reasoner = NativeStubReasoner::new(vec![NativeStubReasoner::text("出貨正常，無異常。")]);
    let (wtx, _wrx) = tokio::sync::mpsc::channel(8);
    let (etx, _erx) = tokio::sync::mpsc::channel(8);
    let state = crate::agent_state::AppState::new(wtx, etx, 4);
    let mut rx = state.subscribe_turn_deltas();
    let mut ctx = ctx();
    ctx.turn_deltas = Some(state.turn_delta_sender());
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx, &store, &outbound_disabled())
        .await
        .unwrap();
    let deltas = drain_until_turn_end(&mut rx).await;
    use crate::agent_state::TurnDelta as TD;
    assert!(
        deltas.iter().all(|d| !matches!(d, TD::TextDelta { .. })),
        "非串流不得發 TextDelta：{:?}",
        deltas
    );
    assert_eq!(deltas.iter().filter(|d| matches!(d, TD::StepStart { .. })).count(), 1);
    // 生命週期幀對非串流步同樣發佈（客戶端步驟進度信號與串流步同構）：
    // StepStart → TextEnd{final_reply:true} → TurnEnd。
    assert_eq!(deltas.len(), 3, "StepStart＋TextEnd＋TurnEnd：{:?}", deltas.len());
    assert!(matches!(
        deltas.get(1),
        Some(TD::TextEnd { final_reply: true, .. })
    ));
    std::fs::remove_dir_all(&dir).ok();
}

// ───────────────── 子代理（spawn_subagent，D-S1..S4）─────────────────

/// legacy 文字 JSON 協議 stub（自包含；依序回預錄 JSON，耗盡則回 done）。
struct LegacyJsonStubReasoner {
    responses: std::sync::Mutex<std::collections::VecDeque<String>>,
}
impl Reasoner for LegacyJsonStubReasoner {
    fn reason<'a>(&'a self, _s: &'a str, _u: &'a str) -> crate::domain::tools::ReasonerFuture<'a> {
        let resp = {
            let mut g = self.responses.lock().unwrap();
            g.pop_front().unwrap_or_else(|| "{\"done\": true}".into())
        };
        Box::pin(async move {
            let v: serde_json::Value = serde_json::from_str(&resp)
                .unwrap_or_else(|_| serde_json::json!({"done": true}));
            Ok(v)
        })
    }
}

/// 錄製每次 chat_step 收到的歷史（fan-out 的 tool_call_id 配對驗證用）。
struct CapturingStubReasoner {
    turns: std::sync::Mutex<std::collections::VecDeque<LlmTurn>>,
    seen: std::sync::Mutex<Vec<Vec<ChatMessage>>>,
}
impl CapturingStubReasoner {
    fn new(turns: Vec<LlmTurn>) -> Self {
        Self {
            turns: std::sync::Mutex::new(turns.into_iter().collect()),
            seen: std::sync::Mutex::new(Vec::new()),
        }
    }
}
impl Reasoner for CapturingStubReasoner {
    fn reason<'a>(&'a self, _s: &'a str, _u: &'a str) -> crate::domain::tools::ReasonerFuture<'a> {
        Box::pin(async { anyhow::bail!("capturing stub 不走 legacy") })
    }
    fn supports_native_tools(&self) -> bool {
        true
    }
    fn chat_step<'a>(&'a self, m: &'a [ChatMessage], _t: &'a [ToolDef]) -> ChatStepFuture<'a> {
        let next = {
            let mut seen = self.seen.lock().unwrap();
            seen.push(m.to_vec());
            let mut g = self.turns.lock().unwrap();
            g.pop_front().unwrap_or_else(|| NativeStubReasoner::text("done"))
        };
        Box::pin(async move { Ok(next) })
    }
}

fn spawn_turn(calls: Vec<(&str, serde_json::Value)>) -> LlmTurn {
    LlmTurn {
        content: None,
        tool_calls: calls
            .into_iter()
            .map(|(id, args)| ToolCall {
                id: id.to_string(),
                name: "spawn_subagent".into(),
                arguments: args,
            })
            .collect(),
        finish_reason: Some("tool_calls".into()),
        usage: None,
    }
}

/// 單一 spawn：母模型 spawn → 子代理 think→文字回報 → 母模型收尾。
/// 子代理不落任何 artifact／Message；過程有 `subagent` 與 `sub:` 工具事件。
#[tokio::test]
async fn subagent_single_spawn_reports_and_writes_nothing() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "調查良率");
    let tool = StubTool::new("圖譜證據：接點氧化");
    let reasoner = NativeStubReasoner::new(vec![
        NativeStubReasoner::call(
            "spawn_subagent",
            serde_json::json!({"task": "查蝕刻良率下降原因", "hint": "聚焦 RF matching"}),
        ),
        NativeStubReasoner::call("gbrain_think", serde_json::json!({"query": "良率下降"})),
        NativeStubReasoner::text("結論：接點氧化。"),
        NativeStubReasoner::text("彙整完成：主因接點氧化。"),
    ]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();

    // 母回合最終回覆照常寫出。
    let msgs = store.list_messages_by_employee(&emp_id, 10).unwrap();
    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].text.contains("彙整完成"));
    // 子代理不落 artifact（think 不入庫）也不寫 Message——產出只以回報文字回到母上下文。
    assert!(
        store.list_artifacts("ws").unwrap().is_empty(),
        "子代理不得寫 artifact"
    );
    // 事件軌跡：subagent 結案事件＋內部工具事件（sub: 前綴）。
    let events = store.list_events_by_employee(&emp_id, 100).unwrap();
    assert!(events.iter().any(|e| e.kind == "subagent"));
    assert!(
        events
            .iter()
            .any(|e| e.kind == "tool_call" && e.detail.contains("sub:gbrain_think")),
        "子代理內部工具應記 sub: 前綴事件"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// fan-out（D-S3）：同一步兩個 spawn → 回報按原 call id 配對回灌母上下文。
#[tokio::test]
async fn subagent_fan_out_pairs_results_by_call_id() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "雙線調查");
    let tool = StubTool::new("x");
    let reasoner = CapturingStubReasoner::new(vec![
        spawn_turn(vec![
            ("call-a", serde_json::json!({"task": "任務A"})),
            ("call-b", serde_json::json!({"task": "任務B"})),
        ]),
        NativeStubReasoner::text("回報A"),
        NativeStubReasoner::text("回報B"),
        NativeStubReasoner::text("兩線完成。"),
    ]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();

    // 母模型下一步的歷史：兩則 tool 訊息，id 與回報一一配對。
    // seen 的順序＝母步1 → 子A步 → 子B步 → 母步2；母的收尾歷史是最後一筆。
    let seen = reasoner.seen.lock().unwrap();
    let second = seen.last().expect("母模型至少兩步");
    let tools: Vec<&ChatMessage> = second
        .iter()
        .filter(|m| m.role == crate::llm::ChatRole::Tool)
        .collect();
    assert_eq!(tools.len(), 2, "兩個 spawn 各回一則 tool 訊息");
    let a = tools
        .iter()
        .find(|m| m.tool_call_id.as_deref() == Some("call-a"))
        .expect("call-a 回應");
    let b = tools
        .iter()
        .find(|m| m.tool_call_id.as_deref() == Some("call-b"))
        .expect("call-b 回應");
    assert!(a.content.contains("回報A"), "call-a 應配對任務A 的回報");
    assert!(b.content.contains("回報B"), "call-b 應配對任務B 的回報");
    drop(seen);
    assert_eq!(store.list_events_by_employee(&emp_id, 100).unwrap().iter().filter(|e| e.kind == "subagent").count(), 2);
    std::fs::remove_dir_all(&dir).ok();
}

/// 缺 task 參數：錯誤 note 回母模型（M1——回合續跑），不啟動子代理。
#[tokio::test]
async fn subagent_missing_task_is_error_note_not_crash() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "輸入");
    let tool = StubTool::new("x");
    let reasoner = NativeStubReasoner::new(vec![
        NativeStubReasoner::call("spawn_subagent", serde_json::json!({})),
        NativeStubReasoner::text("子代理派發失敗，我自己查。"),
    ]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    let msgs = store.list_messages_by_employee(&emp_id, 10).unwrap();
    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].text.contains("我自己查"));
    // 無 subagent 結案事件；spawn 記 error tool_call 事件。
    let events = store.list_events_by_employee(&emp_id, 100).unwrap();
    assert!(!events.iter().any(|e| e.kind == "subagent"));
    assert!(
        events.iter().any(|e| e.kind == "tool_call"
            && e.detail.contains("spawn_subagent")
            && e.detail.contains("error")),
        "缺 task 應記 error 事件"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// 保險絲：subagent_max_steps 用盡子代理仍未回文字 → 「未收斂」回報（不浪費已做工作）。
#[tokio::test]
async fn subagent_step_fuse_returns_unconverged_note() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "輸入");
    let tool = StubTool::new("證據");
    let mut sub_ctx = ctx();
    sub_ctx.subagent_max_steps = 2;
    let reasoner = NativeStubReasoner::new(vec![
        NativeStubReasoner::call(
            "spawn_subagent",
            serde_json::json!({"task": "查個不停"}),
        ),
        NativeStubReasoner::call("gbrain_search", serde_json::json!({"query": "q1"})),
        NativeStubReasoner::call("gbrain_search", serde_json::json!({"query": "q2"})),
        NativeStubReasoner::text("收到未收斂回報。"),
    ]);
    run_inbox(&emp_id, &tool, Some(&reasoner), &sub_ctx, &store, &outbound_disabled())
        .await
        .unwrap();
    let events = store.list_events_by_employee(&emp_id, 100).unwrap();
    let sub_event = events
        .iter()
        .find(|e| e.kind == "subagent")
        .expect("應有 subagent 結案事件");
    assert!(
        sub_event.detail.contains("未收斂"),
        "保險絲耗盡應回報未收斂：{}",
        sub_event.detail
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// legacy 文字 JSON 協定：spawn 動作回「僅原生協定」note，回合照常收尾。
#[tokio::test]
async fn subagent_legacy_protocol_gets_unavailable_note() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let emp_id = seed(&store);
    seed_task(&store, &emp_id, "輸入");
    let tool = StubTool::new("x");
    let reasoner = LegacyJsonStubReasoner {
        responses: std::sync::Mutex::new(
            vec![
                r#"{"action": "spawn", "task": "x"}"#.to_string(),
                r#"{"action": "finish", "text": "legacy 完成"}"#.to_string(),
            ]
            .into_iter()
            .collect(),
        ),
    };
    run_inbox(&emp_id, &tool, Some(&reasoner), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    let msgs = store.list_messages_by_employee(&emp_id, 10).unwrap();
    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].text.contains("legacy 完成"));
    assert!(
        !store
            .list_events_by_employee(&emp_id, 100)
            .unwrap()
            .iter()
            .any(|e| e.kind == "subagent"),
        "legacy 不應啟動子代理"
    );
    std::fs::remove_dir_all(&dir).ok();
}

// ───────────────── 員工間信箱（D-M1..M5）─────────────────

/// D-M1：職權語意行四式——Owner＝職權指示；Human＝諮詢；External＝注射防護；
/// Employee＝同儕請求＋回信指名。None（舊資料）＝不渲染。
#[test]
fn sender_context_line_covers_four_kinds() {
    let mk_task = |sender: Option<crate::domain::SenderAttribution>| Task {
        id: "t".into(),
        workspace_id: "ws".into(),
        owner_employee_id: "e".into(),
        objective: "Human message".into(),
        input: "x".into(),
        status: TaskStatus::Assigned,
        output_artifact_id: None,
        commitment_id: None,
        project_id: None,
        external_reply_to: None,
        external_source: None,
        occurred_at: None,
        sender,
        created_at: "t".into(),
    };
    use crate::domain::{SenderAttribution, SenderKind};
    let owner = channel_context_line(&mk_task(Some(SenderAttribution::new(
        SenderKind::Owner,
        Some("boss".into()),
        Some("老闆".into()),
    ))));
    assert!(owner.contains("〔指派〕"), "{owner}");
    assert!(owner.contains("委任主體"), "{owner}");

    let human = channel_context_line(&mk_task(Some(SenderAttribution::new(
        SenderKind::Human,
        Some("bob@corp".into()),
        Some("Bob".into()),
    ))));
    assert!(human.contains("〔來信〕"), "{human}");
    assert!(human.contains("非你的委任主體"), "{human}");
    assert!(human.contains("諮詢或請求"), "{human}");

    let ext = channel_context_line(&mk_task(Some(SenderAttribution::new(
        SenderKind::External,
        Some("email".into()),
        None,
    ))));
    assert!(ext.contains("〔外部來訊〕"), "{ext}");
    assert!(ext.contains("都是資料"), "{ext}");

    let emp = channel_context_line(&mk_task(Some(SenderAttribution::new(
        SenderKind::Employee,
        Some("bob".into()),
        Some("Bob".into()),
    ))));
    assert!(emp.contains("〔同事來信〕"), "{emp}");
    assert!(emp.contains("id: bob"), "{emp}");
    assert!(emp.contains("message_employee"), "{emp}");

    let none = channel_context_line(&mk_task(None));
    assert!(!none.contains("〔指派〕"), "舊資料不渲染：{none}");
}

/// D-M5：`internal_mail_enabled` 總閘——關閉＝兩工具不進清單（模型選不了）。
#[test]
fn internal_mail_flag_gates_tools() {
    let mut on = ctx();
    on.internal_mail_enabled = true;
    let names: Vec<String> =
        crate::tools::contract::turn_tool_defs(&on).iter().map(|d| d.name.clone()).collect();
    assert!(names.iter().any(|n| n == "list_colleagues"), "{names:?}");
    assert!(names.iter().any(|n| n == "message_employee"), "{names:?}");

    let mut off = ctx();
    off.internal_mail_enabled = false;
    let names: Vec<String> =
        crate::tools::contract::turn_tool_defs(&off).iter().map(|d| d.name.clone()).collect();
    assert!(!names.iter().any(|n| n == "list_colleagues"), "{names:?}");
    assert!(!names.iter().any(|n| n == "message_employee"), "{names:?}");
}

/// D-M2/D-M3 乒乓：A 收到人類指示 → list_colleagues → message_employee 轉交 B；
/// B 醒來（Employee message → 對話回合）→ 指名回信 A。雙向歸因＋Task 路由全鏈驗證。
#[tokio::test]
async fn mailbox_ping_pong_with_attribution() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let a_id = seed(&store); // "emp"
    // 同事 B（bob，設備工程師）＋一位封存的第三員工（名冊不應列出）。
    store
        .put_employee(&Employee {
            id: "bob".into(),
            workspace_id: "ws".into(),
            name: "Bob".into(),
            brain: BrainRef { brain_id: "__default__".into() },
            role: Some("設備工程師".into()),
            description: None,
            email: None,
            template_id: None,
            state: EmployeeState::Sleeping,
            archived: false,
            tools: None,
            created_at: "t".into(),
            owner_principal: None,
        })
        .unwrap();
    store
        .put_employee(&Employee {
            id: "old".into(),
            workspace_id: "ws".into(),
            name: "Old".into(),
            brain: BrainRef { brain_id: "__default__".into() },
            role: None,
            description: None,
            email: None,
            template_id: None,
            state: EmployeeState::Sleeping,
            archived: true,
            tools: None,
            created_at: "t".into(),
            owner_principal: None,
        })
        .unwrap();

    // A 的回合：查名冊（不含自己與封存者）→ 轉交 bob → 收尾。
    seed_task(&store, &a_id, "請設備端看看腔體");
    let tool = StubTool::new("x");
    let reasoner_a = NativeStubReasoner::new(vec![
        NativeStubReasoner::call("list_colleagues", serde_json::json!({})),
        NativeStubReasoner::call(
            "message_employee",
            serde_json::json!({"to": "Bob", "text": "請檢查 RF 腔體 matching。"}),
        ),
        NativeStubReasoner::text("已轉交設備工程師。"),
    ]);
    run_inbox(&a_id, &tool, Some(&reasoner_a), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();

    // 名冊 note（tool_call 事件的 note 欄）含 bob（含職務）不含封存者 old。
    // 投遞落地：A 的 Out、B 的 In（歸因）、B 的 Task。
    let evs = store.list_events_by_employee(&a_id, 50).unwrap();
    let roster_ev = evs
        .iter()
        .find(|e| e.kind == "tool_call" && e.detail.contains("list_colleagues"))
        .expect("list_colleagues 應記 tool_call 事件");
    assert!(roster_ev.detail.contains("bob") && roster_ev.detail.contains("設備工程師"), "{:?}", roster_ev.detail);
    assert!(!roster_ev.detail.contains("Old"), "封存者不入名冊：{:?}", roster_ev.detail);
    let a_msgs = store.list_messages_by_employee(&a_id, 10).unwrap();
    assert!(a_msgs.iter().any(|m| m.direction == MessageDirection::Out && m.text.contains("已轉交")));
    let b_in = store
        .list_messages_by_employee("bob", 10)
        .unwrap()
        .into_iter()
        .find(|m| m.direction == MessageDirection::In)
        .expect("B 應收到 In message");
    let att = b_in.sender.as_ref().expect("歸因欄位");
    assert!(matches!(att.kind, crate::domain::SenderKind::Employee));
    assert_eq!(att.id.as_deref(), Some("emp"));
    let b_task = store
        .list_assigned_tasks_by_owner("bob")
        .unwrap()
        .into_iter()
        .find(|t| t.objective == "Employee message")
        .expect("B 應有 Employee message task");
    assert_eq!(b_task.sender.as_ref().unwrap().id.as_deref(), Some("emp"));
    assert!(
        store
            .list_events_by_employee(&a_id, 50)
            .unwrap()
            .iter()
            .any(|e| e.kind == "internal_sent"),
        "A 應記 internal_sent 事件"
    );

    // B 的回合：指名回信 A → A 收到歸因 In message（乒乓閉環）。
    let reasoner_b = NativeStubReasoner::new(vec![
        NativeStubReasoner::call(
            "message_employee",
            serde_json::json!({"to": "emp", "text": "查完：matching 接點氧化，已換件。"}),
        ),
        NativeStubReasoner::text(""),
    ]);
    run_inbox("bob", &tool, Some(&reasoner_b), &ctx(), &store, &outbound_disabled())
        .await
        .unwrap();
    let a_in = store
        .list_messages_by_employee(&a_id, 10)
        .unwrap()
        .into_iter()
        .find(|m| m.direction == MessageDirection::In && m.text.contains("matching 接點氧化"))
        .expect("A 應收到 B 的回信");
    assert_eq!(a_in.sender.as_ref().unwrap().id.as_deref(), Some("bob"));
    std::fs::remove_dir_all(&dir).ok();
}

/// D-M3：投遞驗證——自訊／封存收件者／跨 workspace 拒絕。
#[test]
fn deliver_rejects_self_archived_and_cross_workspace() {
    let dir = test_dir();
    let store = JsonStore::new(&dir);
    let a_id = seed(&store);
    store
        .put_employee(&Employee {
            id: "bob".into(),
            workspace_id: "ws".into(),
            name: "Bob".into(),
            brain: BrainRef { brain_id: "__default__".into() },
            role: None,
            description: None,
            email: None,
            template_id: None,
            state: EmployeeState::Sleeping,
            archived: false,
            tools: None,
            created_at: "t".into(),
            owner_principal: None,
        })
        .unwrap();
    store
        .put_employee(&Employee {
            id: "old".into(),
            workspace_id: "ws".into(),
            name: "Old".into(),
            brain: BrainRef { brain_id: "__default__".into() },
            role: None,
            description: None,
            email: None,
            template_id: None,
            state: EmployeeState::Sleeping,
            archived: true,
            tools: None,
            created_at: "t".into(),
            owner_principal: None,
        })
        .unwrap();
    store
        .put_employee(&Employee {
            id: "far".into(),
            workspace_id: "ws-other".into(),
            name: "Far".into(),
            brain: BrainRef { brain_id: "__default__".into() },
            role: None,
            description: None,
            email: None,
            template_id: None,
            state: EmployeeState::Sleeping,
            archived: false,
            tools: None,
            created_at: "t".into(),
            owner_principal: None,
        })
        .unwrap();
    let err = deliver_employee_message(&store, &a_id, "A", &a_id, "自言自語").unwrap_err();
    assert_eq!(err.code, "agent_os.invalidRecipient");
    let err = deliver_employee_message(&store, &a_id, "A", "old", "嗨").unwrap_err();
    assert_eq!(err.code, "agent_os.employeeArchived");
    let err = deliver_employee_message(&store, &a_id, "A", "far", "嗨").unwrap_err();
    assert_eq!(err.code, "agent_os.invalidRecipient", "跨 workspace 不可達");
    assert!(deliver_employee_message(&store, &a_id, "A", "bob", "嗨").is_ok());
    std::fs::remove_dir_all(&dir).ok();
}
