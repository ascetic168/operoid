//! KnowledgeService（M1-WP-C6）——**唯一的知識檢索邊界**（提示詞 Phase 2；D1/D7/D9）。
//!
//! 流程（I1 檢索前授權）：policy fail-closed 載入（I2）→ `authorized_sources`
//! 構造候選搜尋空間 → 空集合＝DENY（中性文字＋receipt，Rule 9 不洩漏內部細節）
//! → **逐 source** 呼叫 GBrain `query`（M0-V1：`source_id` 逐呼叫純量；M0-V2：
//! `think` 不吃 source 參數，故 Think 亦導向 `query`——員工工具描述不變）→
//! 合併 → Retrieval Receipt＋事件（Test 9；Rule 8）。
//!
//! 掛線：`ToolCtx.knowledge`（`build_tool_ctx`／`agent_run_team` 建構）→ 內層
//! think/search 工具優先走本服務；`ctx.knowledge == None` 時**退回 legacy 直接
//! 路徑**——僅 `real_*` 測試使用（#[ignore]），生產建構點一律 Some（C4/C6 接線）。
//! 退役 inner tools 的直呼程式碼列 C7+（待 real_* 遷移）。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::backend::RetrieveKind;
use super::bootstrap::load_policy_fail_closed;
use super::policy::{authorized_scope_ids_with_grants, authorized_sources};
use super::types::AccessContext;
use crate::domain::store::Store;
use crate::domain::tools::{ToolCtx, ToolOutput};

/// K2：EmbeddingGemma 2 檢索查詢的 model card 建議前綴。gbrain 對查詢字串**原樣**
/// 送嵌（第十章攔截驗證）——呼叫端縫上即可，gbrain 零修改。
/// 實測價值 ≈0.2 MRR（raw 0.707→前綴 0.903；gbrain 混合 RRF 0.794→0.917）。
pub const RETRIEVAL_QUERY_PREFIX: &str = "task: search result | query: ";

/// K2：檢索查詢前綴（MCP／CLI 兩路共用；只影響嵌入檢索——think/ask 的合成問題不縫）。
pub fn retrieval_query(q: &str) -> String {
    format!("{RETRIEVAL_QUERY_PREFIX}{q}")
}

/// K2：`gbrain query` CLI 參數——前綴後的查詢＋`--no-expand`。
/// query expansion 政策：gbrain 預設 `--expand`（多查詢擴張＝額外 chat 計費＋查詢
/// 文字出端點）；實測不加已達 0.917——Operoid 一律 `--no-expand`（純 hybrid RRF）；
/// 未來若要開，以 A/B 實測增益再開。純函式供測試。
pub fn query_cli_args(prefixed_query: &str, limit: u32, source_id: &str) -> Vec<String> {
    vec![
        "query".into(),
        prefixed_query.into(),
        "--limit".into(),
        limit.to_string(),
        "--source".into(),
        source_id.into(),
        "--no-expand".into(),
    ]
}

/// 一次授權檢索的計畫（policy 評估產物；執行前無任何 GBrain 呼叫——I1）。
#[derive(Debug, Clone)]
pub struct RetrievalPlan {
    /// 授權 source 集合（排序去重）。空集合＋`denied`＝拒絕。
    pub source_ids: Vec<String>,
    /// 授權 scope 鏈（C7：receipt 的 Scope 面）。
    pub scope_ids: Vec<String>,
    pub policy_version: u32,
    pub denied: bool,
    pub reason: String,
    /// C8：任務聚焦所依的專案（None＝未聚焦）。
    pub focus_project: Option<String>,
}

/// 檢索收據（提示詞 §6；Test 9）——結構化、可重構 Who→Employee→Task→Policy→Scope 鏈。
/// 不記檢索全文（T12）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalReceipt {
    pub id: String,
    pub principal_id: String,
    #[serde(default)]
    pub employee_id: Option<String>,
    /// "think" | "search"
    pub kind: String,
    pub denied: bool,
    pub authorized_sources: Vec<String>,
    /// 授權 scope 鏈（C7）。
    pub authorized_scopes: Vec<String>,
    /// C9：跨域檢索（授權 scope >1）——Rule 8 稽核面。
    pub cross_domain: bool,
    /// C9：有命中內容的 source 數（Knowledge Objects 粒度）。
    pub returned_sources: usize,
    /// C13a：判定當下的有效 clearance（I9 鏈重構）。
    pub clearance: Option<super::types::SecurityLevel>,
    pub policy_version: u32,
    pub workspace_id: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub purpose: Option<String>,
    pub created_at: String,
}

pub struct KnowledgeService {
    db_path: PathBuf,
}

