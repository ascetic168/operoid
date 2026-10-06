//! Query Planner（M2-C8）——task/project 感知的**範圍聚焦**（提示詞 §15）。
//!
//! 定位（D-C8a）：policy 是授權**下限**（I1/I2）；planner 依任務脈絡把候選集**收窄**
//! 到任務相關的知識（專案 scope＋公司常識），**永不擴權**——不變式
//! `聚焦集 ⊆ 授權集`（有測試固化）。
//!
//! §15 步驟分類（D-C8c）：Identity/Employee/Task/Project＝確定性 metadata lookup
//! （本模組＋C4）；Intent/Entities＝LLM-assisted、**留在 GBrain 內部**（think intent／
//! query expansion——Operoid 端 0 個額外 LLM 呼叫，最小化）；Policy/Allowed Scope＝
//! 確定性純函式（C5/C7）；Query/Retrieval/Reranking＝SQL＋GBrain 內部（M0-V1）。

use anyhow::Result;

use super::types::{KnowledgeScope, Visibility};
use crate::domain::store::Store;

/// 一次聚焦的結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Focus {
    /// 聚焦所依的專案。
    pub project_id: String,
    /// 聚焦後的授權 scope 集合（⊆ 傳入的授權集合）。
    pub scope_ids: Vec<String>,
}

/// 任務聚焦：`task_id` → task.project_id → project 存在 →
/// 聚焦集＝授權 scopes 中（`scope.project == 專案` ∪ `visibility == Company`）。
///
/// 回傳 `None`＝不聚焦（no-op）：task 缺、task 無 project、project 不存在、
/// 聚焦集為空、或聚焦集與原集合相同。
pub fn task_focus(
    store: &dyn Store,
    task_id: &str,
    authorized_scope_ids: &[String],
) -> Result<Option<Focus>> {
    let Some(task) = store.get_task(task_id)? else {
        return Ok(None);
    };
    let Some(project_id) = task.project_id else {
        return Ok(None);
    };
    if store.get_project(&project_id)?.is_none() {
        return Ok(None);
    }
    let scopes = store.list_scopes()?;
    let focused: Vec<String> = scopes
        .iter()
        .filter(|s| {
            authorized_scope_ids.contains(&s.id)
                && (s.project.as_deref() == Some(project_id.as_str())
                    || s.visibility == Visibility::Company)
        })
        .map(|s| s.id.clone())
        .collect();
    // 空集合（縮到不可用）或與原集合相同（無意義）→ no-op。
    if focused.is_empty() || focused.len() == authorized_scope_ids.len() {
        return Ok(None);
    }
    Ok(Some(Focus { project_id, scope_ids: focused }))
}

