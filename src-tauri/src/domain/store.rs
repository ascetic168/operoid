//! 持久化抽象與檔案式 JSON 實作（Phase 0 地基）。
//!
//! [`Store`] 是純 Rust trait（不依賴 Tauri），故可單測，且日後可換成 SQLite
//! （決策 D2）。[`JsonStore`] 為第一份實作：每個 collection 一個 JSON 檔，置於
//! `<base>/domain/` 下，直接以 `std::fs` 讀寫（與 `config::gbrain_config::save_raw`
//! 同手法）。Phase 1 才以 `app.path().app_data_dir()` 當 base 並接出 Tauri 指令。

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::models::{
    Artifact, Commitment, CommitmentStatus, Employee, EmployeeTemplate, Event, Memory, Message,
    Project, Task, TaskStatus, Workspace,
};

/// Domain 持久化介面。Phase 0 提供 collection 層級的讀寫；upsert 以 id 為準。
pub trait Store {
    fn list_workspaces(&self) -> Result<Vec<Workspace>>;
    fn get_workspace(&self, id: &str) -> Result<Option<Workspace>>;
    fn put_workspace(&self, ws: &Workspace) -> Result<()>;

    fn list_projects(&self, workspace_id: &str) -> Result<Vec<Project>>;
    fn get_project(&self, id: &str) -> Result<Option<Project>>;
    fn put_project(&self, p: &Project) -> Result<()>;

    fn list_employees(&self, workspace_id: &str) -> Result<Vec<Employee>>;
    fn get_employee(&self, id: &str) -> Result<Option<Employee>>;
    fn put_employee(&self, emp: &Employee) -> Result<()>;
    fn delete_employee(&self, id: &str) -> Result<()>;

    fn list_templates(&self, workspace_id: &str) -> Result<Vec<EmployeeTemplate>>;
    fn get_template(&self, id: &str) -> Result<Option<EmployeeTemplate>>;
    fn put_template(&self, tmpl: &EmployeeTemplate) -> Result<()>;
    fn delete_template(&self, id: &str) -> Result<()>;

    fn list_tasks(&self, workspace_id: &str) -> Result<Vec<Task>>;
    fn get_task(&self, id: &str) -> Result<Option<Task>>;
    fn put_task(&self, task: &Task) -> Result<()>;

    fn list_artifacts(&self, workspace_id: &str) -> Result<Vec<Artifact>>;
    fn get_artifact(&self, id: &str) -> Result<Option<Artifact>>;
    fn put_artifact(&self, art: &Artifact) -> Result<()>;

    fn list_commitments(&self, workspace_id: &str) -> Result<Vec<Commitment>>;
    fn get_commitment(&self, id: &str) -> Result<Option<Commitment>>;
    fn put_commitment(&self, c: &Commitment) -> Result<()>;

    fn get_memory(&self, employee_id: &str) -> Result<Option<Memory>>;
    fn put_memory(&self, memory: &Memory) -> Result<()>;

    // ── Phase 6：以 owner／producer 為維度的查詢（排程器與自主循環用）──

    /// 列出**所有** workspace 的員工（排程器啟動掃描用）。
    fn list_all_employees(&self) -> Result<Vec<Employee>>;

    /// 列出**所有** workspace 的 commitments（動詞軌跨員工聚合用）。
    fn list_all_commitments(&self) -> Result<Vec<Commitment>> {
        // 預設實作：遍歷 workspaces 再串接。SqliteStore 覆寫為無 where 的 select_all。
        let mut out = Vec::new();
        for ws in self.list_workspaces()? {
            out.extend(self.list_commitments(&ws.id)?);
        }
        Ok(out)
    }

    /// 列出某員工具特定狀態的 tasks（status 存於 data blob，故這裡以 owner 為索引取出後在記憶體過濾）。
    fn list_tasks_by_owner(
        &self,
        owner_employee_id: &str,
        statuses: &[TaskStatus],
    ) -> Result<Vec<Task>>;

    /// 列出某員工的「待辦」tasks——Inbox 裡尚未完成者（Created／Assigned／InProgress）。
    fn list_assigned_tasks_by_owner(&self, owner_employee_id: &str) -> Result<Vec<Task>> {
        self.list_tasks_by_owner(
            owner_employee_id,
            &[TaskStatus::Created, TaskStatus::Assigned, TaskStatus::InProgress],
        )
    }

