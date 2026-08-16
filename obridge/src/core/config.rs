//! Obridge 設定（`obridge.toml`）——與 Operoid 的 app-settings.json 完全分離。
//!
//! 通道＝「傳輸實作」：`[[channels]]` 每項一個通道實例（可多個 email-imap 並存），
//! `source` 是**實例自訂標籤**（Operoid 端 send 分派依此反查，需全 obridge 唯一）。

use serde::Deserialize;

/// 頂層設定。
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub operoid: OperoidCfg,
    pub listen: ListenCfg,
    #[serde(default)]
    pub channels: Vec<ChannelCfg>,
}

/// Operoid ingress 位置與認證（進氣方向）。
#[derive(Debug, Clone, Deserialize)]
pub struct OperoidCfg {
    /// Operoid `POST /event` 完整 URL（含 port，如 `http://127.0.0.1:17341/event`）。
    pub ingress_url: String,
    pub ingress_secret: String,
}

/// Obridge send endpoint（出氣方向：Operoid `event_outbound_url` 指向這裡）。
#[derive(Debug, Clone, Deserialize)]
pub struct ListenCfg {
    pub port: u16,
    pub secret: String,
}

/// 一個通道實例。
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelCfg {
    /// 通道類型。v1 僅 `"email-imap"`；未來 `"wasm"`（外掛）。
    #[serde(rename = "type")]
    pub channel_type: String,
    /// 實例自訂 source 標籤（契約欄位；Operoid 不透明使用）。預設 "email"。
    #[serde(default = "default_source")]
    pub source: String,
    /// email-imap 專屬設定。
    #[serde(default)]
    pub email_imap: Option<EmailImapCfg>,
}

fn default_source() -> String {
    "email".into()
}

/// email-imap 通道設定。
#[derive(Debug, Clone, Deserialize)]
pub struct EmailImapCfg {
    pub imap: ImapCfg,
    pub smtp: SmtpCfg,
    /// poll 間隔（秒），預設 60。
    #[serde(default = "default_poll")]
    pub poll_secs: u64,
    /// 去重狀態檔位置（預設與設定檔同目錄 `<source>-state.json`）。
    pub state_file: Option<String>,
    /// 路由對映：收件地址 → employee_id（可多筆；一信命中多員工則多投）。
    #[serde(default)]
    pub routes: Vec<RouteCfg>,
    /// 寄件身分：employee_id → From 地址。
    #[serde(default)]
    pub senders: Vec<SenderCfg>,
}

fn default_poll() -> u64 {
    60
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImapCfg {
    pub host: String,
    #[serde(default = "default_imap_port")]
    pub port: u16,
    pub username: String,
    pub password: String,
    #[serde(default = "default_folder")]
    pub folder: String,
}

fn default_imap_port() -> u16 {
    993
}

fn default_folder() -> String {
    "INBOX".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct SmtpCfg {
    pub host: String,
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    pub username: String,
    pub password: String,
    /// 回覆信的預設主旨（無法從錨點還原原主旨——無狀態設計的取捨；thread 由
    /// In-Reply-To/References 維持）。預設 "Operoid"。
    #[serde(default = "default_subject")]
    pub subject: String,
}

fn default_smtp_port() -> u16 {
    465
}

fn default_subject() -> String {
    "Operoid".into()
}

/// 路由：`address`（收件地址，比對 To/Cc，大小寫不敏感）→ `employee` 或 `brain`（擇一）。
#[derive(Debug, Clone, Deserialize)]
pub struct RouteCfg {
    pub address: String,
    pub employee: Option<String>,
    pub brain: Option<String>,
}

/// 寄件身分：`employee` → From 地址（＋顯名）。
#[derive(Debug, Clone, Deserialize)]
pub struct SenderCfg {
    pub employee: String,
    pub address: String,
    pub name: Option<String>,
}

/// 解析 `obridge.toml`（含 source 標籤唯一性檢查——Operoid send 分派的反查鍵）。
pub fn parse(toml_str: &str) -> anyhow::Result<Config> {
    let cfg: Config = toml::from_str(toml_str)?;
    let mut seen = std::collections::HashSet::new();
    for ch in &cfg.channels {
        if !seen.insert(ch.source.as_str()) {
            anyhow::bail!("通道 source 標籤重複：{}", ch.source);
        }
        if ch.channel_type == "email-imap" && ch.email_imap.is_none() {
            anyhow::bail!("通道 {}（email-imap）缺少 [channels.email_imap] 設定", ch.source);
        }
    }
    Ok(cfg)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const SAMPLE: &str = r#"
        [operoid]
        ingress_url = "http://127.0.0.1:17341/event"
        ingress_secret = "s1"

        [listen]
        port = 17401
        secret = "s2"

        [[channels]]
        type = "email-imap"
        source = "email"
        [channels.email_imap]
        poll_secs = 30
        [channels.email_imap.imap]
        host = "imap.corp.com"
        username = "bot@corp.com"
        password = "pw"
        [channels.email_imap.smtp]
        host = "smtp.corp.com"
        username = "bot@corp.com"
        password = "pw"
        [[channels.email_imap.routes]]
        address = "steve@corp.com"
        employee = "Steve-TW"
        [[channels.email_imap.senders]]
        employee = "Steve-TW"
        address = "steve@corp.com"
    "#;

    #[test]
    fn sample_parses_with_defaults() {
        let cfg = parse(SAMPLE).unwrap();
        assert_eq!(cfg.channels.len(), 1);
        let ch = &cfg.channels[0];
        assert_eq!(ch.source, "email");
        let e = ch.email_imap.as_ref().unwrap();
        assert_eq!(e.imap.port, 993);
        assert_eq!(e.imap.folder, "INBOX");
        assert_eq!(e.poll_secs, 30);
        assert_eq!(e.smtp.subject, "Operoid");
        assert_eq!(e.routes[0].employee.as_deref(), Some("Steve-TW"));
    }

    #[test]
    fn duplicate_source_rejected() {
        let dup = format!(
            "{SAMPLE}\n[[channels]]\ntype = \"email-imap\"\nsource = \"email\"\n\
             [channels.email_imap.imap]\nhost = \"h\"\nusername = \"u\"\npassword = \"p\"\n\
             [channels.email_imap.smtp]\nhost = \"h\"\nusername = \"u\"\npassword = \"p\"\n"
        );
        assert!(parse(&dup).is_err(), "重複 source 應被拒");
    }

    #[test]
    fn email_imap_without_section_rejected() {
        // 只有 [[channels]] type="email-imap"、無 [channels.email_imap.*] 任何表。
        let bad = r#"
            [operoid]
            ingress_url = "http://127.0.0.1:1/event"
            ingress_secret = "s"
            [listen]
            port = 1
            secret = "s"
            [[channels]]
            type = "email-imap"
            source = "email"
        "#;
        assert!(parse(bad).is_err());
    }
}
