//! Operoid Runtime 指令層（P1b 殼）——29 個 `#[tauri::command]` ＋ AppHandle 相關helper。
//! 核心邏輯已搬入 `ocore::runtime`（見該檔文檔）；此處 re-export 保持
//! `crate::runtime::*` 路徑零改動（scheduler／factories 等呼叫端不變）。

pub use ocore::runtime::*;

use crate::agent_state::{AppState, WakeSignal};
use crate::config::app_config;
use crate::config::DEFAULT_BRAIN_ID;
use crate::config::gbrain_config;
use crate::domain::*; // 指令層大量使用 domain 型別/Store 方法（原 runtime 模組內 use 隨核心搬入 ocore）
use crate::i18n::AppError;


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
    let store = SqliteStore::open(agent_db_path(&app)?)?;

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
    let store = SqliteStore::open(agent_db_path(&app)?)?;

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
    let store = SqliteStore::open(agent_db_path(&app)?)?;

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
    let store = SqliteStore::open(agent_db_path(&app)?)?;
    let employee_id = deploy_instance(&store, &template_id, &instance_name)?;
    Ok(DeployResult { employee_id })
}

/// Agent-OS DB 路徑：**Local AppData**（避免 Roaming 被 OneDrive／網域同步導致 WAL 損壞——
/// WAL 的 `-wal`／`-shm` 必須是共置本地檔案）。
/// Agent-OS DB 路徑：**Local AppData**（避免 Roaming 被 OneDrive／網域同步導致 WAL 損壞——
/// WAL 的 `-wal`／`-shm` 必須是共置本地檔案）。衍生邏輯在 `ocore::runtime::agent_db_path_in`
/// （P1d 上移——殼層只負責解析 Tauri 的 Local AppData 目錄）。
pub(crate) fn agent_db_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<std::path::PathBuf, AppError> {
    use tauri::Manager;
    let local_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(agent_db_path_in(&local_dir))
}

/// 共用：Agent-OS flag 檢查 ＋ 開 `operoid.db`（Local AppData）。
pub(crate) fn agent_store<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<SqliteStore, AppError> {
    let cfg = app_config::load(app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    Ok(SqliteStore::open(agent_db_path(app)?)?)
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
/// 為單一員工執行「承諾驅動 session」：acquire busy-lock → 建 tool/ctx/reasoner → 先清 Inbox，
/// 再對每個 Active commitment 跑 [`run_autonomous`]。供 `agent_create_commitment`（交辦後立即喚醒）

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
    let store = SqliteStore::open(agent_db_path(&app)?)?;

    let emp = store
        .get_employee(&employee_id)?
        .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", &employee_id))?;
    let ws = emp.workspace_id.clone();
    let wake_id = employee_id.clone();
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
    // 交辦後立即喚醒該員工跑承諾（背景、非阻塞；busy-lock 把關）。
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = run_commitments_for_employee(&app2, &wake_id).await;
    });
    Ok(CommitmentResult { commitment_id })
}

/// 手動標記一個 Commitment 已滿足（Satisfied）。
/// 自動判斷已由 `run_autonomous` 的 `evaluate_done`（Reasoner 評估 completion_condition）實作；
/// 本指令為人工覆寫——使用者可直接標記完成，不必等自主循環。
#[tauri::command]
pub async fn agent_satisfy_commitment<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    commitment_id: String,
) -> Result<(), AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let store = SqliteStore::open(agent_db_path(&app)?)?;

    let mut com = store
        .get_commitment(&commitment_id)?
        .ok_or_else(|| AppError::new("agent_os.commitmentNotFound").p("id", &commitment_id))?;
    com.status = CommitmentStatus::Satisfied;
    com.updated_at = now_rfc3339();
    store.put_commitment(&com)?;
    Ok(())
}

