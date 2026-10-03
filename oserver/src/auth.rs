//! 認證（P2→C12a→R2 遠端化）：`AuthProvider` trait——「這是誰？」的唯一回答點。
//!
//! 歷史：P2 `TokenProvider`（shared token）→ C12a `PrincipalTokenProvider`
//! （master token→operator、principal token→該身份）→ **R2**：principal token 改由
//! `principal_tokens` 表解析（一 principal 多 token、到期檢查、停用檢查、last_used
//! 節流更新）；master token 仍直通 operator（個人模式相容，語意＝admin）。
//! 版次策略不變：企業版換 provider（如 IdP）＝「再加一個實作」，handler 零改動。

/// 認證通過的身分——`name` 即 **principal id**。
/// R2 擴充：`token_id`（logout/refresh 定位用；master token 為 None）、
/// `roles`（RBAC 用，出自 principal.attrs）、`must_change_password`（首次登入強改閘）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub token_id: Option<String>,
    pub roles: Vec<String>,
    pub must_change_password: bool,
}

impl Identity {
    /// 帳號管理/測試用的便捷建構（非測試編譯下僅測試消費——標記允許 dead_code）。
    #[allow(dead_code)]
    pub fn new(name: impl Into<String>, roles: &[&str]) -> Self {
        Self {
            name: name.into(),
            token_id: None,
            roles: roles.iter().map(|s| s.to_string()).collect(),
            must_change_password: false,
        }
    }
}

/// 認證失敗原因（HTTP 層統一映射 401）。
#[derive(Debug, PartialEq, Eq)]
pub struct AuthError;

/// 認證提供者：檢查請求的 `Authorization` header，回答「這是誰」。
/// 中介層只依賴此 trait——換 provider（token→帳號→IdP）零改動。
pub trait AuthProvider: Send + Sync {
    fn check(&self, auth_header: Option<&str>) -> Result<Identity, AuthError>;
}

/// R2：token-per-principal authn——master token（向後相容）→ operator（admin）；
/// principal token（SHA-256 比對 `principal_tokens` 表）→ 該身份。
/// 務實取捨：check 內同步開本機 SQLite（請求路徑本就開）；多 token／TTL／
/// 停用／last_used 皆在 `find_valid_token` 一處裁定。
pub struct PrincipalTokenProvider {
    master_token: String,
    db_path: std::path::PathBuf,
}

impl PrincipalTokenProvider {
    pub fn new(master_token: impl Into<String>, db_path: impl Into<std::path::PathBuf>) -> Self {
        Self { master_token: master_token.into(), db_path: db_path.into() }
    }
}

