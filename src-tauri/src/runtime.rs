//! Runtime（Handbook Ch.13）——Emploid 的執行引擎：管理 wake→restore→execute→commit→sleep
//! 的循環，**永不介入推理**（Principle 10）。
//!
//! Phase 1：單一 Employee、單發（一次 think → 一個 Artifact）、人工觸發（`agent_run` 指令）。
//! 推理第一版固定走 gbrain think/ask（決策 D4），故 [`GbrainThinkTool`] 是第一個 Tool。
//! Tool 邊界由 [`crate::domain::tools::Tool`] trait 結構保證（只有 `invoke`，見 Principle 5）。

use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::agent_state::{AppState, WakeSignal};
use crate::config::app_config;
use crate::config::gbrain_config;
use crate::config::DEFAULT_BRAIN_ID;
use crate::domain::tools::ToolFuture;
use crate::domain::{
    id_from_name, next_id, now_rfc3339, Artifact, ArtifactStatus, BrainRef, Commitment,
    CommitmentStatus, Employee, EmployeeState, EmployeeTemplate, Memory, Project, ProjectStatus,
    SqliteStore, Store, Task, TaskStatus, Tool, ToolCtx, ToolInput, Workspace, WorkspaceStatus,
};
use crate::domain::tools::{parse_json_value, Reasoner, ReasonerFuture, ToolOutput, ToolSpec};
use crate::i18n::AppError;
use crate::llm;

// ───────────────── 循環結果 ─────────────────

/// 一次執行循環的結果（回給呼叫端／前端）。
#[derive(Serialize)]
pub struct CycleResult {
    pub task_id: String,
    pub artifact_id: String,
    pub artifact_content: String,
    pub tool_meta: serde_json::Value,
}

// ───────────────── 執行循環（Ch.13 五階段）─────────────────

/// 跑完一輪 Wake → Restore → Execute → Commit → Sleep。
///
/// 泛用於任意 `Tool`（`&dyn Tool`）與任意 `Store`（`&(dyn Store + Send + Sync)`）：
/// 正式呼叫塞 [`GbrainThinkTool`]，測試塞 `StubTool`。Runtime 只編排，不決定「要想什麼」。
pub async fn run_cycle(
    employee_id: &str,
    query: String,
    anchor: Option<String>,
    commitment_id: Option<&str>,
    project_id: Option<&str>,
    tool: &dyn Tool,
    ctx: &ToolCtx,
    store: &(dyn Store + Send + Sync),
) -> anyhow::Result<CycleResult> {
    // 1. Wake：載入 Employee，狀態置 Working。
    let mut emp = store
        .get_employee(employee_id)?
        .ok_or_else(|| anyhow::anyhow!("employee not found: {employee_id}"))?;
    let workspace_id = emp.workspace_id.clone();
    emp.state = EmployeeState::Working;
    store.put_employee(&emp)?;

    // 2. Restore Context：還原工作記憶（無則空）。Principle 8——每次重建，不假設駐留。
    let mut memory = restore_memory(store, employee_id)?;

    // 3. Execute：單發——以 query 呼叫 Tool。要不要呼叫屬 Runtime，要想什麼由（Phase 1 暫固定的）query 決定。
    let output = tool
        .invoke(
            ToolInput {
                query: query.clone(),
                anchor: anchor.clone(),
            },
            ctx,
        )
        .await?;

    // 4. Commit Artifact：產出固化為 first-class Artifact（Committed），帶完整 provenance。
    let existing_task_ids: Vec<String> = store
        .list_tasks(&workspace_id)?
        .into_iter()
        .map(|t| t.id)
        .collect();
    let task_id = next_id("task", &existing_task_ids);
    let artifact_id = commit_artifact(
        store,
        &workspace_id,
        employee_id,
        &query,
        &output.text,
        Some(&task_id),
        commitment_id,
        project_id,
    )?;

    // 本週期的最小工作單位 Task：Created→Completed（產出已提交），連到所屬 commitment。
    let task = Task {
        id: task_id.clone(),
        workspace_id: workspace_id.clone(),
        owner_employee_id: employee_id.to_string(),
        objective: query.clone(),
        input: query.clone(),
        status: TaskStatus::Completed,
        output_artifact_id: Some(artifact_id.clone()),
        commitment_id: commitment_id.map(str::to_string),
        project_id: project_id.map(str::to_string),
        created_at: now_rfc3339(),
    };
    store.put_task(&task)?;

    // 若綁定 commitment：更新活動時間（狀態維持 Active——不自動 Satisfied；Principle 9）。
    if let Some(cid) = commitment_id {
        if let Some(mut com) = store.get_commitment(cid)? {
            com.updated_at = now_rfc3339();
            store.put_commitment(&com)?;
        }
    }

    // 5. Sleep：持久化 memory（附本週期 note）＋ employee 狀態。離開前一切已落地。
    memory
        .notes
        .push(format!("ran \"{query}\" → artifact {artifact_id}"));
    memory.last_artifact_id = Some(artifact_id.clone());
    memory.updated_at = now_rfc3339();
    store.put_memory(&memory)?;

    emp.state = EmployeeState::Sleeping;
    store.put_employee(&emp)?;

    Ok(CycleResult {
        task_id,
        artifact_id,
        artifact_content: output.text,
        tool_meta: output.meta,
    })
}

// ───────────────── Phase 6 共用 helpers ─────────────────

/// 還原員工工作記憶（無則空）。Principle 8——每次喚醒都重建，不假設駐留。
fn restore_memory(store: &dyn Store, employee_id: &str) -> anyhow::Result<Memory> {
    Ok(store.get_memory(employee_id)?.unwrap_or_else(|| Memory {
        employee_id: employee_id.to_string(),
        notes: Vec::new(),
        last_artifact_id: None,
        updated_at: now_rfc3339(),
    }))
}

/// 把 Tool 輸出固化為 first-class Committed Artifact（完整 provenance），回傳 artifact_id。
/// 供 `run_cycle`（單發）與 `run_inbox`／`run_autonomous`（持續）共用——commit 是「暫時→真實」的邊界。
fn commit_artifact(
    store: &dyn Store,
    workspace_id: &str,
    producer: &str,
    query_for_title: &str,
    content: &str,
    source_task_id: Option<&str>,
    source_commitment_id: Option<&str>,
    project_id: Option<&str>,
) -> anyhow::Result<String> {
    let existing: Vec<String> = store
        .list_artifacts(workspace_id)?
        .into_iter()
        .map(|a| a.id)
        .collect();
    let artifact_id = id_from_name(&format!("think-{}", query_for_title), &existing);
    let artifact = Artifact {
        id: artifact_id.clone(),
        workspace_id: workspace_id.to_string(),
        title: format!("think: {}", query_for_title),
        artifact_type: "think".into(),
        content: content.to_string(),
        produced_by: producer.to_string(),
        source_task_id: source_task_id.map(str::to_string),
        source_commitment_id: source_commitment_id.map(str::to_string),
        revised_from_id: None,
        project_id: project_id.map(str::to_string),
        version: 1,
        status: ArtifactStatus::Committed,
        created_at: now_rfc3339(),
    };
    store.put_artifact(&artifact)?;
    Ok(artifact_id)
}

/// 收件匣驅動喚醒（Phase 6）：員工醒來、依序吃完所有 Inbox（Assigned）tasks、再睡。
///
/// 與 `run_cycle` 的差別：不接收外部 query——query 來自員工自己的 inbox task；
/// 不在每個 task 之間切 Sleeping（整段維持 Working，避免破壞 busy-lock 與監看的真實性）；
/// 不另建 task——inbox task 本身就是工作單位（標 InProgress→Completed 並連結產出）。
pub async fn run_inbox(
    employee_id: &str,
    tool: &dyn Tool,
    ctx: &ToolCtx,
    store: &(dyn Store + Send + Sync),
) -> anyhow::Result<()> {
    // Wake：整段維持 Working。
    let mut emp = store
        .get_employee(employee_id)?
        .ok_or_else(|| anyhow::anyhow!("employee not found: {employee_id}"))?;
    let workspace_id = emp.workspace_id.clone();
    emp.state = EmployeeState::Working;
    store.put_employee(&emp)?;
    let mut memory = restore_memory(store, employee_id)?;

    loop {
        // 取一個 Inbox task（Assigned/Created/InProgress）；無則結束。
        let Some(mut task) = store
            .list_assigned_tasks_by_owner(employee_id)?
            .into_iter()
            .next()
        else {
            break;
        };
        // 標 InProgress（讓監看可見「正在做這件」）。
        task.status = TaskStatus::InProgress;
        store.put_task(&task)?;

        let output = tool
            .invoke(
                ToolInput {
                    query: task.input.clone(),
                    anchor: None,
                },
                ctx,
            )
            .await?;
        let artifact_id = commit_artifact(
            store,
            &workspace_id,
            employee_id,
            &task.input,
            &output.text,
            Some(&task.id),
            task.commitment_id.as_deref(),
            task.project_id.as_deref(),
        )?;
        // 原任務完成、連結產出。
        task.status = TaskStatus::Completed;
        task.output_artifact_id = Some(artifact_id.clone());
        store.put_task(&task)?;
        if let Some(cid) = task.commitment_id.as_deref() {
            if let Some(mut com) = store.get_commitment(cid)? {
                com.updated_at = now_rfc3339();
                store.put_commitment(&com)?;
            }
        }
        memory
            .notes
            .push(format!("inbox \"{}\" → artifact {}", task.objective, artifact_id));
        memory.last_artifact_id = Some(artifact_id);
    }

    // Sleep：Inbox 吃光才睡。
    memory.updated_at = now_rfc3339();
    store.put_memory(&memory)?;
    emp.state = EmployeeState::Sleeping;
    store.put_employee(&emp)?;
    Ok(())
}

// ───────────────── 承諾驅動自主循環（Phase 6b）─────────────────

/// 自主循環的執行預算——避免一個 commitment 無限燃燒 LLM。
pub struct CycleBudget {
    pub max_cycles: u32,
    pub max_duration: Duration,
}

impl CycleBudget {
    /// 一次喚醒 session 的預設預算（10 輪、5 分鐘）。
    pub fn default_session() -> Self {
        Self {
            max_cycles: 10,
            max_duration: Duration::from_secs(300),
        }
    }
}

/// 一次自主 session 的結果。
#[derive(Debug, Serialize)]
pub enum AutonomousOutcome {
    /// 完成條件已滿足 → commitment `Satisfied`。
    Satisfied {
        artifact_ids: Vec<String>,
        cycles: u32,
    },
    /// 卡住——規劃重複／預算用盡。0 產出 → commitment `Suspended`；有產出 → 維持 `Active`（下次喚醒續跑）。
    Stalled {
        reason: String,
        cycles: u32,
    },
    /// 硬錯誤（tool／reasoner 例外）→ 員工 `Error`。
    Errored {
        detail: String,
    },
}

