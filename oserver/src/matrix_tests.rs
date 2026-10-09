//! R2 驗收：權限矩陣自動化測試——**列舉全部 routes ×（admin/manager/user/未認證）**
//! 四情境斷言（axum oneshot，全路由都過 RBAC 中介層）。
//!
//! 判準（計畫 §五 R2 驗收）：
//! - 允許 → 狀態**不得**為 401／403（可能是 200/404/409/422/500——資料面結果非授權面）；
//! - 拒絕 → **必為 403** `auth.forbidden`；
//! - 未認證 → **必為 401**；
//! - fail-closed：未列於矩陣的路由（含不存在的）→ Admin——admin 過、user 擋。
//!
//! ⚠️ 新增路由時必須同步：① crate::rbac::requirement 矩陣 ② 本檔 INVENTORY——否則測試
//! 面不完整（矩陣本身 fail-closed 兜底，不會破防，但會被 admin-only 擋到）。

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

use crate::auth::{AuthProvider, AuthError, Identity};
use crate::routes::ServerState;
use crate::rbac::Req;

/// 測試用 stub auth：token → 固定身份（不碰 SQLite——矩陣測試只驗授權面）。
struct MatrixAuth;

impl AuthProvider for MatrixAuth {
    fn check(&self, auth_header: Option<&str>) -> Result<Identity, AuthError> {
        let t = auth_header
            .and_then(|h| h.strip_prefix("Bearer ").map(str::trim))
            .filter(|s| !s.is_empty())
            .ok_or(AuthError)?;
        match t {
            "adm" => Ok(Identity::new("principal-admin", &["admin"])),
            "mgr" => Ok(Identity::new("principal-manager", &["manager"])),
            "usr" => Ok(Identity::new("principal-user", &["user"])),
            _ => Err(AuthError),
        }
    }
}

fn test_state(dir: &std::path::Path) -> Arc<ServerState> {
    Arc::new(ServerState {
        auth: Arc::new(MatrixAuth),
        cfg: ocore::app_config::AppConfig::default(),
        db_path: dir.join("matrix.db"),
        ready: Arc::new(AtomicBool::new(true)),
        agent_state: None,
        ops: Arc::new(crate::operations::OpRegistry::new()),
        settings_dir: dir.to_path_buf(),
        server_token: Some("master-token".into()),
        server_port: 7340,
    })
}

fn test_router(dir: &std::path::Path) -> axum::Router {
    let state = test_state(dir);
    crate::routes::router(state.clone())
        .merge(crate::writes::write_routes().with_state(state.clone()))
        .merge(crate::knowledge_admin::knowledge_admin_routes().with_state(state.clone()))
        .merge(crate::accounts::account_routes().with_state(state.clone()))
        .merge(crate::sse::sse_routes().with_state(state.clone()))
        .merge(crate::gbrain::gbrain_routes().with_state(state.clone()))
        .merge(crate::obridge_admin::obridge_routes().with_state(state.clone()))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::rbac::rbac_middleware,
        ))
}

