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
            },
            PolicyRule {
                id: "bob".into(),
                priority: 10,
                effect: Effect::Allow,
                principals: Some(vec!["ai:bob".into()]),
                principal_types: None,
                scopes: Some(vec!["co-common".into()]),
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