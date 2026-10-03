//! operoid.toml（遠端化計畫 R1，DR-E6）——**企業模式服務組態**。
//!
//! **存在即企業模式**：settings 目錄下有 `operoid.toml` → oserver 以它為服務層
//! 組態（bind／TLS／frontends／LLM env／gbrain／ingress 覆寫）；沒有 → 個人模式
//! （回落 `app-settings.json`，行為零變化）。解析失敗 → **明確報錯退出**（非靜默）。
//!
//! 範例（註解即文件）：
//!
//! ```toml
//! [server]
//! host = "0.0.0.0"             # 內網 IP 或 0.0.0.0；非 loopback 必須啟用 [tls]（DR-E5 fail-closed）
//! port = 7340                  # 可省（預設 7340）
//! token = "okt-master-..."     # master token；可省（改用 OSERVER_TOKEN env）
//! frontends_dir = "./frontends" # 可省：三前端靜態檔目錄（內含 admin/ manager/ user/ 子目錄）
//!
//! [tls]
//! cert = "./certs/server.crt"  # PEM；cert 與 key 齊備才啟用 TLS（半配置 → 拒絕啟動）
//! key = "./certs/server.key"
//!
//! [llm.env]                    # 服務端自填 provider keys（差距清單第 3 條）；
//! ZHIPUAI_API_KEY = "..."      # 覆蓋 app-settings.json 的 llm_env 同名鍵
//!
//! [gbrain]                     # 可省；覆寫 app-settings.json 同名欄位
//! exe_path = "C:/gbrain/gbrain.exe"
//!
//! [ingress]                    # 可省；覆寫 event_ingress_port / event_ingress_secret
//! port = 7341
//! secret = "..."
//! ```
//!
//! 熱生效邊界：`host`/`port`/`tls`/`frontends_dir` 屬**啟動期**（改檔須重啟服務）；
//! `[llm] env`／`[gbrain]`／`[ingress]` 經 `config::load_effective` 隨 scheduler
//! 每輪重讀（與 app-settings.json 同一批）。

use std::path::Path;

use serde::{Deserialize, Serialize};

use ocore::app_config::AppConfig;

/// operoid.toml 的根結構（全節省略 → 全預設：無覆寫、bind 127.0.0.1、無 TLS）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OperoidToml {
    pub server: ServerSection,
    pub tls: TlsSection,
    pub llm: LlmSection,
    pub gbrain: GbrainSection,
    pub ingress: IngressSection,
}

/// `[server]`——bind 位址／port／master token／前端靜態檔目錄。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerSection {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub token: Option<String>,
    pub frontends_dir: Option<String>,
}

/// `[tls]`——rustls PEM 憑證。cert 與 key 齊備才啟用；只設其一為**配置錯誤**。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TlsSection {
    pub cert: Option<String>,
    pub key: Option<String>,
}

impl TlsSection {
    /// None＝未啟用；Some＝(cert, key) 路徑。半配置明確報錯（fail-closed）。
    pub fn validate(&self) -> anyhow::Result<Option<(String, String)>> {
        match (&self.cert, &self.key) {
            (None, None) => Ok(None),
            (Some(c), Some(k)) => Ok(Some((c.clone(), k.clone()))),
            (Some(_), None) => anyhow::bail!("operoid.toml [tls]：設了 cert 缺 key——兩者必須同時設定"),
            (None, Some(_)) => anyhow::bail!("operoid.toml [tls]：設了 key 缺 cert——兩者必須同時設定"),
        }
    }
}

/// `[llm]`——服務端自填的 provider key 環境變數（覆蓋 app-settings.json 同名鍵）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmSection {
    pub env: std::collections::BTreeMap<String, String>,
}

/// `[gbrain]`——伺服器機的 gbrain 執行檔路徑覆寫。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GbrainSection {
    pub exe_path: Option<String>,
}

/// `[ingress]`——obridge 投遞口（event_ingress_port/secret 覆寫）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IngressSection {
    pub port: Option<u16>,
    pub secret: Option<String>,
}