    /// 列出**共用某腦**的全部員工（Event 匯流排腦→員工路由用；關係 1:N——多員工可共用一腦）。
    /// `brain_id` 在 SQLite 藏在 JSON blob，故 in-memory 過濾（與既有 `list_tasks_by_owner` 過濾
    /// status 的模式一致）。員工數量小，暫不需加 DB index。
    fn list_employees_by_brain(&self, brain_id: &str) -> Result<Vec<Employee>> {
        Ok(self.list_all_employees()?
            .into_iter()
            .filter(|e| e.brain.brain_id == brain_id)
            .collect())
    }

    /// 列出某員工名下 `Active` 的 commitments（承諾驅動喚醒用）。
    fn list_active_commitments_by_owner(&self, owner_employee_id: &str) -> Result<Vec<Commitment>>;

    /// 列出某員工產出的所有 artifacts（自主循環的進度／評估上下文用）。
    fn list_artifacts_by_producer(&self, produced_by: &str) -> Result<Vec<Artifact>>;

    // ── Phase 6d：生命週期事件（append-only）──

    /// 記錄一則不可變 Event（Ch.14）。
    fn put_event(&self, event: &Event) -> Result<()>;
    /// 列出某員工近期事件（最新在前，最多 `limit` 則）。
    fn list_events_by_employee(&self, employee_id: &str, limit: usize) -> Result<Vec<Event>>;
    /// 列出跨所有員工的近期事件（最新在前，最多 `limit` 則；動詞軌活動流用）。
    fn list_recent_events(&self, limit: usize) -> Result<Vec<Event>>;

    // ── Phase 7b：對話訊息（Message，Ch.16）──

    /// 寫一則對話訊息（人類 In 或員工 Out）。
    fn put_message(&self, message: &Message) -> Result<()>;
    /// 列出某員工的對話訊息（最新在前，最多 `limit` 則）。
    fn list_messages_by_employee(&self, employee_id: &str, limit: usize) -> Result<Vec<Message>>;
    /// 清除某員工的全部對話訊息（不影響 artifact／commitment 等工作產出）。
    fn clear_messages_by_employee(&self, employee_id: &str) -> Result<()>;
}

/// 檔案式 JSON store。所有實體存於 `<base>/domain/{workspaces,employees,
/// artifacts,commitments}.json`，每檔一個 collection。
pub struct JsonStore {
    base: PathBuf,
}

impl JsonStore {
    /// `base` 為資料根目錄（如 app data dir）；實體落在其下的 `domain/`。
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            base: base.as_ref().to_path_buf(),
        }
    }

    fn dir(&self) -> PathBuf {
        self.base.join("domain")
    }
    fn path(&self, file: &str) -> PathBuf {
        self.dir().join(file)
    }

    fn read<T: DeserializeOwned>(&self, file: &str) -> Result<Vec<T>> {
        read_vec(&self.path(file))
    }
}