impl KnowledgeService {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self { db_path: db_path.into() }
    }

    /// 授權檢索入口。`query`/`anchor` 來自工具輸入（I4：scope 意圖**不在**輸入面——
    /// source 集合只由 policy 推導）；transport 資料取自 `ToolCtx`（exe/home/mcp）。
    pub async fn retrieve(
        &self,
        access: &AccessContext,
        kind: RetrieveKind,
        query: &str,
        _anchor: Option<&str>, // M0-V2：query op 無 anchor 參數——think 錨點語意暫緩（C7+）
        limit: u32,
        ctx: &ToolCtx,
    ) -> Result<ToolOutput> {
        let store = crate::domain::SqliteStore::open(&self.db_path)?;
        let plan = self.plan(&store, access)?;
        let plan = self.apply_focus(&store, access, plan)?;
        let kind_str = match kind {
            RetrieveKind::Think => "think",
            RetrieveKind::Search => "search",
        };

        if plan.denied || plan.source_ids.is_empty() {
            self.record(&store, access, kind_str, &plan, false, 0, access.clearance)?;
            return Ok(ToolOutput {
                text: "目前沒有可檢索的授權範圍。".to_string(),
                meta: json!({
                    "denied": true,
                    "reason": plan.reason,
                    "policy_version": plan.policy_version,
                    "receipt_kind": kind_str,
                }),
            });
        }

        // 逐 source 呼叫（M0-V1：`source_id` 為逐呼叫純量）後合併。
        let mut futs = Vec::with_capacity(plan.source_ids.len());
        for sid in &plan.source_ids {
            futs.push(self.execute_source(sid, query, limit, ctx));
        }
        let results = futures::future::join_all(futs).await;
        let mut sections: Vec<String> = Vec::new();
        let mut errors = serde_json::Map::new();
        for (sid, res) in plan.source_ids.iter().zip(results) {
            match res {
                Ok(text) if !text.trim().is_empty() => {
                    sections.push(format!("── {sid} ──\n{}", text.trim()));
                }
                Ok(_) => {}
                Err(e) => {
                    errors.insert(sid.clone(), json!(e));
                }
            }
        }
        let all_failed = sections.is_empty() && !errors.is_empty();
        let text = if all_failed {
            "知識檢索暫時失敗（所有授權來源皆無回應）。".to_string()
        } else if sections.is_empty() {
            "No results.".to_string()
        } else {
            sections.join("\n\n")
        };

        self.record(
            &store,
            access,
            kind_str,
            &plan,
            !errors.is_empty(),
            sections.len(),
            access.clearance,
        )?;
        Ok(ToolOutput {
            text,
            meta: json!({
                "backend": if ctx.mcp.is_some() { "mcp" } else { "cli" },
                "authorized_sources": plan.source_ids,
                "policy_version": plan.policy_version,
                "sources_with_errors": errors,
                "receipt_kind": kind_str,
            }),
        })
    }

    /// 候選搜尋空間構造（I1）＋fail closed（I2）。
    pub fn plan(&self, store: &dyn Store, access: &AccessContext) -> Result<RetrievalPlan> {
        let (policy, invalid) = load_policy_fail_closed(store);
        if invalid {
            crate::runtime::record_event(
                store,
                &access.workspace_id,
                "knowledge",
                "knowledge_policy_invalid",
                format!("principal {} 檢索時 policy 缺/損壞——DENY", access.principal_id),
            );
        }
        let scopes = store.list_scopes()?;
        // C10：本 principal 的 active grants（三段式授權——explicit deny 仍優先）。
        let grants: Vec<_> = store
            .list_grants()?
            .into_iter()
            .filter(|g| g.principal_id == access.principal_id)
            .collect();
        let scope_ids = authorized_scope_ids_with_grants(&policy, access, &scopes, &grants);
        let source_ids = authorized_sources(&policy, access, &scopes);
        let (denied, reason) = if invalid {
            (true, "policy_missing_or_corrupt".to_string())
        } else if source_ids.is_empty() {
            (true, "no_authorized_scope".to_string())
        } else {
            (false, String::new())
        };
        Ok(RetrievalPlan {
            source_ids,
            scope_ids,
            policy_version: policy.version,
            denied,
            reason,
            focus_project: None,
        })
    }

    /// C8：任務聚焦（只縮不擴——D-C8a）。`access.task_id` 有綁專案時，把候選集
    /// 收窄到（專案 scope ∪ Company scope）∩ 授權集；聚焦集空/全同 → no-op。
    pub fn apply_focus(
        &self,
        store: &dyn Store,
        access: &AccessContext,
        mut plan: RetrievalPlan,
    ) -> Result<RetrievalPlan> {
        if let Some(tid) = &access.task_id {
            if let Some(focus) = super::planner::task_focus(store, tid, &plan.scope_ids)? {
                let scopes = store.list_scopes()?;
                plan.source_ids = super::planner::sources_of(&scopes, &focus.scope_ids);
                plan.scope_ids = focus.scope_ids;
                plan.focus_project = Some(focus.project_id);
            }
        }
        Ok(plan)
    }

    /// 單一 source 的檢索（MCP 優先；CLI fallback——M0-V5 實測 `--source` 有效）。
    /// K2：查詢縫 task 前綴（兩路一致）＋停用 expansion（額外計費且無實測增益）。
    async fn execute_source(
        &self,
        sid: &str,
        query: &str,
        limit: u32,
        ctx: &ToolCtx,
    ) -> std::result::Result<String, String> {
        let prefixed = retrieval_query(query);
        if let Some(mcp) = &ctx.mcp {
            let args = json!({ "query": prefixed, "limit": limit, "source_id": sid, "expand": false });
            return mcp.call("query", args).await.map_err(|e| e.to_string());
        }
        let args = query_cli_args(&prefixed, limit, sid);
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (code, out, err) = crate::gbrain_cli::run_capture(
            &ctx.gbrain_exe,
            &refs,
            &crate::proc::env_for_brain(ctx.gbrain_home.as_deref()),
        )
        .await
        .map_err(|e| e.to_string())?;
        if code != 0 {
            return Err(format!("gbrain query exit {code}: {}", err.trim()));
        }
        Ok(out)
    }

    /// 寫入 receipt＋`retrieval` 事件（Test 9；Rule 8）。收據**不記查詢全文**——
    /// 記 query_id 級的授權鏈 metadata（T12；查詢文字屬稽核資產，留在事件 detail 由 C9 定案）。
    pub(crate) fn record(
        &self,
        store: &dyn Store,
        access: &AccessContext,
        kind: &str,
        plan: &RetrievalPlan,
        partial_errors: bool,
        returned_sources: usize,
        ctx_clearance: Option<super::types::SecurityLevel>,
    ) -> Result<String> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        let id = format!("rcpt-{}-{}", nanos, access.principal_id);
        let receipt = RetrievalReceipt {
            id: id.clone(),
            principal_id: access.principal_id.clone(),
            employee_id: access.employee_id.clone(),
            kind: kind.to_string(),
            denied: plan.denied,
            authorized_sources: plan.source_ids.clone(),
            authorized_scopes: plan.scope_ids.clone(),
            cross_domain: plan.scope_ids.len() > 1,
            returned_sources,
            clearance: ctx_clearance,
            policy_version: plan.policy_version,
            workspace_id: access.workspace_id.clone(),
            task_id: access.task_id.clone(),
            purpose: access.purpose.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        store.put_receipt(&receipt)?;
        crate::runtime::record_event(
            store,
            &access.workspace_id,
            access.employee_id.as_deref().unwrap_or("knowledge"),
            "retrieval",
            json!({
                "receipt_id": id,
                "principal_id": access.principal_id,
                "kind": kind,
                "denied": plan.denied,
                "authorized_sources": plan.source_ids,
                "authorized_scopes": plan.scope_ids,
                "cross_domain": plan.scope_ids.len() > 1,
                "returned_sources": returned_sources,
                "clearance": ctx_clearance,
                "policy_version": plan.policy_version,
                "partial_errors": partial_errors,
                "focus_project": plan.focus_project,
                "task_id": access.task_id,
            })
            .to_string(),
        );
        Ok(id)
    }
}

