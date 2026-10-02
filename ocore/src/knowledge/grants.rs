//! 臨時授權 grants（M2-C10，D5／提示詞 §5）——任務級 TTL 知識授權。
//!
//! 語意（D-C10a 三段式，實作於 [`super::policy`]）：policy Allow → 授權；
//! **explicit deny → 拒絕（grant 不可破）**；default deny → valid grant 可破。
//! 時效**查詢時即時**判定 `expires_at`（D-C10b 主閘——無快取漂移）；本模組的
//! `expire_grants_due` 只是狀態翻轉＋事件（冪等，比照 registry expiry 先例）。
//!
//! 稽核（Rule 8）：create/revoke/expire 三事件；`create_grant`/`revoke_grant`
//! 是唯一管理入口（C12 的 UI/API 必經此）。

use anyhow::{anyhow, Result};

use super::types::{GrantState, KnowledgeGrant};
use crate::domain::store::Store;

/// 有效判定（即時）：state==Active 且 `expires_at` 在現在之後（RFC3339 字串序可比較）。
pub fn is_valid_now(grant: &KnowledgeGrant, now: &str) -> bool {
    grant.state == GrantState::Active && grant.expires_at.as_str() >= now
}

/// 某 principal 在某 scope 上是否有 valid grant（grants 由呼叫端自 store 列出）。
pub fn grant_valid_for(grants: &[KnowledgeGrant], principal_id: &str, scope_id: &str, now: &str) -> bool {
    grants
        .iter()
        .any(|g| g.principal_id == principal_id && g.scope_id == scope_id && is_valid_now(g, now))
}

/// 發放臨時授權（唯一入口；Rule 8 留 `grant_created`）。TTL 須為正——過去式即 Err。
#[allow(clippy::too_many_arguments)]
pub fn create_grant(
    store: &dyn Store,
    principal_id: &str,
    scope_id: &str,
    task_id: Option<String>,
    purpose: Option<String>,
    ttl_secs: i64,
) -> Result<KnowledgeGrant> {
    if ttl_secs <= 0 {
        return Err(anyhow!("grant TTL 須為正秒數（收到 {ttl_secs}）"));
    }
    let now = chrono::Utc::now();
    let grant = KnowledgeGrant {
        id: format!("grant-{}", now.timestamp_nanos_opt().unwrap_or_default()),
        principal_id: principal_id.to_string(),
        scope_id: scope_id.to_string(),
        task_id,
        purpose,
        expires_at: (now + chrono::Duration::seconds(ttl_secs)).to_rfc3339(),
        state: GrantState::Active,
        created_at: now.to_rfc3339(),
    };
    store.put_grant(&grant)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "grant_created",
        format!(
            "{} → scope {} 至 {}（task={:?} purpose={:?}）",
            grant.principal_id, grant.scope_id, grant.expires_at, grant.task_id, grant.purpose
        ),
    );
    Ok(grant)
}

/// 撤銷（唯一入口；即時失效——查詢時判定 state；Rule 8 留 `grant_revoked`）。
pub fn revoke_grant(store: &dyn Store, grant_id: &str) -> Result<()> {
    let mut g = store
        .list_grants()?
        .into_iter()
        .find(|g| g.id == grant_id)
        .ok_or_else(|| anyhow!("grant 不存在：{grant_id}"))?;
    if g.state == GrantState::Revoked {
        return Ok(()); // 冪等
    }
    g.state = GrantState::Revoked;
    store.put_grant(&g)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "grant_revoked",
        format!("{} → scope {}（撤銷即時生效）", g.principal_id, g.scope_id),
    );
    Ok(())
}

/// 屆期掃描：active 且 `expires_at ≤ now` → state=Expired＋`grant_expired` 事件。
/// 冪等：已翻轉者不再記事件（翻轉即狀態欄變更，重掃無事可做）。
/// 判定的主閘是查詢時即時檢查——本函式只做狀態收斂與通知（D-C10b）。
pub fn expire_grants_due(store: &dyn Store) -> Result<usize> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut flipped = 0;
    for mut g in store.list_grants()? {
        if g.state == GrantState::Active && g.expires_at.as_str() < now.as_str() {
            g.state = GrantState::Expired;
            store.put_grant(&g)?;
            flipped += 1;
            crate::runtime::record_event(
                store,
                crate::runtime::AGENT_WS,
                "knowledge",
                "grant_expired",
                format!("{} → scope {}（屆期 {}）", g.principal_id, g.scope_id, g.expires_at),
            );
        }
    }
    Ok(flipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SqliteStore;

    fn store() -> SqliteStore {
        let dir = std::env::temp_dir().join(format!(
            "m2c10-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        SqliteStore::open(&dir.join("test.db")).unwrap()
    }

    #[test]
    fn create_revoke_expire_events() {
        let s = store();
        let g = create_grant(&s, "ai:bob", "proj-x", Some("t1".into()), Some("調查".into()), 3600)
            .unwrap();
        assert!(is_valid_now(&g, &chrono::Utc::now().to_rfc3339()));
        // TTL 過去值 → Err
        assert!(create_grant(&s, "ai:bob", "proj-x", None, None, -5).is_err());

        revoke_grant(&s, &g.id).unwrap();
        revoke_grant(&s, &g.id).unwrap(); // 冪等
        let events = s.list_recent_events(20).unwrap();
        assert_eq!(events.iter().filter(|e| e.kind == "grant_created").count(), 1);
        assert_eq!(events.iter().filter(|e| e.kind == "grant_revoked").count(), 1);

        // 屆期掃描：造一個已過期的 active grant（直接 put）→ 翻轉＋事件；再掃冪等。
        let mut expired = create_grant(&s, "ai:carol", "dept-q", None, None, 3600).unwrap();
        expired.expires_at = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        s.put_grant(&expired).unwrap();
        let n = expire_grants_due(&s).unwrap();
        assert_eq!(n, 1);
        let n2 = expire_grants_due(&s).unwrap();
        assert_eq!(n2, 0, "冪等：已翻轉不再記");
        let events = s.list_recent_events(50).unwrap();
        assert_eq!(events.iter().filter(|e| e.kind == "grant_expired").count(), 1);
    }
}
