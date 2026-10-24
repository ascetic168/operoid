//! 更新檢查（企業版 v1）：GitHub Releases 版本檢查＋行程內快取。
//!
//! `GET /api/admin/update/check`（未列 RBAC 矩陣 → fail-closed Admin）：
//! 查 GitHub `releases/latest`（草稿與 prerelease 由 API 自動排除——release.yml
//! 建 draft 供審閱後 publish，正好只提示已正式發布的版本），比對 `tag_name` 與
//! `CARGO_PKG_VERSION`。結果快取 24h——GitHub 匿名 API 上限 60 req/h/IP，
//! 同時避免管理頁頻繁輪詢。離線／氣隔環境以 operoid.toml
//! `[update] check_enabled = false` 關閉（回應帶 `check_enabled=false`，前端隱藏入口）。
//!
//! **僅檢查與提示，不自動安裝**：企業版為 zip 手動部署，升級程序見 DEPLOYMENT.md。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::routes::ServerState;

/// 快取 TTL：GitHub 匿名 API 上限 60 req/h/IP；24h 遠低於此。
const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);
const RELEASES_API: &str = "https://api.github.com/repos/ascetic168/operoid/releases/latest";
/// GitHub API 一律要求 User-Agent。
const USER_AGENT: &str = concat!("operoid-oserver/", env!("CARGO_PKG_VERSION"));
/// 網路逾時（檢查不應拖慢管理頁）。
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
/// 發布說明截斷上限（markdown 原文；避免超大 body 塞爆回應）。
const NOTES_MAX_CHARS: usize = 4000;

/// 單次檢查結果（快取與 API 回應共用）。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UpdateCheck {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub release_notes: Option<String>,
    /// 檢查時間（UNIX 秒；前端顯示「最後檢查」）。
    pub checked_at_secs: u64,
    /// 本次回應是否來自快取（TTL 內重複查詢）。
    pub cached: bool,
}

/// GitHub Releases API 回應的所需欄位（其餘欄位忽略）。
#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

pub fn update_routes() -> Router<Arc<ServerState>> {
    Router::new().route("/api/admin/update/check", get(api_update_check))
}

/// 行程內快取：`(checked_at, result)`。TTL 內直接回（`cached=true`）。
static CACHE: Mutex<Option<(Instant, UpdateCheck)>> = Mutex::new(None);

async fn api_update_check(State(state): State<Arc<ServerState>>) -> Response {
    if !state.update_check_enabled {
        return (StatusCode::OK, Json(json!({ "check_enabled": false }))).into_response();
    }
    // 快取命中（guard 於 await 前離開作用域——Mutex 不得跨 await 持有）。
    {
        let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, hit)) = cache.as_ref() {
            if at.elapsed() < CACHE_TTL {
                let mut v = serde_json::to_value(hit).unwrap_or_else(|_| json!({}));
                v["check_enabled"] = json!(true);
                v["cached"] = json!(true);
                return (StatusCode::OK, Json(v)).into_response();
            }
        }
    }
    match fetch_latest().await {
        Ok(rel) => {
            let check = make_check(env!("CARGO_PKG_VERSION"), &rel);
            *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), check.clone()));
            let mut v = serde_json::to_value(&check).unwrap_or_else(|_| json!({}));
            v["check_enabled"] = json!(true);
            (StatusCode::OK, Json(v)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "code": "update.checkFailed", "detail": e.to_string() })),
        )
            .into_response(),
    }
}

async fn fetch_latest() -> anyhow::Result<GhRelease> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(HTTP_TIMEOUT)
        .build()?;
    let resp = client.get(RELEASES_API).send().await?.error_for_status()?;
    Ok(resp.json().await?)
}

/// 檢查結果組裝（純函式，測試覆蓋）。
fn make_check(current: &str, rel: &GhRelease) -> UpdateCheck {
    let latest = version_from_tag(&rel.tag_name);
    let update_available = match (&latest, semver::Version::parse(current)) {
        (Some(l), Ok(c)) => *l > c,
        // 無法解析（tag 非 semver／current 非法）→ 不提示，寧可漏報不可誤報。
        _ => false,
    };
    UpdateCheck {
        current_version: current.to_string(),
        latest_version: latest.as_ref().map(ToString::to_string),
        update_available,
        release_url: rel.html_url.clone(),
        release_notes: rel.body.as_deref().map(truncate_notes),
        checked_at_secs: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        cached: false,
    }
}

/// `v0.5.5` → `0.5.5`；非 semver tag → None。
fn version_from_tag(tag: &str) -> Option<semver::Version> {
    semver::Version::parse(tag.trim().trim_start_matches('v')).ok()
}

fn truncate_notes(s: &str) -> String {
    if s.chars().count() <= NOTES_MAX_CHARS {
        return s.to_string();
    }
    let mut out: String = s.chars().take(NOTES_MAX_CHARS).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r###"{
        "url": "https://api.github.com/repos/ascetic168/operoid/releases/1",
        "tag_name": "v0.5.5",
        "html_url": "https://github.com/ascetic168/operoid/releases/tag/v0.5.5",
        "draft": false,
        "prerelease": false,
        "body": "## 更新\n- feat(update): 更新功能\n"
    }"###;

    #[test]
    fn parses_github_release_fixture() {
        let rel: GhRelease = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(rel.tag_name, "v0.5.5");
        assert_eq!(
            rel.html_url.as_deref(),
            Some("https://github.com/ascetic168/operoid/releases/tag/v0.5.5")
        );
        assert!(rel.body.as_deref().unwrap_or_default().contains("更新功能"));
    }

    #[test]
    fn version_from_tag_variants() {
        assert_eq!(version_from_tag("v0.5.5").map(|v| v.to_string()), Some("0.5.5".into()));
        assert_eq!(version_from_tag("0.5.5").map(|v| v.to_string()), Some("0.5.5".into()));
        assert_eq!(version_from_tag(" v0.6.0 ").map(|v| v.to_string()), Some("0.6.0".into()));
        // 非 semver → None（寧可漏報不可誤報）。
        assert_eq!(version_from_tag("nightly-2026"), None);
        assert_eq!(version_from_tag(""), None);
    }

    #[test]
    fn make_check_compares_semver() {
        let rel: GhRelease = serde_json::from_str(FIXTURE).unwrap();
        // 舊版 → 提示更新。
        let c = make_check("0.5.4", &rel);
        assert!(c.update_available);
        assert_eq!(c.latest_version.as_deref(), Some("0.5.5"));
        assert!(!c.cached);
        // 同版 → 不提示。
        assert!(!make_check("0.5.5", &rel).update_available);
        // 比新版高（不可能，但防呆）→ 不提示。
        assert!(!make_check("0.6.0", &rel).update_available);
        // current 非法 → 不提示。
        assert!(!make_check("dev-build", &rel).update_available);
        // URL 與說明帶入。
        assert_eq!(
            make_check("0.5.4", &rel).release_url.as_deref(),
            Some("https://github.com/ascetic168/operoid/releases/tag/v0.5.5")
        );
        assert!(make_check("0.5.4", &rel).release_notes.unwrap().contains("更新功能"));
    }

    #[test]
    fn notes_are_truncated_on_char_boundary() {
        let long = "好".repeat(NOTES_MAX_CHARS + 100);
        let rel = GhRelease {
            tag_name: "v0.5.5".into(),
            html_url: None,
            body: Some(long),
        };
        let c = make_check("0.5.4", &rel);
        let notes = c.release_notes.unwrap();
        assert_eq!(notes.chars().count(), NOTES_MAX_CHARS + 1); // 截斷＋…
        assert!(notes.ends_with('…'));
    }
}