impl AuthProvider for PrincipalTokenProvider {
    fn check(&self, auth_header: Option<&str>) -> Result<Identity, AuthError> {
        let h = auth_header.ok_or(AuthError)?;
        let token = h.strip_prefix("Bearer ").map(str::trim).ok_or(AuthError)?;
        if token.is_empty() {
            return Err(AuthError);
        }
        if token == self.master_token {
            return Ok(Identity {
                name: ocore::knowledge::identity::OPERATOR_PRINCIPAL_ID.to_string(),
                token_id: None,
                roles: vec!["admin".to_string()],
                must_change_password: false,
            });
        }
        let store = ocore::domain::SqliteStore::open(&self.db_path).map_err(|_| AuthError)?;
        let found =
            ocore::knowledge::identity::find_valid_token(&store, token).map_err(|_| AuthError)?;
        let Some((tok, p)) = found else {
            return Err(AuthError);
        };
        // last_used 節流更新（60s 內不重寫；失敗不擋請求）。
        let _ = ocore::knowledge::identity::touch_token_last_used(&store, &tok);
        Ok(Identity {
            name: p.id,
            token_id: Some(tok.id),
            roles: p.attrs.roles,
            must_change_password: p.must_change_password,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// stub provider 走同一 trait——證明中介層只依賴 trait（版次策略插座）。
    struct AlwaysOk;
    impl AuthProvider for AlwaysOk {
        fn check(&self, _h: Option<&str>) -> Result<Identity, AuthError> {
            Ok(Identity::new("stub", &[]))
        }
    }
    struct RejectAll;
    impl AuthProvider for RejectAll {
        fn check(&self, _h: Option<&str>) -> Result<Identity, AuthError> {
            Err(AuthError)
        }
    }

    #[test]
    fn providers_are_swappable() {
        let providers: Vec<Box<dyn AuthProvider>> =
            vec![Box::new(AlwaysOk), Box::new(RejectAll)];
        let results: Vec<Option<String>> = providers
            .iter()
            .map(|p| p.check(Some("Bearer anything")).ok().map(|i| i.name))
            .collect();
        assert_eq!(results[0].as_deref(), Some("stub"));
        assert!(results[1].is_none());
    }

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "r2auth-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("test.db")
    }

    /// **R2 完整版 Test 8**：token-per-principal——master→operator（admin 語意）、
    /// principal token→該身份＋token_id、撤銷→401、過期→401、錯誤 header→401。
    #[test]
    fn principal_token_provider_resolves_identities() {
        use ocore::domain::{SqliteStore, Store as _};
        use ocore::knowledge::identity::{
            ensure_operator_principal, issue_principal_token_v2, revoke_token_by_id,
        };
        use ocore::knowledge::types::{Principal, PrincipalType};
        let db = temp_db("full");
        let store = SqliteStore::open(&db).unwrap();
        ensure_operator_principal(&store).unwrap();
        store
            .put_principal(&Principal {
                id: "ai:bob".into(),
                principal_type: PrincipalType::AiEmployee,
                employee_id: Some("bob".into()),
                display_name: "Bob".into(),
                attrs: Default::default(),
                token_hash: None,
                login_name: None,
                password_hash: None,
                disabled: false,
                must_change_password: false,
            })
            .unwrap();
        // 服務 token（不過期）。
        let (bob_token, tok_rec) =
            issue_principal_token_v2(&store, "ai:bob", None, Some("svc")).unwrap();
        // 會話 token（-1 秒 TTL → 立即過期）。
        let (expired_plain, _) =
            issue_principal_token_v2(&store, "ai:bob", Some(-1), Some("web")).unwrap();
        let master = "master-secret".to_string();
        let provider = PrincipalTokenProvider::new(master.clone(), &db);

        // header 四態：無 header／錯格式／空 token／錯 token → 全部 401 語意。
        assert_eq!(provider.check(None), Err(AuthError));
        assert_eq!(provider.check(Some("Basic abc")), Err(AuthError));
        assert_eq!(provider.check(Some("Bearer ")), Err(AuthError));
        assert_eq!(provider.check(Some("Bearer wrong")), Err(AuthError));

        // master → operator（admin 語意）。
        let id = provider
            .check(Some(format!("Bearer {master}").as_str()))
            .unwrap();
        assert_eq!(id.name, "principal-operator");
        assert_eq!(id.roles, vec!["admin".to_string()]);
        assert_eq!(id.token_id, None);

        // bob 服務 token → ai:bob，帶 token_id。
        let id = provider
            .check(Some(format!("Bearer {bob_token}").as_str()))
            .unwrap();
        assert_eq!(id.name, "ai:bob");
        assert_eq!(id.token_id.as_deref(), Some(tok_rec.id.as_str()));

        // 過期 token → 401 語意。
        assert_eq!(
            provider.check(Some(format!("Bearer {expired_plain}").as_str())),
            Err(AuthError)
        );

        // 逐 token 撤銷 → 即時失效。
        revoke_token_by_id(&store, &tok_rec.id).unwrap();
        assert_eq!(
            provider.check(Some(format!("Bearer {bob_token}").as_str())),
            Err(AuthError)
        );

        std::fs::remove_dir_all(db.parent().unwrap()).ok();
    }

    /// R2：停用帳號 → 既有 token 立即失效（401 語意）。
    #[test]
    fn disabled_principal_tokens_rejected() {
        use ocore::domain::{SqliteStore, Store as _};
        use ocore::knowledge::identity::{issue_principal_token_v2, set_account_disabled};
        use ocore::knowledge::types::{Principal, PrincipalType};
        let db = temp_db("disabled");
        let store = SqliteStore::open(&db).unwrap();
        store
            .put_principal(&Principal {
                id: "principal-carol".into(),
                principal_type: PrincipalType::Human,
                employee_id: None,
                display_name: "Carol".into(),
                attrs: Default::default(),
                token_hash: None,
                login_name: Some("carol".into()),
                password_hash: None,
                disabled: false,
                must_change_password: false,
            })
            .unwrap();
        let (token, _) = issue_principal_token_v2(&store, "principal-carol", None, None).unwrap();
        let provider = PrincipalTokenProvider::new("master", &db);
        assert_eq!(
            provider
                .check(Some(format!("Bearer {token}").as_str()))
                .unwrap()
                .name,
            "principal-carol"
        );
        set_account_disabled(&store, "principal-carol", true).unwrap();
        assert_eq!(
            provider.check(Some(format!("Bearer {token}").as_str())),
            Err(AuthError),
            "停用帳號的 token 必須立即失效"
        );
        std::fs::remove_dir_all(db.parent().unwrap()).ok();
    }
}
