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
        Box::pin(async move { Ok(ToolOutput { text, meta: serde_json::json!({}) }) })
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
        tool_result_max_chars: 8_000,
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
