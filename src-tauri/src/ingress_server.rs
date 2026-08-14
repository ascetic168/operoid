//! 外部事件 ingress HTTP server（E7 進氣口）——外部 bridge（Email／IM／…）投遞事件的統一入口。
//!
//! 啟動條件：`AppConfig.event_ingress_port` 與 `event_ingress_secret` **皆**有設（opt-in、最安全）。
//! 綁 `127.0.0.1:<port>`，暴露單一 `POST /event`：Bearer 認證 → JSON `InboundEvent` →
//! `(source, external_ref)` 去重 → `dispatch_event` 路由喚醒員工。
//!
//! 設計見 `docs/Operoid-設計-統一事件ingress契約.md`。模式鏡像 [`crate::note_server`]：
//! listener 以 `async_runtime::block_on` 綁定（`.setup()` 階段無 Tokio context），server loop
//! 以 `async_runtime::spawn` 排程；生命週期與 App 同壽。

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use tauri::{async_runtime, AppHandle, Manager, Runtime};

use crate::agent_state::{AppState, InboundEvent};
use crate::config::app_config;
use crate::event_bus::dispatch_event;

/// 啟動 ingress server（若設定未啟用則回 `None`，不啟動、不報錯）。
///
/// 回傳實際綁定的 port（一般等於設定值）。綁定失敗（如 port 占用）則記 log、回 `None`、
/// **不**讓 app 啟動失敗（ingress 是附加能力，不該拖垮主程式）。
pub fn start(app: AppHandle) -> Option<u16> {
    let cfg = match app_config::load(&app) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[ingress] 讀設定失敗，不啟動進氣口：{e}");
            return None;
        }
    };
    let port = cfg.event_ingress_port?;
    let secret = cfg.event_ingress_secret?; // port／secret 缺一即不啟動
    let _ = secret; // 認證時逐請求重讀設定（反應運行期變更）；此處僅驗啟動條件

    let listener = async_runtime::block_on(async {
        tokio::net::TcpListener::bind(("127.0.0.1", port)).await
    });
    let listener = match listener {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[ingress] 綁 port {port} 失敗，不啟動進氣口：{e}");
            return None;
        }
    };
    let bound = listener.local_addr().ok()?.port();
    eprintln!("[ingress] 進氣口啟動於 127.0.0.1:{bound}（POST /event）");
    async_runtime::spawn(serve(listener, app));
    Some(bound)
}

async fn serve(listener: tokio::net::TcpListener, app: AppHandle) {
    let router = Router::new().route("/event", post(event_handler)).with_state(app);
    if let Err(e) = axum::serve(listener, router).await {
        eprintln!("[ingress] server 結束：{e}");
    }
}

/// `POST /event`：Bearer 認證 → 解析 `InboundEvent` → 去重 → dispatch。
///
/// 回應：`202 accepted`（已投遞，含 best-effort 丟棄如無路由／agent_os 未開）；
/// `200 duplicate; ignored`（`(source, external_ref)` 已見，冪等）；`401`（認證失敗）；
/// `400`（JSON 格式錯，由 axum `Json` extractor 自動回）；`500`（dispatch 硬錯）。
async fn event_handler<R: Runtime>(
    State(app): State<AppHandle<R>>,
    headers: HeaderMap,
    Json(ev): Json<InboundEvent>,
) -> Response {
    // 認證：逐請求重讀設定（反應運行期變更 secret）。
    let secret = match app_config::load(&app) {
        Ok(c) => c.event_ingress_secret,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "config load failed").into_response(),
    };
    let Some(secret) = secret else {
        return (StatusCode::UNAUTHORIZED, "no secret configured").into_response();
    };
    if !check_auth(&headers, &secret) {
        return (StatusCode::UNAUTHORIZED, "bad credentials").into_response();
    }

    // 去重：(source, external_ref) session 內首見才 dispatch（無 external_ref 則跳過去重）。
    if let Some(ext_ref) = &ev.external_ref {
        let state = app.state::<AppState>();
        if !state.is_new_external_ref(&ev.source, ext_ref) {
            return (StatusCode::OK, "duplicate; ignored").into_response();
        }
    }

    match dispatch_event(&app, ev).await {
        Ok(_) => (StatusCode::ACCEPTED, "accepted").into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("dispatch failed: {e}"),
        )
            .into_response(),
    }
}

/// 檢查 `Authorization: Bearer <secret>`。缺失／格式不對／值不符皆回 `false`。
///
/// 抽成獨立純函式以便單元測試。時序攻擊在 localhost 共用密鑰情境下風險低，用直接比較（v1）。
pub(crate) fn check_auth(headers: &HeaderMap, expected: &str) -> bool {
    let Some(Ok(value)) = headers.get("authorization").map(|h| h.to_str()) else {
        return false;
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return false;
    };
    !expected.is_empty() && token.trim() == expected
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn hdr(value: &str) -> HeaderMap {
        let mut m = HeaderMap::new();
        m.insert(
            "authorization",
            HeaderValue::from_str(value).expect("valid header"),
        );
        m
    }

    #[test]
    fn check_auth_accepts_valid_bearer() {
        assert!(check_auth(&hdr("Bearer s3cr3t"), "s3cr3t"));
    }

    #[test]
    fn check_auth_rejects_wrong_token() {
        assert!(!check_auth(&hdr("Bearer wrong"), "s3cr3t"));
    }

    #[test]
    fn check_auth_rejects_missing_or_malformed() {
        let empty = HeaderMap::new();
        assert!(!check_auth(&empty, "s3cr3t"), "缺標頭應拒");
        assert!(!check_auth(&hdr("Basic abc"), "s3cr3t"), "非 Bearer 應拒");
        assert!(!check_auth(&hdr("Bearer"), "s3cr3t"), "無 token 應拒");
    }

    #[test]
    fn check_auth_rejects_empty_expected() {
        // secret 未設（空）→ 一律拒（防無認證暴露）。
        assert!(!check_auth(&hdr("Bearer "), ""));
    }
}
