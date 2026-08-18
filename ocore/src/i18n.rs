//! 國際化訊息模型 — 給前端 vue-i18n 翻譯用的「代碼 + 具名參數」。
//!
//! Rust 端不再回傳組好的中文字串，而是回傳 `L10n { code, params }`（結果欄位）
//! 或 `AppError { code, params }`（命令錯誤）。前端用 vue-i18n 的 `t(code, params)` 翻譯。
//!
//! 高價值/常見錯誤在源頭顯式編碼（如 `AppError::new("llm.noApiKey")`）；
//! 其餘透過 `From<anyhow::Error>` / `From<String>` 自動包成 `error.unexpected`
//! （原始明細保留於 `params.detail`），命令 `?` 無需逐處改寫即可被前端翻譯。

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

/// 可在地化訊息：代碼 + 具名參數（直接餵 vue-i18n `t(code, params)`）。
///
/// 空參數不序列化（前端 `t(code)` 亦可）。用於結果 struct 的 `summary` / `note` /
/// `errors` 等欄位。
#[derive(Debug, Clone, Serialize, Default)]
pub struct L10n {
    pub code: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

impl L10n {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            params: BTreeMap::new(),
        }
    }

    /// 加一個具名參數（builder）。值會 `to_string()`。
    pub fn p(mut self, key: impl Into<String>, val: impl ToString) -> Self {
        self.params.insert(key.into(), val.to_string());
        self
    }
}

/// 統一命令錯誤型別。序列化為 `{code, params}`，前端 `invoke().catch(e)` 直接收物件，
/// 用 vue-i18n `t(e.code, e.params)` 翻譯。
#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

impl AppError {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            params: BTreeMap::new(),
        }
    }

    /// 加一個具名參數（builder）。
    pub fn p(mut self, key: impl Into<String>, val: impl ToString) -> Self {
        self.params.insert(key.into(), val.to_string());
        self
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.params.is_empty() {
            write!(f, "{}", self.code)
        } else {
            let kv: Vec<String> = self.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
            write!(f, "{} ({})", self.code, kv.join(", "))
        }
    }
}

impl std::error::Error for AppError {}

/// `AppError` 與 `L10n` 同為 `{code, params}`，可互轉（結果欄位用 L10n、錯誤用 AppError）。
impl From<AppError> for L10n {
    fn from(e: AppError) -> Self {
        L10n {
            code: e.code,
            params: e.params,
        }
    }
}

/// 未顯式編碼的 anyhow 錯誤自動包成 `error.unexpected`（保留原始明細於 `detail`）。
impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::new("error.unexpected").p("detail", e.to_string())
    }
}

/// 既有的 `Result<_, String>`（helper / `map_err(|e| e.to_string())`）自動包成
/// `error.unexpected`；高價值錯誤請改用顯式代碼（如 `AppError::new("brain.notFound")`）。
impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::new("error.unexpected").p("detail", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l10n_serializes_with_params() {
        let m = L10n::new("factory.writtenN").p("n", 3);
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["code"], "factory.writtenN");
        assert_eq!(v["params"]["n"], "3");
    }

    #[test]
    fn l10n_omits_empty_params() {
        let m = L10n::new("common.ok");
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["code"], "common.ok");
        assert!(v.get("params").is_none() || v["params"].as_object().unwrap().is_empty());
    }

    #[test]
    fn app_error_from_anyhow_is_unexpected_with_detail() {
        let e: AppError = anyhow::anyhow!("boom").into();
        assert_eq!(e.code, "error.unexpected");
        assert_eq!(e.params.get("detail").unwrap(), "boom");
    }

    #[test]
    fn app_error_from_string_is_unexpected_with_detail() {
        let e: AppError = String::from("舊字串錯誤").into();
        assert_eq!(e.code, "error.unexpected");
        assert_eq!(e.params.get("detail").unwrap(), "舊字串錯誤");
    }

    #[test]
    fn app_error_display() {
        let e = AppError::new("llm.noApiKey").p("provider", "groq");
        assert_eq!(e.to_string(), "llm.noApiKey (provider=groq)");
        assert_eq!(AppError::new("common.ok").to_string(), "common.ok");
    }

    #[test]
    fn app_error_serializes_to_code_params() {
        let e = AppError::new("factory.unknown").p("factory", "x");
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["code"], "factory.unknown");
        assert_eq!(v["params"]["factory"], "x");
    }
}
