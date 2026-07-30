//! SQLite 持久化實作（Phase 2，決策 D2）——正式後端。
//!
//! 實作 [`Store`] trait。`Mutex<rusqlite::Connection>` 使其 `Send + Sync`（Connection 本身
//! 僅 Send），符合 `run_cycle` 對 `&(dyn Store + Send + Sync)` 的要求。方法為同步（本地 sqlite
//! 操作極快；run_cycle 本就以同步方式呼叫 store）。
//!
//! Schema：每表一個實體，可查關鍵欄位為欄、其餘完整 struct 放 `data` JSON blob。
//! 保留 `JsonStore` 作測試／抽象對照。

use std::path::Path;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::models::{Artifact, Commitment, Employee, EmployeeTemplate, Memory, Task, Workspace};
use super::store::Store;

/// SQLite-backed [`Store`]。
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// 開／建 `<path>` 的資料庫並確保 schema 存在。
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| anyhow!("open sqlite: {e}"))?;
        Self::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| anyhow!("open in-memory: {e}"))?;
        Self::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn init(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS workspaces (id TEXT PRIMARY KEY, data TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS employees (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, data TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_employees_ws ON employees(workspace_id);
             CREATE TABLE IF NOT EXISTS templates (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, data TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_templates_ws ON templates(workspace_id);
             CREATE TABLE IF NOT EXISTS tasks (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, owner_employee_id TEXT, commitment_id TEXT, data TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_tasks_ws ON tasks(workspace_id);
             CREATE TABLE IF NOT EXISTS artifacts (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, produced_by TEXT, data TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_artifacts_ws ON artifacts(workspace_id);
             CREATE TABLE IF NOT EXISTS commitments (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, owner_employee_id TEXT, data TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_commitments_ws ON commitments(workspace_id);
             CREATE TABLE IF NOT EXISTS memories (employee_id TEXT PRIMARY KEY, data TEXT NOT NULL);",
        )
        .map_err(|e| anyhow!("init schema: {e}"))?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| anyhow!("db lock poisoned: {e}"))
    }

    /// 一次性、冪等的 JSON→SQLite 遷移。讀 `<json_base>/domain/*.json`（若存在）並 upsert。
    /// 真實 app 目前 `domain/` 為空，故實質 no-op；保留供未來／測試用。
    pub fn migrate_from_json(&self, json_base: &Path) -> Result<()> {
        let dir = json_base.join("domain");
        let read = |file: &str| -> Option<serde_json::Value> {
            std::fs::read_to_string(dir.join(file))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
        };
        if let Some(v) = read("workspaces.json") {
            for ws in decode_vec::<Workspace>(&v)? {
                self.put_workspace(&ws)?;
            }
        }
        if let Some(v) = read("employees.json") {
            for e in decode_vec::<Employee>(&v)? {
                self.put_employee(&e)?;
            }
        }
        if let Some(v) = read("tasks.json") {
            for t in decode_vec::<Task>(&v)? {
                self.put_task(&t)?;
            }
        }
        if let Some(v) = read("artifacts.json") {
            for a in decode_vec::<Artifact>(&v)? {
                self.put_artifact(&a)?;
            }
        }
        if let Some(v) = read("commitments.json") {
            for c in decode_vec::<Commitment>(&v)? {
                self.put_commitment(&c)?;
            }
        }
        if let Some(v) = read("memories.json") {
            for m in decode_vec::<Memory>(&v)? {
                self.put_memory(&m)?;
            }
        }
        Ok(())
    }
}

// ───────────────── helpers ─────────────────

fn encode<T: Serialize>(t: &T) -> Result<String> {
    serde_json::to_string(t).map_err(|e| anyhow!("encode: {e}"))
}

fn decode<T: DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_str(s).map_err(|e| anyhow!("decode: {e}"))
}

fn decode_vec<T: DeserializeOwned>(v: &serde_json::Value) -> Result<Vec<T>> {
    serde_json::from_value(v.clone()).map_err(|e| anyhow!("decode_vec: {e}"))
}

/// 查 `SELECT data FROM <table> [WHERE <where>]`，逐列解碼。
fn select_all<T: DeserializeOwned>(
    conn: &Connection,
    table: &str,
    where_clause: &str,
    args: &[&dyn rusqlite::ToSql],
) -> Result<Vec<T>> {
    let sql = format!("SELECT data FROM {table} {where_clause}");
    let mut stmt = conn.prepare(&sql).map_err(|e| anyhow!("prepare: {e}"))?;
    let rows = stmt
        .query_map(args, |row| row.get::<_, String>(0))
        .map_err(|e| anyhow!("query: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(decode(&r.map_err(|e| anyhow!("row: {e}"))?)?);
    }
    Ok(out)
}

fn select_one<T: DeserializeOwned>(
    conn: &Connection,
    table: &str,
    key_col: &str,
    id: &str,
) -> Result<Option<T>> {
    let sql = format!("SELECT data FROM {table} WHERE {key_col} = ?1 LIMIT 1");
    let mut stmt = conn.prepare(&sql).map_err(|e| anyhow!("prepare: {e}"))?;
    let mut rows = stmt
        .query_map(params![id], |row| row.get::<_, String>(0))
        .map_err(|e| anyhow!("query: {e}"))?;
    match rows.next() {
        Some(r) => Ok(Some(decode(&r.map_err(|e| anyhow!("row: {e}"))?)?)),
        None => Ok(None),
    }
}

// ───────────────── Store impl ─────────────────

impl Store for SqliteStore {
    fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        let conn = self.lock()?;
        select_all(&conn, "workspaces", "", params![])
    }
    fn get_workspace(&self, id: &str) -> Result<Option<Workspace>> {
        let conn = self.lock()?;
        select_one(&conn, "workspaces", "id", id)
    }
    fn put_workspace(&self, ws: &Workspace) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO workspaces (id, data) VALUES (?1, ?2)",
            params![ws.id, encode(ws)?],
        )
        .map_err(|e| anyhow!("put_workspace: {e}"))?;
        Ok(())
    }

    fn list_employees(&self, workspace_id: &str) -> Result<Vec<Employee>> {
        let conn = self.lock()?;
        select_all(
            &conn,
            "employees",
            "WHERE workspace_id = ?1",
            params![workspace_id],
        )
    }
    fn get_employee(&self, id: &str) -> Result<Option<Employee>> {
        let conn = self.lock()?;
        select_one(&conn, "employees", "id", id)
    }
    fn put_employee(&self, emp: &Employee) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO employees (id, workspace_id, data) VALUES (?1, ?2, ?3)",
            params![emp.id, emp.workspace_id, encode(emp)?],
        )
        .map_err(|e| anyhow!("put_employee: {e}"))?;
        Ok(())
    }

    fn list_templates(&self, workspace_id: &str) -> Result<Vec<EmployeeTemplate>> {
        let conn = self.lock()?;
        select_all(&conn, "templates", "WHERE workspace_id = ?1", params![workspace_id])
    }
    fn get_template(&self, id: &str) -> Result<Option<EmployeeTemplate>> {
        let conn = self.lock()?;
        select_one(&conn, "templates", "id", id)
    }
    fn put_template(&self, tmpl: &EmployeeTemplate) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO templates (id, workspace_id, data) VALUES (?1, ?2, ?3)",
            params![tmpl.id, tmpl.workspace_id, encode(tmpl)?],
        )
        .map_err(|e| anyhow!("put_template: {e}"))?;
        Ok(())
    }

    fn list_tasks(&self, workspace_id: &str) -> Result<Vec<Task>> {
        let conn = self.lock()?;
        select_all(&conn, "tasks", "WHERE workspace_id = ?1", &[&workspace_id])
    }
    fn put_task(&self, task: &Task) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO tasks (id, workspace_id, owner_employee_id, commitment_id, data) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                task.id,
                task.workspace_id,
                task.owner_employee_id,
                task.commitment_id,
                encode(task)?
            ],
        )
        .map_err(|e| anyhow!("put_task: {e}"))?;
        Ok(())
    }

    fn list_artifacts(&self, workspace_id: &str) -> Result<Vec<Artifact>> {
        let conn = self.lock()?;
        select_all(
            &conn,
            "artifacts",
            "WHERE workspace_id = ?1",
            params![workspace_id],
        )
    }
    fn get_artifact(&self, id: &str) -> Result<Option<Artifact>> {
        let conn = self.lock()?;
        select_one(&conn, "artifacts", "id", id)
    }
    fn put_artifact(&self, art: &Artifact) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO artifacts (id, workspace_id, produced_by, data) VALUES (?1, ?2, ?3, ?4)",
            params![art.id, art.workspace_id, art.produced_by, encode(art)?],
        )
        .map_err(|e| anyhow!("put_artifact: {e}"))?;
        Ok(())
    }

    fn list_commitments(&self, workspace_id: &str) -> Result<Vec<Commitment>> {
        let conn = self.lock()?;
        select_all(
            &conn,
            "commitments",
            "WHERE workspace_id = ?1",
            params![workspace_id],
        )
    }
    fn get_commitment(&self, id: &str) -> Result<Option<Commitment>> {
        let conn = self.lock()?;
        select_one(&conn, "commitments", "id", id)
    }
    fn put_commitment(&self, c: &Commitment) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO commitments (id, workspace_id, owner_employee_id, data) \
             VALUES (?1, ?2, ?3, ?4)",
            params![c.id, c.workspace_id, c.owner_employee_id, encode(c)?],
        )
        .map_err(|e| anyhow!("put_commitment: {e}"))?;
        Ok(())
    }

    fn get_memory(&self, employee_id: &str) -> Result<Option<Memory>> {
        let conn = self.lock()?;
        select_one(&conn, "memories", "employee_id", employee_id)
    }
    fn put_memory(&self, memory: &Memory) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO memories (employee_id, data) VALUES (?1, ?2)",
            params![memory.employee_id, encode(memory)?],
        )
        .map_err(|e| anyhow!("put_memory: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::{
        ArtifactStatus, BrainRef, CommitmentStatus, EmployeeState, Memory, TaskStatus,
        WorkspaceStatus,
    };
    use std::path::PathBuf;

    fn test_db() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("emploid-sqlite-test-{}-{n}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn sample_workspace(id: &str) -> Workspace {
        Workspace {
            id: id.into(),
            name: id.into(),
            description: None,
            status: WorkspaceStatus::Active,
            created_at: "t".into(),
        }
    }
    fn sample_employee(id: &str, ws: &str) -> Employee {
        Employee {
            id: id.into(),
            workspace_id: ws.into(),
            name: id.into(),
            brain: BrainRef { brain_id: "__default__".into() },
            role: None,
            template_id: None,
            state: EmployeeState::Sleeping,
            created_at: "t".into(),
        }
    }

    #[test]
    fn roundtrip_all_entities_in_memory() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.put_workspace(&sample_workspace("ws")).unwrap();
        assert_eq!(s.list_workspaces().unwrap().len(), 1);
        assert!(s.get_workspace("ws").unwrap().is_some());

        s.put_employee(&sample_employee("e1", "ws")).unwrap();
        s.put_employee(&sample_employee("e2", "ws")).unwrap();
        assert_eq!(s.list_employees("ws").unwrap().len(), 2);
        assert!(s.get_employee("e1").unwrap().is_some());

        let task = Task {
            id: "t1".into(),
            workspace_id: "ws".into(),
            owner_employee_id: "e1".into(),
            objective: "o".into(),
            input: "i".into(),
            status: TaskStatus::Completed,
            output_artifact_id: Some("a1".into()),
            commitment_id: Some("c1".into()),
            created_at: "t".into(),
        };
        s.put_task(&task).unwrap();
        assert_eq!(s.list_tasks("ws").unwrap(), vec![task.clone()]);

        let art = Artifact {
            id: "a1".into(),
            workspace_id: "ws".into(),
            title: "T".into(),
            artifact_type: "report".into(),
            content: "body".into(),
            produced_by: "e1".into(),
            source_task_id: Some("t1".into()),
            source_commitment_id: Some("c1".into()),
            revised_from_id: None,
            version: 1,
            status: ArtifactStatus::Committed,
            created_at: "t".into(),
        };
        s.put_artifact(&art).unwrap();
        assert_eq!(s.get_artifact("a1").unwrap(), Some(art.clone()));
        assert_eq!(s.list_artifacts("ws").unwrap(), vec![art]);

        let com = Commitment {
            id: "c1".into(),
            workspace_id: "ws".into(),
            owner_employee_id: "e1".into(),
            title: "track".into(),
            completion_condition: "done".into(),
            status: CommitmentStatus::Active,
            created_at: "t".into(),
            updated_at: "t".into(),
        };
        s.put_commitment(&com).unwrap();
        assert_eq!(s.get_commitment("c1").unwrap(), Some(com.clone()));
        assert_eq!(s.list_commitments("ws").unwrap(), vec![com]);

        let mem = Memory {
            employee_id: "e1".into(),
            notes: vec!["n".into()],
            last_artifact_id: Some("a1".into()),
            updated_at: "t".into(),
        };
        s.put_memory(&mem).unwrap();
        assert_eq!(s.get_memory("e1").unwrap(), Some(mem));
    }

    #[test]
    fn persists_across_reopen() {
        let db = test_db();
        {
            let s = SqliteStore::open(&db).unwrap();
            s.put_workspace(&sample_workspace("ws")).unwrap();
            s.put_employee(&sample_employee("e1", "ws")).unwrap();
        }
        // 重開同一 db（模擬重啟）
        let s = SqliteStore::open(&db).unwrap();
        assert!(s.get_workspace("ws").unwrap().is_some());
        assert_eq!(s.list_employees("ws").unwrap().len(), 1);
        std::fs::remove_file(&db).ok();
    }

    #[test]
    fn upsert_replaces_by_id() {
        let s = SqliteStore::open_in_memory().unwrap();
        let mut ws = sample_workspace("ws");
        ws.status = WorkspaceStatus::Active;
        s.put_workspace(&ws).unwrap();
        ws.status = WorkspaceStatus::Suspended;
        s.put_workspace(&ws).unwrap(); // 同 id → 取代
        assert_eq!(s.list_workspaces().unwrap().len(), 1);
        assert_eq!(s.get_workspace("ws").unwrap().unwrap().status, WorkspaceStatus::Suspended);
    }
}
