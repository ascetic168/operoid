//! 本地（迴環）HTTP server：瀏覽器預覽筆記時的回呼通道。
//!
//! App 啟動時綁 `127.0.0.1:0`（OS 派 port），只暴露 `GET /n/{*path}`：把該筆記
//! `.md` 按需渲染成自足 HTML 當回應回傳。瀏覽器內的 wikilink 是
//! `<a href="/n/{target}" target="_blank">`，點下去開新分頁向本 server 請求——
//! 可任意深度跳、每層都開新分頁（方便並排參考）。
//!
//! 只綁迴環位址，外部機器連不到；不寫任何磁碟檔案（HTML 直接當 HTTP body 回傳）。

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use tauri::{async_runtime, AppHandle, Runtime};

use crate::note_view;

/// 注入 Tauri managed state，讓 [`note_view::open_note`] 取得 server port 組 URL。
#[derive(Clone, Copy)]
pub struct NoteServer {
    pub port: u16,
}

/// 啟動本地 HTTP server（只綁迴環）。回傳 OS 派給的 port。
///
/// listener 在 `async_runtime::block_on` 內綁定（同步階段就能拿到 port，呼叫者立即可
/// 組 URL），server loop 再用 `async_runtime::spawn` 排程進 Tauri 的 tokio runtime。
/// 注意：`.setup()` 在 event loop 起來「之前」跑，當下沒有 Tokio runtime context，
/// 故**不能**直接用 `tokio::spawn`/`tokio::net::TcpListener::bind`（會 panic
/// "no reactor running"）；必須走 `tauri::async_runtime`，它綁定到 Tauri 內建的 runtime。
/// server 生命週期與 App 同壽——App 退出即進程結束，無需顯式 shutdown。
pub fn start(app: AppHandle) -> u16 {
    let listener = async_runtime::block_on(async {
        tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind 127.0.0.1:0")
    });
    let port = listener.local_addr().expect("local_addr").port();
    async_runtime::spawn(serve(listener, app));
    port
}

async fn serve(listener: tokio::net::TcpListener, app: AppHandle) {
    let router = Router::new()
        .route("/n/{*path}", get(note_handler))
        .with_state(app);
    axum::serve(listener, router).await.ok();
}

/// `GET /n/{*path}` → 按需渲染該筆記 HTML。`path` 已由 axum percent-decode。
async fn note_handler<R: Runtime>(
    State(app): State<AppHandle<R>>,
    Path(path): Path<String>,
) -> Response {
    // path 形如 "people/JLin"；與前端 wikilink 的 target 同形式（parse_target 處理括號/|）。
    match note_view::render_target(&app, &path).await {
        Ok((_title, html)) => Html(html).into_response(),
        Err(e) => {
            let body = not_found_html(&path, &e.code);
            (StatusCode::NOT_FOUND, Html(body)).into_response()
        }
    }
}

/// 找不到筆記時的友善頁面（沿用筆記預覽的紙本樣式）。
fn not_found_html(target: &str, code: &str) -> String {
    format!(
        concat!(
            "<!doctype html>\n<html lang=\"zh-Hant\">\n<head>\n<meta charset=\"utf-8\">\n",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
            "<title>找不到筆記</title>\n<style>\n",
            ":root {{ color-scheme: light; }}\n",
            "body {{ margin:0; background:#f5f6f8; color:#1b1b1b;",
            "  font-family: ui-sans-serif, system-ui, -apple-system, \"Segoe UI\",",
            "  \"Microsoft JhengHei\", \"PingFang TC\", \"Noto Sans CJK TC\", sans-serif;",
            "  line-height:1.75; }}\n",
            ".paper {{ max-width:780px; margin:0 auto; padding:42px 30px 90px;",
            "  background:#fff; min-height:100vh; box-shadow:0 0 0 1px #eceeef; }}\n",
            "h1 {{ font-size:1.6em; color:#9ca3af; }}\n",
            "code {{ background:#f1f3f5; padding:.12em .4em; border-radius:4px;",
            "  font-family: ui-monospace, Consolas, monospace; }}\n",
            ".muted {{ color:#6b7280; font-size:.9em; }}\n",
            "</style>\n</head>\n<body>\n<div class=\"paper\">\n",
            "<h1>找不到筆記</h1>\n",
            "<p>找不到對應的筆記：<code>{}</code></p>\n",
            "<p class=\"muted\">{}</p>\n",
            "</div>\n</body>\n</html>\n"
        ),
        html_escape_attr(target),
        html_escape_text(code),
    )
}

fn html_escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_escape_attr(s: &str) -> String {
    html_escape_text(s).replace('"', "&quot;")
}