// ───────────────── 承諾審核（Phase 7c，Ch.11/Ch.20 §5）─────────────────
// 提案的建立由 run_inbox 內聯 → create_proposed_commitment（去重 + record_event）。
/// 人類核可：Proposed → Active ＋ 喚醒該員工跑 run_commitments_for_employee。
/// 狀態守衛：僅 Proposed 可被核可（缺陷 3）。
#[tauri::command]
pub async fn agent_approve_commitment<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    commitment_id: String,
) -> Result<(), AppError> {
    let store = agent_store(&app)?;
    let mut com = store
        .get_commitment(&commitment_id)?
        .ok_or_else(|| AppError::new("agent_os.commitmentNotFound").p("id", &commitment_id))?;
    if com.status != CommitmentStatus::Proposed {
        return Err(AppError::new("agent_os.invalidTransition")
            .p("id", &commitment_id)
            .p("from", format!("{:?}", com.status).to_lowercase())
            .p("to", "active"));
    }
    com.status = CommitmentStatus::Active;
    com.updated_at = now_rfc3339();
    let emp_id = com.owner_employee_id.clone();
    store.put_commitment(&com)?;
    // 喚醒該員工跑承諾（同 7a 交辦後喚醒）。
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = run_commitments_for_employee(&app2, &emp_id).await;
    });
    Ok(())
}

/// 人類拒絕：Proposed → Rejected。
/// 狀態守衛：僅 Proposed 可被拒絕（缺陷 3）。
#[tauri::command]
pub async fn agent_reject_commitment<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    commitment_id: String,
) -> Result<(), AppError> {
    let store = agent_store(&app)?;
    let mut com = store
        .get_commitment(&commitment_id)?
        .ok_or_else(|| AppError::new("agent_os.commitmentNotFound").p("id", &commitment_id))?;
    if com.status != CommitmentStatus::Proposed {
        return Err(AppError::new("agent_os.invalidTransition")
            .p("id", &commitment_id)
            .p("from", format!("{:?}", com.status).to_lowercase())
            .p("to", "rejected"));
    }
    com.status = CommitmentStatus::Rejected;
    com.updated_at = now_rfc3339();
    store.put_commitment(&com)?;
    Ok(())
}

/// 人類封存：任意狀態 → Archived（軟刪除；資料保留可稽核）。
/// 已 Archived 者拒絕重複封存。封存不喚醒員工（退出，非啟動）。
#[tauri::command]
pub async fn agent_archive_commitment<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    commitment_id: String,
) -> Result<(), AppError> {
    let store = agent_store(&app)?;
    let mut com = store
        .get_commitment(&commitment_id)?
        .ok_or_else(|| AppError::new("agent_os.commitmentNotFound").p("id", &commitment_id))?;
    if com.status == CommitmentStatus::Archived {
        return Err(AppError::new("agent_os.invalidTransition")
            .p("id", &commitment_id)
            .p("from", "archived")
            .p("to", "archived"));
    }
    com.status = CommitmentStatus::Archived;
    com.updated_at = now_rfc3339();
    store.put_commitment(&com)?;
    Ok(())
}