/// notes 環狀緩衝上限（避免無限增長；P8 的「遺忘是錯誤」適用於未提交工作，非無限暫存）。
const NOTE_CAP: usize = 50;

const PLAN_SYSTEM: &str = "你是一名自主工作者。根據你的承諾與目前進度，決定下一個該採取的具體行動。該行動會被當作知識檢索的查詢。只回 JSON 物件，不附加其他文字。";

const EVAL_SYSTEM: &str = "你是一名完成條件評估者。只根據完成條件與已產出的成果，判斷承諾是否已滿足。只回 JSON 物件，不附加其他文字。";

/// 承諾驅動喚醒（Phase 6b）：員工憑一個 Active commitment 自主運行——每輪先**清一個 Inbox task**
/// （若有），否則 **Plan→Act→Evaluate**，直到 Satisfied 或卡住。
///
/// Runtime 只編排循環形狀與 Tool 副作用；規劃／行動結論／評估判斷皆由 Brain（`reasoner`／`knowledge`）
/// 做出（Principle 10，Handbook Ch.13 §4 修訂）。員工在整段 session 維持 `Working`，結束才睡。
#[allow(clippy::too_many_arguments)]
pub async fn run_autonomous(
    employee_id: &str,
    commitment_id: &str,
    budget: &CycleBudget,
    knowledge: &dyn Tool,
    reasoner: &dyn Reasoner,
    ctx: &ToolCtx,
    store: &(dyn Store + Send + Sync),
) -> anyhow::Result<AutonomousOutcome> {
    // Wake：整段維持 Working。
    let mut emp = store
        .get_employee(employee_id)?
        .ok_or_else(|| anyhow::anyhow!("employee not found: {employee_id}"))?;
    let workspace_id = emp.workspace_id.clone();
    emp.state = EmployeeState::Working;
    store.put_employee(&emp)?;
    let mut memory = restore_memory(store, employee_id)?;
    let mut commitment = store
        .get_commitment(commitment_id)?
        .ok_or_else(|| anyhow::anyhow!("commitment not found: {commitment_id}"))?;

    let deadline = std::time::Instant::now() + budget.max_duration;
    let mut artifact_ids: Vec<String> = Vec::new();
    let mut produced_any = false;
    let mut cycles = 0u32;
    let mut last_query: Option<String> = None;
    let outcome: AutonomousOutcome;

    loop {
        cycles += 1;
        if cycles > budget.max_cycles {
            outcome = AutonomousOutcome::Stalled {
                reason: "達 cycle 上限".into(),
                cycles: cycles - 1,
            };
            break;
        }
        if std::time::Instant::now() > deadline {
            outcome = AutonomousOutcome::Stalled {
                reason: "達時間上限".into(),
                cycles: cycles - 1,
            };
            break;
        }

        // PLAN：reasoner 決定下一步（或判斷已 done）。近期 notes 提供進度脈絡，避免天真重來。
        let recent: Vec<String> = memory.notes.iter().rev().take(5).cloned().collect();
        let plan_user = format!(
            "承諾：{title}\n完成條件：{cond}\n近期已做：\n{recent}\n\
             請決定下一個要調查／執行的具體問題（一句）。只回 JSON：\
             {{\"next_query\": \"...\", \"rationale\": \"...\"}}；若你判斷承諾已完成，回 {{\"done\": true}}。",
            title = commitment.title,
            cond = commitment.completion_condition,
            recent = if recent.is_empty() { "(尚無)".into() } else { recent.join("\n") },
        );
        let plan = match reasoner.reason(PLAN_SYSTEM, &plan_user).await {
            Ok(v) => v,
            Err(e) => {
                outcome = AutonomousOutcome::Errored {
                    detail: format!("plan: {e}"),
                };
                break;
            }
        };
        // 規劃器主張已完成 → 進 EVAL 確認（仍由 Brain 判斷，非 Runtime 決定）。
        if plan.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
            match evaluate_done(reasoner, &commitment, &artifact_ids).await {
                Ok(true) => {
                    outcome = AutonomousOutcome::Satisfied {
                        artifact_ids,
                        cycles: cycles - 1,
                    };
                    break;
                }
                Ok(false) => {}
                Err(e) => {
                    outcome = AutonomousOutcome::Errored {
                        detail: format!("eval: {e}"),
                    };
                    break;
                }
            }
        }
        let Some(next_query) = plan
            .get("next_query")
            .and_then(|v| v.as_str())
            .map(str::to_string)
        else {
            outcome = AutonomousOutcome::Stalled {
                reason: "規劃器未給出 next_query".into(),
                cycles: cycles - 1,
            };
            break;
        };
        // 重複偵測：與上一輪相同 → 視為無進展。
        if last_query.as_deref() == Some(next_query.as_str()) {
            outcome = AutonomousOutcome::Stalled {
                reason: "規劃重複，無進展".into(),
                cycles: cycles - 1,
            };
            break;
        }
        last_query = Some(next_query.clone());

        // ACT：知識工具檢索。
        let output = match knowledge
            .invoke(
                ToolInput {
                    query: next_query.clone(),
                    anchor: None,
                },
                ctx,
            )
            .await
        {
            Ok(o) => o,
            Err(e) => {
                outcome = AutonomousOutcome::Errored {
                    detail: format!("act: {e}"),
                };
                break;
            }
        };
        let artifact_id = commit_artifact(
            store,
            &workspace_id,
            employee_id,
            &next_query,
            &output.text,
            None,
            Some(commitment_id),
            None,
        )?;
        artifact_ids.push(artifact_id.clone());
        produced_any = true;
        memory
            .notes
            .push(format!("承諾「{}」：查「{}」→ artifact {}", commitment.title, next_query, artifact_id));
        cap_notes(&mut memory);
        memory.last_artifact_id = Some(artifact_id);
        memory.updated_at = now_rfc3339();
        store.put_memory(&memory)?;
        commitment.updated_at = now_rfc3339();
        store.put_commitment(&commitment)?;

        // EVALUATE：reasoner 判斷完成條件。
        match evaluate_done(reasoner, &commitment, &artifact_ids).await {
            Ok(true) => {
                outcome = AutonomousOutcome::Satisfied {
                    artifact_ids,
                    cycles: cycles - 1,
                };
                break;
            }
            Ok(false) => {}
            Err(e) => {
                outcome = AutonomousOutcome::Errored {
                    detail: format!("eval: {e}"),
                };
                break;
            }
        }
    }

    // 依結果更新 commitment／員工，然後睡。
    let errored = matches!(outcome, AutonomousOutcome::Errored { .. });
    match &outcome {
        AutonomousOutcome::Satisfied { .. } => {
            commitment.status = CommitmentStatus::Satisfied;
            store.put_commitment(&commitment)?;
        }
        AutonomousOutcome::Stalled { reason, .. } => {
            // 0 產出 → Suspended（避免每次喚醒狂跑）；有產出 → 維持 Active（下次喚醒續跑）。
            if !produced_any {
                commitment.status = CommitmentStatus::Suspended;
                memory
                    .notes
                    .push(format!("承諾「{}」卡住（{reason}）→ Suspended", commitment.title));
            } else {
                memory
                    .notes
                    .push(format!("承諾「{}」本輪未完成但已有進展，下次喚醒再續", commitment.title));
            }
            cap_notes(&mut memory);
            memory.updated_at = now_rfc3339();
            store.put_memory(&memory)?;
            store.put_commitment(&commitment)?;
        }
        AutonomousOutcome::Errored { detail } => {
            memory
                .notes
                .push(format!("承諾「{}」錯誤：{detail}", commitment.title));
            cap_notes(&mut memory);
            memory.updated_at = now_rfc3339();
            store.put_memory(&memory)?;
        }
    }
    emp.state = if errored {
        EmployeeState::Error
    } else {
        EmployeeState::Sleeping
    };
    store.put_employee(&emp)?;
    Ok(outcome)
}

/// 諮詢 Brain 判斷 completion_condition 是否已滿足（Handbook Ch.13 §4 修訂：完成評估＝生命週期控制）。
async fn evaluate_done(
    reasoner: &dyn Reasoner,
    commitment: &Commitment,
    artifact_ids: &[String],
) -> anyhow::Result<bool> {
    let eval_user = format!(
        "完成條件：{cond}\n本次產出的 artifacts：{arts}\n\
         判斷完成條件是否已滿足。只回 JSON：{{\"done\": true 或 false, \"rationale\": \"...\"}}。",
        cond = commitment.completion_condition,
        arts = if artifact_ids.is_empty() {
            "(尚無)".into()
        } else {
            artifact_ids.iter().take(8).cloned().collect::<Vec<_>>().join(", ")
        },
    );
    let v = reasoner.reason(EVAL_SYSTEM, &eval_user).await?;
    Ok(v.get("done").and_then(|x| x.as_bool()).unwrap_or(false))
}

/// notes 環狀緩衝：保留最後 NOTE_CAP 則。
fn cap_notes(memory: &mut Memory) {
    if memory.notes.len() > NOTE_CAP {
        let drain = memory.notes.len() - NOTE_CAP;
        memory.notes.drain(..drain);
    }
}

// ───────────────── 第一個 Tool：gbrain think ─────────────────

/// 以 gbrain `think` 對知識圖譜做圖譜增強檢索。Tool 只執行、不決策。
pub struct GbrainThinkTool {
    spec: ToolSpec,
}

impl GbrainThinkTool {
    pub fn new() -> Self {
        Self {
            spec: ToolSpec {
                id: "gbrain-think".into(),
                description: "Query the GBrain knowledge graph (graph-augmented retrieval).".into(),
            },
        }
    }
}

impl Tool for GbrainThinkTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn invoke<'a>(&'a self, input: ToolInput, ctx: &'a ToolCtx) -> ToolFuture<'a> {
        Box::pin(async move {
            let env = crate::gbrain_cli::env_for_brain(ctx.gbrain_home.as_deref());
            let mut args: Vec<String> = vec!["think".into(), input.query.clone()];
            if let Some(a) = &input.anchor {
                args.push("--anchor".into());
                args.push(a.clone());
            }
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

            let mut cmd = tokio::process::Command::new(&ctx.gbrain_exe);
            crate::gbrain_cli::no_console_async(&mut cmd);
            cmd.args(&arg_refs);
            for (k, v) in &env {
                cmd.env(k, v);
            }
            cmd.stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let out = cmd
                .output()
                .await
                .map_err(|e| anyhow::anyhow!("spawn gbrain think failed: {e}"))?;
            let code = out.status.code().unwrap_or(-1);
            let stdout = crate::gbrain_cli::decode_buf(&out.stdout);
            let stderr = crate::gbrain_cli::decode_buf(&out.stderr);
            if code != 0 && stdout.trim().is_empty() {
                anyhow::bail!("gbrain think failed (exit {code}): {}", stderr.trim());
            }
            let meta = parse_think_meta(&stdout);
            Ok(ToolOutput { text: stdout, meta })
        })
    }
}