impl Store for JsonStore {
    fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        self.read("workspaces.json")
    }

    fn get_workspace(&self, id: &str) -> Result<Option<Workspace>> {
        Ok(self.list_workspaces()?.into_iter().find(|w| w.id == id))
    }

    fn put_workspace(&self, ws: &Workspace) -> Result<()> {
        upsert_by_id(&self.path("workspaces.json"), ws, |w| &w.id)
    }

    fn list_projects(&self, workspace_id: &str) -> Result<Vec<Project>> {
        Ok(self
            .read::<Project>("projects.json")?
            .into_iter()
            .filter(|p| p.workspace_id == workspace_id)
            .collect())
    }

    fn get_project(&self, id: &str) -> Result<Option<Project>> {
        Ok(self
            .read::<Project>("projects.json")?
            .into_iter()
            .find(|p| p.id == id))
    }

    fn put_project(&self, p: &Project) -> Result<()> {
        upsert_by_id(&self.path("projects.json"), p, |p| &p.id)
    }

    fn list_employees(&self, workspace_id: &str) -> Result<Vec<Employee>> {
        Ok(self
            .read::<Employee>("employees.json")?
            .into_iter()
            .filter(|e| e.workspace_id == workspace_id)
            .collect())
    }

    fn get_employee(&self, id: &str) -> Result<Option<Employee>> {
        Ok(self
            .read::<Employee>("employees.json")?
            .into_iter()
            .find(|e| e.id == id))
    }

    fn put_employee(&self, emp: &Employee) -> Result<()> {
        upsert_by_id(&self.path("employees.json"), emp, |e| &e.id)
    }

    fn delete_employee(&self, id: &str) -> Result<()> {
        delete_by_id::<Employee, _>(&self.path("employees.json"), id, |e| &e.id)
    }

    fn list_templates(&self, workspace_id: &str) -> Result<Vec<EmployeeTemplate>> {
        Ok(self
            .read::<EmployeeTemplate>("templates.json")?
            .into_iter()
            .filter(|t| t.workspace_id == workspace_id)
            .collect())
    }

    fn get_template(&self, id: &str) -> Result<Option<EmployeeTemplate>> {
        Ok(self
            .read::<EmployeeTemplate>("templates.json")?
            .into_iter()
            .find(|t| t.id == id))
    }

    fn put_template(&self, tmpl: &EmployeeTemplate) -> Result<()> {
        upsert_by_id(&self.path("templates.json"), tmpl, |t| &t.id)
    }

    fn delete_template(&self, id: &str) -> Result<()> {
        delete_by_id::<EmployeeTemplate, _>(&self.path("templates.json"), id, |t| &t.id)
    }

    fn list_tasks(&self, workspace_id: &str) -> Result<Vec<Task>> {
        Ok(self
            .read::<Task>("tasks.json")?
            .into_iter()
            .filter(|t| t.workspace_id == workspace_id)
            .collect())
    }

    fn put_task(&self, task: &Task) -> Result<()> {
        upsert_by_id(&self.path("tasks.json"), task, |t| &t.id)
    }

    fn get_task(&self, id: &str) -> Result<Option<Task>> {
        Ok(self
            .read::<Task>("tasks.json")?
            .into_iter()
            .find(|t| t.id == id))
    }

    fn list_artifacts(&self, workspace_id: &str) -> Result<Vec<Artifact>> {
        Ok(self
            .read::<Artifact>("artifacts.json")?
            .into_iter()
            .filter(|a| a.workspace_id == workspace_id)
            .collect())
    }

    fn get_artifact(&self, id: &str) -> Result<Option<Artifact>> {
        Ok(self
            .read::<Artifact>("artifacts.json")?
            .into_iter()
            .find(|a| a.id == id))
    }

    fn put_artifact(&self, art: &Artifact) -> Result<()> {
        upsert_by_id(&self.path("artifacts.json"), art, |a| &a.id)
    }

    fn list_commitments(&self, workspace_id: &str) -> Result<Vec<Commitment>> {
        Ok(self
            .read::<Commitment>("commitments.json")?
            .into_iter()
            .filter(|c| c.workspace_id == workspace_id)
            .collect())
    }

    fn get_commitment(&self, id: &str) -> Result<Option<Commitment>> {
        Ok(self
            .read::<Commitment>("commitments.json")?
            .into_iter()
            .find(|c| c.id == id))
    }

    fn put_commitment(&self, c: &Commitment) -> Result<()> {
        upsert_by_id(&self.path("commitments.json"), c, |c| &c.id)
    }

    fn get_memory(&self, employee_id: &str) -> Result<Option<Memory>> {
        Ok(self
            .read::<Memory>("memories.json")?
            .into_iter()
            .find(|m| m.employee_id == employee_id))
    }

    fn put_memory(&self, memory: &Memory) -> Result<()> {
        // Memory 以 employee_id 為鍵（每 Employee 一份）。
        upsert_by_id(&self.path("memories.json"), memory, |m| &m.employee_id)
    }

    // ── Phase 6：owner／producer 維度查詢（JsonStore 全集合讀後 in-memory 過濾）──

    fn list_all_employees(&self) -> Result<Vec<Employee>> {
        self.read("employees.json")
    }

    fn list_tasks_by_owner(
        &self,
        owner_employee_id: &str,
        statuses: &[TaskStatus],
    ) -> Result<Vec<Task>> {
        Ok(self
            .read::<Task>("tasks.json")?
            .into_iter()
            .filter(|t| {
                t.owner_employee_id == owner_employee_id && statuses.contains(&t.status)
            })
            .collect())
    }

    fn list_active_commitments_by_owner(&self, owner_employee_id: &str) -> Result<Vec<Commitment>> {
        Ok(self
            .read::<Commitment>("commitments.json")?
            .into_iter()
            .filter(|c| {
                c.owner_employee_id == owner_employee_id && c.status == CommitmentStatus::Active
            })
            .collect())
    }

    fn list_artifacts_by_producer(&self, produced_by: &str) -> Result<Vec<Artifact>> {
        Ok(self
            .read::<Artifact>("artifacts.json")?
            .into_iter()
            .filter(|a| a.produced_by == produced_by)
            .collect())
    }

    // ── Phase 6d：生命週期事件（JsonStore：全集合讀後 in-memory 過濾）──

    fn put_event(&self, event: &Event) -> Result<()> {
        upsert_by_id(&self.path("events.json"), event, |e| &e.id)
    }
    fn list_events_by_employee(&self, employee_id: &str, limit: usize) -> Result<Vec<Event>> {
        let mut events: Vec<Event> = self
            .read::<Event>("events.json")?
            .into_iter()
            .filter(|e| e.employee_id == employee_id)
            .collect();
        events.reverse(); // Vec 末尾為最新 → 反轉成最新在前
        events.truncate(limit);
        Ok(events)
    }
    fn list_recent_events(&self, limit: usize) -> Result<Vec<Event>> {
        let mut events: Vec<Event> = self.read::<Event>("events.json")?;
        events.reverse(); // Vec 末尾為最新 → 反轉成最新在前
        events.truncate(limit);
        Ok(events)
    }

    // ── Phase 7b：對話訊息（JsonStore：全集合讀後 in-memory 過濾）──

    fn put_message(&self, message: &Message) -> Result<()> {
        upsert_by_id(&self.path("messages.json"), message, |m| &m.id)
    }
    fn list_messages_by_employee(&self, employee_id: &str, limit: usize) -> Result<Vec<Message>> {
        let mut msgs: Vec<Message> = self
            .read::<Message>("messages.json")?
            .into_iter()
            .filter(|m| m.employee_id == employee_id)
            .collect();
        msgs.reverse();
        msgs.truncate(limit);
        Ok(msgs)
    }
    fn clear_messages_by_employee(&self, employee_id: &str) -> Result<()> {
        let path = self.path("messages.json");
        let remaining: Vec<Message> = read_vec::<Message>(&path)?
            .into_iter()
            .filter(|m| m.employee_id != employee_id)
            .collect();
        write_vec(&path, &remaining)
    }
}

