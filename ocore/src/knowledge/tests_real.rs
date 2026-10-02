//! M1 鐵律的實機證明（#[ignore]：需本機 gbrain＋ollama embedding；手動跑）。
//!
//! 跑法：`cargo test -p ocore real_m1 -- --ignored --nocapture`

use crate::domain::tools::{ToolCtx, ToolInput};
use crate::knowledge::backend::RetrieveKind;
use crate::knowledge::service::KnowledgeService;
use crate::knowledge::types::AccessContext;

/// **提示詞 §26 第一程式里程碑驗收**：
/// `同一 GBrain ＋ 同一查詢 ＋ 不同 AccessContext ＝ 不同授權檢索結果`。
///
/// 建臨時腦＋兩 source（s1=alpha、s2=beta）＋兩 AccessContext（operator=co-common+proj-x、
/// ai:bob=僅 co-common）→ 同一查詢、不同結果；receipts＋retrieval 事件落帳（Test 9 實機版）。
#[ignore = "真實環境相依：需本機 gbrain.exe（~/.bun/bin）與 ollama embedding；手動驗證"]
#[tokio::test]
async fn real_m1_milestone_two_contexts() {
    use crate::domain::Store as _;
    use crate::knowledge::bootstrap::{bootstrap_with_sources, save_policy_new_version};
    use crate::knowledge::types::{Effect, KnowledgeScope, PolicyRule, Visibility};

    // exe：~/.bun/bin/gbrain.exe（Windows 預設）或 PATH 上的 gbrain。
    let exe = dirs::home_dir()
        .map(|h| h.join(".bun").join("bin").join("gbrain.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from("gbrain"));
    let exe = exe.to_string_lossy().to_string();

    let dir = std::env::temp_dir().join(format!(
        "m1-real-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let home = dir.join("home");
    let home_s = home.to_string_lossy().to_string();
    setup_two_source_brain(&exe, &dir, &home_s).await;

    // 授權面：co-common←[s1,s2]（bootstrap）＋proj-x←[s2]；
    // v2 policy：operator 兩個 scope、ai:bob 僅 co-common。
    let db = dir.join("operoid.db");
    let store = crate::domain::SqliteStore::open(&db).unwrap();
    bootstrap_with_sources(&store, &["s1".into()]).unwrap(); // co-common←s1（proj-x←s2 另建）
    store
        .put_scope(&KnowledgeScope {
            id: "proj-x".into(),
            visibility: Visibility::Project,
            classification: "internal".into(),
            source_ids: vec!["s2".into()],
            owner: None,
            department: None,
            project: Some("x".into()),
        })
        .unwrap();
    save_policy_new_version(
        &store,
        vec![
            PolicyRule {
                id: "op".into(),
                priority: 10,
                effect: Effect::Allow,
                principals: Some(vec!["principal-operator".into()]),
                principal_types: None,
                scopes: Some(vec!["co-common".into(), "proj-x".into()]),
                departments: None,
                projects: None,
                classifications: None,
                department_membership: false,
                project_membership: false,
            },
            PolicyRule {
                id: "bob".into(),
                priority: 10,
                effect: Effect::Allow,
                principals: Some(vec!["ai:bob".into()]),
                principal_types: None,
                scopes: Some(vec!["co-common".into()]),
                departments: None,
                projects: None,
                classifications: None,
                department_membership: false,
                project_membership: false,
            },
        ],
    )
    .unwrap();

    let svc = std::sync::Arc::new(KnowledgeService::new(&db));
    let mk_ctx = |access| ToolCtx {
        gbrain_exe: exe.clone(),
        gbrain_home: Some(home_s.clone()),
        chat_model: None,
        mcp: None, // CLI transport（M0-V5：--source 端到端有效）
        allowed_tools: Default::default(),
        employee_output_root: std::env::temp_dir(),
        registry: None,
        access,
        knowledge: Some(std::sync::Arc::clone(&svc)),
    };
    let ctx_op = mk_ctx(crate::knowledge::identity::operator_access_context(
        crate::runtime::AGENT_WS,
    ));
    let ctx_bob = mk_ctx(AccessContext {
        principal_id: "ai:bob".into(),
        principal_type: crate::knowledge::types::PrincipalType::AiEmployee,
        employee_id: Some("bob".into()),
        workspace_id: crate::runtime::AGENT_WS.into(),
        roles: vec![],
        departments: vec![],
        projects: vec![],
        task_id: None,
        purpose: None,
    });

    let q = "protocol";
    let op = svc
        .retrieve(
            &ctx_op.access,
            RetrieveKind::Search,
            q,
            None,
            10,
            &ctx_op,
        )
        .await
        .unwrap();
    let bob = svc
        .retrieve(
            &ctx_bob.access,
            RetrieveKind::Search,
            q,
            None,
            10,
            &ctx_bob,
        )
        .await
        .unwrap();

    // 鐵律：同一腦＋同一查詢＋不同 AccessContext＝不同結果。
    assert!(op.text.contains("alpha"), "operator 應見 s1：{}", op.text);
    assert!(op.text.contains("beta"), "operator 應見 s2：{}", op.text);
    assert!(bob.text.contains("alpha"), "bob 應見 s1：{}", bob.text);
    assert!(
        !bob.text.contains("beta"),
        "bob 不得見 s2（未授權 proj-x）：{}",
        bob.text
    );

    // Test 9 實機版：兩筆 receipt＋兩筆 retrieval 事件。
    let receipts = store.list_recent_receipts(10).unwrap();
    assert_eq!(receipts.len(), 2, "兩次檢索各一筆 receipt");
    assert!(receipts
        .iter()
        .any(|r| r.authorized_sources == vec!["s1".to_string(), "s2".to_string()]));
    assert!(receipts.iter().any(|r| r.authorized_sources == vec!["s1".to_string()]));
    let events = store.list_recent_events(10).unwrap();
    assert_eq!(events.iter().filter(|e| e.kind == "retrieval").count(), 2);

    eprintln!(
        "[real_m1] 鐵律成立：operator → {} 位元組（s1+s2）；bob → {} 位元組（僅 s1）",
        op.text.len(),
        bob.text.len()
    );
    // ToolInput 僅為契約完整性檢查（無身份欄位可走私——I4 的反面證據）。
    let _ = ToolInput { query: q.to_string(), anchor: None, params: None };
    std::fs::remove_dir_all(&dir).ok();
}

/// gbrain 子命令包裝（帶 GBRAIN_HOME 環境；exit code 由呼叫端斷言）。
async fn g(exe: &str, home: &str, args: &[&str]) -> (i32, String, String) {
    crate::gbrain_cli::run_capture(exe, args, &crate::proc::env_for_brain(Some(home)))
        .await
        .expect("spawn gbrain")
}
/// 兩 source 腦建置（s1=alpha／s2=beta；git 內容＋init＋sources add＋逐 source sync）。
/// real_m1 與 G5 fan-out 量測共用。
async fn setup_two_source_brain(exe: &str, dir: &std::path::Path, home_s: &str) {
    // 兩個 source 目錄（git 內容——sync 必要條件，M0 實測）。各含一份標記文件。
    for (name, slug, body) in [
        ("s1", "alpha", "The alpha protocol exists only in source A."),
        ("s2", "beta", "The beta protocol exists only in source B."),
    ] {
        let d = dir.join("src").join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join(format!("{slug}.md")),
            format!("---\ntitle: {slug} Note\ntype: note\n---\n\n{body}\n"),
        )
        .unwrap();
        let st = std::process::Command::new("git")
            .current_dir(&d)
            .args(["init", "-q"])
            .status()
            .unwrap();
        assert!(st.success());
        for a in [
            vec!["add", "-A"],
            vec!["-c", "user.email=m1@test", "-c", "user.name=m1", "commit", "-qm", "seed"],
        ] {
            let st = std::process::Command::new("git")
                .current_dir(&d)
                .args(&a)
                .status()
                .unwrap();
            assert!(st.success(), "git {a:?} 失敗");
        }
    }


    // init＝best-effort：M0/C6 實測 schema 會建好，但 Windows 上末段 skill publication
    // 可能 exit 1（非必要步驟）——真正的閘是 sources add／sync 的 exit 0。
    let (c, _, err) = g(&exe, &home_s, &["init", "--embedding-model", "ollama:embeddinggemma"]).await;
    if c != 0 {
        let _ = g(&exe, &home_s, &["init"]).await;
        eprintln!("[real_m1] gbrain init exit {c}（skill publication 等）——續行：{err}");
    }
    for s in ["s1", "s2"] {
        let path = dir.join("src").join(s).to_string_lossy().to_string();
        let (c, _, err) = g(&exe, &home_s, &["sources", "add", s, "--path", &path, "--force"])
            .await;
        assert_eq!(c, 0, "sources add {s} 失敗：{err}");
    }
    // 逐 source 同步（--all 會踩到空的 default source——M0/C6 實測）。
    for s in ["s1", "s2"] {
        let (c, _, err) = g(
            &exe,
            &home_s,
            &["sync", "--source", s, "--no-pull", "--yes", "--no-hard-deadline"],
        )
        .await;
        assert_eq!(c, 0, "sync {s} 失敗：{err}");
    }
}
// ── G5 效能量測（§17；數據餵 C7 專檔的裁決建議）────────────────────────────

/// **G5 量測 1**：plan() 開銷＝policy 載入＋scopes 讀取＋純函式評估（非 gbrain 路徑）。
#[test]
fn bench_g5_plan_overhead() {
    use crate::domain::Store as _;
    use crate::knowledge::bootstrap::bootstrap_with_sources;
    use crate::knowledge::service::KnowledgeService;
    use std::time::Instant;
    let dir = std::env::temp_dir().join(format!(
        "g5plan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("test.db");
    let store = crate::domain::SqliteStore::open(&db).unwrap();
    bootstrap_with_sources(&store, &["s1".into(), "s2".into(), "s3".into()]).unwrap();
    let svc = KnowledgeService::new(&db);
    let access = crate::knowledge::identity::operator_access_context(crate::runtime::AGENT_WS);
    for _ in 0..10 { let _ = svc.plan(&store, &access).unwrap(); } // 暖機
    let n = 500;
    let t0 = Instant::now();
    for _ in 0..n { let _ = svc.plan(&store, &access).unwrap(); }
    let avg_us = t0.elapsed().as_micros() as f64 / n as f64;
    eprintln!("[G5] plan() 平均 {avg_us:.1} µs/次（{n} 次；SQLite 本地＋純函式評估）");
    assert!(avg_us < 5_000.0, "plan 平均應遠低於 5ms（實測 {avg_us:.1} µs）");
    std::fs::remove_dir_all(&dir).ok();
}

/// **G5 量測 2**：fan-out 線性度——授權 1 vs 2 個 source 的 retrieve 時間（CLI transport）。
#[ignore = "真實環境相依：需本機 gbrain；手動跑"]
#[tokio::test]
async fn real_g5_fanout_scaling() {
    use crate::domain::Store as _;
    use crate::knowledge::backend::RetrieveKind;
    use crate::knowledge::bootstrap::bootstrap_with_sources;
    use crate::knowledge::service::KnowledgeService;
    use crate::knowledge::types::KnowledgeScope;
    use std::time::Instant;
    let exe = dirs::home_dir()
        .map(|h| h.join(".bun").join("bin").join("gbrain.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from("gbrain"));
    let exe = exe.to_string_lossy().to_string();
    let dir = std::env::temp_dir().join(format!(
        "g5fan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let home_s = dir.join("home").to_string_lossy().to_string();
    setup_two_source_brain(&exe, &dir, &home_s).await;
    let db = dir.join("operoid.db");
    let store = crate::domain::SqliteStore::open(&db).unwrap();
    bootstrap_with_sources(&store, &["s1".into()]).unwrap(); // co-common←[s1]
    let svc = std::sync::Arc::new(KnowledgeService::new(&db));
    let access = crate::knowledge::identity::operator_access_context(crate::runtime::AGENT_WS);
    let ctx = ToolCtx {
        gbrain_exe: exe.clone(),
        gbrain_home: Some(home_s.clone()),
        chat_model: None,
        mcp: None,
        allowed_tools: Default::default(),
        employee_output_root: std::env::temp_dir(),
        registry: None,
        access: access.clone(),
        knowledge: Some(std::sync::Arc::clone(&svc)),
    };
    async fn timed(svc: &KnowledgeService, ctx: &ToolCtx) -> (f64, crate::domain::tools::ToolOutput) {
        let t0 = Instant::now();
        let out = svc
            .retrieve(&ctx.access, RetrieveKind::Search, "protocol", None, 10, ctx)
            .await
            .expect("retrieve");
        (t0.elapsed().as_millis() as f64, out)
    }
    // 1 source
    let mut one = Vec::new();
    for _ in 0..3 { one.push(timed(&svc, &ctx).await); }
    // 2 sources：co-common 擴為 [s1,s2]
    store
        .put_scope(&KnowledgeScope {
            id: "co-common".into(),
            visibility: crate::knowledge::types::Visibility::Company,
            classification: "internal".into(),
            source_ids: vec!["s1".into(), "s2".into()],
            owner: None,
            department: None,
            project: None,
        })
        .unwrap();
    let mut two = Vec::new();
    for _ in 0..3 { two.push(timed(&svc, &ctx).await); }
    let avg = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    eprintln!(
        "[G5] fan-out：1 source 平均 {:.0} ms；2 sources 平均 {:.0} ms（CLI spawn 主導；線性於授權 source 數）",
        avg(&one.iter().map(|t| t.0).collect::<Vec<_>>()),
        avg(&two.iter().map(|t| t.0).collect::<Vec<_>>()),
    );
    assert!(two.iter().all(|t| t.1.text.contains("beta")), "2-source 授權應見 s2");
    std::fs::remove_dir_all(&dir).ok();
}