/// 全路由清單（method, pattern）——與四個 router + accounts 逐一對應。
const INVENTORY: &[(&str, &str)] = &[
    // routes.rs（讀取面＋ingress）
    ("POST", "/event"),
    ("GET", "/api/state"),
    ("GET", "/api/employees"),
    ("GET", "/api/templates"),
    ("GET", "/api/employees/{id}/watch"),
    ("GET", "/api/artifacts/{id}"),
    ("GET", "/api/inbox"),
    ("GET", "/api/events"),
    ("GET", "/api/registry"),
    // writes.rs（寫入面）
    ("POST", "/api/workspace/ensure"),
    ("POST", "/api/templates"),
    ("DELETE", "/api/templates/{id}"),
    ("PATCH", "/api/templates/{id}"),
    ("DELETE", "/api/employees/{id}"),
    ("PATCH", "/api/employees/{id}"),
    ("POST", "/api/employees/deploy"),
    ("POST", "/api/employees/{id}/stop"),
    ("POST", "/api/employees/{id}/unarchive"),
    ("POST", "/api/employees/{id}/messages"),
    ("DELETE", "/api/employees/{id}/messages"),
    ("POST", "/api/commitments"),
    ("POST", "/api/commitments/{id}/approve"),
    ("POST", "/api/commitments/{id}/reject"),
    ("POST", "/api/commitments/{id}/archive"),
    ("POST", "/api/commitments/{id}/review"),
    ("POST", "/api/registry/drill"),
    ("POST", "/api/tasks/{id}/cancel"),
    ("POST", "/api/registry"),
    // knowledge_admin.rs
    ("GET", "/api/knowledge/overview"),
    ("POST", "/api/knowledge/policy"),
    ("POST", "/api/knowledge/scopes"),
    ("POST", "/api/knowledge/principals/{id}/attrs"),
    ("POST", "/api/knowledge/grants"),
    ("POST", "/api/knowledge/grants/{id}/revoke"),
    ("POST", "/api/knowledge/principals"),
    ("POST", "/api/knowledge/principals/{id}/token"),
    ("DELETE", "/api/knowledge/principals/{id}/token"),
    // gbrain.rs（GBrain 能力域）
    ("GET", "/api/gbrain/config"),
    ("POST", "/api/gbrain/model"),
    ("POST", "/api/gbrain/models-all"),
    ("POST", "/api/gbrain/model/unset"),
    ("POST", "/api/gbrain/db-overrides/clear"),
    ("POST", "/api/gbrain/provider-base-url"),
    ("POST", "/api/gbrain/config-raw"),
    ("GET", "/api/brains"),
    ("POST", "/api/brains"),
    ("DELETE", "/api/brains/{id}"),
    ("POST", "/api/brains/{id}/active"),
    ("POST", "/api/brains/active-source"),
    ("GET", "/api/brains/{id}/sources"),
    ("POST", "/api/brains/{id}/sources"),
    ("DELETE", "/api/brains/{id}/sources/{source_id}"),
    ("POST", "/api/brains/{id}/sync"),
    ("POST", "/api/brains/{id}/bind-path"),
    ("POST", "/api/operations"),
    ("GET", "/api/operations/{id}"),
    ("GET", "/api/factories/types"),
    ("POST", "/api/factories/upload"),
    ("POST", "/api/factories/upload/cleanup"),
    ("POST", "/api/factories/run"),
    ("POST", "/api/factories/write-pages"),
    ("POST", "/api/factories/extract-companies"),
    ("POST", "/api/factories/save-authored"),
    ("POST", "/api/factories/classify"),
    ("GET", "/api/prereq"),
    // K6 知識管線健康（未列 rbac 矩陣 → Admin fail-closed——探測面屬管理資訊）
    ("GET", "/api/knowledge/health"),
    // R3 新增
    ("POST", "/api/commitments/{id}/satisfy"),
    ("GET", "/api/stream"),
    ("POST", "/api/stream/ticket"),
    ("GET", "/api/service/status"),
    // accounts.rs（R2 新增）
    ("POST", "/api/auth/logout"),
    ("POST", "/api/auth/refresh"),
    ("POST", "/api/auth/password"),
    ("POST", "/api/accounts"),
    ("POST", "/api/accounts/{id}/disable"),
    ("POST", "/api/accounts/{id}/enable"),
    ("POST", "/api/accounts/{id}/password"),
    ("POST", "/api/accounts/{id}/roles"),
    // obridge_admin.rs（企業模式 obridge 管理；未列 matrix → Admin fail-closed）
    ("GET", "/api/obridge/status"),
    ("GET", "/api/obridge/config"),
    ("PUT", "/api/obridge/config"),
    ("POST", "/api/obridge/restart"),
];

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "r2matrix-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 對全路由 × 四身份做矩陣斷言（R2 退出條件的核心一條）。
#[tokio::test]
async fn permission_matrix_all_routes_four_identities() {
    let dir = temp_dir("main");
    let app = test_router(&dir);
    for (method, pattern) in INVENTORY {
        let expected = crate::rbac::requirement(method, Some(pattern));
        assert_ne!(expected, Req::Public, "矩陣測試清單不收公開路由：{method} {pattern}");
        for (token, label) in [("adm", "admin"), ("mgr", "manager"), ("usr", "user")] {
            let identity = Identity::new(format!("principal-{label}"), &[label]);
            let allowed = crate::rbac::satisfies(&identity, expected);
            let req = Request::builder()
                .method(Method::from_bytes(method.as_bytes()).unwrap())
                .uri(pattern.replace("{id}", "x").replace("{source_id}", "s1"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            let status = resp.status();
            if allowed {
                assert!(
                    status != StatusCode::UNAUTHORIZED && status != StatusCode::FORBIDDEN,
                    "{label} 應可用 {method} {pattern}（要求 {expected:?}）——實得 {status}"
                );
            } else {
                assert_eq!(
                    status,
                    StatusCode::FORBIDDEN,
                    "{label} 對 {method} {pattern}（要求 {expected:?}）必須 403——實得 {status}"
                );
            }
        }
        // 未認證 → 401。
        let req = Request::builder()
            .method(Method::from_bytes(method.as_bytes()).unwrap())
            .uri(pattern.replace("{id}", "x").replace("{source_id}", "s1"))
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "未認證對 {method} {pattern} 必須 401"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// K6：`/api/knowledge/health`——admin 200＋三區塊形狀（探測皆可失敗，回報不中斷；
/// GUI 橫幅／設定頁能力卡消費此形狀）。
#[tokio::test]
async fn knowledge_health_capability_shape() {
    let dir = temp_dir("k6health");
    let app = test_router(&dir);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/knowledge/health")
        .header("authorization", "Bearer adm")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let e = &v["embedding"];
    assert!(e["reachable"].is_boolean());
    assert!(e["vision"].is_boolean());
    assert!(e["long_input_ok"].is_boolean());
    assert!(e["input_modalities"].is_array());
    let m = &v["mineru"];
    assert!(m["resolved"].is_boolean());
    let _ = &v["vlm"]["capable"];
    std::fs::remove_dir_all(&dir).ok();
}

/// fail-closed：矩陣未列（不存在）的路由 → Admin 語意——admin 不被授權面擋、user 403、未認證 401。
#[tokio::test]
async fn fail_closed_unknown_route_defaults_admin() {
    let dir = temp_dir("unknown");
    let app = test_router(&dir);
    let cases = [
        ("adm", "admin", false),
        ("usr", "user", true),
    ];
    for (token, _label, expect_403) in cases {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/never-heard-of")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        if expect_403 {
            assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        } else {
            assert_ne!(resp.status(), StatusCode::FORBIDDEN, "admin 不應被授權面擋（fail-closed 的預設是 Admin）");
            assert_ne!(resp.status(), StatusCode::UNAUTHORIZED);
        }
    }
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/never-heard-of")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    std::fs::remove_dir_all(&dir).ok();
}

/// 公開路由：healthz 免認證 200；登入端點免授權（body 缺 → 422，絕非 403）。
#[tokio::test]
async fn public_routes_pass_without_auth() {
    let dir = temp_dir("public");
    let app = test_router(&dir);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/auth/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::FORBIDDEN, "登入端點不得被 RBAC 擋");
    std::fs::remove_dir_all(&dir).ok();
}

// ── 登入 e2e（真 PrincipalTokenProvider：建號 → 登入 → 授權 → 強改密 → 鎖定）──

fn real_state(dir: &std::path::Path) -> Arc<ServerState> {
    Arc::new(ServerState {
        auth: Arc::new(crate::auth::PrincipalTokenProvider::new("master-token", dir.join("e2e.db"))),
        cfg: ocore::app_config::AppConfig {
            agent_os_enabled: true,
            ..ocore::app_config::AppConfig::default()
        },
        db_path: dir.join("e2e.db"),
        ready: Arc::new(AtomicBool::new(true)),
        agent_state: None,
        ops: Arc::new(crate::operations::OpRegistry::new()),
        settings_dir: dir.to_path_buf(),
        server_token: Some("master-token".into()),
        server_port: 7340,
    })
}

fn real_router(dir: &std::path::Path) -> axum::Router {
    let state = real_state(dir);
    crate::routes::router(state.clone())
        .merge(crate::writes::write_routes().with_state(state.clone()))
        .merge(crate::knowledge_admin::knowledge_admin_routes().with_state(state.clone()))
        .merge(crate::accounts::account_routes().with_state(state.clone()))
        .merge(crate::sse::sse_routes().with_state(state.clone()))
        .merge(crate::gbrain::gbrain_routes().with_state(state.clone()))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::rbac::rbac_middleware,
        ))
}