/// 讀 settings 目錄下的 `operoid.toml`。檔案不存在 → None（個人模式）；
/// **存在但解析失敗 → Err**（啟動期明確報錯，不靜默退預設）。
pub fn load(settings_dir: &Path) -> anyhow::Result<Option<OperoidToml>> {
    let path = settings_dir.join("operoid.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let t: OperoidToml = toml::from_str(&text)
        .map_err(|e| anyhow::anyhow!("operoid.toml 解析失敗（{}）：{e}", path.display()))?;
    Ok(Some(t))
}

/// 將 toml 覆寫疊加到 AppConfig（llm_env 合併在此之外做——見 config::load_effective）。
pub fn apply_overrides(cfg: &mut AppConfig, t: Option<&OperoidToml>) {
    let Some(t) = t else { return };
    if let Some(p) = &t.gbrain.exe_path {
        cfg.gbrain_exe_path = p.clone();
    }
    if let Some(p) = t.ingress.port {
        cfg.event_ingress_port = Some(p);
    }
    if let Some(s) = &t.ingress.secret {
        cfg.event_ingress_secret = Some(s.clone());
    }
}

/// loopback 判定（fail-closed 的「本機」集合）。
pub fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// DR-E5 傳輸安全紀律：非 loopback bind **必須**啟用 TLS（Bearer 不得明文過網路）。
pub fn ensure_transport_safe(host: &str, tls_enabled: bool) -> anyhow::Result<()> {
    if !is_loopback(host) && !tls_enabled {
        anyhow::bail!(
            "遠端化安全紀律（DR-E5）：bind host「{host}」非 loopback，Bearer token 不得走明文網路\
             ——請在 operoid.toml 設定 [tls]（cert＋key），或改 bind 127.0.0.1"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[server]
host = "0.0.0.0"
port = 7340
token = "okt-master-abc"
frontends_dir = "./frontends"

[tls]
cert = "./certs/server.crt"
key = "./certs/server.key"

[llm.env]
ZHIPUAI_API_KEY = "key-1"
OPENAI_API_KEY = "key-2"

[gbrain]
exe_path = "C:/gbrain/gbrain.exe"

[ingress]
port = 7341
secret = "ing-secret"
"#;

    #[test]
    fn parses_sample() {
        let t: OperoidToml = toml::from_str(SAMPLE).unwrap();
        assert_eq!(t.server.host.as_deref(), Some("0.0.0.0"));
        assert_eq!(t.server.port, Some(7340));
        assert_eq!(t.server.token.as_deref(), Some("okt-master-abc"));
        assert_eq!(t.server.frontends_dir.as_deref(), Some("./frontends"));
        let (cert, key) = t.tls.validate().unwrap().unwrap();
        assert_eq!(cert, "./certs/server.crt");
        assert_eq!(key, "./certs/server.key");
        assert_eq!(t.llm.env.get("ZHIPUAI_API_KEY").map(String::as_str), Some("key-1"));
        assert_eq!(t.gbrain.exe_path.as_deref(), Some("C:/gbrain/gbrain.exe"));
        assert_eq!(t.ingress.port, Some(7341));
        assert_eq!(t.ingress.secret.as_deref(), Some("ing-secret"));
    }

    /// round-trip（驗收條件）：serialize → deserialize 恆等。
    #[test]
    fn round_trips() {
        let t: OperoidToml = toml::from_str(SAMPLE).unwrap();
        let text = toml::to_string(&t).unwrap();
        let back: OperoidToml = toml::from_str(&text).unwrap();
        assert_eq!(t, back);
        // 空結構也應 round-trip（全節省略的合法檔）。
        let empty: OperoidToml = toml::from_str("").unwrap();
        let text = toml::to_string(&empty).unwrap();
        let back: OperoidToml = toml::from_str(&text).unwrap();
        assert_eq!(empty, back);
    }

    #[test]
    fn malformed_is_error_not_default() {
        let r: Result<OperoidToml, _> = toml::from_str("host = [not a table");
        assert!(r.is_err(), "壞 toml 必須報錯（啟動期明確失敗）");
        // 型別錯誤同樣要報錯（port 是數字）。
        let r: Result<OperoidToml, _> = toml::from_str("[server]\nport = \"abc\"");
        assert!(r.is_err(), "port 型別錯誤必須報錯");
    }

    #[test]
    fn tls_half_config_is_error() {
        assert!(TlsSection { cert: None, key: None }.validate().unwrap().is_none());
        assert!(TlsSection { cert: Some("c".into()), key: Some("k".into()) }
            .validate()
            .unwrap()
            .is_some());
        assert!(TlsSection { cert: Some("c".into()), key: None }.validate().is_err());
        assert!(TlsSection { cert: None, key: Some("k".into()) }.validate().is_err());
    }

    #[test]
    fn transport_safety_fail_closed() {
        assert!(ensure_transport_safe("127.0.0.1", false).is_ok());
        assert!(ensure_transport_safe("localhost", false).is_ok());
        assert!(ensure_transport_safe("::1", false).is_ok());
        // 非 loopback 無 TLS → 拒絕；有 TLS → 通過。
        assert!(ensure_transport_safe("0.0.0.0", false).is_err());
        assert!(ensure_transport_safe("192.168.1.10", false).is_err());
        assert!(ensure_transport_safe("0.0.0.0", true).is_ok());
    }

    #[test]
    fn overrides_apply_to_app_config() {
        let t: OperoidToml = toml::from_str(SAMPLE).unwrap();
        let mut cfg = AppConfig::default();
        apply_overrides(&mut cfg, Some(&t));
        assert_eq!(cfg.gbrain_exe_path, "C:/gbrain/gbrain.exe");
        assert_eq!(cfg.event_ingress_port, Some(7341));
        assert_eq!(cfg.event_ingress_secret.as_deref(), Some("ing-secret"));
        // None → 零改動。
        let mut cfg2 = AppConfig::default();
        let before = cfg2.gbrain_exe_path.clone();
        apply_overrides(&mut cfg2, None);
        assert_eq!(cfg2.gbrain_exe_path, before);
    }

    /// load()：缺檔 → None；存在 → Some；壞檔 → Err（模擬 settings 目錄）。
    #[test]
    fn load_from_dir() {
        let tmp = std::env::temp_dir().join(format!(
            "otoml-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        assert!(load(&tmp).unwrap().is_none(), "缺檔 → 個人模式");
        std::fs::write(tmp.join("operoid.toml"), SAMPLE).unwrap();
        let t = load(&tmp).unwrap().unwrap();
        assert_eq!(t.server.host.as_deref(), Some("0.0.0.0"));
        std::fs::write(tmp.join("operoid.toml"), "bad = [").unwrap();
        assert!(load(&tmp).is_err(), "壞檔 → 明確報錯");
        std::fs::remove_dir_all(&tmp).ok();
    }
}