// ───────────────── IO helpers ─────────────────

/// 讀整個 collection；檔不存在視為空。
fn read_vec<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

/// 寫整個 collection（pretty JSON）；保證父目錄存在。
fn write_vec<T: Serialize>(path: &Path, items: &[T]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(items)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// 以 id 為準 upsert：命中則取代，否則附加。
fn upsert_by_id<T, F>(path: &Path, item: &T, id_of: F) -> Result<()>
where
    T: Serialize + DeserializeOwned + Clone,
    F: Fn(&T) -> &str,
{
    let mut items: Vec<T> = read_vec(path)?;
    let target = id_of(item);
    if let Some(slot) = items.iter_mut().find(|x| id_of(x) == target) {
        *slot = item.clone();
    } else {
        items.push(item.clone());
    }
    write_vec(path, &items)
}

/// 以 id 為準刪除一筆（不存在則 no-op）。
fn delete_by_id<T, F>(path: &Path, id: &str, id_of: F) -> Result<()>
where
    T: Serialize + DeserializeOwned,
    F: Fn(&T) -> &str,
{
    let mut items: Vec<T> = read_vec(path)?;
    let before = items.len();
    items.retain(|x| id_of(x) != id);
    if items.len() != before {
        write_vec(path, &items)?;
    }
    Ok(())
}

// ───────────────── ID 與時間戳 helpers ─────────────────

/// 在既有 id 集合中取唯一 id：首次用 base，衝突則 base-2、base-3 …
/// （仿 `brains::unique_id` 的 base-N 邏輯，但不依賴 `AppConfig`）。
pub fn next_id(base: &str, existing: &[String]) -> String {
    if !existing.iter().any(|id| id == base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let cand = format!("{base}-{n}");
        if !existing.iter().any(|id| id == &cand) {
            return cand;
        }
        n += 1;
    }
}

/// 由名稱 slug 衍生唯一 id（重用 [`crate::converters::slug::slugify`]；保留 CJK）。
pub fn id_from_name(name: &str, existing: &[String]) -> String {
    let base = crate::converters::slug::slugify(name, "entity");
    next_id(&base, existing)
}

/// 當下 RFC3339（UTC）時間戳字串。
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::{
        ArtifactStatus, BrainRef, CommitmentStatus, EmployeeState, Memory, TaskStatus,
        WorkspaceStatus,
    };
    use std::path::PathBuf;

    /// 測試用暫存目錄（免新增 dep）：以 process id ＋ 計數器保證唯一。
    fn test_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "operoid-domain-test-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn ws(id: &str) -> Workspace {
        Workspace {
            id: id.into(),
            name: id.into(),
            description: None,
            status: WorkspaceStatus::Created,
            created_at: "2026-07-30T00:00:00+00:00".into(),
        }
    }

    #[test]
    fn workspace_put_get_list_and_persist_across_reopen() {
        let dir = test_dir();
        {
            let s = JsonStore::new(&dir);
            s.put_workspace(&ws("acme")).unwrap();
            assert_eq!(s.list_workspaces().unwrap().len(), 1);
            assert!(s.get_workspace("acme").unwrap().is_some());
            assert!(s.get_workspace("nope").unwrap().is_none());
        }
        // 模擬「重啟」：用同一 base 重建 store，資料仍在。
        let s = JsonStore::new(&dir);
        let loaded = s.list_workspaces().unwrap();
        assert_eq!(loaded, vec![ws("acme")]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn workspace_upsert_replaces_existing() {
        let dir = test_dir();
        let s = JsonStore::new(&dir);
        let mut w = ws("acme");
        w.status = WorkspaceStatus::Created;
        s.put_workspace(&w).unwrap();
        w.status = WorkspaceStatus::Active;
        s.put_workspace(&w).unwrap(); // 同 id → 取代，不新增
        let loaded = s.list_workspaces().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].status, WorkspaceStatus::Active);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn employee_filtered_by_workspace_and_persists() {
        let dir = test_dir();
        let s = JsonStore::new(&dir);
        let steve = Employee {
            id: "steve".into(),
            workspace_id: "acme".into(),
            name: "Steve".into(),
            brain: BrainRef {
                brain_id: "__default__".into(),
            },
            role: None,
            template_id: None,
            state: EmployeeState::Created,
            created_at: "t".into(),
        };
        let mary = Employee {
            id: "mary".into(),
            workspace_id: "other".into(),
            name: "Mary".into(),
            brain: BrainRef {
                brain_id: "__default__".into(),
            },
            role: None,
            template_id: None,
            state: EmployeeState::Created,
            created_at: "t".into(),
        };
        s.put_employee(&steve).unwrap();
        s.put_employee(&mary).unwrap();

        // 只列出 acme 的員工
        let acme = s.list_employees("acme").unwrap();
        assert_eq!(acme, vec![steve]);

        // 重啟後仍在
        let s2 = JsonStore::new(&dir);
        assert_eq!(s2.list_employees("acme").unwrap().len(), 1);
        assert_eq!(s2.list_employees("other").unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_employees_by_brain_returns_all_sharing_brain() {
        let dir = test_dir();
        let s = JsonStore::new(&dir);
        // 兩名員工（跨 workspace）共用 "__default__" 腦；一名用 "other-brain"。
        for id in ["steve", "mary"] {
            s.put_employee(&Employee {
                id: id.into(),
                workspace_id: "acme".into(),
                name: id.into(),
                brain: BrainRef {
                    brain_id: "__default__".into(),
                },
                role: None,
                template_id: None,
                state: EmployeeState::Created,
                created_at: "t".into(),
            })
            .unwrap();
        }
        s.put_employee(&Employee {
            id: "alex".into(),
            workspace_id: "other".into(),
            name: "Alex".into(),
            brain: BrainRef {
                brain_id: "other-brain".into(),
            },
            role: None,
            template_id: None,
            state: EmployeeState::Created,
            created_at: "t".into(),
        })
        .unwrap();

        // 1:N 路由：list_employees_by_brain 回全部共用此腦的員工（跨 workspace）。
        let default_brain = s.list_employees_by_brain("__default__").unwrap();
        assert_eq!(default_brain.len(), 2);
        assert!(default_brain.iter().any(|e| e.id == "steve"));
        assert!(default_brain.iter().any(|e| e.id == "mary"));

        let other = s.list_employees_by_brain("other-brain").unwrap();
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].id, "alex");

        // 不存在的腦 → 空。
        assert!(s.list_employees_by_brain("nonexistent").unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn artifact_roundtrip_and_filter() {
        let dir = test_dir();
        let s = JsonStore::new(&dir);
        let art = Artifact {
            id: "a1".into(),
            workspace_id: "acme".into(),
            title: "Report".into(),
            artifact_type: "report".into(),
            content: "body".into(),
            produced_by: "steve".into(),
            source_task_id: None,
            source_commitment_id: None,
            revised_from_id: None,
            project_id: None,
            version: 1,
            status: ArtifactStatus::Draft,
            created_at: "t".into(),
        };
        s.put_artifact(&art).unwrap();
        assert_eq!(s.list_artifacts("acme").unwrap(), vec![art.clone()]);
        assert!(s.list_artifacts("other").unwrap().is_empty());

        // 改版後 upsert
        let mut art2 = art.clone();
        art2.version = 2;
        art2.status = ArtifactStatus::Committed;
        s.put_artifact(&art2).unwrap();
        assert_eq!(s.list_artifacts("acme").unwrap().len(), 1);
        assert_eq!(s.list_artifacts("acme").unwrap()[0].version, 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn commitment_roundtrip_and_filter() {
        let dir = test_dir();
        let s = JsonStore::new(&dir);
        let c = Commitment {
            id: "c1".into(),
            workspace_id: "acme".into(),
            owner_employee_id: "steve".into(),
            title: "Track PO".into(),
            completion_condition: "goods received".into(),
            status: CommitmentStatus::Active,
            created_at: "t".into(),
            updated_at: "t".into(),
        };
        s.put_commitment(&c).unwrap();
        assert_eq!(s.list_commitments("acme").unwrap(), vec![c]);
        assert!(s.list_commitments("other").unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn next_id_uniqueness() {
        let existing: Vec<String> = vec!["steve".into(), "steve-2".into()];
        assert_eq!(next_id("steve", &existing), "steve-3");
        assert_eq!(next_id("mary", &existing), "mary");
        assert_eq!(next_id("steve-2", &existing), "steve-2-2");
    }

    #[test]
    fn id_from_name_slugifies_and_disambiguates() {
        let existing: Vec<String> = vec![];
        assert_eq!(id_from_name("Procurement Steve", &existing), "procurement-steve");
        // CJK 保留
        assert_eq!(id_from_name("採購史蒂夫", &existing), "採購史蒂夫");
        // 衝突時附加後綴
        let taken = vec!["steve".to_string()];
        assert_eq!(id_from_name("Steve", &taken), "steve-2");
    }

    #[test]
    fn now_rfc3339_is_parseable() {
        let s = now_rfc3339();
        assert!(chrono::DateTime::parse_from_rfc3339(&s).is_ok());
    }

    #[test]
    fn task_and_memory_roundtrip_and_persist() {
        use crate::domain::models::Task;
        let dir = test_dir();
        let s = JsonStore::new(&dir);

        let task = Task {
            id: "t1".into(),
            workspace_id: "acme".into(),
            owner_employee_id: "steve".into(),
            objective: "answer q".into(),
            input: "who?".into(),
            status: TaskStatus::Completed,
            output_artifact_id: Some("a1".into()),
            commitment_id: None,
            project_id: None,
            created_at: "t".into(),
        };
        s.put_task(&task).unwrap();
        assert_eq!(s.list_tasks("acme").unwrap(), vec![task.clone()]);
        assert!(s.list_tasks("other").unwrap().is_empty());

        let mem = Memory {
            employee_id: "steve".into(),
            notes: vec!["ran something".into()],
            last_artifact_id: Some("a1".into()),
            updated_at: "t".into(),
        };
        s.put_memory(&mem).unwrap();
        assert_eq!(s.get_memory("steve").unwrap(), Some(mem.clone()));
        assert!(s.get_memory("nobody").unwrap().is_none());

        // 重啟後仍在
        let s2 = JsonStore::new(&dir);
        assert_eq!(s2.list_tasks("acme").unwrap().len(), 1);
        assert_eq!(s2.get_memory("steve").unwrap().unwrap().notes.len(), 1);

        // memory upsert（同 employee_id → 取代，不新增）
        let mut mem2 = mem.clone();
        mem2.notes.push("second".into());
        s2.put_memory(&mem2).unwrap();
        assert_eq!(s2.get_memory("steve").unwrap().unwrap().notes.len(), 2);
        // memories.json 仍只有一筆
        assert_eq!(s2.read::<Memory>("memories.json").unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
