//! 認證（P2）：`AuthProvider` trait——「這是誰？」的唯一回答點。
//!
//! 首個實作 `TokenProvider`（shared token，比照 ingress/obridge 既有模式）。
//! 為**版次策略**（計畫 §六）預留插座：企業版的 `AccountProvider`/RBAC 是
//! 「再加一個實作」，kernel 與 API handler 零改動。

/// 認證通過的身分（未來帳號版擴展 name→roles 等；P2 全域單人，name 固定 "operator"）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    #[allow(dead_code)] // 未來帳號版（RBAC）使用；P2 全域單人。
    pub name: String,
}

/// 認證失敗原因（HTTP 層統一映射 401）。
#[derive(Debug, PartialEq, Eq)]
pub struct AuthError;

/// 認證提供者：檢查請求的 `Authorization` header，回答「這是誰」。
/// 中介層只依賴此 trait——換 provider（token→帳號）零改動。
pub trait AuthProvider: Send + Sync {
    fn check(&self, auth_header: Option<&str>) -> Result<Identity, AuthError>;
}

/// Shared-token 認證（P2 唯一實作）：`Authorization: Bearer <token>` 比對。
pub struct TokenProvider {
    token: String,
}

impl TokenProvider {
    pub fn new(token: impl Into<String>) -> Self {
        Self { token: token.into() }
    }
}

impl AuthProvider for TokenProvider {
    fn check(&self, auth_header: Option<&str>) -> Result<Identity, AuthError> {
        let h = auth_header.ok_or(AuthError)?;
        let token = h.strip_prefix("Bearer ").map(str::trim).ok_or(AuthError)?;
        if token.is_empty() || token != self.token {
            return Err(AuthError);
        }
        Ok(Identity { name: "operator".into() })
    }
}

/// C4（D-C4）：帳號型身份提供者插座——`Identity → Principal` 的唯一映射點
/// （`auth.rs:1-5` 檔頭預留的企業版插座；kernel 與 handler 零改動）。
/// M1 唯一實作 [`SingleOperatorProvider`]：authN 通過者恆映射 operator bootstrap
/// principal——呼叫端無法自稱他人（Test 8 的服務端語意；多 principal 屬 WP-C12）。
pub trait AccountProvider: Send + Sync {
    fn principal_for(&self, identity: &Identity) -> ocore::knowledge::types::Principal;
}

/// 單人版實作：任何通過 authN 的請求＝operator。
pub struct SingleOperatorProvider;

impl AccountProvider for SingleOperatorProvider {
    fn principal_for(&self, _identity: &Identity) -> ocore::knowledge::types::Principal {
        ocore::knowledge::identity::operator_principal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 認證四情境：無 header／錯格式／錯 token／正確。
    #[test]
    fn token_provider_four_cases() {
        let p = TokenProvider::new("s3cret");
        assert_eq!(p.check(None), Err(AuthError));
        assert_eq!(p.check(Some("Basic abc")), Err(AuthError));
        assert_eq!(p.check(Some("Bearer wrong")), Err(AuthError));
        assert!(p.check(Some("Bearer s3cret")).is_ok());
    }

    /// stub provider 走同一 trait——證明中介層只依賴 trait（版次策略插座）。
    struct AlwaysOk;
    impl AuthProvider for AlwaysOk {
        fn check(&self, _h: Option<&str>) -> Result<Identity, AuthError> {
            Ok(Identity { name: "stub".into() })
        }
    }

    #[test]
    fn provider_is_swappable() {
        let providers: Vec<Box<dyn AuthProvider>> =
            vec![Box::new(TokenProvider::new("x")), Box::new(AlwaysOk)];
        let results: Vec<Result<Identity, AuthError>> = providers
            .iter()
            .map(|p| p.check(Some("Bearer anything")))
            .collect();
        assert_eq!(results[0], Err(AuthError)); // token 版仍把關
        assert_eq!(results[1].as_ref().unwrap().name, "stub"); // stub 版放行
    }

    /// C4：AccountProvider 插座——authN 通過者恆映射 operator principal（不可自稱）。
    #[test]
    fn account_provider_maps_to_operator_principal() {
        let p = SingleOperatorProvider.principal_for(&Identity { name: "operator".into() });
        assert_eq!(p.id, ocore::knowledge::identity::OPERATOR_PRINCIPAL_ID);
        assert_eq!(
            p.principal_type,
            ocore::knowledge::types::PrincipalType::Human
        );
    }
}
