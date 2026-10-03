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

use super::models::{
    Artifact, Commitment, CommitmentStatus, Employee, EmployeeTemplate, Event, Memory, Message,
    Project, Task, TaskStatus, Workspace,
};
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
            "PRAGMA journal_mode=WAL;          \
             PRAGMA busy_timeout=5000;         \
             CREATE TABLE IF NOT EXISTS workspaces (id TEXT PRIMARY KEY, data TEXT NOT NULL); \
             CREATE TABLE IF NOT EXISTS projects (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_projects_ws ON projects(workspace_id); \
             CREATE TABLE IF NOT EXISTS employees (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_employees_ws ON employees(workspace_id); \
             CREATE TABLE IF NOT EXISTS templates (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_templates_ws ON templates(workspace_id); \
             CREATE TABLE IF NOT EXISTS tasks (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, owner_employee_id TEXT, commitment_id TEXT, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_tasks_ws ON tasks(workspace_id); \
             CREATE INDEX IF NOT EXISTS idx_tasks_owner ON tasks(owner_employee_id); \
             CREATE INDEX IF NOT EXISTS idx_tasks_commitment ON tasks(commitment_id); \
             CREATE TABLE IF NOT EXISTS artifacts (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, produced_by TEXT, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_artifacts_ws ON artifacts(workspace_id); \
             CREATE INDEX IF NOT EXISTS idx_artifacts_producer ON artifacts(produced_by); \
             CREATE TABLE IF NOT EXISTS commitments (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, owner_employee_id TEXT, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_commitments_ws ON commitments(workspace_id); \
             CREATE INDEX IF NOT EXISTS idx_commitments_owner ON commitments(owner_employee_id); \
             CREATE TABLE IF NOT EXISTS memories (employee_id TEXT PRIMARY KEY, data TEXT NOT NULL); \
             CREATE TABLE IF NOT EXISTS events (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, employee_id TEXT NOT NULL, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_events_employee ON events(employee_id); \
             CREATE TABLE IF NOT EXISTS messages (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, employee_id TEXT NOT NULL, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_messages_employee ON messages(employee_id); \
             CREATE TABLE IF NOT EXISTS principals (id TEXT PRIMARY KEY, principal_type TEXT NOT NULL, employee_id TEXT, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_principals_employee ON principals(employee_id); \
             CREATE TABLE IF NOT EXISTS principal_tokens (id TEXT PRIMARY KEY, principal_id TEXT NOT NULL, token_hash TEXT NOT NULL, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_principal_tokens_principal ON principal_tokens(principal_id); \
             CREATE INDEX IF NOT EXISTS idx_principal_tokens_hash ON principal_tokens(token_hash); \
             CREATE TABLE IF NOT EXISTS knowledge_scopes (id TEXT PRIMARY KEY, visibility TEXT NOT NULL, data TEXT NOT NULL); \
             CREATE TABLE IF NOT EXISTS knowledge_policies (id TEXT PRIMARY KEY, version INTEGER NOT NULL, data TEXT NOT NULL); \
             CREATE TABLE IF NOT EXISTS retrieval_receipts (id TEXT PRIMARY KEY, principal_id TEXT NOT NULL, employee_id TEXT, denied INTEGER NOT NULL, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_receipts_principal ON retrieval_receipts(principal_id); \
             CREATE TABLE IF NOT EXISTS knowledge_grants (id TEXT PRIMARY KEY, principal_id TEXT NOT NULL, scope_id TEXT NOT NULL, state TEXT NOT NULL, data TEXT NOT NULL); \
             CREATE INDEX IF NOT EXISTS idx_grants_principal ON knowledge_grants(principal_id); \
             CREATE INDEX IF NOT EXISTS idx_grants_scope ON knowledge_grants(scope_id);",
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
    // ── C4（D-C4）：Principal（operator bootstrap；ai_employee 讀時推導不落表）──

    fn put_principal(&self, principal: &crate::knowledge::types::Principal) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO principals (id, principal_type, employee_id, data) VALUES (?1, ?2, ?3, ?4)",
            params![
                principal.id,
                encode(&principal.principal_type)?,
                principal.employee_id,
                encode(principal)?
            ],
        )
        .map_err(|e| anyhow!("put_principal: {e}"))?;
        Ok(())
    }
    fn get_principal(&self, id: &str) -> Result<Option<crate::knowledge::types::Principal>> {
        let conn = self.lock()?;
        select_one(&conn, "principals", "id", id)
    }
    fn list_principals(&self) -> Result<Vec<crate::knowledge::types::Principal>> {
        let conn = self.lock()?;
        select_all(&conn, "principals", "ORDER BY id", params![])
    }

    // ── R2（遠端化）：principal token 生命週期 ──

    fn put_token(&self, token: &crate::knowledge::types::PrincipalToken) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO principal_tokens (id, principal_id, token_hash, data) VALUES (?1, ?2, ?3, ?4)",
            params![token.id, token.principal_id, token.token_hash, encode(token)?],
        )
        .map_err(|e| anyhow!("put_token: {e}"))?;
        Ok(())
    }
    fn get_token_by_hash(&self, token_hash: &str) -> Result<Option<crate::knowledge::types::PrincipalToken>> {
        let conn = self.lock()?;
        select_one(&conn, "principal_tokens", "token_hash", token_hash)
    }
    fn list_tokens_for_principal(&self, principal_id: &str) -> Result<Vec<crate::knowledge::types::PrincipalToken>> {
        let conn = self.lock()?;
        select_all(
            &conn,
            "principal_tokens",
            "WHERE principal_id = ?1 ORDER BY created_at",
            params![principal_id],
        )
    }
    fn delete_token(&self, token_id: &str) -> Result<bool> {
        let conn = self.lock()?;
        let n = conn
            .execute("DELETE FROM principal_tokens WHERE id = ?1", params![token_id])
            .map_err(|e| anyhow!("delete_token: {e}"))?;
        Ok(n > 0)
    }
    fn delete_tokens_for_principal(&self, principal_id: &str) -> Result<usize> {
        let conn = self.lock()?;
        let n = conn
            .execute("DELETE FROM principal_tokens WHERE principal_id = ?1", params![principal_id])
            .map_err(|e| anyhow!("delete_tokens_for_principal: {e}"))?;
        Ok(n)
    }

    fn put_scope(&self, scope: &crate::knowledge::types::KnowledgeScope) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO knowledge_scopes (id, visibility, data) VALUES (?1, ?2, ?3)",
            params![scope.id, encode(&scope.visibility)?, encode(scope)?],
        )
        .map_err(|e| anyhow!("put_scope: {e}"))?;
        Ok(())
    }
    fn list_scopes(&self) -> Result<Vec<crate::knowledge::types::KnowledgeScope>> {
        let conn = self.lock()?;
        select_all(&conn, "knowledge_scopes", "ORDER BY id", params![])
    }
    fn put_policy(&self, policy: &crate::knowledge::types::KnowledgePolicy) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO knowledge_policies (id, version, data) VALUES ('active', ?1, ?2)",
            params![policy.version, encode(policy)?],
        )
        .map_err(|e| anyhow!("put_policy: {e}"))?;
        Ok(())
    }
    fn get_policy(&self) -> Result<Option<crate::knowledge::types::KnowledgePolicy>> {
        let conn = self.lock()?;
        select_one(&conn, "knowledge_policies", "id", "active")
    }

    fn put_receipt(&self, receipt: &crate::knowledge::service::RetrievalReceipt) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO retrieval_receipts (id, principal_id, employee_id, denied, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                receipt.id,
                receipt.principal_id,
                receipt.employee_id,
                receipt.denied as i64,
                encode(receipt)?
            ],
        )
        .map_err(|e| anyhow!("put_receipt: {e}"))?;
        Ok(())
    }
    fn list_recent_receipts(
        &self,
        limit: usize,
    ) -> Result<Vec<crate::knowledge::service::RetrievalReceipt>> {
        let conn = self.lock()?;
        select_all(
            &conn,
            "retrieval_receipts",
            &format!("ORDER BY rowid DESC LIMIT {limit}"),
            params![],
        )
    }
    fn delete_receipts_before(&self, cutoff: &str) -> Result<usize> {
        let conn = self.lock()?;
        let n = conn
            .execute(
                // created_at 在 JSON blob 內（表無此欄）——json_extract 免遷移。
                "DELETE FROM retrieval_receipts WHERE json_extract(data, '$.created_at') < ?1",
                params![cutoff],
            )
            .map_err(|e| anyhow!("delete_receipts_before: {e}"))?;
        Ok(n)
    }

    fn put_grant(&self, grant: &crate::knowledge::types::KnowledgeGrant) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO knowledge_grants (id, principal_id, scope_id, state, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                grant.id,
                grant.principal_id,
                grant.scope_id,
                encode(&grant.state)?,
                encode(grant)?
            ],
        )
        .map_err(|e| anyhow!("put_grant: {e}"))?;
        Ok(())
    }
    fn list_grants(&self) -> Result<Vec<crate::knowledge::types::KnowledgeGrant>> {
        let conn = self.lock()?;
        select_all(&conn, "knowledge_grants", "ORDER BY id", params![])
    }

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

    fn list_projects(&self, workspace_id: &str) -> Result<Vec<Project>> {
        let conn = self.lock()?;
        select_all(&conn, "projects", "WHERE workspace_id = ?1", params![workspace_id])
    }
    fn get_project(&self, id: &str) -> Result<Option<Project>> {
        let conn = self.lock()?;
        select_one(&conn, "projects", "id", id)
    }
    fn put_project(&self, p: &Project) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO projects (id, workspace_id, data) VALUES (?1, ?2, ?3)",
            params![p.id, p.workspace_id, encode(p)?],
        )
        .map_err(|e| anyhow!("put_project: {e}"))?;
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
    fn delete_employee(&self, id: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM employees WHERE id = ?1", params![id])
            .map_err(|e| anyhow!("delete_employee: {e}"))?;
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
    fn delete_template(&self, id: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM templates WHERE id = ?1", params![id])
            .map_err(|e| anyhow!("delete_template: {e}"))?;
        Ok(())
    }

    fn list_tasks(&self, workspace_id: &str) -> Result<Vec<Task>> {
        let conn = self.lock()?;
        select_all(&conn, "tasks", "WHERE workspace_id = ?1", &[&workspace_id])
    }
    fn get_task(&self, id: &str) -> Result<Option<Task>> {
        let conn = self.lock()?;
        select_one(&conn, "tasks", "id", id)
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

    // ── Phase 6：owner／producer 維度查詢（owner 欄已建 index；status 在 data blob，取出後 in-memory 過濾）──

    fn list_all_employees(&self) -> Result<Vec<Employee>> {
        let conn = self.lock()?;
        select_all(&conn, "employees", "", params![])
    }

    fn list_all_commitments(&self) -> Result<Vec<Commitment>> {
        let conn = self.lock()?;
        select_all(&conn, "commitments", "", params![])
    }

    fn list_tasks_by_owner(
        &self,
        owner_employee_id: &str,
        statuses: &[TaskStatus],
    ) -> Result<Vec<Task>> {
        let conn = self.lock()?;
        let tasks: Vec<Task> = select_all(
            &conn,
            "tasks",
            "WHERE owner_employee_id = ?1",
            &[&owner_employee_id],
        )?;
        Ok(tasks
            .into_iter()
            .filter(|t| statuses.contains(&t.status))
            .collect())
    }

    fn list_active_commitments_by_owner(&self, owner_employee_id: &str) -> Result<Vec<Commitment>> {
        let conn = self.lock()?;
        let coms: Vec<Commitment> = select_all(
            &conn,
            "commitments",
            "WHERE owner_employee_id = ?1",
            &[&owner_employee_id],
        )?;
        Ok(coms
            .into_iter()
            .filter(|c| c.status == CommitmentStatus::Active)
            .collect())
    }

    fn list_artifacts_by_producer(&self, produced_by: &str) -> Result<Vec<Artifact>> {
        let conn = self.lock()?;
        select_all(
            &conn,
            "artifacts",
            "WHERE produced_by = ?1",
            &[&produced_by],
        )
    }

    // ── Phase 6d：生命週期事件（append-only；最新在前 via rowid DESC）──

    fn put_event(&self, event: &Event) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO events (id, workspace_id, employee_id, data) VALUES (?1, ?2, ?3, ?4)",
            params![event.id, event.workspace_id, event.employee_id, encode(event)?],
        )
        .map_err(|e| anyhow!("put_event: {e}"))?;
        Ok(())
    }
    fn list_events_by_employee(&self, employee_id: &str, limit: usize) -> Result<Vec<Event>> {
        let conn = self.lock()?;
        let sql = "SELECT data FROM events WHERE employee_id = ?1 ORDER BY rowid DESC LIMIT ?2";
        let mut stmt = conn.prepare(sql).map_err(|e| anyhow!("prepare: {e}"))?;
        let rows = stmt
            .query_map(params![employee_id, limit as i64], |row| row.get::<_, String>(0))
            .map_err(|e| anyhow!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(decode(&r.map_err(|e| anyhow!("row: {e}"))?)?);
        }
        Ok(out)
    }
    fn list_recent_events(&self, limit: usize) -> Result<Vec<Event>> {
        let conn = self.lock()?;
        let sql = "SELECT data FROM events ORDER BY rowid DESC LIMIT ?1";
        let mut stmt = conn.prepare(sql).map_err(|e| anyhow!("prepare: {e}"))?;
        let rows = stmt
            .query_map(params![limit as i64], |row| row.get::<_, String>(0))
            .map_err(|e| anyhow!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(decode(&r.map_err(|e| anyhow!("row: {e}"))?)?);
        }
        Ok(out)
    }

    // ── Phase 7b：對話訊息（最新在前 via rowid DESC）──

    fn put_message(&self, message: &Message) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO messages (id, workspace_id, employee_id, data) VALUES (?1, ?2, ?3, ?4)",
            params![message.id, message.workspace_id, message.employee_id, encode(message)?],
        )
        .map_err(|e| anyhow!("put_message: {e}"))?;
        Ok(())
    }
    fn list_messages_by_employee(&self, employee_id: &str, limit: usize) -> Result<Vec<Message>> {
        let conn = self.lock()?;
        let sql = "SELECT data FROM messages WHERE employee_id = ?1 ORDER BY rowid DESC LIMIT ?2";
        let mut stmt = conn.prepare(sql).map_err(|e| anyhow!("prepare: {e}"))?;
        let rows = stmt
            .query_map(params![employee_id, limit as i64], |row| row.get::<_, String>(0))
            .map_err(|e| anyhow!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(decode(&r.map_err(|e| anyhow!("row: {e}"))?)?);
        }
        Ok(out)
    }
    fn clear_messages_by_employee(&self, employee_id: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM messages WHERE employee_id = ?1",
            params![employee_id],
        )
        .map_err(|e| anyhow!("clear_messages_by_employee: {e}"))?;
        Ok(())
    }

    // ── W1e：串聯刪除（hard-delete 員工用——僅開發／測試情境）──

    fn delete_tasks_by_owner(&self, owner_employee_id: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM tasks WHERE owner_employee_id = ?1",
            params![owner_employee_id],
        )
        .map_err(|e| anyhow!("delete_tasks_by_owner: {e}"))?;
        Ok(())
    }
    fn delete_events_by_employee(&self, employee_id: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM events WHERE employee_id = ?1",
            params![employee_id],
        )
        .map_err(|e| anyhow!("delete_events_by_employee: {e}"))?;
        Ok(())
    }
    fn delete_commitments_by_owner(&self, owner_employee_id: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM commitments WHERE owner_employee_id = ?1",
            params![owner_employee_id],
        )
        .map_err(|e| anyhow!("delete_commitments_by_owner: {e}"))?;
        Ok(())
    }
    fn delete_artifacts_by_producer(&self, produced_by: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM artifacts WHERE produced_by = ?1",
            params![produced_by],
        )
        .map_err(|e| anyhow!("delete_artifacts_by_producer: {e}"))?;
        Ok(())
    }
    fn delete_memory(&self, employee_id: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM memories WHERE employee_id = ?1",
            params![employee_id],
        )
        .map_err(|e| anyhow!("delete_memory: {e}"))?;
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
        let p = std::env::temp_dir().join(format!("operoid-sqlite-test-{}-{n}.db", std::process::id()));
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
            archived: false,
            tools: None,
            created_at: "t".into(),
            owner_principal: None,
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
            project_id: None,
            external_reply_to: None,
            external_source: None,
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
            project_id: None,
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
            category_id: None,
            gate_reason: None,
            review_pending: false,
            retry_count: 0,
            next_retry_at: None,
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

    /// Phase 6：WAL + busy_timeout 的實證——兩條獨立 connection（兩個 thread）並發寫入，
    /// 第二者靠 busy_timeout 等待而非 `database is locked`，兩筆都成功落地。
    #[test]
    fn wal_concurrent_writers_with_busy_timeout() {
        let db = test_db();
        {
            let s = SqliteStore::open(&db).unwrap();
            s.put_workspace(&sample_workspace("ws")).unwrap();
        }
        let db1 = db.clone();
        let h1 = std::thread::spawn(move || {
            let s = SqliteStore::open(&db1).unwrap();
            s.put_employee(&sample_employee("e1", "ws")).unwrap();
        });
        let db2 = db.clone();
        let h2 = std::thread::spawn(move || {
            let s = SqliteStore::open(&db2).unwrap();
            s.put_employee(&sample_employee("e2", "ws")).unwrap();
        });
        h1.join().unwrap();
        h2.join().unwrap();

        let s = SqliteStore::open(&db).unwrap();
        assert_eq!(s.list_employees("ws").unwrap().len(), 2);
        std::fs::remove_file(&db).ok();
        // 清掉 WAL/SHM 副檔（若有）。
        let _ = std::fs::remove_file(format!("{}-wal", db.to_string_lossy()));
        let _ = std::fs::remove_file(format!("{}-shm", db.to_string_lossy()));
    }

    /// Phase 6d：events append-only、最新在前（rowid DESC）、不跨員工洩漏。
    #[test]
    fn events_append_only_and_newest_first() {
        let s = SqliteStore::open_in_memory().unwrap();
        s.put_workspace(&sample_workspace("ws")).unwrap();
        for i in 0..3 {
            s.put_event(&Event {
                id: format!("e{i}"),
                workspace_id: "ws".into(),
                employee_id: "e1".into(),
                kind: "wake".into(),
                detail: format!("run {i}"),
                created_at: format!("t{i}"),
            })
            .unwrap();
        }
        let evs = s.list_events_by_employee("e1", 10).unwrap();
        assert_eq!(evs.len(), 3);
        assert_eq!(evs[0].id, "e2"); // 最新在前
        assert_eq!(evs[2].id, "e0");
        assert!(s.list_events_by_employee("other", 10).unwrap().is_empty());
    }
}