/// C9 保留策略：receipts 保留天數（D-C9d）。
pub const RECEIPT_RETENTION_DAYS: i64 = 90;

/// 清除超過保留期的 receipts（scheduler 日界臂呼叫）；清除 >0 筆時記事件。
pub fn prune_receipts(store: &dyn Store, keep_days: i64) -> Result<usize> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(keep_days)).to_rfc3339();
    let n = store.delete_receipts_before(&cutoff)?;
    if n > 0 {
        crate::runtime::record_event(
            store,
            crate::runtime::AGENT_WS,
            "knowledge",
            "receipts_pruned",
            format!("清除 {n} 筆 {keep_days} 天前的 receipts"),
        );
    }
    Ok(n)
}

/// 便利建構：放進 `ToolCtx.knowledge`。
pub fn service_arc(db_path: impl Into<PathBuf>) -> Arc<KnowledgeService> {
    Arc::new(KnowledgeService::new(db_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::bootstrap::{bootstrap_with_sources, save_policy_new_version};
    use crate::knowledge::types::{Effect, PolicyRule};

    /// **K2**：查詢前綴逐字對齊 model card／實驗格式（第十章攔截：51 字元驗證字串
    /// 原樣通過 gbrain）。
    #[test]
    fn k2_query_prefix_format() {
        assert_eq!(
            retrieval_query("protocol"),
            "task: search result | query: protocol"
        );
        assert_eq!(RETRIEVAL_QUERY_PREFIX.len(), 29);
    }

    /// **K2**：CLI 參數帶前綴查詢＋`--no-expand`＋授權 source 過濾。
    #[test]
    fn k2_cli_args_carry_prefix_and_no_expand() {
        let args = query_cli_args(&retrieval_query("two-chip IVR"), 5, "k7");
        assert_eq!(
            args,
            vec![
                "query",
                "task: search result | query: two-chip IVR",
                "--limit",
                "5",
                "--source",
                "k7",
                "--no-expand"
            ]
        );
    }

    fn store() -> crate::domain::SqliteStore {
        let dir = std::env::temp_dir().join(format!(
            "m1c6-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        crate::domain::SqliteStore::open(&dir.join("test.db")).unwrap()
    }

    fn operator() -> AccessContext {
        crate::knowledge::identity::operator_access_context(crate::runtime::AGENT_WS)
    }

    /// **Test 9（M1 版）**：每次授權檢索產出可稽核 receipt（結構化列＋`retrieval` 事件）。
    #[test]
    fn m1_t9_receipt() {
        use crate::domain::Store as _;
        let s = store();
        bootstrap_with_sources(&s, &["src-a".into()]).unwrap();
        let svc = KnowledgeService::new("unused-for-plan.db");

        // plan（policy 評估）→ record（receipt＋事件）——retrieve 的授權鏈兩段皆可稽核。
        let plan = svc.plan(&s, &operator()).unwrap();
        assert!(!plan.denied);
        assert_eq!(plan.source_ids, vec!["src-a".to_string()]);
        assert_eq!(plan.policy_version, 1);

        let rid = svc.record(&s, &operator(), "search", &plan, false, 1, None).unwrap();
        assert!(rid.starts_with("rcpt-"));
        let receipts = s.list_recent_receipts(10).unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].id, rid);
        assert_eq!(receipts[0].authorized_sources, vec!["src-a".to_string()]);
        assert!(!receipts[0].denied);
        // 事件面：EventsView 可見的 `retrieval` 事件也存在。
        let events = s.list_recent_events(10).unwrap();
        assert!(events.iter().any(|e| e.kind == "retrieval" && e.detail.contains(&rid)));
    }

    /// 拒絕路徑也留稽核（deny receipt）。
    #[test]
    fn m1_t9_deny_also_recorded() {
        use crate::domain::Store as _;
        let s = store();
        bootstrap_with_sources(&s, &["src-a".into()]).unwrap();
        // ai:bob 未在任何規則中被允許（bootstrap 是 allow-all——先收緊）。
        save_policy_new_version(
            &s,
            vec![PolicyRule {
                id: "operator-only".into(),
                priority: 1,
                effect: Effect::Allow,
                principals: Some(vec!["principal-operator".into()]),
                principal_types: None,
                scopes: None,
                departments: None,
                projects: None,
                classifications: None,
                department_membership: false,
                project_membership: false,
            }],
        )
        .unwrap();
        let svc = KnowledgeService::new("unused.db");
        let plan = svc.plan(&s, &crate::knowledge::identity::access_context_for_employee(&test_emp(), None, None)).unwrap();
        assert!(plan.denied);
        assert_eq!(plan.reason, "no_authorized_scope");
        svc.record(&s, &plan_principal(), "think", &plan, false, 0, None).unwrap();
        let receipts = s.list_recent_receipts(10).unwrap();
        assert!(receipts.iter().any(|r| r.denied));
    }

    fn plan_principal() -> AccessContext {
        crate::knowledge::identity::access_context_for_employee(&test_emp(), None, None)
    }

    fn test_emp() -> crate::domain::models::Employee {
        crate::domain::models::Employee {
            id: "bob".into(),
            workspace_id: crate::runtime::AGENT_WS.into(),
            name: "Bob".into(),
            brain: crate::domain::models::BrainRef { brain_id: "__default__".into() },
            role: None,
            template_id: None,
            state: crate::domain::models::EmployeeState::Sleeping,
            archived: false,
            tools: None,
            created_at: "2026-10-02T00:00:00Z".into(),
            owner_principal: None,
        }
    }
}
