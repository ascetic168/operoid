//! obridge（Operoid Bridge）企業模式管理——表單式設定 API＋子行程代管。
//!
//! 企業包內建 obridge 執行檔（sibling of oserver.exe）；本模組讓 admin 前端以
//! **表單**（非 raw TOML，對比桌面 ServicesTab 的純編輯器）管理 obridge：
//!
//! - **設定模型**：`obridge.toml` 的 `[operoid]`/`[listen]`/email-imap 頻道子集 ↔
//!   JSON 模型。讀寫以 `toml::Value` splice 往返——wasm 頻道、無法解析的頻道與
//!   未知頂層鍵**原樣保留**（round-trip 不破壞手寫設定），表單只重產生它管的區塊。
//! - **密鑰遮蔽**：API 不回傳任何密碼/密鑰——`null`＋`has_*` 旗標，表單留空＝保留
//!   原值；managed 模式的 ingress secret（server token）僅伺服器端寫入設定檔，
//!   **零回傳**。
//! - **存檔驗證**：寫暫存檔 → `obridge --check`（obridge 自己的解析規則當單一真相；
//!   exe 不存在則略過）→ 原子改名。
//! - **子行程代管**（沿桌面殼 `src-tauri/obridge_cfg.rs` 模式）：`obridge_autostart`
//!   開啟時 spawn（**僅設定檔存在時**——obridge 缺檔會寫範本後 exit 1）；存檔後
//!   `[listen]`/`[operoid]` 有變才重啟（頻道走 obridge 自身的 2s mtime 熱載入）；
//!   關機收尾 kill；stderr 導入 `obridge.log`（服務行程下部署者唯一的 log 來源）。
//! - **回信閉環**：managed 存檔同步 AppConfig 的 `event_outbound_url`/
//!   `event_outbound_secret` 指向 obridge send endpoint——漏了這步 oserver 的
//!   回信永遠出不去。（遠端橋接模式 obridge 的 `[listen]` 僅綁 127.0.0.1，跨機
//!   回信本就不可達，outbound 設定歸部署者，本模組不代管。）
//!
//! **模式分際**：行程代管**僅限企業模式**（settings 目錄有 `operoid.toml`）。
//! 個人模式的 obridge 由桌面殼（src-tauri `obridge_cfg.rs`）代管——oserver 若也
//! spawn，會與 GUI 讀同一份 `obridge_autostart`＋同一個 app-data 目錄的同一份
//! 設定檔，變成雙 obridge 行程（雙重 IMAP 輪詢＋send endpoint 綁定衝突）。

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use ocore::app_config::AppConfig;
use ocore::i18n::AppError;

use crate::routes::{ServerState, cors_layer, err_response, require_auth};

/// 企業模式判定：settings 目錄有 `operoid.toml`（與 main.rs 的模式判準一致）。
/// toml 存在但解析失敗也視為企業模式（fail-closed——該狀態下啟動本就會退出）。
fn is_enterprise(settings_dir: &Path) -> bool {
    !matches!(crate::operoid_toml::load(settings_dir), Ok(None))
}

/// obridge 設定檔慣例路徑：`<settings_dir>/obridge/obridge.toml`
/// （與桌面 `<app-data>/obridge/obridge.toml` 同構；state 檔亦落同目錄）。
pub fn config_path(settings_dir: &Path) -> PathBuf {
    settings_dir.join("obridge").join("obridge.toml")
}

fn invalid(detail: impl std::fmt::Display) -> AppError {
    AppError::new("obridge.configInvalid").p("detail", detail.to_string())
}

// ── 設定模型（表單 JSON ↔ obridge.toml 子集）─────────────────────────

pub const DEFAULT_LISTEN_PORT: u16 = 17401;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObridgeConfigModel {
    #[serde(default)]
    pub autostart: bool,
    /// `"managed"`（同機代管：oserver 自填 ingress url＋secret）｜`"manual"`（遠端橋接：手填）。
    #[serde(default = "default_managed")]
    pub ingress_mode: String,
    /// manual 模式的 ingress 完整 URL；managed 模式由伺服器端填入（讀取時不回傳）。
    #[serde(default)]
    pub ingress_url: Option<String>,
    /// None／空字串＝保留原值（遮蔽語意）。
    #[serde(default)]
    pub ingress_secret: Option<String>,
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    /// None／空字串＝保留原值；首次設定留空 → 伺服器生成 CSPRNG。
    #[serde(default)]
    pub listen_secret: Option<String>,
    #[serde(default)]
    pub channels: Vec<EmailChannelModel>,
    /// 表單未涵蓋、保留於設定檔的頻道（wasm 等）——唯讀資訊（skip_deserializing：
    /// 寫入時一律由伺服器端從現有檔重算，不收用戶端值）。
    #[serde(default, skip_deserializing)]
    pub preserved_channels: Vec<PreservedChannel>,
}

impl Default for ObridgeConfigModel {
    fn default() -> Self {
        Self {
            autostart: false,
            ingress_mode: "managed".into(),
            ingress_url: None,
            ingress_secret: None,
            listen_port: DEFAULT_LISTEN_PORT,
            listen_secret: None,
            channels: Vec::new(),
            preserved_channels: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreservedChannel {
    pub source: String,
    pub channel_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailChannelModel {
    pub source: String,
    #[serde(default = "default_poll")]
    pub poll_secs: u64,
    pub imap: ImapModel,
    pub smtp: SmtpModel,
    #[serde(default)]
    pub routes: Vec<RouteModel>,
    #[serde(default)]
    pub senders: Vec<SenderModel>,
}

/// 遮蔽欄位：`password: None`＝保留原值；`has_password` 僅讀取面資訊（寫入面忽略）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapModel {
    pub host: String,
    #[serde(default = "d993")]
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default = "d_folder")]
    pub folder: String,
    #[serde(default)]
    pub tls_insecure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpModel {
    pub host: String,
    #[serde(default = "d465")]
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default = "d_subject")]
    pub subject: String,
    #[serde(default)]
    pub tls_insecure: bool,
}

/// 路由：收件地址 → 員工**或**腦（擇一；與 obridge RouteCfg 同規則）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteModel {
    pub address: String,
    #[serde(default)]
    pub employee: Option<String>,
    #[serde(default)]
    pub brain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderModel {
    pub employee: String,
    pub address: String,
    #[serde(default)]
    pub name: Option<String>,
}

fn default_managed() -> String {
    "managed".into()
}
fn default_listen_port() -> u16 {
    DEFAULT_LISTEN_PORT
}
fn default_poll() -> u64 {
    60
}
fn d993() -> u16 {
    993
}
fn d465() -> u16 {
    465
}
fn d_folder() -> String {
    "INBOX".into()
}
fn d_subject() -> String {
    "Operoid".into()
}

// ── toml 文件遍歷輔助 ────────────────────────────────────────────────