/// 人類取消：活躍 task（Created/Assigned/InProgress）→ Cancelled（軟刪除）。
/// 已 Completed/Failed/Cancelled 者拒絕。取消不喚醒員工（退出，非啟動）。
#[tauri::command]
pub async fn agent_cancel_task<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    task_id: String,
) -> Result<(), AppError> {
    let store = agent_store(&app)?;
    let mut tk = store
        .get_task(&task_id)?
        .ok_or_else(|| AppError::new("agent_os.taskNotFound").p("id", &task_id))?;
    if !matches!(tk.status, TaskStatus::Created | TaskStatus::Assigned | TaskStatus::InProgress) {
        return Err(AppError::new("agent_os.invalidTransition")
            .p("id", &task_id)
            .p("from", format!("{:?}", tk.status).to_lowercase())
            .p("to", "cancelled"));
    }
    tk.status = TaskStatus::Cancelled;
    store.put_task(&tk)?;
    Ok(())
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
    let store = SqliteStore::open(agent_db_path(&app)?)?;
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
    let store = SqliteStore::open(agent_db_path(&app)?)?;
    list_state_payload(&store, &workspace_id)
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
    let store = SqliteStore::open(agent_db_path(&app)?)?;
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
    let store = SqliteStore::open(agent_db_path(&app)?)?;

    // 各 assignment 解析其 employee 的腦 → ToolCtx（團隊成員可能用不同腦）。
    let mut ctxs: Vec<ToolCtx> = Vec::with_capacity(assignments.len());
    for a in &assignments {
        let emp = store
            .get_employee(&a.employee_id)?
            .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", &a.employee_id))?;
        let entry = app_config::brain_entry(&cfg, &emp.brain.brain_id)?;
        let chat_model = gbrain_config::load_for(entry.env_home())
            .ok()
            .and_then(|l| l.config.chat_model);
        ctxs.push(ToolCtx {
            gbrain_exe: cfg.gbrain_exe_path.clone(),
            gbrain_home: entry.env_home().map(|s| s.to_string()),
            chat_model,
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
    let store = SqliteStore::open(agent_db_path(&app)?)?;
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
        external_reply_to: None,
        external_source: None,
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
    let ws = emp.workspace_id.clone();
    let existing: Vec<String> = store
        .list_tasks(&ws)?
        .into_iter()
        .map(|t| t.id)
        .collect();
    let task_id = next_id("msg", &existing);
    let now = now_rfc3339();
    // Message{In}：對話紀錄（Ch.16）；與下面的 Task 是同一趟往返的兩面。
    store.put_message(&Message {
        id: fresh_id("msg-in"),
        workspace_id: ws.clone(),
        employee_id: employee_id.clone(),
        direction: MessageDirection::In,
        text: text.clone(),
        source_commitment_id: commitment_id.clone(),
        proposed_commitment_id: None,
        artifact_id: None,
        created_at: now.clone(),
    })?;
    // Task：給 scan_inbox／run_inbox 消化的工作項（喚醒員工）。
    store.put_task(&Task {
        id: task_id.clone(),
        workspace_id: ws,
        owner_employee_id: employee_id.clone(),
        objective: "Human message".into(),
        input: text,
        status: TaskStatus::Assigned,
        output_artifact_id: None,
        commitment_id,
        project_id: None,
        external_reply_to: None,
        external_source: None,
        created_at: now,
    })?;
    // 推喚醒信號（best-effort；即便 channel 滿，下次 30s tick 也會掃到這個 Assigned task）。
    state.wake(WakeSignal {
        employee_id,
        reason: "message".into(),
    });
    Ok(SendMessageResult { task_id })
}

/// 清除某員工的全部對話訊息（Message）。
///
/// 僅清互動層（對話往返）；工作產出（Artifact／Commitment）與事件（Event）不受影響。
/// 用於對話頁的「清除對話內容」。
#[tauri::command]
pub async fn agent_clear_messages<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    employee_id: String,
) -> Result<(), AppError> {
    let store = agent_store(&app)?;
    store
        .get_employee(&employee_id)?
        .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", &employee_id))?;
    store.clear_messages_by_employee(&employee_id)?;
    Ok(())
}

// ───────────────── 監看（Phase 6d）─────────────────
/// 監看：取回某員工的即時觀察快照——給監看 modal 每 ~1.5s 輪詢。
/// 含 state、Active commitments、待辦 tasks、近期 artifacts、memory、近期生命週期 events。
#[tauri::command]
pub async fn agent_watch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    employee_id: String,
) -> Result<serde_json::Value, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let store = SqliteStore::open(agent_db_path(&app)?)?;
    watch_payload(&cfg, &store, &employee_id)
}

#[tauri::command]
pub async fn agent_inbox_summary<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<InboxSummary, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let store = SqliteStore::open(agent_db_path(&app)?)?;
    inbox_summary_payload(&store)
}

#[tauri::command]
pub async fn agent_recent_events<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    limit: Option<usize>,
) -> Result<Vec<EventWithMeta>, AppError> {
    let cfg = app_config::load(&app)?;
    if !cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    let store = SqliteStore::open(agent_db_path(&app)?)?;
    recent_events_payload(&store, limit.unwrap_or(50))
}

/// 殼層包裝（P1b）：載入 cfg/state/db_path 後委派 ocore 版。供 `agent_create_commitment`
/// （交辦後立即喚醒）與啟動掃描使用。
pub async fn run_commitments_for_employee<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    employee_id: &str,
) -> anyhow::Result<()> {
    use tauri::Manager;
    let state = app.state::<AppState>();
    let cfg = app_config::load(app)?;
    ocore::runtime::run_commitments_for_employee(&state, &cfg, &agent_db_path(app)?, employee_id).await
}
