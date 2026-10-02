//! C8 Query Planner 的測試——任務聚焦（只縮不擴）＋receipt 反映。

use crate::domain::models::{Project, ProjectStatus, Task, TaskStatus};
use crate::domain::{SqliteStore, Store as _};
use crate::knowledge::bootstrap::bootstrap_with_sources;
use crate::knowledge::identity::operator_access_context;
use crate::knowledge::service::KnowledgeService;
use crate::knowledge::types::{AccessContext, KnowledgeScope, Visibility};
use crate::runtime::AGENT_WS;

fn store_with_task(project_id: Option<&str>) -> SqliteStore {
    let dir = std::env::temp_dir().join(format!(
        "m2c8-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let s = SqliteStore::open(&dir.join("test.db")).unwrap();
    bootstrap_with_sources(&s, &["src-common".into()]).unwrap(); // co-common←src-common
    if let Some(pid) = project_id {
        s.put_scope(&KnowledgeScope {
            id: "proj-x".into(),
            visibility: Visibility::Project,
            classification: "internal".into(),
            source_ids: vec!["src-x".into()],
            owner: None,
            department: None,
            project: Some(pid.into()),
        })
        .unwrap();
        s.put_scope(&KnowledgeScope {
            id: "dept-quality".into(),
            visibility: Visibility::Department,
            classification: "internal".into(),
            source_ids: vec!["src-q".into()],
            owner: None,
            department: Some("quality".into()),
            project: None,
        })
        .unwrap();
        s.put_project(&Project {
            id: pid.into(),
            workspace_id: AGENT_WS.into(),
            name: "X".into(),
            status: ProjectStatus::Active,
            created_at: "2026-10-02T00:00:00Z".into(),
        })
        .unwrap();
    }
    s.put_task(&Task {
        id: "t1".into(),
        workspace_id: AGENT_WS.into(),
        owner_employee_id: "e1".into(),
        objective: "o".into(),
        input: "i".into(),
        status: TaskStatus::InProgress,
        output_artifact_id: None,
        commitment_id: None,
        project_id: project_id.map(|p| p.to_string()),
        external_reply_to: None,
        external_source: None,
        created_at: "2026-10-02T00:00:00Z".into(),
    })
    .unwrap();
    s
}

fn operator() -> AccessContext {
    operator_access_context(AGENT_WS)
}

fn operator_with_task(task_id: &str) -> AccessContext {
    AccessContext { task_id: Some(task_id.into()), ..operator() }
}

/// **Test 2 任務範圍調整**：綁 proj-x 任務的檢索聚焦到（proj-x ∪ co-common），
/// dept-quality 縮出候選集；無 task 時全授權集。
#[test]
fn c8_task_focus_narrows_candidates() {
    let s = store_with_task(Some("proj-x"));
    let svc = KnowledgeService::new("unused.db");

    // 無 task：plan = 全授權集（三個 scope）。
    let plan = svc.plan(&s, &operator()).unwrap();
    assert_eq!(plan.scope_ids.len(), 3);
    assert_eq!(plan.focus_project, None);

    // 綁 proj-x 任務：聚焦（proj-x ∪ co-common）、dept-quality 縮出。
    let focused = svc
        .apply_focus(&s, &operator_with_task("t1"), svc.plan(&s, &operator()).unwrap())
        .unwrap();
    assert_eq!(focused.focus_project.as_deref(), Some("proj-x"));
    assert_eq!(focused.scope_ids, vec!["co-common".to_string(), "proj-x".to_string()]);
    assert_eq!(
        focused.source_ids,
        vec!["src-common".to_string(), "src-x".to_string()]
    );
    assert!(
        !focused.source_ids.contains(&"src-q".to_string()),
        "聚焦後 dept-quality 的 source 不在候選集"
    );
}

/// **不變式（D-C8a）**：聚焦永不擴權——未授權 proj-x 者，綁 proj-x 任務也不會看到它。
#[test]
fn c8_focus_never_widens() {
    let s = store_with_task(Some("proj-x"));
    let svc = KnowledgeService::new("unused.db");

    // carol 僅授權 dept-quality（operator-only 規則改成逐 principal 白名單）。
    crate::knowledge::bootstrap::save_policy_new_version(
        &s,
        vec![crate::knowledge::types::PolicyRule {
            id: "carol-quality".into(),
            priority: 10,
            effect: crate::knowledge::types::Effect::Allow,
            principals: Some(vec!["ai:carol".into()]),
            principal_types: None,
            scopes: Some(vec!["dept-quality".into()]),
            departments: None,
            projects: None,
            classifications: None,
            department_membership: false,
            project_membership: false,
        }],
    )
    .unwrap();
    let carol = AccessContext {
        principal_id: "ai:carol".into(),
        principal_type: crate::knowledge::types::PrincipalType::AiEmployee,
        employee_id: Some("carol".into()),
        workspace_id: AGENT_WS.into(),
        roles: vec![],
        departments: vec![],
        projects: vec![],
        task_id: Some("t1".into()), // 綁 proj-x 任務
        purpose: None,
    };
    let plan = svc.plan(&s, &carol).unwrap();
    assert_eq!(plan.scope_ids, vec!["dept-quality".to_string()]);
    let focused = svc.apply_focus(&s, &carol, plan).unwrap();
    // 聚焦集＝授權 ∩ (proj-x ∪ Company)＝空 → no-op（不縮到不可用）；proj-x 依舊被 policy 擋。
    assert_eq!(focused.focus_project, None);
    assert_eq!(focused.scope_ids, vec!["dept-quality".to_string()]);
    assert_eq!(focused.source_ids, vec!["src-q".to_string()]);
}

/// no-op 三態回歸：task 無 project → 聚焦不生效（plan 原樣）。
#[test]
fn c8_noop_when_task_has_no_project() {
    let s = store_with_task(None);
    let svc = KnowledgeService::new("unused.db");
    let plan = svc.plan(&s, &operator()).unwrap();
    let after = svc.apply_focus(&s, &operator_with_task("t1"), plan).unwrap();
    assert_eq!(after.focus_project, None);
    assert_eq!(after.scope_ids, vec!["co-common".to_string()]);
}