async fn post_json(app: &axum::Router, path: &str, token: Option<&str>, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let mut b = Request::builder().method(Method::POST).uri(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let resp = app
        .clone()
        .oneshot(b.header("content-type", "application/json").body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let v = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, v)
}

async fn get(app: &axum::Router, path: &str, token: Option<&str>) -> StatusCode {
    let mut b = Request::builder().method(Method::GET).uri(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    app.clone().oneshot(b.body(Body::empty()).unwrap()).await.unwrap().status()
}

/// F3 測試 helper：帳號登入取 token（密碼須先於帳號建立迴圈中改過）。
async fn login_user(app: &axum::Router, name: &str, pw: &str) -> String {
    let (status, body) = post_json(
        app,
        "/api/auth/login",
        None,
        serde_json::json!({"login_name": name, "password": pw}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["token"].as_str().unwrap().to_string()
}

/// R2 登入 e2e：建號 → 首登（must_change_password）→ 除改密外全擋 → 改密 →
/// user 權限生效（可讀 state、擋 admin 面）→ 停用 → token 失效。
#[tokio::test]
async fn login_flow_end_to_end() {
    let dir = temp_dir("e2e");
    let app = real_router(&dir);

    // 1) admin（master token）建 user 帳號。
    let (status, body) = post_json(
        &app,
        "/api/accounts",
        Some("master-token"),
        serde_json::json!({"login_name": "alice", "roles": ["user"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "建號應成功：{body}");
    let temp_pw = body["temporary_password"].as_str().unwrap().to_string();
    assert!(temp_pw.len() >= 12);

    // 2) 登入 → must_change_password=true。
    let (status, body) = post_json(
        &app,
        "/api/auth/login",
        None,
        serde_json::json!({"login_name": "alice", "password": temp_pw}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "登入應成功：{body}");
    assert_eq!(body["must_change_password"], serde_json::json!(true));
    let token = body["token"].as_str().unwrap().to_string();

    // 3) 強改密閘：除 /api/auth/password 外全 403 mustChangePassword。
    assert_eq!(get(&app, "/api/state", Some(&token)).await, StatusCode::FORBIDDEN);

    // 4) 弱密碼被拒；改密成功後閘解除。
    let (status, weak_body) = post_json(
        &app,
        "/api/auth/password",
        Some(&token),
        serde_json::json!({"old_password": temp_pw, "new_password": "short"}),
    )
    .await;
    println!("weak-pw body: {weak_body}");
    assert_eq!(status, StatusCode::BAD_REQUEST, "弱密碼必須被拒：{weak_body}");
    let new_pw = "a-much-longer-password-42";
    let (status, _) = post_json(
        &app,
        "/api/auth/password",
        Some(&token),
        serde_json::json!({"old_password": temp_pw, "new_password": new_pw}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "改密應成功");

    // 5) user 權限面：可讀 state；管理面（建號）403。
    assert_eq!(get(&app, "/api/state", Some(&token)).await, StatusCode::OK);
    let (status, _) = post_json(
        &app,
        "/api/accounts",
        Some(&token),
        serde_json::json!({"login_name": "eve", "roles": ["admin"]}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "user 建號必須 403");

    // 6) 停用 → token 立即失效（401）＋登入被拒。
    let (status, _) = post_json(&app, "/api/accounts/principal-alice/disable", Some("master-token"), serde_json::json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(get(&app, "/api/state", Some(&token)).await, StatusCode::UNAUTHORIZED);
    let (status, _) = post_json(
        &app,
        "/api/auth/login",
        None,
        serde_json::json!({"login_name": "alice", "password": new_pw}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "停用帳號登入 → 403 accountDisabled");

    std::fs::remove_dir_all(&dir).ok();
}

/// F3（對話表現力）：artifact 端點的「限自身」細化——owner user 可讀、他人 user 403、
/// manager/admin 可讀、不存在 404。資料面用真 SQLite 種子（員工歸屬 + artifact）。
#[tokio::test]
async fn artifact_access_control_end_to_end() {
    let dir = temp_dir("artifact");
    let db = dir.join("e2e.db");
    {
        let store = ocore::domain::SqliteStore::open(&db).unwrap();
        use ocore::domain::Store;
        store
            .put_workspace(&ocore::domain::Workspace {
                id: "ws-default".into(),
                name: "WS".into(),
                description: None,
                status: ocore::domain::WorkspaceStatus::Active,
                created_at: "t".into(),
            })
            .unwrap();
        store
            .put_employee(&ocore::domain::Employee {
                id: "emp-alice".into(),
                workspace_id: "ws-default".into(),
                name: "瀚青".into(),
                brain: ocore::domain::BrainRef { brain_id: "__default__".into() },
                role: None,
                template_id: None,
                state: ocore::domain::EmployeeState::Sleeping,
                archived: false,
                tools: None,
                created_at: "t".into(),
                owner_principal: Some("principal-alice".into()),
            })
            .unwrap();
        store
            .put_artifact(&ocore::domain::Artifact {
                id: "art-1".into(),
                workspace_id: "ws-default".into(),
                title: "良率報告".into(),
                artifact_type: "report".into(),
                content: "……".into(),
                produced_by: "emp-alice".into(),
                source_task_id: None,
                source_commitment_id: None,
                revised_from_id: None,
                project_id: None,
                version: 1,
                status: ocore::domain::ArtifactStatus::Committed,
                created_at: "t".into(),
            })
            .unwrap();
    }
    let app = real_router(&dir);
    // 建兩個 user 帳號：alice（員工歸屬人）與 mallory（他人）。
    for (name, temp_pw) in [
        ("alice", "alice-temp-password-1"),
        ("mallory", "mallory-temp-password-1"),
    ] {
        let (status, body) = post_json(
            &app,
            "/api/accounts",
            Some("master-token"),
            serde_json::json!({"login_name": name, "roles": ["user"], "temp_password": temp_pw}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, body) = post_json(
            &app,
            "/api/auth/login",
            None,
            serde_json::json!({"login_name": name, "password": temp_pw}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, _) = post_json(
            &app,
            "/api/auth/password",
            Some(&body["token"].as_str().unwrap().to_string()),
            serde_json::json!({"old_password": temp_pw, "new_password": format!("{name}-new-password-42")}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    let alice = login_user(&app, "alice", "alice-new-password-42").await;
    let mallory = login_user(&app, "mallory", "mallory-new-password-42").await;

    // owner user → 200 且含 content。
    let mut b = Request::builder()
        .method(Method::GET)
        .uri("/api/artifacts/art-1")
        .header("authorization", format!("Bearer {alice}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(b).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["id"], "art-1");
    assert_eq!(v["content"], "……");
    // 他人 user → 403。
    b = Request::builder()
        .method(Method::GET)
        .uri("/api/artifacts/art-1")
        .header("authorization", format!("Bearer {mallory}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(b).await.unwrap().status(), StatusCode::FORBIDDEN);
    // 管理面放行：master token（operator＝admin 語意）→ 200。
    // （MatrixAuth 的 mgr 固定身份只掛在 matrix router；real_router 用 PrincipalTokenProvider。）
    b = Request::builder()
        .method(Method::GET)
        .uri("/api/artifacts/art-1")
        .header("authorization", "Bearer master-token")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(b).await.unwrap().status(), StatusCode::OK);
    // 不存在 → 404。
    b = Request::builder()
        .method(Method::GET)
        .uri("/api/artifacts/art-nope")
        .header("authorization", format!("Bearer {alice}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(b).await.unwrap().status(), StatusCode::NOT_FOUND);
    std::fs::remove_dir_all(&dir).ok();
}

/// R2 防暴：連續 5 次錯密碼 → 429 鎖定（密碼正確也鎖）。
#[tokio::test]
async fn login_lockout_after_repeated_failures() {
    let dir = temp_dir("lockout");
    let app = real_router(&dir);
    let (status, body) = post_json(
        &app,
        "/api/accounts",
        Some("master-token"),
        serde_json::json!({"login_name": "bob", "roles": ["user"], "temp_password": "bob-temp-password-1"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for i in 0..5 {
        let (status, _) = post_json(
            &app,
            "/api/auth/login",
            None,
            serde_json::json!({"login_name": "bob", "password": format!("wrong-{i}")}),
        )
        .await;
        if i < 4 {
            assert_eq!(status, StatusCode::UNAUTHORIZED, "前 4 次失敗 → 401");
        } else {
            assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "第 5 次失敗 → 鎖定 429");
        }
    }
    // 密碼正確也鎖。
    let (status, _) = post_json(
        &app,
        "/api/auth/login",
        None,
        serde_json::json!({"login_name": "bob", "password": "bob-temp-password-1"}),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "鎖定期間正確密碼也拒");
    std::fs::remove_dir_all(&dir).ok();
}
