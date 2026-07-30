//! Runtime（Handbook Ch.13）——Emploid 的執行引擎：管理 wake→restore→execute→commit→sleep
//! 的循環，**永不介入推理**（Principle 10）。
//!
//! Phase 1：單一 Employee、單發（一次 think → 一個 Artifact）、人工觸發（`agent_run` 指令）。
//! 推理第一版固定走 gbrain think/ask（決策 D4），故 [`GbrainThinkTool`] 是第一個 Tool。
//! Tool 邊界由 [`crate::domain::tools::Tool`] trait 結構保證（只有 `invoke`，見 Principle 5）。

use std::process::Stdio;

use serde::Serialize;
use tauri::Manager;

use crate::config::app_config;
use crate::config::DEFAULT_BRAIN_ID;
use crate::domain::tools::ToolFuture;
use crate::domain::{
    id_from_name, next_id, now_rfc3339, Artifact, ArtifactStatus, BrainRef, Commitment,
    CommitmentStatus, Employee, EmployeeState, Memory, SqliteStore, Store, Task, TaskStatus, Tool,
    ToolCtx, ToolInput, Workspace, WorkspaceStatus,
};
use crate::domain::tools::{ToolOutput, ToolSpec};
use crate::i18n::AppError;

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
    let mut memory = store.get_memory(employee_id)?.unwrap_or(Memory {
        employee_id: employee_id.to_string(),
        notes: Vec::new(),
        last_artifact_id: None,
        updated_at: now_rfc3339(),
    });

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

    let existing_artifact_ids: Vec<String> = store
        .list_artifacts(&workspace_id)?
        .into_iter()
        .map(|a| a.id)
        .collect();
    let artifact_id = id_from_name(&format!("think-{}", query), &existing_artifact_ids);
    let artifact = Artifact {
        id: artifact_id.clone(),
        workspace_id: workspace_id.clone(),
        title: format!("think: {query}"),
        artifact_type: "think".into(),
        content: output.text.clone(),
        produced_by: employee_id.to_string(),
        source_task_id: Some(task_id.clone()),
        source_commitment_id: commitment_id.map(str::to_string),
        revised_from_id: None,
        version: 1,
        status: ArtifactStatus::Committed,
        created_at: now_rfc3339(),
    };
    store.put_artifact(&artifact)?;

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
        state: EmployeeState::Sleeping,
        created_at: now_rfc3339(),
    })?;
    Ok(RecruitResult { employee_id: emp_id })
}

/// 跑一輪 Employee 循環：載入 employee→解析腦→建 ToolCtx→run_cycle（gbrain think）。
/// `commitment_id` 可選：綁定則此循環的 task／artifact 連到該長期責任。
#[tauri::command]
pub async fn agent_run<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    employee_id: String,
    query: String,
    anchor: Option<String>,
    commitment_id: Option<String>,
) -> Result<CycleResult, AppError> {
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
    let entry = crate::brains::brain_entry(&cfg, &emp.brain.brain_id)?;
    let ctx = ToolCtx {
        gbrain_exe: cfg.gbrain_exe_path.clone(),
        gbrain_home: entry.env_home().map(|s| s.to_string()),
    };

    let tool = GbrainThinkTool::new();
    let result = run_cycle(
        &employee_id,
        query,
        anchor,
        commitment_id.as_deref(),
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
        "employees": store.list_employees(&workspace_id)?,
        "commitments": store.list_commitments(&workspace_id)?,
        "tasks": store.list_tasks(&workspace_id)?,
        "artifacts": store.list_artifacts(&workspace_id)?,
    }))
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

        let res = run_cycle(&emp_id, "測試問題".into(), None, None, &tool, &ctx(), &store)
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
    async fn context_restored_across_restart() {
        let dir = test_dir();
        let emp_id = {
            let store = JsonStore::new(&dir);
            let id = seed(&store);
            let tool = StubTool::new("first");
            run_cycle(&id, "第一題".into(), None, None, &tool, &ctx(), &store)
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
        run_cycle(&emp_id, "第二題".into(), None, None, &tool, &ctx(), &store)
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
            run_cycle(&emp_id, "q1".into(), None, Some(&cid), &tool, &ctx(), &store)
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
        run_cycle(&emp_id, "q2".into(), None, Some(&cid), &tool, &ctx(), &store)
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
                    state: EmployeeState::Sleeping,
                    created_at: now_rfc3339(),
                })
                .unwrap();
        }
        let s = StubTool::new("steve-out");
        run_cycle("steve", "q".into(), None, None, &s, &ctx(), &store)
            .await
            .unwrap();
        let m = StubTool::new("mary-out");
        run_cycle("mary", "q".into(), None, None, &m, &ctx(), &store)
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
                    state: EmployeeState::Sleeping,
                    created_at: now_rfc3339(),
                })
                .unwrap();
        }
        // steve 在腦 v1 下跑
        let s1 = StubTool::new("v1");
        let r1 = run_cycle("steve", "q".into(), None, None, &s1, &ctx(), &store)
            .await
            .unwrap();
        let steve_notes_before = store.get_memory("steve").unwrap().unwrap().notes.len();

        // 腦「升級」：同一 brain_id，知識演化為 v2；mary 採用。
        let m2 = StubTool::new("v2");
        let r2 = run_cycle("mary", "q".into(), None, None, &m2, &ctx(), &store)
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
}