/// best-effort 解析 think 輸出末段的 `Pages/Takes/Graph/Citations` 量化指標。
fn parse_think_meta(stdout: &str) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for key in ["Pages", "Takes", "Graph", "Citations"] {
        let needle = format!("{key}:");
        if let Some(idx) = stdout.find(&needle) {
            let rest = &stdout[idx + needle.len()..];
            let num: String = rest
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(n) = num.parse::<i64>() {
                m.insert(key.to_lowercase(), serde_json::Value::from(n));
            }
        }
    }
    serde_json::Value::Object(m)
}

// ───────────────── Tauri 指令（執行期以 agent_os_enabled 把關）─────────────────

#[derive(Serialize)]
pub struct SeedResult {
    pub workspace_id: String,
    pub employee_id: String,
}

/// 招募一個 Employee（供無 UI 時的手動驗證用）：建立 default workspace ＋ 一個指向
/// 作用中腦的 employee。回傳其 id。
#[tauri::command]
pub async fn agent_seed<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<SeedResult, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;

    let ws_id = "ws-default".to_string();
    if store.get_workspace(&ws_id)?.is_none() {
        store.put_workspace(&Workspace {
            id: ws_id.clone(),
            name: "Default".into(),
            description: None,
            status: WorkspaceStatus::Active,
            created_at: now_rfc3339(),
        })?;
    }
    let active_brain = cfg
        .active_brain_id
        .clone()
        .unwrap_or_else(|| DEFAULT_BRAIN_ID.to_string());
    let emp_id = "employee-1".to_string();
    store.put_employee(&Employee {
        id: emp_id.clone(),
        workspace_id: ws_id.clone(),
        name: "Employee One".into(),
        brain: BrainRef {
            brain_id: active_brain,
        },
        role: Some("general".into()),
        template_id: None,
        state: EmployeeState::Sleeping,
        created_at: now_rfc3339(),
    })?;

    Ok(SeedResult {
        workspace_id: ws_id,
        employee_id: emp_id,
    })
}

#[derive(Serialize)]
pub struct RecruitResult {
    pub employee_id: String,
}

/// 招募另一個 Employee（可與既有員工**共用同一腦**，Principle 6）。
/// `brain_id` 缺省＝作用中腦；腦是否存在於執行（`agent_run`）時驗證。
#[tauri::command]
pub async fn agent_recruit<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    workspace_id: String,
    name: String,
    brain_id: Option<String>,
) -> Result<RecruitResult, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;

    let brain_id = brain_id.unwrap_or_else(|| {
        cfg.active_brain_id
            .clone()
            .unwrap_or_else(|| DEFAULT_BRAIN_ID.to_string())
    });
    let existing: Vec<String> = store
        .list_employees(&workspace_id)?
        .into_iter()
        .map(|e| e.id)
        .collect();
    let emp_id = id_from_name(&name, &existing);
    store.put_employee(&Employee {
        id: emp_id.clone(),
        workspace_id,
        name,
        brain: BrainRef { brain_id },
        role: None,
        template_id: None,
        state: EmployeeState::Sleeping,
        created_at: now_rfc3339(),
    })?;
    Ok(RecruitResult { employee_id: emp_id })
}

/// 從 template 部署一個獨立 Instance（Ch.04 §7）：抄襲 brain／role、設 `template_id`、
/// fresh Sleeping。抽成函式以便單測（免 AppHandle）。
pub fn deploy_instance(
    store: &(dyn Store + Send + Sync),
    template_id: &str,
    instance_name: &str,
) -> anyhow::Result<String> {
    let tmpl = store
        .get_template(template_id)?
        .ok_or_else(|| anyhow::anyhow!("template not found: {template_id}"))?;
    let existing: Vec<String> = store
        .list_employees(&tmpl.workspace_id)?
        .into_iter()
        .map(|e| e.id)
        .collect();
    let emp_id = id_from_name(instance_name, &existing);
    store.put_employee(&Employee {
        id: emp_id.clone(),
        workspace_id: tmpl.workspace_id.clone(),
        name: instance_name.to_string(),
        brain: tmpl.brain.clone(),
        role: tmpl.role.clone(),
        template_id: Some(template_id.to_string()),
        state: EmployeeState::Sleeping,
        created_at: now_rfc3339(),
    })?;
    Ok(emp_id)
}

#[derive(Serialize)]
pub struct TemplateResult {
    pub template_id: String,
}

/// 建立一個可重用的 Employee Template（一種員工的定義：name＋brain＋role）。
#[tauri::command]
pub async fn agent_create_template<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    workspace_id: String,
    name: String,
    brain_id: Option<String>,
    role: Option<String>,
) -> Result<TemplateResult, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;

    let brain_id = brain_id.unwrap_or_else(|| {
        cfg.active_brain_id
            .clone()
            .unwrap_or_else(|| DEFAULT_BRAIN_ID.to_string())
    });
    let existing: Vec<String> = store
        .list_templates(&workspace_id)?
        .into_iter()
        .map(|t| t.id)
        .collect();
    let template_id = id_from_name(&name, &existing);
    store.put_template(&EmployeeTemplate {
        id: template_id.clone(),
        workspace_id,
        name,
        brain: BrainRef { brain_id },
        role,
        created_at: now_rfc3339(),
    })?;
    Ok(TemplateResult { template_id })
}

#[derive(Serialize)]
pub struct DeployResult {
    pub employee_id: String,
}

/// 從 Template 部署一個獨立 Instance。
#[tauri::command]
pub async fn agent_deploy_instance<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    template_id: String,
    instance_name: String,
) -> Result<DeployResult, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;
    let employee_id = deploy_instance(&store, &template_id, &instance_name)?;
    Ok(DeployResult { employee_id })
}

/// 預設 workspace id（GUI 用）。
const AGENT_WS: &str = "ws-default";

/// Agent-OS DB 路徑：**Local AppData**（避免 Roaming 被 OneDrive／網域同步導致 WAL 損壞——
/// WAL 的 `-wal`／`-shm` 必須是共置本地檔案）。首次啟動若舊位置（Roaming app_data_dir）有
/// `emploid.db`，一次性遷移複製過來。
pub(crate) fn agent_db_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<std::path::PathBuf, AppError> {
    let local_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(&local_dir);
    let new_path = local_dir.join("emploid.db");
    if !new_path.exists() {
        // 一次性遷移：舊 Roaming 位置有 DB → 複製到 Local。
        if let Ok(old_dir) = app.path().app_data_dir() {
            let old_path = old_dir.join("emploid.db");
            if old_path.exists() {
                let _ = std::fs::copy(&old_path, &new_path);
            }
        }
    }
    Ok(new_path)
}

/// 共用：Agent-OS flag 檢查 ＋ 開 `emploid.db`（Local AppData）。
pub(crate) fn agent_store<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<SqliteStore, AppError> {
    let cfg = app_config::load(app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    Ok(SqliteStore::open(agent_db_path(app)?)?)
}

/// 為某員工解析其腦並建構（GbrainThinkTool, ToolCtx）。`agent_run` 與排程器共用。
pub(crate) fn build_tool_ctx(
    cfg: &app_config::AppConfig,
    store: &SqliteStore,
    employee_id: &str,
) -> Result<(GbrainThinkTool, ToolCtx), AppError> {
    let emp = store
        .get_employee(employee_id)?
        .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", employee_id))?;
    let entry = crate::brains::brain_entry(cfg, &emp.brain.brain_id)?;
    Ok((
        GbrainThinkTool::new(),
        ToolCtx {
            gbrain_exe: cfg.gbrain_exe_path.clone(),
            gbrain_home: entry.env_home().map(|s| s.to_string()),
        },
    ))
}

// ───────────────── Reasoner（推理器，Phase 6b）─────────────────

/// 以 LLM（Employee Brain 的推理層）實作的 [`Reasoner`]：包 [`llm::complete`]，回結構化 JSON。
/// 與 [`GbrainThinkTool`]（知識檢索）成對——推理 vs 檢索（Principle 1：知識≠工作者）。
pub struct LlmReasoner {
    endpoint: gbrain_config::LlmEndpoint,
    cfg: app_config::AppConfig,
}

impl LlmReasoner {
    pub fn new(endpoint: gbrain_config::LlmEndpoint, cfg: app_config::AppConfig) -> Self {
        Self { endpoint, cfg }
    }
}

impl Reasoner for LlmReasoner {
    fn reason<'a>(&'a self, system: &'a str, user: &'a str) -> ReasonerFuture<'a> {
        Box::pin(async move {
            let raw = llm::complete(&self.endpoint, &self.cfg, system, user).await?;
            parse_json_value(&raw)
        })
    }
}

/// 為某員工解析其腦的 LLM endpoint，建構 [`LlmReasoner`]（與 [`build_tool_ctx`] 用同一個腦）。
/// 缺 API key（且非 ollama）→ `llm.noApiKey`。
pub(crate) fn build_reasoner(
    cfg: &app_config::AppConfig,
    store: &SqliteStore,
    employee_id: &str,
) -> Result<LlmReasoner, AppError> {
    let emp = store
        .get_employee(employee_id)?
        .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", employee_id))?;
    let entry = crate::brains::brain_entry(cfg, &emp.brain.brain_id)?;
    let loaded = gbrain_config::load_for(entry.env_home())?;
    let endpoint = gbrain_config::resolve_endpoint(&loaded.config)?;
    if !endpoint.has_api_key && endpoint.provider != "ollama" {
        return Err(AppError::new("llm.noApiKey")
            .p("provider", &endpoint.provider)
            .p(
                "envKey",
                gbrain_config::env_key(&endpoint.provider).unwrap_or("?"),
            ));
    }
    Ok(LlmReasoner::new(endpoint, cfg.clone()))
}

#[derive(Serialize)]
pub struct WorkspaceResult {
    pub workspace_id: String,
}

/// 冪等確保 `ws-default` workspace 存在（不建 employee-1）。回其 id。
#[tauri::command]
pub async fn agent_ensure_workspace<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<WorkspaceResult, AppError> {
    let store = agent_store(&app)?;
    if store.get_workspace(AGENT_WS)?.is_none() {
        store.put_workspace(&Workspace {
            id: AGENT_WS.into(),
            name: "Default".into(),
            description: None,
            status: WorkspaceStatus::Active,
            created_at: now_rfc3339(),
        })?;
    }
    Ok(WorkspaceResult {
        workspace_id: AGENT_WS.into(),
    })
}

/// 列出某 workspace 的模板（typed）。
#[tauri::command]
pub async fn agent_list_templates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    workspace_id: String,
) -> Result<Vec<EmployeeTemplate>, AppError> {
    let store = agent_store(&app)?;
    Ok(store.list_templates(&workspace_id)?)
}