/// 可被表單編輯的 email-imap 頻道（type 正確**且** [channels.email_imap] 可解析）。
/// 其餘（wasm、缺區塊的殘缺條目、無 type 的未知條目）一律視為 preserved 原樣保留。
fn editable_email(ch: &toml::Value) -> bool {
    ch.as_table()
        .map(|t| {
            t.get("type").and_then(|v| v.as_str()) == Some("email-imap")
                && t.get("email_imap").map(|v| v.is_table()).unwrap_or(false)
        })
        .unwrap_or(false)
}

/// 非 editable 頻道 → preserved 清單（表單唯讀顯示＋寫入時原樣保留）。
fn preserved_from_doc(doc: &toml::Value) -> Vec<PreservedChannel> {
    doc.get("channels")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter(|ch| !editable_email(ch))
                .filter_map(|ch| {
                    let t = ch.as_table()?;
                    Some(PreservedChannel {
                        source: t
                            .get("source")
                            .and_then(|v| v.as_str())
                            .unwrap_or("(未命名)")
                            .to_string(),
                        channel_type: t
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("(未知類型)")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn old_secret(doc: Option<&toml::Value>, path: &[&str]) -> Option<String> {
    let mut cur = doc?;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_str().filter(|s| !s.is_empty()).map(String::from)
}

/// 舊檔中（依 source 對應）email 頻道的密碼——留空＝保留的依據。
fn old_channel_secret(
    doc: Option<&toml::Value>,
    source: &str,
    side: &str,
    key: &str,
) -> Option<String> {
    let channels = doc?.get("channels")?.as_array()?;
    for ch in channels {
        if !editable_email(ch) {
            continue;
        }
        let t = ch.as_table().expect("editable_email 已檢查 table");
        if t.get("source").and_then(|v| v.as_str()).unwrap_or("email") != source {
            continue;
        }
        if let Some(v) = t
            .get("email_imap")?
            .get(side)?
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(v.to_string());
        }
    }
    None
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

// ── 讀取：現有 toml → 模型（密鑰遮蔽）────────────────────────────────

/// 無檔 → 空白模型（managed、預設 listen）；有檔 → 解析抽出管理的區塊。
/// managed 判定：ingress_url ≡ `http://127.0.0.1:<server_port>/event` 且
/// ingress_secret ≡ server token（與寫入面 managed 填值互為鏡像）。
pub fn load_model(
    cfg_path: &Path,
    server_port: u16,
    server_token: Option<&str>,
) -> Result<ObridgeConfigModel, AppError> {
    let text = match std::fs::read_to_string(cfg_path) {
        Ok(t) => t,
        Err(_) => return Ok(ObridgeConfigModel::default()),
    };
    let doc: toml::Value = toml::from_str(&text).map_err(|e| invalid(e))?;
    let mut model = ObridgeConfigModel {
        preserved_channels: preserved_from_doc(&doc),
        ..ObridgeConfigModel::default()
    };
    if let Some(op) = doc.get("operoid").and_then(|v| v.as_table()) {
        let url = op.get("ingress_url").and_then(|v| v.as_str()).unwrap_or("");
        let secret = op
            .get("ingress_secret")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if url == format!("http://127.0.0.1:{server_port}/event")
            && !secret.is_empty()
            && server_token == Some(secret)
        {
            model.ingress_mode = "managed".into();
        } else {
            model.ingress_mode = "manual".into();
            model.ingress_url = Some(url.to_string());
            model.ingress_secret = None; // 遮蔽
        }
    }
    if let Some(li) = doc.get("listen").and_then(|v| v.as_table()) {
        if let Some(p) = li.get("port").and_then(|v| v.as_integer()) {
            model.listen_port = p as u16;
        }
    }
    if let Some(channels) = doc.get("channels").and_then(|v| v.as_array()) {
        for ch in channels.iter().filter(|ch| editable_email(ch)) {
            let cht = ch.as_table().expect("editable_email 已檢查 table");
            let ei = cht.get("email_imap").and_then(|v| v.as_table()).expect("同上");
            let source = cht
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("email")
                .to_string();
            let imap_t = ei.get("imap").and_then(|v| v.as_table());
            let smtp_t = ei.get("smtp").and_then(|v| v.as_table());
            let str_of = |t: Option<&toml::Table>, k: &str| {
                t.and_then(|t| t.get(k))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let str_opt = |t: Option<&toml::Table>, k: &str| -> Option<String> {
                t.and_then(|t| t.get(k))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            };
            let num_of =
                |t: Option<&toml::Table>, k: &str, d: u16| -> u16 {
                    t.and_then(|t| t.get(k))
                        .and_then(|v| v.as_integer())
                        .map(|v| v as u16)
                        .unwrap_or(d)
                };
            let bool_of = |t: Option<&toml::Table>, k: &str| {
                t.and_then(|t| t.get(k))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            };
            let has_pw = |t: Option<&toml::Table>, k: &str| {
                t.and_then(|t| t.get(k))
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false)
            };
            model.channels.push(EmailChannelModel {
                source,
                poll_secs: ei
                    .get("poll_secs")
                    .and_then(|v| v.as_integer())
                    .map(|v| v as u64)
                    .unwrap_or(60),
                imap: ImapModel {
                    host: str_of(imap_t, "host"),
                    port: num_of(imap_t, "port", 993),
                    username: str_of(imap_t, "username"),
                    password: None,
                    has_password: has_pw(imap_t, "password"),
                    folder: str_of(imap_t, "folder"),
                    tls_insecure: bool_of(imap_t, "tls_insecure"),
                },
                smtp: SmtpModel {
                    host: str_of(smtp_t, "host"),
                    port: num_of(smtp_t, "port", 465),
                    username: str_of(smtp_t, "username"),
                    password: None,
                    has_password: has_pw(smtp_t, "password"),
                    subject: str_of(smtp_t, "subject"),
                    tls_insecure: bool_of(smtp_t, "tls_insecure"),
                },
                routes: ei
                    .get("routes")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|r| {
                                let t = r.as_table()?;
                                Some(RouteModel {
                                    address: str_of(Some(t), "address"),
                                    employee: str_opt(Some(t), "employee"),
                                    brain: str_opt(Some(t), "brain"),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                senders: ei
                    .get("senders")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|r| {
                                let t = r.as_table()?;
                                Some(SenderModel {
                                    employee: str_of(Some(t), "employee"),
                                    address: str_of(Some(t), "address"),
                                    name: str_opt(Some(t), "name"),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
    }
    Ok(model)
}

// ── 寫入：模型 → toml（驗證＋密鑰保留＋managed 填值）──────────────────

/// managed 模式伺服器端填入值（ingress url＋secret）——不出 API。
pub struct ManagedFill {
    pub ingress_url: String,
    pub ingress_secret: String,
}

/// 驗證＋正規化模型（與 obridge `config::parse` 規則同構：必填欄位、source 唯一
/// 含 preserved、route 擇一）。空字串密碼正規化為 None（＝保留原值）。
pub fn validate(model: &mut ObridgeConfigModel, old: Option<&toml::Value>) -> Result<(), AppError> {
    // 先正規化（空字串 → None＝保留），再驗證——語意才一致。
    model.ingress_secret = non_empty(model.ingress_secret.take());
    model.listen_secret = non_empty(model.listen_secret.take());
    model.ingress_url = non_empty(model.ingress_url.take());
    for ch in &mut model.channels {
        ch.source = ch.source.trim().to_string();
        ch.imap.password = non_empty(ch.imap.password.take());
        ch.smtp.password = non_empty(ch.smtp.password.take());
        for r in &mut ch.routes {
            r.address = r.address.trim().to_string();
            r.employee = non_empty(r.employee.take());
            r.brain = non_empty(r.brain.take());
        }
        for s in &mut ch.senders {
            s.employee = s.employee.trim().to_string();
            s.address = s.address.trim().to_string();
            s.name = non_empty(s.name.take());
        }
    }

    match model.ingress_mode.as_str() {
        "managed" => {
            if model.ingress_secret.is_some() {
                return Err(invalid("managed 模式不接受手填 ingress secret"));
            }
        }
        "manual" => {
            let url = model.ingress_url.as_deref().unwrap_or("");
            if url.is_empty() {
                return Err(invalid("遠端橋接模式需填 ingress_url"));
            }
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err(invalid(format!("ingress_url 需為 http(s) URL：{url}")));
            }
            if model.ingress_secret.is_none()
                && old_secret(old, &["operoid", "ingress_secret"]).is_none()
            {
                return Err(invalid("遠端橋接模式需填 ingress_secret（首次設定不可留空）"));
            }
        }
        other => return Err(invalid(format!("未知 ingress_mode：{other}"))),
    }
    if !(1..=65535).contains(&model.listen_port) {
        return Err(invalid(format!("listen port 不合法：{}", model.listen_port)));
    }

    let mut sources = std::collections::HashSet::new();
    for ch in &model.channels {
        if ch.source.is_empty() {
            return Err(invalid("通道 source 不可空白"));
        }
        if !sources.insert(ch.source.clone()) {
            return Err(invalid(format!("通道 source 標籤重複：{}", ch.source)));
        }
        if ch.poll_secs == 0 {
            return Err(invalid(format!("通道 {} poll_secs 不可為 0", ch.source)));
        }
        if ch.imap.host.is_empty() || ch.imap.username.is_empty() {
            return Err(invalid(format!("通道 {} IMAP host/username 必填", ch.source)));
        }
        if ch.smtp.host.is_empty() || ch.smtp.username.is_empty() {
            return Err(invalid(format!("通道 {} SMTP host/username 必填", ch.source)));
        }
        if ch.imap.password.is_none()
            && old_channel_secret(old, &ch.source, "imap", "password").is_none()
        {
            return Err(invalid(format!(
                "通道 {} IMAP 密碼必填（首次設定不可留空）",
                ch.source
            )));
        }
        if ch.smtp.password.is_none()
            && old_channel_secret(old, &ch.source, "smtp", "password").is_none()
        {
            return Err(invalid(format!(
                "通道 {} SMTP 密碼必填（首次設定不可留空）",
                ch.source
            )));
        }
        for r in &ch.routes {
            if r.address.is_empty() {
                return Err(invalid(format!("通道 {} 路由 address 必填", ch.source)));
            }
            if r.employee.is_some() == r.brain.is_some() {
                return Err(invalid(format!(
                    "通道 {} 路由 {} 需擇一填 employee 或 brain",
                    ch.source, r.address
                )));
            }
        }
        for s in &ch.senders {
            if s.employee.is_empty() || s.address.is_empty() {
                return Err(invalid(format!(
                    "通道 {} 寄件身分 employee/address 必填",
                    ch.source
                )));
            }
        }
    }
    if let Some(old) = old {
        for p in preserved_from_doc(old) {
            if sources.contains(&p.source) {
                return Err(invalid(format!(
                    "通道 source 標籤與既有 {} 頻道重複：{}",
                    p.channel_type, p.source
                )));
            }
            sources.insert(p.source);
        }
    }
    Ok(())
}

/// 模型 → 完整 toml 文件。回傳（文件, 最終 listen secret——AppConfig 回信同步用）。
/// 保留 preserved 頻道與未知頂層鍵；None 密碼從舊檔帶回；managed 由 fill 填 ingress。
pub fn build_doc(
    model: &ObridgeConfigModel,
    old: Option<&toml::Value>,
    fill: Option<&ManagedFill>,
) -> Result<(toml::Value, String), AppError> {
    let mut doc = match old {
        Some(o @ toml::Value::Table(_)) => o.clone(),
        _ => toml::Value::Table(toml::map::Map::new()),
    };
    {
        let t = doc.as_table_mut().expect("table");
        // 頂層鍵：operoid/listen/channels 由本模組接管，其餘（未知鍵）保留。
        t.remove("operoid");
        t.remove("listen");
        t.remove("channels");
    }

    // ingress：managed → 伺服器端填；manual → 模型值（None＝舊值）。
    let (ingress_url, ingress_secret) = match model.ingress_mode.as_str() {
        "managed" => {
            let f = fill.ok_or_else(|| AppError::new("obridge.noServerToken"))?;
            (f.ingress_url.clone(), f.ingress_secret.clone())
        }
        _ => {
            let secret = match &model.ingress_secret {
                Some(s) => s.clone(),
                None => old
                    .and_then(|o| old_secret(Some(o), &["operoid", "ingress_secret"]))
                    .ok_or_else(|| invalid("缺 ingress_secret"))?,
            };
            (
                model.ingress_url.clone().unwrap_or_default(),
                secret,
            )
        }
    };
    let mut operoid = toml::map::Map::new();
    operoid.insert("ingress_url".into(), toml::Value::String(ingress_url));
    operoid.insert("ingress_secret".into(), toml::Value::String(ingress_secret));

    let listen_secret = match &model.listen_secret {
        Some(s) => s.clone(),
        None => old
            .and_then(|o| old_secret(Some(o), &["listen", "secret"]))
            .unwrap_or_else(gen_secret),
    };
    let mut listen = toml::map::Map::new();
    listen.insert("port".into(), toml::Value::Integer(model.listen_port as i64));
    listen.insert("secret".into(), toml::Value::String(listen_secret.clone()));

    // channels：preserved（原樣）＋ email 頻道（重產生）。
    let mut channels: Vec<toml::Value> = Vec::new();
    if let Some(arr) = old.and_then(|o| o.get("channels")).and_then(|v| v.as_array()) {
        channels.extend(arr.iter().filter(|ch| !editable_email(ch)).cloned());
    }
    for ch in &model.channels {
        let imap_pw = match &ch.imap.password {
            Some(p) => p.clone(),
            None => old_channel_secret(old, &ch.source, "imap", "password").unwrap_or_default(),
        };
        let smtp_pw = match &ch.smtp.password {
            Some(p) => p.clone(),
            None => old_channel_secret(old, &ch.source, "smtp", "password").unwrap_or_default(),
        };
        let mut imap = toml::map::Map::new();
        imap.insert("host".into(), s_v(&ch.imap.host));
        imap.insert("port".into(), i_v(ch.imap.port as i64));
        imap.insert("username".into(), s_v(&ch.imap.username));
        imap.insert("password".into(), s_v(&imap_pw));
        imap.insert("folder".into(), s_v(&ch.imap.folder));
        imap.insert("tls_insecure".into(), b_v(ch.imap.tls_insecure));
        let mut smtp = toml::map::Map::new();
        smtp.insert("host".into(), s_v(&ch.smtp.host));
        smtp.insert("port".into(), i_v(ch.smtp.port as i64));
        smtp.insert("username".into(), s_v(&ch.smtp.username));
        smtp.insert("password".into(), s_v(&smtp_pw));
        smtp.insert("subject".into(), s_v(&ch.smtp.subject));
        smtp.insert("tls_insecure".into(), b_v(ch.smtp.tls_insecure));
        let mut ei = toml::map::Map::new();
        ei.insert("imap".into(), toml::Value::Table(imap));
        ei.insert("smtp".into(), toml::Value::Table(smtp));
        ei.insert("poll_secs".into(), i_v(ch.poll_secs as i64));
        let routes: Vec<toml::Value> = ch
            .routes
            .iter()
            .map(|r| {
                let mut t = toml::map::Map::new();
                t.insert("address".into(), s_v(&r.address));
                if let Some(e) = &r.employee {
                    t.insert("employee".into(), s_v(e));
                }
                if let Some(b) = &r.brain {
                    t.insert("brain".into(), s_v(b));
                }
                toml::Value::Table(t)
            })
            .collect();
        if !routes.is_empty() {
            ei.insert("routes".into(), toml::Value::Array(routes));
        }
        let senders: Vec<toml::Value> = ch
            .senders
            .iter()
            .map(|s| {
                let mut t = toml::map::Map::new();
                t.insert("employee".into(), s_v(&s.employee));
                t.insert("address".into(), s_v(&s.address));
                if let Some(n) = &s.name {
                    t.insert("name".into(), s_v(n));
                }
                toml::Value::Table(t)
            })
            .collect();
        if !senders.is_empty() {
            ei.insert("senders".into(), toml::Value::Array(senders));
        }
        let mut cht = toml::map::Map::new();
        cht.insert("type".into(), s_v("email-imap"));
        cht.insert("source".into(), s_v(&ch.source));
        cht.insert("email_imap".into(), toml::Value::Table(ei));
        channels.push(toml::Value::Table(cht));
    }

    {
        let t = doc.as_table_mut().expect("table");
        t.insert("operoid".into(), toml::Value::Table(operoid));
        t.insert("listen".into(), toml::Value::Table(listen));
        t.insert("channels".into(), toml::Value::Array(channels));
    }
    Ok((doc, listen_secret))
}

#[inline]
fn s_v(s: &str) -> toml::Value {
    toml::Value::String(s.to_string())
}
#[inline]
fn i_v(v: i64) -> toml::Value {
    toml::Value::Integer(v)
}
#[inline]
fn b_v(v: bool) -> toml::Value {
    toml::Value::Boolean(v)
}

/// `[listen]`/`[operoid]` 是否變更（熱載入只蓋頻道——這兩區塊變更需重啟子行程）。
pub fn sections_changed(old: Option<&toml::Value>, new: &toml::Value) -> bool {
    let key = |d: &toml::Value, k: &str| {
        d.get(k)
            .cloned()
            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()))
    };
    match old {
        None => true,
        Some(o) => {
            key(o, "operoid") != key(new, "operoid") || key(o, "listen") != key(new, "listen")
        }
    }
}

fn gen_secret() -> String {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).expect("OS 熵源不可用");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// 寫暫存 → `obridge --check`（exe 存在時；obridge 自身解析規則當單一真相）→ 原子改名。
/// 驗證子行程有 **10 秒逾時**——check 不退出（例如 obridge 執行檔損壞、旗標被舊版
/// 二進位無視）時 kill 並 fail-closed（保留舊檔），API 不會被卡死。
fn write_checked(cfg_path: &Path, doc: &toml::Value, exe: Option<&Path>) -> Result<(), AppError> {
    let text = toml::to_string_pretty(doc)
        .map_err(|e| AppError::new("obridge.configWriteFail").p("detail", e.to_string()))?;
    if let Some(dir) = cfg_path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| AppError::new("obridge.configWriteFail").p("detail", e.to_string()))?;
    }
    let tmp = PathBuf::from(format!("{}.tmp", cfg_path.display()));
    std::fs::write(&tmp, &text)
        .map_err(|e| AppError::new("obridge.configWriteFail").p("detail", e.to_string()))?;
    if let Some(exe) = exe {
        match check_config(exe, &tmp) {
            Ok(()) => {}
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
        }
    }
    std::fs::rename(&tmp, cfg_path)
        .map_err(|e| AppError::new("obridge.configWriteFail").p("detail", e.to_string()))?;
    Ok(())
}

/// 跑 `obridge --check --config <path>`（10s 逾時）。stderr 導回呼叫端供錯誤明細。
fn check_config(exe: &Path, cfg: &Path) -> Result<(), AppError> {
    let mut child = match std::process::Command::new(exe)
        .arg("--check")
        .arg("--config")
        .arg(cfg)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            // exe 在但跑不起來——不擋存檔（驗證是加分項），log 後繼續。
            eprintln!("[oserver] obridge --check 執行失敗（略過驗證）：{e}");
            return Ok(());
        }
    };
    // stderr 由執行緒持續排水（子行程結束／被殺時 read_to_string 自然收尾）。
    let stderr_shared = Arc::new(std::sync::Mutex::new(String::new()));
    if let Some(mut pipe) = child.stderr.take() {
        let sink = Arc::clone(&stderr_shared);
        std::thread::spawn(move || {
            use std::io::Read as _;
            let mut buf = String::new();
            let _ = pipe.read_to_string(&mut buf);
            *sink.lock().expect("stderr mutex") = buf;
        });
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                eprintln!("[oserver] obridge --check 逾時（10s）——kill 並拒絕存檔");
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::new("obridge.checkTimeout"));
            }
            Err(_) => break None, // 偵測失敗——比照逾時處置（fail-closed）
        }
    };
    match status {
        Some(st) if st.success() => Ok(()),
        Some(st) => {
            let detail = stderr_shared.lock().expect("stderr mutex").trim().to_string();
            Err(invalid(if detail.is_empty() {
                format!("obridge --check 未通過（{st}）")
            } else {
                detail.chars().take(500).collect::<String>()
            }))
        }
        None => Err(AppError::new("obridge.checkTimeout")),
    }
}

// ── 子行程代管（沿 src-tauri/obridge_cfg.rs 模式）────────────────────

static CHILD: Mutex<Option<std::process::Child>> = Mutex::new(None);

/// exe 解析：AppConfig 覆寫 → 現行執行檔同目錄（企業包 stage 佈局＝sibling）。
pub fn resolve_exe(cfg: &AppConfig) -> Option<PathBuf> {
    if let Some(p) = cfg
        .obridge_executable
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(windows) { "obridge.exe" } else { "obridge" };
    let p = dir.join(name);
    p.exists().then_some(p)
}

/// 跑 `obridge --version` 取回 build id（5s 逾時——防損壞的執行檔卡住呼叫端）。
/// 無法判定（exe 跑不起來／逾時／輸出無 build id）→ None。
fn obridge_build_id(exe: &Path) -> Option<String> {
    let mut child = std::process::Command::new(exe)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let exited = loop {
        match child.try_wait() {
            Ok(Some(_)) => break true,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            _ => break false,
        }
    };
    if !exited {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let mut out = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        use std::io::Read as _;
        let _ = pipe.read_to_string(&mut out);
    }
    let _ = child.wait();
    parse_build_id(&out)
}

/// 從 `obridge --version` 的輸出抽 build id（`obridge <ver> (build <hash> <time>)`）。
fn parse_build_id(version_output: &str) -> Option<String> {
    version_output
        .split("build ")
        .nth(1)?
        .split_whitespace()
        .next()
        .map(String::from)
}

/// 比對兩個 build id。任一端 `unknown`（無 git 建置）→ None（無從判定，不誤報）。
fn build_matches(own: &str, child_id: &str) -> Option<bool> {
    if own == "unknown" || child_id == "unknown" {
        return None;
    }
    Some(own == child_id)
}

/// spawn 前核對 obridge 的 build id——「跑到舊版二進位」在開發期是隱性事故
/// （sibling 解析只認路徑不認版本；Windows 鎖檔／artifact 重連結都會造成新舊混用）。
/// 判定不出來（None）不警告；**確定不一致**才大聲警告。
fn warn_on_build_mismatch(exe: &Path) {
    let Some(child_id) = obridge_build_id(exe) else { return };
    match build_matches(env!("OPEROID_BUILD_HASH"), &child_id) {
        Some(false) => eprintln!(
            "[oserver] ⚠️ 建置版本不一致：oserver build {}，obridge build {child_id}（{}）——obridge 是舊版二進位！請 `cargo dev-build`（或重建部署包）",
            env!("OPEROID_BUILD_HASH"),
            exe.display()
        ),
        _ => {}
    }
}

/// spawn 子行程（呼叫端保證設定檔已存在且 autostart 語意已驗）。
fn spawn_managed(cfg_path: &Path, cfg: &AppConfig) -> Result<u32, AppError> {
    {
        let mut g = CHILD.lock().expect("CHILD mutex");
        if let Some(ch) = g.as_mut() {
            match ch.try_wait() {
                // 仍在跑 → 拒絕重複 spawn；已退出/偵測失敗 → 清掉換新。
                Ok(None) => return Err(AppError::new("obridge.alreadyRunning")),
                _ => {
                    *g = None;
                }
            }
        }
    }
    let exe = resolve_exe(cfg).ok_or_else(|| AppError::new("obridge.exeNotFound"))?;
    warn_on_build_mismatch(&exe);
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("--config").arg(cfg_path);
    cmd.stdin(Stdio::null()).stdout(Stdio::null());
    // stderr → obridge.log（append）：服務行程（LocalSystem/systemd）下部署者唯一
    // 的 obridge 訊息來源（啟動摘要、poll 失敗、熱載入記錄）。
    let log = cfg_path
        .parent()
        .map(|d| d.join("obridge.log"))
        .and_then(|p| std::fs::OpenOptions::new().create(true).append(true).open(p).ok());
    match log {
        Some(f) => cmd.stderr(Stdio::from(f)),
        None => cmd.stderr(Stdio::null()),
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW——服務子行程不閃 console
    }
    let child = cmd
        .spawn()
        .map_err(|e| AppError::new("obridge.spawnFail").p("detail", e.to_string()))?;
    let pid = child.id();
    *CHILD.lock().expect("CHILD mutex") = Some(child);
    Ok(pid)
}

/// 關機收尾／存檔重啟前：kill＋reap（managed 實例；部署者手動起的不在管轄）。
pub fn kill_managed() {
    let mut g = CHILD.lock().expect("CHILD mutex");
    if let Some(mut ch) = g.take() {
        let _ = ch.kill();
        let _ = ch.wait();
        eprintln!("[oserver] obridge 子行程已停止");
    }
}

/// (running, pid)——僅反映**本行程代管**的實例；部署者手動起的不在內。
pub fn status_managed() -> (bool, Option<u32>) {
    let mut g = CHILD.lock().expect("CHILD mutex");
    match g.as_mut() {
        Some(ch) => match ch.try_wait() {
            Ok(Some(_)) => {
                *g = None;
                (false, None)
            }
            Ok(None) => (true, Some(ch.id())),
            Err(_) => {
                *g = None;
                (false, None)
            }
        },
        None => (false, None),
    }
}

/// 啟動期自動帶起（**企業模式限定**——個人模式的 obridge 歸桌面殼代管，oserver
/// 再 spawn 就是雙行程）；autostart 開＋設定檔存在才帶，失敗僅 log 不擋啟動。
pub fn autostart(settings_dir: &Path, cfg: &AppConfig) {
    if !is_enterprise(settings_dir) {
        return;
    }
    if !cfg.obridge_autostart {
        return;
    }
    let p = config_path(settings_dir);
    if !p.exists() {
        eprintln!(
            "[oserver] obridge autostart 略過：設定檔不存在（{}）——到 /admin 的 obridge 頁完成設定",
            p.display()
        );
        return;
    }
    match spawn_managed(&p, cfg) {
        Ok(pid) => eprintln!("[oserver] obridge 已啟動（pid={pid}，config={}）", p.display()),
        Err(e) => eprintln!("[oserver] obridge 啟動失敗：{}", e.code),
    }
}

fn restart_managed(cfg_path: &Path, cfg: &AppConfig) -> Result<u32, AppError> {
    kill_managed();
    spawn_managed(cfg_path, cfg)
}

// ── HTTP handlers ────────────────────────────────────────────────────

pub fn obridge_routes() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/obridge/status", get(api_status))
        .route("/api/obridge/config", get(api_get_config).put(api_put_config))
        .route("/api/obridge/restart", post(api_restart))
        .layer(cors_layer())
}

/// 存檔結果動作（前端據此顯示訊息）：
/// restarted（listen/operoid 變更→重啟）｜started（原本沒跑→帶起）｜
/// stopped（關閉 autostart→帶走）｜hot_reload（僅頻道變更→obridge 熱載入）｜none。
#[derive(Debug, Serialize)]
struct PutResult {
    action: &'static str,
    pid: Option<u32>,
}

async fn api_status(State(state): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let cfg = crate::config::load_config(&st.settings_dir);
        let exe = resolve_exe(&cfg);
        let (running, pid) = status_managed();
        let cfg_path = config_path(&st.settings_dir);
        // build id 核對（admin 介面顯示——舊版二進位混用的辨識點）；
        // exe 不存在／無法判定 → null。
        let exe_build_match = exe.as_ref().and_then(|p| {
            obridge_build_id(p).and_then(|id| build_matches(env!("OPEROID_BUILD_HASH"), &id))
        });
        Ok(json!({
            "autostart": cfg.obridge_autostart,
            "running": running,
            "pid": pid,
            "exe_found": exe.is_some(),
            "exe_path": exe.as_ref().map(|p| p.display().to_string()),
            "exe_build_match": exe_build_match,
            "config_path": cfg_path.display().to_string(),
            "config_exists": cfg_path.exists(),
        }))
    })
    .await;
    finish_json(res)
}

async fn api_get_config(State(state): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let cfg_path = config_path(&st.settings_dir);
        let mut model = load_model(&cfg_path, st.server_port, st.server_token.as_deref())?;
        // autostart 存 AppConfig（obridge.toml 沒有這個概念）——以服務設定為準。
        model.autostart = crate::config::load_config(&st.settings_dir).obridge_autostart;
        Ok(serde_json::to_value(model).unwrap_or(serde_json::Value::Null))
    })
    .await;
    finish_json(res)
}