/// 聚焦後的 source 集合重算（聚焦 scopes 的 source 聯集，排序去重）。
pub fn sources_of(scopes: &[KnowledgeScope], scope_ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = scopes
        .iter()
        .filter(|s| scope_ids.contains(&s.id))
        .flat_map(|s| s.source_ids.clone())
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::types::SecurityLevel;
    use crate::domain::models::{Project, ProjectStatus, Task, TaskStatus};
    use crate::domain::SqliteStore;

    fn store_with_task(project_id: Option<&str>) -> SqliteStore {
        let dir = std::env::temp_dir().join(format!(
            "m1c8-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let s = SqliteStore::open(&dir.join("test.db")).unwrap();
        if let Some(pid) = project_id {
            s.put_project(&Project {
                id: pid.into(),
                workspace_id: "ws".into(),
                name: "P".into(),
                status: ProjectStatus::Active,
                created_at: "2026-10-02T00:00:00Z".into(),
            })
            .unwrap();
        }
        s.put_task(&Task {
            id: "t1".into(),
            workspace_id: "ws".into(),
            owner_employee_id: "e1".into(),
            objective: "o".into(),
            input: "i".into(),
            status: TaskStatus::InProgress,
            output_artifact_id: None,
            commitment_id: None,
            project_id: project_id.map(|p| p.to_string()),
            external_reply_to: None,
            external_source: None,
            occurred_at: None,
            created_at: "2026-10-02T00:00:00Z".into(),
        })
        .unwrap();
        s
    }

    fn scope(id: &str, project: Option<&str>, visibility: Visibility) -> KnowledgeScope {
        KnowledgeScope {
            id: id.into(),
            visibility,
            classification: SecurityLevel::Internal,
            source_ids: vec![format!("src-{id}")],
            owner: None,
            department: None,
            project: project.map(|p| p.to_string()),
        }
    }

    #[test]
    fn focus_narrows_to_project_plus_company() {
        let s = store_with_task(Some("proj-x"));
        let scopes = vec![
            scope("co-common", None, Visibility::Company),
            scope("proj-x", Some("proj-x"), Visibility::Project),
            scope("dept-quality", None, Visibility::Department),
        ];
        for sc in &scopes { s.put_scope(sc).unwrap(); }
        let authorized = vec!["co-common".to_string(), "proj-x".to_string(), "dept-quality".to_string()];
        let f = task_focus(&s, "t1", &authorized).unwrap().expect("應聚焦");
        assert_eq!(f.project_id, "proj-x");
        // 專案 scope 留下、公司常識留下、其他部門縮出——聚焦 ⊆ 授權（不變式）。
        assert_eq!(f.scope_ids, vec!["co-common".to_string(), "proj-x".to_string()]);
        assert!(f.scope_ids.iter().all(|id| authorized.contains(id)));
    }

    #[test]
    fn focus_never_widens() {
        let s = store_with_task(Some("proj-x"));
        for sc in [
            scope("co-common", None, Visibility::Company),
            scope("proj-x", Some("proj-x"), Visibility::Project),
        ] {
            s.put_scope(&sc).unwrap();
        }
        // 授權集只有 co-common——聚焦不得把未授權的 proj-x 塞進來。
        let f = task_focus(&s, "t1", &["co-common".to_string()]).unwrap();
        // 聚焦集＝co-common（Company 留下）＝原集合 → no-op → None。
        assert!(f.is_none(), "聚焦 ⊆ 授權：未授權的 proj-x 不得因 task 出現");
    }

    #[test]
    fn focus_noop_cases() {
        // 無 task
        let s = store_with_task(Some("proj-x"));
        assert!(task_focus(&s, "missing", &["co-common".to_string()]).unwrap().is_none());
        // task 無 project
        let s2 = store_with_task(None);
        assert!(task_focus(&s2, "t1", &["co-common".to_string()]).unwrap().is_none());
        // project 不存在（store 有 task 指向缺列 project——直接用無 project store 造列再補）
        let s3 = store_with_task(None);
        s3.put_task(&Task {
            id: "t2".into(),
            workspace_id: "ws".into(),
            owner_employee_id: "e".into(),
            objective: "o".into(),
            input: "i".into(),
            status: TaskStatus::InProgress,
            output_artifact_id: None,
            commitment_id: None,
            project_id: Some("ghost".into()),
            external_reply_to: None,
            external_source: None,
            occurred_at: None,
            created_at: "2026-10-02T00:00:00Z".into(),
        })
        .unwrap();
        assert!(task_focus(&s3, "t2", &["co-common".to_string()]).unwrap().is_none());
    }

    #[test]
    fn sources_of_dedup_sorted() {
        let scopes = vec![
            scope("a", None, Visibility::Company),
            scope("b", None, Visibility::Company),
        ];
        let mut b = scope("b", None, Visibility::Company);
        b.source_ids = vec!["src-a".into(), "src-c".into()];
        let scopes = vec![scopes[0].clone(), b];
        assert_eq!(
            sources_of(&scopes, &["a".into(), "b".into()]),
            vec!["src-a".to_string(), "src-c".to_string()]
        );
    }
}