/// 列出某 workspace 的員工實體（typed）。
#[tauri::command]
pub async fn agent_list_employees<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    workspace_id: String,
) -> Result<Vec<Employee>, AppError> {
    let store = agent_store(&app)?;
    Ok(store.list_employees(&workspace_id)?)
}

/// 刪除模板。
#[tauri::command]
pub async fn agent_delete_template<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    template_id: String,
) -> Result<(), AppError> {
    let store = agent_store(&app)?;
    store.delete_template(&template_id)?;
    Ok(())
}

/// 刪除員工實體。
#[tauri::command]
pub async fn agent_delete_employee<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    employee_id: String,
) -> Result<(), AppError> {
    let store = agent_store(&app)?;
    store.delete_employee(&employee_id)?;
    Ok(())
}

/// 重新命名模板。
#[tauri::command]
pub async fn agent_rename_template<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    template_id: String,
    name: String,
) -> Result<(), AppError> {
    let store = agent_store(&app)?;
    let mut t = store
        .get_template(&template_id)?
        .ok_or_else(|| AppError::new("agent_os.templateNotFound").p("id", &template_id))?;
    t.name = name;
    store.put_template(&t)?;
    Ok(())
}

/// 重新命名員工實體（個別命名，如 Steve@TW）。
#[tauri::command]
pub async fn agent_rename_employee<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    employee_id: String,
    name: String,
) -> Result<(), AppError> {
    let store = agent_store(&app)?;
    let mut e = store
        .get_employee(&employee_id)?
        .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", &employee_id))?;
    e.name = name;
    store.put_employee(&e)?;
    Ok(())
}

/// 跑一輪 Employee 循環：載入 employee→解析腦→建 ToolCtx→run_cycle（gbrain think）。
/// `commitment_id` 可選：綁定則此循環的 task／artifact 連到該長期責任。
#[tauri::command]
pub async fn agent_run<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    employee_id: String,
    query: String,
    anchor: Option<String>,
    commitment_id: Option<String>,
    project_id: Option<String>,
) -> Result<CycleResult, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    // busy-lock：同一員工被排程器／其他指令佔用中則拒絕，防競態。
    let _guard = state
        .try_acquire(&employee_id)
        .ok_or_else(|| AppError::new("agent_os.employeeBusy").p("id", &employee_id))?;
    let store = SqliteStore::open(agent_db_path(&app)?)?;

    let (tool, ctx) = build_tool_ctx(&cfg, &store, &employee_id)?;
    let result = run_cycle(
        &employee_id,
        query,
        anchor,
        commitment_id.as_deref(),
        project_id.as_deref(),
        &tool,
        &ctx,
        &store,
    )
    .await?;
    Ok(result)
}

// ───────────────── Commitment 與 Artifact 版本指令 ─────────────────

#[derive(Serialize)]
pub struct CommitmentResult {
    pub commitment_id: String,
}

/// 建立一個長期責任（Commitment）：Created→Active，擁有某 Employee，帶完成條件。
#[tauri::command]
pub async fn agent_create_commitment<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    employee_id: String,
    title: String,
    completion_condition: String,
) -> Result<CommitmentResult, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;

    let emp = store
        .get_employee(&employee_id)?
        .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", &employee_id))?;
    let ws = emp.workspace_id.clone();
    let now = now_rfc3339();
    let commitment_id = id_from_name(&title, &{
        let mut v: Vec<String> = store
            .list_commitments(&ws)?
            .into_iter()
            .map(|c| c.id)
            .collect();
        v.sort();
        v
    });
    store.put_commitment(&Commitment {
        id: commitment_id.clone(),
        workspace_id: ws,
        owner_employee_id: employee_id,
        title,
        completion_condition,
        status: CommitmentStatus::Active,
        created_at: now.clone(),
        updated_at: now,
    })?;
    Ok(CommitmentResult { commitment_id })
}

/// 手動標記一個 Commitment 已滿足（Satisfied）。完成條件的自動判斷屬更成熟 Runtime。
#[tauri::command]
pub async fn agent_satisfy_commitment<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    commitment_id: String,
) -> Result<(), AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;

    let mut com = store
        .get_commitment(&commitment_id)?
        .ok_or_else(|| AppError::new("agent_os.commitmentNotFound").p("id", &commitment_id))?;
    com.status = CommitmentStatus::Satisfied;
    com.updated_at = now_rfc3339();
    store.put_commitment(&com)?;
    Ok(())
}

#[derive(Serialize)]
pub struct ReviseResult {
    pub artifact_id: String,
    pub version: u32,
}

/// 修訂一個 Artifact：舊版→Superseded、新版 Committed（version+1），承襲 provenance。
/// 邏輯抽成此函式以便單測（免 AppHandle）。
pub fn revise_artifact(
    store: &(dyn Store + Send + Sync),
    artifact_id: &str,
    employee_id: &str,
    new_content: String,
) -> anyhow::Result<(String, u32)> {
    let mut old = store
        .get_artifact(artifact_id)?
        .ok_or_else(|| anyhow::anyhow!("artifact not found: {artifact_id}"))?;
    old.status = ArtifactStatus::Superseded;
    store.put_artifact(&old)?;

    let new_version = old.version + 1;
    let new_id = id_from_name(
        &old.id,
        &store
            .list_artifacts(&old.workspace_id)?
            .into_iter()
            .map(|a| a.id)
            .collect::<Vec<_>>(),
    );
    let now = now_rfc3339();
    let new_art = Artifact {
        id: new_id.clone(),
        workspace_id: old.workspace_id.clone(),
        title: old.title.clone(),
        artifact_type: old.artifact_type.clone(),
        content: new_content,
        produced_by: employee_id.to_string(),
        source_task_id: old.source_task_id.clone(),
        source_commitment_id: old.source_commitment_id.clone(),
        revised_from_id: Some(old.id),
        project_id: old.project_id.clone(),
        version: new_version,
        status: ArtifactStatus::Committed,
        created_at: now,
    };
    store.put_artifact(&new_art)?;
    Ok((new_id, new_version))
}

#[tauri::command]
pub async fn agent_revise_artifact<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    artifact_id: String,
    employee_id: String,
    new_content: String,
) -> Result<ReviseResult, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;
    let (artifact_id, version) = revise_artifact(&store, &artifact_id, &employee_id, new_content)?;
    Ok(ReviseResult {
        artifact_id,
        version,
    })
}

/// 一次取回某 workspace 的 commitments／tasks／artifacts 摘要（手動驗證用）。
#[tauri::command]
pub async fn agent_list_state<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    workspace_id: String,
) -> Result<serde_json::Value, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;
    Ok(serde_json::json!({
        "projects": store.list_projects(&workspace_id)?,
        "templates": store.list_templates(&workspace_id)?,
        "employees": store.list_employees(&workspace_id)?,
        "commitments": store.list_commitments(&workspace_id)?,
        "tasks": store.list_tasks(&workspace_id)?,
        "artifacts": store.list_artifacts(&workspace_id)?,
    }))
}

// ───────────────── Project 與團隊協作（Handbook Milestone 5）─────────────────

#[derive(Serialize)]
pub struct ProjectResult {
    pub project_id: String,
}

/// 建立一個 Project（有界的協作倡議，Ch.09）。
#[tauri::command]
pub async fn agent_create_project<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    workspace_id: String,
    name: String,
) -> Result<ProjectResult, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;
    let existing: Vec<String> = store
        .list_projects(&workspace_id)?
        .into_iter()
        .map(|p| p.id)
        .collect();
    let project_id = id_from_name(&name, &existing);
    store.put_project(&Project {
        id: project_id.clone(),
        workspace_id,
        name,
        status: ProjectStatus::Active,
        created_at: now_rfc3339(),
    })?;
    Ok(ProjectResult { project_id })
}

/// 一個團隊 assignment（給 [`agent_run_team`]）。
#[derive(Deserialize)]
pub struct TeamAssignment {
    pub employee_id: String,
    pub query: String,
    #[serde(default)]
    pub anchor: Option<String>,
    #[serde(default)]
    pub commitment_id: Option<String>,
}

/// **併發**跑一隊 Employee（各 assignment 須是**相異** employee）。各員工的腦各自解析；
/// gbrain 子行程真並行；Store 經 Mutex 序列化 DB 寫入。回傳各結果。
#[tauri::command]
pub async fn agent_run_team<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    assignments: Vec<TeamAssignment>,
    project_id: Option<String>,
) -> Result<Vec<CycleResult>, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;

    // 各 assignment 解析其 employee 的腦 → ToolCtx（團隊成員可能用不同腦）。
    let mut ctxs: Vec<ToolCtx> = Vec::with_capacity(assignments.len());
    for a in &assignments {
        let emp = store
            .get_employee(&a.employee_id)?
            .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", &a.employee_id))?;
        let entry = crate::brains::brain_entry(&cfg, &emp.brain.brain_id)?;
        ctxs.push(ToolCtx {
            gbrain_exe: cfg.gbrain_exe_path.clone(),
            gbrain_home: entry.env_home().map(|s| s.to_string()),
        });
    }

    let tool = GbrainThinkTool::new();
    let futs = assignments.iter().zip(ctxs.iter()).map(|(a, ctx)| {
        run_cycle(
            &a.employee_id,
            a.query.clone(),
            a.anchor.clone(),
            a.commitment_id.as_deref(),
            project_id.as_deref(),
            &tool,
            ctx,
            &store,
        )
    });
    let results: Vec<anyhow::Result<CycleResult>> = futures::future::join_all(futs).await;
    let out: Vec<CycleResult> = results
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()
        .map_err(|e| AppError::new("agent_os.teamFailed").p("detail", e.to_string()))?;
    Ok(out)
}

#[derive(Serialize)]
pub struct TaskIdResult {
    pub task_id: String,
}

/// **交接**：把一個 Task 指派給另一個 Employee（owner=to），狀態 Assigned（Ch.10／Milestone 5）。
#[tauri::command]
pub async fn agent_handoff_task<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    to_employee_id: String,
    objective: String,
    project_id: Option<String>,
) -> Result<TaskIdResult, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = SqliteStore::open(data_dir.join("emploid.db"))?;
    let emp = store
        .get_employee(&to_employee_id)?
        .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", &to_employee_id))?;
    let ws = emp.workspace_id.clone();
    let existing: Vec<String> = store
        .list_tasks(&ws)?
        .into_iter()
        .map(|t| t.id)
        .collect();
    let task_id = id_from_name("task", &existing);
    store.put_task(&Task {
        id: task_id.clone(),
        workspace_id: ws,
        owner_employee_id: to_employee_id,
        objective: objective.clone(),
        input: objective,
        status: TaskStatus::Assigned,
        output_artifact_id: None,
        commitment_id: None,
        project_id,
        created_at: now_rfc3339(),
    })?;
    Ok(TaskIdResult { task_id })
}