async fn api_put_config(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Json(model): Json<ObridgeConfigModel>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let cfg_path = config_path(&st.settings_dir);
        let old: Option<toml::Value> = std::fs::read_to_string(&cfg_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok());
        let mut model = model;
        validate(&mut model, old.as_ref())?;

        // managed：伺服器端填 ingress（url 由本服務 port 推導；secret＝server token，
        // 只寫進設定檔、不回傳）。
        let fill = if model.ingress_mode == "managed" {
            Some(ManagedFill {
                ingress_url: format!("http://127.0.0.1:{}/event", st.server_port),
                ingress_secret: st
                    .server_token
                    .clone()
                    .ok_or_else(|| AppError::new("obridge.noServerToken"))?,
            })
        } else {
            None
        };
        let (doc, listen_secret) = build_doc(&model, old.as_ref(), fill.as_ref())?;
        let cfg = crate::config::load_config(&st.settings_dir);
        let exe = resolve_exe(&cfg);
        write_checked(&cfg_path, &doc, exe.as_deref())?;

        // AppConfig 同步：obridge_autostart 一併存；回信閉環（event_outbound_*）
        // 僅 managed 代管——遠端橋接的 [listen] 只綁 127.0.0.1，跨機回信歸部署者。
        {
            let mut c = cfg;
            c.obridge_autostart = model.autostart;
            if model.ingress_mode == "managed" {
                c.event_outbound_url = Some(format!("http://127.0.0.1:{}/send", model.listen_port));
                c.event_outbound_secret = Some(listen_secret);
            }
            crate::config::save_config(&st.settings_dir, &c)
                .map_err(|e| AppError::new("server.cfgSaveFail").p("detail", e.to_string()))?;
        }

        // 行程動作（**企業模式限定**）：重啟（listen/operoid 變更）＞帶起（沒跑＋
        // autostart）＞停止（關 autostart）＞熱載入（僅頻道變更，obridge 2s mtime
        // watch 自行重建）。個人模式由桌面殼代管 obridge 行程——oserver 只寫設定檔
        // （桌面 obridge 自身會 mtime 熱載入頻道），不碰行程。
        let enterprise = is_enterprise(&st.settings_dir);
        let changed = sections_changed(old.as_ref(), &doc);
        let (running, _) = status_managed();
        let (action, pid) = if !enterprise {
            ("none", None)
        } else if !model.autostart {
            if running {
                kill_managed();
                ("stopped", None)
            } else {
                ("none", None)
            }
        } else if running && changed {
            ("restarted", Some(restart_managed(&cfg_path, &crate::config::load_config(&st.settings_dir))?))
        } else if running {
            ("hot_reload", None)
        } else {
            ("started", Some(spawn_managed(&cfg_path, &crate::config::load_config(&st.settings_dir))?))
        };
        eprintln!(
            "[oserver] obridge 設定已存檔（action={action}，mode={}）→ {}",
            if enterprise { "enterprise" } else { "personal" },
            cfg_path.display()
        );
        Ok(PutResult { action, pid })
    })
    .await;
    finish_json(res)
}

