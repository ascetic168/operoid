//! 認證（P2→C12a）：`AuthProvider` trait——「這是誰？」的唯一回答點。
//!
//! 歷史：P2 首個實作 `TokenProvider`（shared token → 恆等 operator）；C4 預留
//! `AccountProvider`/RBAC 插座；**C12a 起兩者退役**——`PrincipalTokenProvider`
//! 直接在 authn 一步完成「master token → operator principal、principal token →
//! 該 principal」，`Identity.name` 攜 **principal id**，ocore 端以
//! `knowledge::identity::access_context_for_principal` 映射為 AccessContext。
//! 版次策略不變：企業版換 provider（如 IdP）＝「再加一個實作」，handler 零改動。

/// 認證通過的身分——C12a 起 `name` 即 **principal id**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
}

/// 認證失敗原因（HTTP 層統一映射 401）。
#[derive(Debug, PartialEq, Eq)]
pub struct AuthError;

/// 認證提供者：檢查請求的 `Authorization` header，回答「這是誰」。
/// 中介層只依賴此 trait——換 provider（token→帳號→IdP）零改動。
pub trait AuthProvider: Send + Sync {
    fn check(&self, auth_header: Option<&str>) -> Result<Identity, AuthError>;
}

/// C12a（D-C12a-2）：token-per-principal authn——master token（向後相容）→ operator；
/// principal token（SHA-256 比對 store）→ 該 principal。
/// 務實取捨：check 內同步開本機 SQLite（請求路徑本就開；企業遠端化時重新評估——Q9）。
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
            });
        }
        let store = ocore::domain::SqliteStore::open(&self.db_path).map_err(|_| AuthError)?;
        let p = ocore::knowledge::identity::find_principal_by_token(&store, token)
            .map_err(|_| AuthError)?;
        p.map(|p| Identity { name: p.id }).ok_or(AuthError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// stub provider 走同一 trait——證明中介層只依賴 trait（版次策略插座）。
    struct AlwaysOk;
    impl AuthProvider for AlwaysOk {
        fn check(&self, _h: Option<&str>) -> Result<Identity, AuthError> {
            Ok(Identity { name: "stub".into() })
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

    /// **Test 8（C12a 完整版）**：token-per-principal——master→operator、
    /// principal token→該身份、錯誤/撤銷→401；身份出自 token 鏈，冒名不可能。
    #[test]
    fn principal_token_provider_resolves_identities() {
        use ocore::domain::{SqliteStore, Store as _};
        use ocore::knowledge::identity::{
            ensure_operator_principal, issue_principal_token, revoke_principal_token,
        };
        use ocore::knowledge::types::{Principal, PrincipalType};
        let dir = std::env::temp_dir().join(format!(
            "c12a-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("test.db");
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
            })
            .unwrap();
        let bob_token = issue_principal_token(&store, "ai:bob").unwrap();
        let master = "master-secret".to_string();
        let provider = PrincipalTokenProvider::new(master.clone(), &db);

        // header 四態：無 header／錯格式／空 token／錯 token → 全部 401 語意。
        assert_eq!(provider.check(None), Err(AuthError));
        assert_eq!(provider.check(Some("Basic abc")), Err(AuthError));
        assert_eq!(provider.check(Some("Bearer ")), Err(AuthError));
        assert_eq!(provider.check(Some("Bearer wrong")), Err(AuthError));

        // master → operator；bob token → ai:bob。
        assert_eq!(
            provider.check(Some(format!("Bearer {master}").as_str())).unwrap().name,
            "principal-operator"
        );
        assert_eq!(
            provider.check(Some(format!("Bearer {bob_token}").as_str())).unwrap().name,
            "ai:bob"
        );

        // 撤銷即時失效；輪替＝重簽（舊失效）。
        revoke_principal_token(&store, "ai:bob").unwrap();
        assert_eq!(
            provider.check(Some(format!("Bearer {bob_token}").as_str())),
            Err(AuthError)
        );
        let bob2 = issue_principal_token(&store, "ai:bob").unwrap();
        assert_eq!(
            provider.check(Some(format!("Bearer {bob2}").as_str())).unwrap().name,
            "ai:bob"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