/// **接手**：執行一個已存在的 Task（為其 owner、用其 input 跑循環），並將原 task 標 Completed。
#[tauri::command]
pub async fn agent_run_task<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> Result<CycleResult, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let store = SqliteStore::open(agent_db_path(&app)?)?;
    let mut task = store
        .get_task(&task_id)?
        .ok_or_else(|| AppError::new("agent_os.taskNotFound").p("id", &task_id))?;
    // busy-lock：以 task owner 為準。
    let _guard = state
        .try_acquire(&task.owner_employee_id)
        .ok_or_else(|| AppError::new("agent_os.employeeBusy").p("id", &task.owner_employee_id))?;
    let (tool, ctx) = build_tool_ctx(&cfg, &store, &task.owner_employee_id)?;
    let result = run_cycle(
        &task.owner_employee_id,
        task.input.clone(),
        None,
        task.commitment_id.as_deref(),
        task.project_id.as_deref(),
        &tool,
        &ctx,
        &store,
    )
    .await?;
    // 將原交接 task 標 Completed、連結產出。
    task.status = TaskStatus::Completed;
    task.output_artifact_id = Some(result.artifact_id.clone());
    store.put_task(&task)?;
    Ok(result)
}

// ───────────────── 溝通（Message-driven Trigger，Phase 6c）─────────────────

#[derive(Serialize)]
pub struct SendMessageResult {
    pub task_id: String,
}