async fn api_restart(State(state): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        // 個人模式的 obridge 行程歸桌面殼——oserver 不代管，重啟請用 GUI 設定頁。
        if !is_enterprise(&st.settings_dir) {
            return Err(AppError::new("obridge.notManaged"));
        }
        let cfg = crate::config::load_config(&st.settings_dir);
        if !cfg.obridge_autostart {
            return Err(AppError::new("obridge.notEnabled"));
        }
        let cfg_path = config_path(&st.settings_dir);
        if !cfg_path.exists() {
            return Err(invalid("設定檔不存在"));
        }
        let pid = restart_managed(&cfg_path, &cfg)?;
        Ok(json!({"ok": true, "pid": pid}))
    })
    .await;
    finish_json(res)
}

fn finish_json<T: Serialize>(
    res: Result<Result<T, AppError>, tokio::task::JoinError>,
) -> Response {
    match res {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

// ── 測試（純函式：validate／build_doc／sections_changed／load_model）──

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_DOC: &str = r#"
        [operoid]
        ingress_url = "http://127.0.0.1:7340/event"
        ingress_secret = "server-token"

        [listen]
        port = 17401
        secret = "old-listen-secret"

        [[channels]]
        type = "email-imap"
        source = "email"
        [channels.email_imap]
        poll_secs = 30
        [channels.email_imap.imap]
        host = "imap.corp.com"
        port = 993
        username = "bot@corp.com"
        password = "imap-pw"
        folder = "INBOX"
        [channels.email_imap.smtp]
        host = "smtp.corp.com"
        username = "bot@corp.com"
        password = "smtp-pw"
        subject = "Operoid"
        [[channels.email_imap.routes]]
        address = "steve@corp.com"
        employee = "Steve-TW"
        [[channels.email_imap.senders]]
        employee = "Steve-TW"
        address = "steve@corp.com"

        [[channels]]
        type = "wasm"
        source = "slack"
        [channels.wasm]
        plugin = "plugins/slack.wasm"
        poll_secs = 30
        [channels.wasm.config]
        token = "xoxb-secret"

        [unknown_top]
        key = "keep-me"
    "#;

    fn mk_channel(source: &str, imap_pw: Option<&str>, smtp_pw: Option<&str>) -> EmailChannelModel {
        EmailChannelModel {
            source: source.into(),
            poll_secs: 60,
            imap: ImapModel {
                host: "imap.corp.com".into(),
                port: 993,
                username: "bot@corp.com".into(),
                password: imap_pw.map(String::from),
                has_password: false,
                folder: "INBOX".into(),
                tls_insecure: false,
            },
            smtp: SmtpModel {
                host: "smtp.corp.com".into(),
                port: 465,
                username: "bot@corp.com".into(),
                password: smtp_pw.map(String::from),
                has_password: false,
                subject: "Operoid".into(),
                tls_insecure: false,
            },
            routes: vec![RouteModel {
                address: "steve@corp.com".into(),
                employee: Some("Steve-TW".into()),
                brain: None,
            }],
            senders: vec![SenderModel {
                employee: "Steve-TW".into(),
                address: "steve@corp.com".into(),
                name: None,
            }],
        }
    }

    fn parse_doc(s: &str) -> toml::Value {
        toml::from_str(s).unwrap()
    }

    fn managed_fill() -> ManagedFill {
        ManagedFill {
            ingress_url: "http://127.0.0.1:7340/event".into(),
            ingress_secret: "server-token".into(),
        }
    }

    /// round-trip：wasm 頻道與未知頂層鍵原樣保留；email 頻道重產生。
    #[test]
    fn build_doc_preserves_wasm_and_unknown_keys() {
        let old = parse_doc(OLD_DOC);
        let mut model = ObridgeConfigModel {
            ingress_mode: "managed".into(),
            listen_port: 17401,
            listen_secret: Some("new-listen".into()),
            channels: vec![mk_channel("email", Some("imap-pw2"), Some("smtp-pw2"))],
            ..Default::default()
        };
        validate(&mut model, Some(&old)).unwrap();
        let (doc, listen_secret) = build_doc(&model, Some(&old), Some(&managed_fill())).unwrap();
        assert_eq!(listen_secret, "new-listen");
        let channels = doc.get("channels").unwrap().as_array().unwrap();
        assert_eq!(channels.len(), 2, "wasm 頻道應保留");
        let slack = &channels[0];
        assert_eq!(slack.get("type").unwrap().as_str(), Some("wasm"));
        assert_eq!(
            slack
                .get("wasm")
                .unwrap()
                .get("config")
                .unwrap()
                .get("token")
                .unwrap()
                .as_str(),
            Some("xoxb-secret"),
            "wasm 任意設定區塊原樣保留"
        );
        assert_eq!(
            doc.get("unknown_top").unwrap().get("key").unwrap().as_str(),
            Some("keep-me"),
            "未知頂層鍵保留"
        );
        let email = &channels[1];
        assert_eq!(email.get("source").unwrap().as_str(), Some("email"));
        assert_eq!(
            email
                .get("email_imap")
                .unwrap()
                .get("poll_secs")
                .unwrap()
                .as_integer(),
            Some(60)
        );
    }

    /// 留空＝保留：password None 從舊檔帶回原值；listen secret 同理。
    #[test]
    fn keep_semantics_carries_old_secrets() {
        let old = parse_doc(OLD_DOC);
        let mut model = ObridgeConfigModel {
            ingress_mode: "managed".into(),
            listen_port: 17401,
            listen_secret: None,
            channels: vec![mk_channel("email", None, None)],
            ..Default::default()
        };
        validate(&mut model, Some(&old)).unwrap();
        let (doc, listen_secret) = build_doc(&model, Some(&old), Some(&managed_fill())).unwrap();
        assert_eq!(listen_secret, "old-listen-secret", "listen secret 保留");
        let channels = doc.get("channels").unwrap().as_array().unwrap();
        let email = channels
            .iter()
            .find(|c| c.get("type").and_then(|v| v.as_str()) == Some("email-imap"))
            .expect("email 頻道存在");
        let ei = email.get("email_imap").unwrap();
        assert_eq!(
            ei.get("imap").unwrap().get("password").unwrap().as_str(),
            Some("imap-pw")
        );
        assert_eq!(
            ei.get("smtp").unwrap().get("password").unwrap().as_str(),
            Some("smtp-pw")
        );
    }

    /// managed 填值寫入 operoid 區塊；空字串密碼正規化為保留（有舊值時）。
    #[test]
    fn managed_fill_writes_ingress() {
        let mut model = ObridgeConfigModel {
            ingress_mode: "managed".into(),
            listen_port: 17401,
            listen_secret: Some("ls".into()),
            channels: vec![mk_channel("email", Some(""), Some("smtp-pw"))],
            ..Default::default()
        };
        // 無舊檔：空字串密碼＝無原值可保留 → 應報錯。
        assert!(validate(&mut model, None).is_err());
        // 有舊檔：空字串 → 保留原值。
        let old = parse_doc(OLD_DOC);
        validate(&mut model, Some(&old)).unwrap();
        let (doc, _) = build_doc(&model, Some(&old), Some(&managed_fill())).unwrap();
        assert_eq!(
            doc.get("operoid").unwrap().get("ingress_secret").unwrap().as_str(),
            Some("server-token")
        );
    }

    /// 產生的文件可被 toml 反解析，語意完整（type/source/routes）。
    #[test]
    fn generated_doc_reparses() {
        let mut model = ObridgeConfigModel {
            ingress_mode: "manual".into(),
            ingress_url: Some("http://10.0.0.5:7340/event".into()),
            ingress_secret: Some("ingress-s".into()),
            listen_port: 17401,
            listen_secret: Some("ls".into()),
            channels: vec![mk_channel("email", Some("p1"), Some("p2"))],
            ..Default::default()
        };
        validate(&mut model, None).unwrap();
        let (doc, _) = build_doc(&model, None, None).unwrap();
        let text = toml::to_string_pretty(&doc).unwrap();
        let back = parse_doc(&text);
        assert_eq!(
            back.get("operoid").unwrap().get("ingress_url").unwrap().as_str(),
            Some("http://10.0.0.5:7340/event")
        );
        let ch = &back.get("channels").unwrap().as_array().unwrap()[0];
        assert_eq!(ch.get("type").unwrap().as_str(), Some("email-imap"));
        assert_eq!(
            ch.get("email_imap")
                .unwrap()
                .get("routes")
                .unwrap()
                .as_array()
                .unwrap()[0]
                .get("employee")
                .unwrap()
                .as_str(),
            Some("Steve-TW")
        );
    }

    /// 驗證規則：source 重複／route 擇一／managed 帶手填 secret／無原值的空密碼。
    #[test]
    fn validate_rejects() {
        let old = parse_doc(OLD_DOC);
        let mut m = ObridgeConfigModel {
            ingress_mode: "managed".into(),
            listen_port: 17401,
            listen_secret: Some("ls".into()),
            channels: vec![
                mk_channel("email", Some("p"), Some("p")),
                mk_channel("email", Some("p"), Some("p")),
            ],
            ..Default::default()
        };
        assert!(validate(&mut m, None).is_err(), "source 重複應拒");

        let mut m = ObridgeConfigModel {
            ingress_mode: "managed".into(),
            listen_port: 17401,
            listen_secret: Some("ls".into()),
            channels: vec![{
                let mut c = mk_channel("email", Some("p"), Some("p"));
                c.routes[0].brain = Some("demo".into());
                c
            }],
            ..Default::default()
        };
        assert!(validate(&mut m, None).is_err(), "route 雙填應拒");

        let mut m = ObridgeConfigModel {
            ingress_mode: "managed".into(),
            ingress_secret: Some("hand-written".into()),
            listen_port: 17401,
            listen_secret: Some("ls".into()),
            channels: vec![mk_channel("email", Some("p"), Some("p"))],
            ..Default::default()
        };
        assert!(validate(&mut m, None).is_err(), "managed 不接受手填 secret");

        let mut m = ObridgeConfigModel {
            ingress_mode: "manual".into(),
            ingress_url: Some("http://10.0.0.5:7340/event".into()),
            ingress_secret: None,
            listen_port: 17401,
            listen_secret: Some("ls".into()),
            channels: vec![mk_channel("email", Some("p"), Some("p"))],
            ..Default::default()
        };
        assert!(validate(&mut m, None).is_err(), "manual 缺 secret 且無舊值應拒");
        validate(&mut m, Some(&old)).unwrap(); // 有舊檔 → 保留原值，通過

        let mut m = ObridgeConfigModel {
            ingress_mode: "managed".into(),
            listen_port: 17401,
            listen_secret: Some("ls".into()),
            channels: vec![mk_channel("slack", Some("p"), Some("p"))],
            ..Default::default()
        };
        // source 與 preserved 的 wasm 頻道撞名。
        assert!(
            validate(&mut m, Some(&old)).is_err(),
            "與 preserved source 撞名應拒"
        );
    }

    /// 讀取面：密鑰遮蔽（null＋has_*）、managed 判定、preserved 清單。
    #[test]
    fn load_model_masks_secrets_and_detects_mode() {
        let dir = std::env::temp_dir().join(format!(
            "obridge-admin-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("obridge.toml");
        std::fs::write(&p, OLD_DOC).unwrap();
        let m = load_model(&p, 7340, Some("server-token")).unwrap();
        assert_eq!(m.ingress_mode, "managed", "url＋secret 與 server 一致 → managed");
        assert!(
            m.ingress_url.is_none() && m.ingress_secret.is_none(),
            "managed 不回傳 ingress 欄位"
        );
        assert_eq!(m.channels.len(), 1);
        let ch = &m.channels[0];
        assert!(ch.imap.password.is_none() && ch.imap.has_password);
        assert!(ch.smtp.password.is_none() && ch.smtp.has_password);
        assert_eq!(ch.poll_secs, 30);
        assert_eq!(m.preserved_channels.len(), 1);
        assert_eq!(m.preserved_channels[0].source, "slack");
        assert_eq!(m.preserved_channels[0].channel_type, "wasm");
        // token 不同 → manual，且不回傳 secret 本體。
        let m2 = load_model(&p, 7340, Some("other-token")).unwrap();
        assert_eq!(m2.ingress_mode, "manual");
        assert_eq!(m2.ingress_url.as_deref(), Some("http://127.0.0.1:7340/event"));
        assert!(m2.ingress_secret.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// sections_changed：listen 變更 → true；僅頻道變更 → false；無舊檔 → true。
    #[test]
    fn sections_changed_detects_restart_scope() {
        let old = parse_doc(OLD_DOC);
        let mut m = ObridgeConfigModel {
            ingress_mode: "managed".into(),
            listen_port: 17401,
            listen_secret: Some("old-listen-secret".into()),
            channels: vec![mk_channel("email", Some("new-pw"), Some("new-pw"))],
            ..Default::default()
        };
        validate(&mut m, Some(&old)).unwrap();
        let (doc, _) = build_doc(&m, Some(&old), Some(&managed_fill())).unwrap();
        assert!(!sections_changed(Some(&old), &doc), "僅頻道變更 → 熱載入");
        let mut m2 = m.clone();
        m2.listen_port = 17402;
        let (doc2, _) = build_doc(&m2, Some(&old), Some(&managed_fill())).unwrap();
        assert!(sections_changed(Some(&old), &doc2), "listen 變更 → 重啟");
        assert!(sections_changed(None, &doc), "無舊檔 → 視為變更");
    }

    /// build id：`--version` 輸出的解析＋兩 id 的比對語意。
    #[test]
    fn build_id_parse_and_match() {
        let out = "obridge 0.1.0 (build abc1234 1700000000)";
        assert_eq!(parse_build_id(out).as_deref(), Some("abc1234"));
        assert_eq!(parse_build_id("garbage"), None, "無法解析不誤報");
        assert_eq!(parse_build_id("oserver 0.4.1 (build abc1234-dirty 1)").as_deref(), Some("abc1234-dirty"));

        assert_eq!(build_matches("abc1234", "abc1234"), Some(true));
        assert_eq!(build_matches("abc1234", "def5678"), Some(false));
        assert_eq!(build_matches("unknown", "abc1234"), None, "自身無 git → 無從判定");
        assert_eq!(build_matches("abc1234", "unknown"), None, "子行程無 git → 無從判定");
        // -dirty 後綴也要能對上（同一 commit、工作區有改動）。
        assert_eq!(build_matches("abc1234-dirty", "abc1234-dirty"), Some(true));
    }

    /// 模式分際：個人模式（無 operoid.toml）就算 autostart 開＋設定檔存在也**不得**
    /// spawn——那是桌面殼的管轄（雙重 spawn＝雙 IMAP 輪詢＋send port 衝突）。
    /// 企業模式（有 operoid.toml）閘打開、允許帶起（spawn 成敗依測試環境的 exe
    /// 解析而定——deps 內可能有 obridge.exe——但清理後必須回到非執行中狀態）。
    #[test]
    fn autostart_gated_to_enterprise_mode() {
        let dir = std::env::temp_dir().join(format!(
            "obridge-gate-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        std::fs::create_dir_all(dir.join("obridge")).unwrap();
        std::fs::write(config_path(&dir), OLD_DOC).unwrap();
        let cfg = AppConfig {
            obridge_autostart: true,
            ..AppConfig::default()
        };
        // 個人模式：不 spawn（本測試要守的核心回歸線）。
        autostart(&dir, &cfg);
        assert_eq!(status_managed(), (false, None), "個人模式不得由 oserver spawn");
        // 企業模式：閘打開；結束前收掉可能帶起的子行程。
        std::fs::write(dir.join("operoid.toml"), "[server]\ntoken = \"t\"\n").unwrap();
        assert!(is_enterprise(&dir), "有 operoid.toml ＝ 企業模式");
        autostart(&dir, &cfg);
        if status_managed().0 {
            kill_managed();
        }
        assert_eq!(status_managed(), (false, None), "清理後不得殘留 managed 狀態");
        std::fs::remove_dir_all(&dir).ok();
    }
}