/// 溝通：人類的一則訊息 → 目標員工 Inbox 裡一個 `Assigned` Task，並喚醒該員工。
///
/// 訊息即 Message-driven Trigger（Handbook Ch.12 §2／Ch.04 Inbox）——其內容成為 Inbox 裡的一個
/// Task；排程器 `scan_inbox` 會以 [`run_inbox`] 消化（訊息無 commitment 也會被處理）。
/// 本指令不執行員工、不搶 busy-lock——只投遞工作＋發喚醒信號。
#[tauri::command]
pub async fn agent_send_message<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    employee_id: String,
    text: String,
    commitment_id: Option<String>,
) -> Result<SendMessageResult, AppError> {
    let store = agent_store(&app)?;
    let emp = store
        .get_employee(&employee_id)?
        .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", &employee_id))?;
    let existing: Vec<String> = store
        .list_tasks(&emp.workspace_id)?
        .into_iter()
        .map(|t| t.id)
        .collect();
    let task_id = next_id("msg", &existing);
    store.put_task(&Task {
        id: task_id.clone(),
        workspace_id: emp.workspace_id.clone(),
        owner_employee_id: employee_id.clone(),
        objective: "Human message".into(),
        input: text,
        status: TaskStatus::Assigned,
        output_artifact_id: None,
        commitment_id,
        project_id: None,
        created_at: now_rfc3339(),
    })?;
    // 推喚醒信號（best-effort；即便 channel 滿，下次 30s tick 也會掃到這個 Assigned task）。
    state.wake(WakeSignal {
        employee_id,
        reason: "message".into(),
    });
    Ok(SendMessageResult { task_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::JsonStore;
    use crate::domain::tools::{ToolFuture, ToolOutput};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// 測試用 stub：回固定輸出，並計數被 invoke 幾次（驗 Tool 邊界——不被呼叫就不動）。
    struct StubTool {
        spec: ToolSpec,
        canned: String,
        calls: AtomicU32,
    }
    impl StubTool {
        fn new(canned: impl Into<String>) -> Self {
            Self {
                spec: ToolSpec {
                    id: "stub".into(),
                    description: "test stub".into(),
                },
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
            Box::pin(async move {
                Ok(ToolOutput {
                    text,
                    meta: serde_json::json!({"stub": true}),
                })
            })
        }
    }

    /// 帶人工延遲的 stub——用於證明 run_cycle 併發（join_all）真的重疊（計時）。
    struct SlowStub {
        spec: ToolSpec,
        delay_ms: u64,
        canned: String,
    }
    impl SlowStub {
        fn new(delay_ms: u64, canned: impl Into<String>) -> Self {
            Self {
                spec: ToolSpec { id: "slow".into(), description: "delayed stub".into() },
                delay_ms,
                canned: canned.into(),
            }
        }
    }
    impl Tool for SlowStub {
        fn spec(&self) -> &ToolSpec {
            &self.spec
        }
        fn invoke<'a>(&'a self, _input: ToolInput, _ctx: &'a ToolCtx) -> ToolFuture<'a> {
            let delay = self.delay_ms;
            let text = self.canned.clone();
            Box::pin(async move {
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                Ok(ToolOutput { text, meta: serde_json::json!({}) })
            })
        }
    }

    /// 推理器 stub：依序回傳預錄 JSON 字串（耗盡則回 `{"done": true}`）。測 run_autonomous 用。
    struct StubReasoner {
        responses: std::sync::Mutex<std::collections::VecDeque<String>>,
    }
    impl StubReasoner {
        fn new(responses: Vec<&str>) -> Self {
            Self {
                responses: std::sync::Mutex::new(
                    responses.into_iter().map(str::to_string).collect(),
                ),
            }
        }
    }
    impl Reasoner for StubReasoner {
        fn reason<'a>(&'a self, _system: &'a str, _user: &'a str) -> ReasonerFuture<'a> {
            let resp = {
                let mut g = self.responses.lock().unwrap();
                g.pop_front().unwrap_or_else(|| "{\"done\": true}".into())
            };
            Box::pin(async move {
                let v: serde_json::Value =
                    serde_json::from_str(&resp).unwrap_or_else(|_| serde_json::json!({"done": true}));
                Ok(v)
            })
        }
    }

    fn test_dir() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "emploid-runtime-test-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 種一個 workspace＋employee，回 employee_id（泛用於任一 store）。
    fn seed(store: &dyn Store) -> String {
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
                brain: BrainRef {
                    brain_id: "__default__".into(),
                },
                role: None,
                template_id: None,
                state: EmployeeState::Sleeping,
                created_at: "t".into(),
            })
            .unwrap();
        emp_id
    }

    fn ctx() -> ToolCtx {
        ToolCtx {
            gbrain_exe: String::new(),
            gbrain_home: None,
        }
    }

    #[tokio::test]
    async fn cycle_runs_commits_artifact_and_sleeps() {
        let dir = test_dir();
        let store = JsonStore::new(&dir);
        let emp_id = seed(&store);
        let tool = StubTool::new("# 合成結果\n答案在這裡");

        let res = run_cycle(&emp_id, "測試問題".into(), None, None, None, &tool, &ctx(), &store)
            .await
            .unwrap();

        // Tool 恰被呼叫一次（Runtime 驅動；非自发）。
        assert_eq!(tool.call_count(), 1);

        // Employee → Sleeping
        let emp = store.get_employee(&emp_id).unwrap().unwrap();
        assert_eq!(emp.state, EmployeeState::Sleeping);

        // 一個 Committed Artifact，produced_by 正確
        let arts = store.list_artifacts(&emp.workspace_id).unwrap();
        assert_eq!(arts.len(), 1);
        assert_eq!(arts[0].status, ArtifactStatus::Committed);
        assert_eq!(arts[0].produced_by, emp_id);
        assert_eq!(arts[0].id, res.artifact_id);
        assert!(res.artifact_content.contains("合成結果"));

        // 一個 Completed Task，連到該 artifact
        let tasks = store.list_tasks(&emp.workspace_id).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Completed);
        assert_eq!(tasks[0].output_artifact_id.as_deref(), Some(res.artifact_id.as_str()));

        // Memory 記了一筆 note、指向該 artifact
        let mem = store.get_memory(&emp_id).unwrap().unwrap();
        assert_eq!(mem.notes.len(), 1);
        assert_eq!(mem.last_artifact_id.as_deref(), Some(res.artifact_id.as_str()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn run_inbox_drains_assigned_tasks() {
        let dir = test_dir();
        let store = JsonStore::new(&dir);
        let emp_id = seed(&store);
        // 放兩個 Assigned task 進員工的 Inbox（不同 input）。
        for (i, input) in ["第一件", "第二件"].iter().enumerate() {
            store
                .put_task(&Task {
                    id: format!("t{i}"),
                    workspace_id: "ws".into(),
                    owner_employee_id: emp_id.clone(),
                    objective: input.to_string(),
                    input: input.to_string(),
                    status: TaskStatus::Assigned,
                    output_artifact_id: None,
                    commitment_id: None,
                    project_id: None,
                    created_at: "t".into(),
                })
                .unwrap();
        }
        let tool = StubTool::new("答案");
        run_inbox(&emp_id, &tool, &ctx(), &store).await.unwrap();

        // 兩件都被處理（Tool 兩次）、標 Completed、各連一個 Committed artifact。
        assert_eq!(tool.call_count(), 2);
        let tasks = store.list_tasks("ws").unwrap();
        assert_eq!(tasks.len(), 2);
        assert!(tasks.iter().all(|t| t.status == TaskStatus::Completed));
        assert_eq!(store.list_artifacts("ws").unwrap().len(), 2);

        // 員工 Sleeping、memory 累積兩筆。
        let emp = store.get_employee(&emp_id).unwrap().unwrap();
        assert_eq!(emp.state, EmployeeState::Sleeping);
        assert_eq!(store.get_memory(&emp_id).unwrap().unwrap().notes.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn run_inbox_empty_inbox_just_sleeps() {
        let dir = test_dir();
        let store = JsonStore::new(&dir);
        let emp_id = seed(&store);
        let tool = StubTool::new("x");
        run_inbox(&emp_id, &tool, &ctx(), &store).await.unwrap();
        // 無待辦 → 不執行 Tool、不產 artifact，直接睡。
        assert_eq!(tool.call_count(), 0);
        let emp = store.get_employee(&emp_id).unwrap().unwrap();
        assert_eq!(emp.state, EmployeeState::Sleeping);
        assert!(store.list_artifacts("ws").unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 承諾驅動：規劃→行動→評估（done）→ commitment Satisfied、產 artifact、員工睡回。
    #[tokio::test]
    async fn run_autonomous_satisfies_commitment() {
        let dir = test_dir();
        let store = JsonStore::new(&dir);
        let emp_id = seed(&store);
        store
            .put_commitment(&Commitment {
                id: "c1".into(),
                workspace_id: "ws".into(),
                owner_employee_id: emp_id.clone(),
                title: "查答案".into(),
                completion_condition: "找到答案".into(),
                status: CommitmentStatus::Active,
                created_at: "t".into(),
                updated_at: "t".into(),
            })
            .unwrap();
        let knowledge = StubTool::new("答案是 42");
        let reasoner = StubReasoner::new(vec![
            r#"{"next_query": "答案是什麼？", "rationale": "先查"}"#,
            r#"{"done": true, "rationale": "已找到"}"#,
        ]);
        let budget = CycleBudget {
            max_cycles: 5,
            max_duration: Duration::from_secs(10),
        };
        let outcome = run_autonomous(&emp_id, "c1", &budget, &knowledge, &reasoner, &ctx(), &store)
            .await
            .unwrap();
        assert!(matches!(outcome, AutonomousOutcome::Satisfied { .. }));
        assert_eq!(
            store.get_commitment("c1").unwrap().unwrap().status,
            CommitmentStatus::Satisfied
        );
        assert_eq!(store.list_artifacts("ws").unwrap().len(), 1);
        assert_eq!(
            store.get_employee(&emp_id).unwrap().unwrap().state,
            EmployeeState::Sleeping
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 卡住且 0 產出（規劃器不給 next_query）→ Stalled、commitment Suspended（避免下次狂跑）。
    #[tokio::test]
    async fn run_autonomous_suspends_on_zero_progress() {
        let dir = test_dir();
        let store = JsonStore::new(&dir);
        let emp_id = seed(&store);
        store
            .put_commitment(&Commitment {
                id: "c-stuck".into(),
                workspace_id: "ws".into(),
                owner_employee_id: emp_id.clone(),
                title: "不可能的任務".into(),
                completion_condition: "做不到".into(),
                status: CommitmentStatus::Active,
                created_at: "t".into(),
                updated_at: "t".into(),
            })
            .unwrap();
        let knowledge = StubTool::new("(不該被呼叫)");
        // 規劃器回的 JSON 沒有 next_query 也沒有 done → Stalled「未給出 next_query」。
        let reasoner = StubReasoner::new(vec![r#"{"rationale": "我不知道下一步"}"#]);
        let budget = CycleBudget {
            max_cycles: 5,
            max_duration: Duration::from_secs(10),
        };
        let outcome =
            run_autonomous(&emp_id, "c-stuck", &budget, &knowledge, &reasoner, &ctx(), &store)
                .await
                .unwrap();
        assert!(matches!(outcome, AutonomousOutcome::Stalled { .. }));
        assert_eq!(knowledge.call_count(), 0); // 未行動
        assert_eq!(
            store.get_commitment("c-stuck").unwrap().unwrap().status,
            CommitmentStatus::Suspended
        );
        assert!(store.list_artifacts("ws").unwrap().is_empty());
        assert_eq!(
            store.get_employee(&emp_id).unwrap().unwrap().state,
            EmployeeState::Sleeping
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn context_restored_across_restart() {
        let dir = test_dir();
        let emp_id = {
            let store = JsonStore::new(&dir);
            let id = seed(&store);
            let tool = StubTool::new("first");
            run_cycle(&id, "第一題".into(), None, None, None, &tool, &ctx(), &store)
                .await
                .unwrap();
            id
        };

        // 模擬重啟：同一 base 重建 store。先前 artifact＋memory 仍在。
        let store = JsonStore::new(&dir);
        let emp = store.get_employee(&emp_id).unwrap().unwrap();
        assert_eq!(store.list_artifacts(&emp.workspace_id).unwrap().len(), 1);
        assert_eq!(store.get_memory(&emp_id).unwrap().unwrap().notes.len(), 1);

        // 再跑一輪：memory 還原後累積第二筆、artifact 兩個。
        let tool = StubTool::new("second");
        run_cycle(&emp_id, "第二題".into(), None, None, None, &tool, &ctx(), &store)
            .await
            .unwrap();
        assert_eq!(store.list_artifacts(&emp.workspace_id).unwrap().len(), 2);
        let mem = store.get_memory(&emp_id).unwrap().unwrap();
        assert_eq!(mem.notes.len(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tool_never_acts_unless_invoked() {
        // Principle 5 邊界：Tool 結構上只有 invoke（見 trait）；不被呼叫時零副作用。
        let tool = StubTool::new("x");
        assert_eq!(tool.call_count(), 0); // 建立後從未自發執行
    }

    /// 真實 gbrain 端到端驗證（需本機 demo 腦＋gbrain.exe）。`#[ignore]`：環境相依，僅手動跑。
    /// 直接讀真實 app-settings.json 取 gbrain exe 與作用中腦，跑 `run_cycle`＋`GbrainThinkTool`，
    /// 不經 Tauri／UI——等同 `agent_run` 但免開 app。
    /// 跑法：`cargo test --manifest-path src-tauri/Cargo.toml runtime::tests::real_gbrain_think_cycle -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn real_gbrain_think_cycle() {
        let cfg_path = dirs::config_dir()
            .expect("no config dir")
            .join("com.emploid.studio")
            .join("app-settings.json");
        let raw: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&cfg_path).expect("read app-settings.json"),
        )
        .expect("parse app-settings.json");
        let app_cfg = raw.get("app_config").expect("app_config");
        let exe = app_cfg
            .get("gbrain_exe_path")
            .and_then(|v| v.as_str())
            .expect("gbrain_exe_path")
            .to_string();
        let active = app_cfg
            .get("active_brain_id")
            .and_then(|v| v.as_str())
            .expect("active_brain_id")
            .to_string();
        let brains = app_cfg.get("brains").and_then(|v| v.as_array()).expect("brains");
        let home = brains
            .iter()
            .find(|b| b.get("id").and_then(|v| v.as_str()) == Some(active.as_str()))
            .and_then(|b| b.get("gbrain_home"))
            .and_then(|v| v.as_str())
            .map(str::to_string); // None = 預設腦

        let dir = test_dir();
        let store = SqliteStore::open(dir.join("test.db")).unwrap();
        store
            .put_workspace(&Workspace {
                id: "ws".into(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        let emp_id = "emp".to_string();
        store
            .put_employee(&Employee {
                id: emp_id.clone(),
                workspace_id: "ws".into(),
                name: "E".into(),
                brain: BrainRef { brain_id: active },
                role: None,
                template_id: None,
                state: EmployeeState::Sleeping,
                created_at: now_rfc3339(),
            })
            .unwrap();

        let tool = GbrainThinkTool::new();
        let ctx = ToolCtx {
            gbrain_exe: exe,
            gbrain_home: home,
        };
        let res = run_cycle(
            &emp_id,
            "晶瀚半導體開過幾場會議？".into(),
            Some("晶瀚半導體".into()),
            None,
            None,
            &tool,
            &ctx,
            &store,
        )
        .await
        .expect("run_cycle");

        println!("== tool_meta ==\n{}", res.tool_meta);
        println!("== artifact_content ==\n{}", res.artifact_content);
        let graph = res
            .tool_meta
            .get("graph")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert!(graph > 0, "Graph 應 > 0（實際 {graph}）；可能 think 掉到 opus 或未連邊");
        assert!(!res.artifact_content.trim().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Phase 6 真實驗證：員工帶著 Assigned inbox task → `run_inbox` 喚醒並以真實 gbrain think
    /// 處理、產 artifact、標 task Completed、睡回（等同排程器啟動掃描喚醒，但免開 app／排程器）。
    /// 跑法：`cargo test --manifest-path src-tauri/Cargo.toml runtime::tests::real_inbox_wake -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn real_inbox_wake() {
        let cfg_path = dirs::config_dir()
            .expect("no config dir")
            .join("com.emploid.studio")
            .join("app-settings.json");
        let raw: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&cfg_path).expect("read app-settings.json"),
        )
        .expect("parse app-settings.json");
        let app_cfg = raw.get("app_config").expect("app_config");
        let exe = app_cfg
            .get("gbrain_exe_path")
            .and_then(|v| v.as_str())
            .expect("gbrain_exe_path")
            .to_string();
        let active = app_cfg
            .get("active_brain_id")
            .and_then(|v| v.as_str())
            .expect("active_brain_id")
            .to_string();
        let home = app_cfg
            .get("brains")
            .and_then(|v| v.as_array())
            .expect("brains")
            .iter()
            .find(|b| b.get("id").and_then(|v| v.as_str()) == Some(active.as_str()))
            .and_then(|b| b.get("gbrain_home"))
            .and_then(|v| v.as_str())
            .map(str::to_string); // None = 預設腦

        let dir = test_dir();
        let store = SqliteStore::open(dir.join("test.db")).unwrap();
        store
            .put_workspace(&Workspace {
                id: "ws".into(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        let emp_id = "emp".to_string();
        store
            .put_employee(&Employee {
                id: emp_id.clone(),
                workspace_id: "ws".into(),
                name: "E".into(),
                brain: BrainRef { brain_id: active },
                role: None,
                template_id: None,
                state: EmployeeState::Sleeping,
                created_at: now_rfc3339(),
            })
            .unwrap();
        // 一個 Assigned inbox task（訊息或交接投遞）。
        store
            .put_task(&Task {
                id: "t-inbox".into(),
                workspace_id: "ws".into(),
                owner_employee_id: emp_id.clone(),
                objective: "查會議".into(),
                input: "晶瀚半導體開過幾場會議？".into(),
                status: TaskStatus::Assigned,
                output_artifact_id: None,
                commitment_id: None,
                project_id: None,
                created_at: now_rfc3339(),
            })
            .unwrap();

        let tool = GbrainThinkTool::new();
        let ctx = ToolCtx {
            gbrain_exe: exe,
            gbrain_home: home,
        };
        run_inbox(&emp_id, &tool, &ctx, &store)
            .await
            .expect("run_inbox");

        // task → Completed 連 artifact；員工睡回。
        let task = store.get_task("t-inbox").unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        let artifact_id = task.output_artifact_id.clone().expect("output artifact");
        let art = store.get_artifact(&artifact_id).unwrap().unwrap();
        let emp = store.get_employee(&emp_id).unwrap().unwrap();
        assert_eq!(emp.state, EmployeeState::Sleeping);
        println!("== inbox artifact ==\n{}", art.content);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Commitment 跨重啟連續生成多個 task（Principle 9），且 artifact 帶 commitment provenance。
    #[tokio::test]
    async fn commitment_spans_tasks_across_restart() {
        let dir = test_dir();
        let db = dir.join("test.db");
        let ws = "ws".to_string();
        let emp_id = "emp".to_string();
        let cid = "track-po".to_string();
        {
            let store = SqliteStore::open(&db).unwrap();
            store
                .put_workspace(&Workspace {
                    id: ws.clone(),
                    name: "WS".into(),
                    description: None,
                    status: WorkspaceStatus::Active,
                    created_at: now_rfc3339(),
                })
                .unwrap();
            store
                .put_employee(&Employee {
                    id: emp_id.clone(),
                    workspace_id: ws.clone(),
                    name: "E".into(),
                    brain: BrainRef { brain_id: "__default__".into() },
                    role: None,
                    template_id: None,
                    state: EmployeeState::Sleeping,
                    created_at: now_rfc3339(),
                })
                .unwrap();
            store
                .put_commitment(&Commitment {
                    id: cid.clone(),
                    workspace_id: ws.clone(),
                    owner_employee_id: emp_id.clone(),
                    title: "track".into(),
                    completion_condition: "done".into(),
                    status: CommitmentStatus::Active,
                    created_at: now_rfc3339(),
                    updated_at: now_rfc3339(),
                })
                .unwrap();
            let tool = StubTool::new("first");
            run_cycle(&emp_id, "q1".into(), None, Some(&cid), None, &tool, &ctx(), &store)
                .await
                .unwrap();
        }

        // 模擬重啟：重開同一 db。commitment／task／artifact 皆在。
        let store = SqliteStore::open(&db).unwrap();
        assert_eq!(
            store.get_commitment(&cid).unwrap().unwrap().status,
            CommitmentStatus::Active
        );
        assert_eq!(store.list_tasks(&ws).unwrap().len(), 1);
        assert_eq!(store.list_artifacts(&ws).unwrap().len(), 1);

        // 第二個 task（同 commitment）——commitment 活過 task
        let tool = StubTool::new("second");
        run_cycle(&emp_id, "q2".into(), None, Some(&cid), None, &tool, &ctx(), &store)
            .await
            .unwrap();
        assert_eq!(store.list_tasks(&ws).unwrap().len(), 2);
        assert_eq!(store.list_artifacts(&ws).unwrap().len(), 2);
        for a in store.list_artifacts(&ws).unwrap() {
            assert_eq!(a.source_commitment_id.as_deref(), Some(cid.as_str()));
            assert!(a.source_task_id.is_some());
        }
        assert_eq!(
            store.get_commitment(&cid).unwrap().unwrap().status,
            CommitmentStatus::Active
        );

        // 手動滿足
        let mut com = store.get_commitment(&cid).unwrap().unwrap();
        com.status = CommitmentStatus::Satisfied;
        store.put_commitment(&com).unwrap();
        assert_eq!(
            store.get_commitment(&cid).unwrap().unwrap().status,
            CommitmentStatus::Satisfied
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Artifact 修訂保留歷史（Principle 3）：舊版 Superseded、新版 Committed、revised_from 鍊正確。
    #[test]
    fn revise_artifact_keeps_history() {
        let dir = test_dir();
        let store = JsonStore::new(&dir);
        store
            .put_workspace(&Workspace {
                id: "ws".into(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: "t".into(),
            })
            .unwrap();
        let v1 = Artifact {
            id: "a1".into(),
            workspace_id: "ws".into(),
            title: "T".into(),
            artifact_type: "report".into(),
            content: "v1".into(),
            produced_by: "e".into(),
            source_task_id: None,
            source_commitment_id: None,
            revised_from_id: None,
            project_id: None,
            version: 1,
            status: ArtifactStatus::Committed,
            created_at: "t".into(),
        };
        store.put_artifact(&v1).unwrap();

        let (new_id, ver) = revise_artifact(&store, "a1", "e", "v2 body".into()).unwrap();
        assert_eq!(ver, 2);

        let v1b = store.get_artifact("a1").unwrap().unwrap();
        let v2 = store.get_artifact(&new_id).unwrap().unwrap();
        assert_eq!(v1b.status, ArtifactStatus::Superseded);
        assert_eq!(v2.status, ArtifactStatus::Committed);
        assert_eq!(v2.revised_from_id.as_deref(), Some("a1"));
        assert_eq!(v2.version, 2);
        assert_eq!(store.list_artifacts("ws").unwrap().len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 多員工共用同一腦，各自產出、記憶互不污染（Principle 6）。
    #[tokio::test]
    async fn shared_brain_two_employees_independent() {
        let dir = test_dir();
        let db = dir.join("test.db");
        let store = SqliteStore::open(&db).unwrap();
        let ws = "ws".to_string();
        let brain = "__default__".to_string();
        store
            .put_workspace(&Workspace {
                id: ws.clone(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        for name in ["steve", "mary"] {
            store
                .put_employee(&Employee {
                    id: name.into(),
                    workspace_id: ws.clone(),
                    name: name.into(),
                    brain: BrainRef { brain_id: brain.clone() },
                    role: None,
                    template_id: None,
                    state: EmployeeState::Sleeping,
                    created_at: now_rfc3339(),
                })
                .unwrap();
        }
        let s = StubTool::new("steve-out");
        run_cycle("steve", "q".into(), None, None, None, &s, &ctx(), &store)
            .await
            .unwrap();
        let m = StubTool::new("mary-out");
        run_cycle("mary", "q".into(), None, None, None, &m, &ctx(), &store)
            .await
            .unwrap();

        let arts = store.list_artifacts(&ws).unwrap();
        assert_eq!(arts.len(), 2);
        let by: Vec<String> = arts.iter().map(|a| a.produced_by.clone()).collect();
        assert!(by.contains(&"steve".into()) && by.contains(&"mary".into()));
        assert_eq!(store.get_memory("steve").unwrap().unwrap().notes.len(), 1);
        assert_eq!(store.get_memory("mary").unwrap().unwrap().notes.len(), 1);
        let emps = store.list_employees(&ws).unwrap();
        assert!(emps.iter().all(|e| e.brain.brain_id == brain));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Milestone 3 核心：升級共享腦，新員工採用，舊員工的進行中工作不失（Principle 1）。
    #[tokio::test]
    async fn brain_upgrade_preserves_inflight_work() {
        let dir = test_dir();
        let db = dir.join("test.db");
        let store = SqliteStore::open(&db).unwrap();
        let ws = "ws".to_string();
        let brain = "demo".to_string();
        store
            .put_workspace(&Workspace {
                id: ws.clone(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        for name in ["steve", "mary"] {
            store
                .put_employee(&Employee {
                    id: name.into(),
                    workspace_id: ws.clone(),
                    name: name.into(),
                    brain: BrainRef { brain_id: brain.clone() },
                    role: None,
                    template_id: None,
                    state: EmployeeState::Sleeping,
                    created_at: now_rfc3339(),
                })
                .unwrap();
        }
        // steve 在腦 v1 下跑
        let s1 = StubTool::new("v1");
        let r1 = run_cycle("steve", "q".into(), None, None, None, &s1, &ctx(), &store)
            .await
            .unwrap();
        let steve_notes_before = store.get_memory("steve").unwrap().unwrap().notes.len();

        // 腦「升級」：同一 brain_id，知識演化為 v2；mary 採用。
        let m2 = StubTool::new("v2");
        let r2 = run_cycle("mary", "q".into(), None, None, None, &m2, &ctx(), &store)
            .await
            .unwrap();

        // steve 的進行中工作（v1 artifact）未因升級／mary 而失。
        let steve_art = store.get_artifact(&r1.artifact_id).unwrap().unwrap();
        assert_eq!(steve_art.content, "v1");
        // mary 採用升級後的腦（v2）。
        let mary_art = store.get_artifact(&r2.artifact_id).unwrap().unwrap();
        assert_eq!(mary_art.content, "v2");
        // steve memory 不受 mary 影響。
        assert_eq!(
            store.get_memory("steve").unwrap().unwrap().notes.len(),
            steve_notes_before
        );
        let emps = store.list_employees(&ws).unwrap();
        assert!(emps.iter().all(|e| e.brain.brain_id == brain));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 真實 gbrain：兩員工共用 demo 腦，各自 think、狀態獨立。
    #[tokio::test]
    #[ignore]
    async fn real_shared_brain() {
        let cfg_path = dirs::config_dir()
            .expect("no config dir")
            .join("com.emploid.studio")
            .join("app-settings.json");
        let raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cfg_path).expect("read app-settings.json"))
                .expect("parse app-settings.json");
        let app_cfg = raw.get("app_config").expect("app_config");
        let exe = app_cfg
            .get("gbrain_exe_path")
            .and_then(|v| v.as_str())
            .expect("gbrain_exe_path")
            .to_string();
        let active = app_cfg
            .get("active_brain_id")
            .and_then(|v| v.as_str())
            .expect("active_brain_id")
            .to_string();
        let brains = app_cfg.get("brains").and_then(|v| v.as_array()).expect("brains");
        let home = brains
            .iter()
            .find(|b| b.get("id").and_then(|v| v.as_str()) == Some(active.as_str()))
            .and_then(|b| b.get("gbrain_home"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let ctx = ToolCtx {
            gbrain_exe: exe,
            gbrain_home: home,
        };

        let dir = test_dir();
        let db = dir.join("test.db");
        let store = SqliteStore::open(&db).unwrap();
        let ws = "ws".to_string();
        store
            .put_workspace(&Workspace {
                id: ws.clone(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        for name in ["emp-a", "emp-b"] {
            store
                .put_employee(&Employee {
                    id: name.into(),
                    workspace_id: ws.clone(),
                    name: name.into(),
                    brain: BrainRef { brain_id: active.clone() },
                    role: None,
                    template_id: None,
                    state: EmployeeState::Sleeping,
                    created_at: now_rfc3339(),
                })
                .unwrap();
        }

        let tool = GbrainThinkTool::new();
        let ra = run_cycle(
            "emp-a",
            "晶瀚半導體開過幾場會議？".into(),
            Some("晶瀚半導體".into()),
            None,
            None,
            &tool,
            &ctx,
            &store,
        )
        .await
        .expect("run emp-a");
        let rb = run_cycle(
            "emp-b",
            "誰主持了良率檢討會？".into(),
            Some("晶瀚半導體".into()),
            None,
            None,
            &tool,
            &ctx,
            &store,
        )
        .await
        .expect("run emp-b");

        println!("== emp-a meta ==\n{}", ra.tool_meta);
        println!("== emp-b meta ==\n{}", rb.tool_meta);
        let arts = store.list_artifacts(&ws).unwrap();
        assert_eq!(arts.len(), 2);
        assert!(arts.iter().any(|a| a.produced_by == "emp-a"));
        assert!(arts.iter().any(|a| a.produced_by == "emp-b"));
        assert_eq!(store.get_memory("emp-a").unwrap().unwrap().notes.len(), 1);
        assert_eq!(store.get_memory("emp-b").unwrap().unwrap().notes.len(), 1);
        let ga = ra.tool_meta.get("graph").and_then(|v| v.as_i64()).unwrap_or(0);
        assert!(ga > 0, "emp-a Graph 應 > 0（實際 {ga}）");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Milestone 4：一個 Template 部署多個獨立 Instance，共享 brain／role，各自現實。
    #[tokio::test]
    async fn template_deploys_independent_instances() {
        let dir = test_dir();
        let db = dir.join("test.db");
        let store = SqliteStore::open(&db).unwrap();
        let ws = "ws".to_string();
        store
            .put_workspace(&Workspace {
                id: ws.clone(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        // 建 template "steve"（腦 demo、role procurement）
        store
            .put_template(&EmployeeTemplate {
                id: "steve".into(),
                workspace_id: ws.clone(),
                name: "Procurement Steve".into(),
                brain: BrainRef { brain_id: "demo".into() },
                role: Some("procurement".into()),
                created_at: now_rfc3339(),
            })
            .unwrap();
        // 部署 3 個 instance
        let tw = deploy_instance(&store, "steve", "Steve-TW").unwrap();
        let nj = deploy_instance(&store, "steve", "Steve-NJ").unwrap();
        let vn = deploy_instance(&store, "steve", "Steve-VN").unwrap();

        let emps = store.list_employees(&ws).unwrap();
        assert_eq!(emps.len(), 3);
        for e in &emps {
            assert_eq!(e.brain.brain_id, "demo");
            assert_eq!(e.role.as_deref(), Some("procurement"));
            assert_eq!(e.template_id.as_deref(), Some("steve"));
        }
        let ids: Vec<String> = emps.iter().map(|e| e.id.clone()).collect();
        assert!(ids.contains(&tw) && ids.contains(&nj) && ids.contains(&vn));

        // 各自跑一圈，獨立產出
        for id in [&tw, &nj, &vn] {
            let tool = StubTool::new(format!("out-{id}"));
            run_cycle(id, "q".into(), None, None, None, &tool, &ctx(), &store)
                .await
                .unwrap();
        }
        assert_eq!(store.list_artifacts(&ws).unwrap().len(), 3);
        assert_eq!(store.get_memory(&tw).unwrap().unwrap().notes.len(), 1);
        assert_eq!(store.get_memory(&nj).unwrap().unwrap().notes.len(), 1);

        // 一個 instance 的 commitment 不影響他人
        store
            .put_commitment(&Commitment {
                id: "c-tw".into(),
                workspace_id: ws.clone(),
                owner_employee_id: tw.clone(),
                title: "tw only".into(),
                completion_condition: "x".into(),
                status: CommitmentStatus::Active,
                created_at: now_rfc3339(),
                updated_at: now_rfc3339(),
            })
            .unwrap();
        let coms = store.list_commitments(&ws).unwrap();
        assert_eq!(coms.len(), 1);
        assert_eq!(coms[0].owner_employee_id, tw);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Milestone 5：一隊 Employee 併發跑、產出共享 Artifact（同 Project）。
    #[tokio::test]
    async fn team_runs_concurrently() {
        let dir = test_dir();
        let db = dir.join("test.db");
        let store = SqliteStore::open(&db).unwrap();
        let ws = "ws".to_string();
        let proj = "proj".to_string();
        store
            .put_workspace(&Workspace {
                id: ws.clone(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        store
            .put_project(&Project {
                id: proj.clone(),
                workspace_id: ws.clone(),
                name: "P".into(),
                status: ProjectStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        let names = ["a", "b", "c"];
        let brain = "__default__".to_string();
        for n in names {
            store
                .put_employee(&Employee {
                    id: n.into(),
                    workspace_id: ws.clone(),
                    name: n.into(),
                    brain: BrainRef { brain_id: brain.clone() },
                    role: None,
                    template_id: None,
                    state: EmployeeState::Sleeping,
                    created_at: now_rfc3339(),
                })
                .unwrap();
        }
        let tools: Vec<StubTool> = names.iter().map(|n| StubTool::new(format!("out-{n}"))).collect();
        let ctx = ctx();
        let futs = names.iter().enumerate().map(|(i, &n)| {
            run_cycle(
                n,
                format!("q-{n}"),
                None,
                None,
                Some(proj.as_str()),
                &tools[i],
                &ctx,
                &store,
            )
        });
        let results: Vec<_> = futures::future::join_all(futs).await;
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_ok()));

        let arts = store.list_artifacts(&ws).unwrap();
        assert_eq!(arts.len(), 3);
        assert!(arts.iter().all(|a| a.project_id.as_deref() == Some("proj")));
        let by: Vec<String> = arts.iter().map(|a| a.produced_by.clone()).collect();
        assert!(by.contains(&"a".into()) && by.contains(&"b".into()) && by.contains(&"c".into()));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Milestone 5：Task 交接（A→B）＋ B 接手執行。
    #[tokio::test]
    async fn handoff_task_between_employees() {
        let dir = test_dir();
        let db = dir.join("test.db");
        let store = SqliteStore::open(&db).unwrap();
        let ws = "ws".to_string();
        store
            .put_workspace(&Workspace {
                id: ws.clone(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        for n in ["alice", "bob"] {
            store
                .put_employee(&Employee {
                    id: n.into(),
                    workspace_id: ws.clone(),
                    name: n.into(),
                    brain: BrainRef { brain_id: "__default__".into() },
                    role: None,
                    template_id: None,
                    state: EmployeeState::Sleeping,
                    created_at: now_rfc3339(),
                })
                .unwrap();
        }
        // alice 交接到 bob：建立 Assigned task owned by bob。
        store
            .put_task(&Task {
                id: "task".into(),
                workspace_id: ws.clone(),
                owner_employee_id: "bob".into(),
                objective: "查 E-07 根因".into(),
                input: "查 E-07 根因".into(),
                status: TaskStatus::Assigned,
                output_artifact_id: None,
                commitment_id: None,
                project_id: None,
                created_at: now_rfc3339(),
            })
            .unwrap();
        assert_eq!(
            store.get_task("task").unwrap().unwrap().owner_employee_id,
            "bob"
        );

        // bob 接手執行（run_cycle）→ 標記原 task Completed。
        let tool = StubTool::new("bob-answer");
        let ctx = ctx();
        let res = run_cycle("bob", "查 E-07 根因".into(), None, None, None, &tool, &ctx, &store)
            .await
            .unwrap();
        let mut t = store.get_task("task").unwrap().unwrap();
        t.status = TaskStatus::Completed;
        t.output_artifact_id = Some(res.artifact_id.clone());
        store.put_task(&t).unwrap();

        let task = store.get_task("task").unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.output_artifact_id.as_deref(), Some(res.artifact_id.as_str()));
        let art = store.get_artifact(&res.artifact_id).unwrap().unwrap();
        assert_eq!(art.produced_by, "bob");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 併發實證：兩員工在 demo 腦上併發 think，wall-clock ≈ 單人（非兩倍）。
    #[tokio::test]
    #[ignore]
    async fn real_team_concurrent() {
        let cfg_path = dirs::config_dir()
            .expect("no config dir")
            .join("com.emploid.studio")
            .join("app-settings.json");
        let raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cfg_path).expect("read app-settings.json"))
                .expect("parse app-settings.json");
        let app_cfg = raw.get("app_config").expect("app_config");
        let exe = app_cfg
            .get("gbrain_exe_path")
            .and_then(|v| v.as_str())
            .expect("gbrain_exe_path")
            .to_string();
        let active = app_cfg
            .get("active_brain_id")
            .and_then(|v| v.as_str())
            .expect("active_brain_id")
            .to_string();
        let brains = app_cfg.get("brains").and_then(|v| v.as_array()).expect("brains");
        let home = brains
            .iter()
            .find(|b| b.get("id").and_then(|v| v.as_str()) == Some(active.as_str()))
            .and_then(|b| b.get("gbrain_home"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let ctx = ToolCtx {
            gbrain_exe: exe,
            gbrain_home: home,
        };

        let dir = test_dir();
        let db = dir.join("test.db");
        let store = SqliteStore::open(&db).unwrap();
        let ws = "ws".to_string();
        store
            .put_workspace(&Workspace {
                id: ws.clone(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        for n in ["emp-a", "emp-b"] {
            store
                .put_employee(&Employee {
                    id: n.into(),
                    workspace_id: ws.clone(),
                    name: n.into(),
                    brain: BrainRef { brain_id: active.clone() },
                    role: None,
                    template_id: None,
                    state: EmployeeState::Sleeping,
                    created_at: now_rfc3339(),
                })
                .unwrap();
        }

        let tool = GbrainThinkTool::new();
        let queries = [
            ("emp-a", "晶瀚半導體開過幾場會議？"),
            ("emp-b", "誰主持了良率檢討會？"),
        ];
        let start = std::time::Instant::now();
        let futs = queries.iter().map(|(emp, q)| {
            run_cycle(emp, (*q).into(), Some("晶瀚半導體".into()), None, None, &tool, &ctx, &store)
        });
        let results: Vec<_> = futures::future::join_all(futs).await;
        let elapsed = start.elapsed();
        println!("concurrent 2 employees elapsed: {elapsed:?}");

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
        let arts = store.list_artifacts(&ws).unwrap();
        assert_eq!(arts.len(), 2);
        let ga = results[0]
            .as_ref()
            .unwrap()
            .tool_meta
            .get("graph")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert!(ga > 0, "Graph 應 > 0（實際 {ga}）");
        // 注意：同腦（demo）下 gbrain 對該腦 DB 序列化，故 wall-clock 不證 Emploid 併行；
        // Emploid 的併發機制由 `concurrent_cycles_overlap_in_time`（延遲 stub）證明。
        println!("concurrent 2 employees elapsed (同腦，gbrain 序列化): {elapsed:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Emploid 併發實證：3 個 run_cycle（各 800ms 延遲）併發跑，總時間 ≈ 1×（非 3×）。
    #[tokio::test]
    async fn concurrent_cycles_overlap_in_time() {
        let dir = test_dir();
        let db = dir.join("test.db");
        let store = SqliteStore::open(&db).unwrap();
        let ws = "ws".to_string();
        store
            .put_workspace(&Workspace {
                id: ws.clone(),
                name: "WS".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: now_rfc3339(),
            })
            .unwrap();
        let names = ["a", "b", "c"];
        for n in names {
            store
                .put_employee(&Employee {
                    id: n.into(),
                    workspace_id: ws.clone(),
                    name: n.into(),
                    brain: BrainRef { brain_id: "x".into() },
                    role: None,
                    template_id: None,
                    state: EmployeeState::Sleeping,
                    created_at: now_rfc3339(),
                })
                .unwrap();
        }
        let tools: Vec<SlowStub> = names.iter().map(|_| SlowStub::new(800, "out")).collect();
        let ctx = ctx();
        let start = std::time::Instant::now();
        let futs = names.iter().enumerate().map(|(i, &n)| {
            run_cycle(n, "q".into(), None, None, None, &tools[i], &ctx, &store)
        });
        let results: Vec<_> = futures::future::join_all(futs).await;
        let elapsed = start.elapsed();
        assert!(results.iter().all(|r| r.is_ok()));
        // 併發：3×800ms ≈ 800ms；循序會 ≈ 2400ms。以 <1.6s 證重疊。
        assert!(
            elapsed.as_millis() < 1600,
            "併發未重疊：3×800ms 應 <1.6s，實際 {elapsed:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
