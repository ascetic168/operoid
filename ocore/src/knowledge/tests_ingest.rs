//! PDF 知識入庫 real test（#[ignore]——需本機 MinerU、gbrain、llama-server；手動跑）。
//!
//! 跑法：`cargo test -p ocore real_ingest -- --ignored --nocapture`
//!
//! **生產閉環驗證**：mueller2016.pdf → ingest_pdf（K1 轉換→notes 入 repo→git→
//! sync→sidecar 合併→向量回填）→ **二次入庫冪等**（列數不變、向量攜帶、零重嵌）。

use crate::converters::mineru::ConvertConfig;
use crate::converters::mineru::router::ConvertPolicy;
use crate::knowledge::ingest::ingest_pdf;
use crate::gbrain_cli::run_capture;
use crate::knowledge::figures::Sidecar;
use std::path::Path;

const PDF: &str = r"C:\Users\charl\Documents\python\pdf2zh\mueller2016.pdf";
const MINERU: &str = r"C:\Users\charl\Documents\python\.mineru\Scripts\mineru-kit.exe";

#[ignore = "真實環境相依：需 MinerU venv、本機 gbrain＋llama-server（mmproj）"]
#[tokio::test]
async fn real_ingest_pdf_end_to_end_idempotent() {
    let pdf = Path::new(PDF);
    let mineru = Path::new(MINERU);
    for p in [pdf, mineru] {
        assert!(p.exists(), "缺少語料/工具：{}", p.display());
    }
    let gbrain_exe = dirs::home_dir()
        .map(|h| h.join(".bun").join("bin").join("gbrain.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from("gbrain"));
    let gbrain_exe = gbrain_exe.to_string_lossy().into_owned();

    let dir = std::env::temp_dir().join(format!(
        "ingest-real-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let notes_repo = dir.join("notes");
    let figures_db = dir.join("figures.sqlite");
    let home_s = dir.join("home").to_string_lossy().into_owned();
    let env = crate::proc::env_for_brain(Some(&home_s));

    // notes repo＋gbrain 腦＋source 註冊（repo 初始 commit 由 ingest 的
    // git_add_commit 完成——先建可註冊的空 repo）。
    std::fs::create_dir_all(&notes_repo).unwrap();
    for args in [
        vec!["init", "-q"],
        vec!["-c", "user.email=i@test", "-c", "user.name=i", "commit", "-qm", "seed", "--allow-empty"],
    ] {
        let st = std::process::Command::new("git")
            .current_dir(&notes_repo)
            .args(&args)
            .status()
            .unwrap();
        assert!(st.success() || args[0] == "commit", "git {args:?} 失敗");
    }
    let (c, _, err) = run_capture(
        &gbrain_exe,
        &[
            "init",
            "--embedding-model",
            "llama-server:embeddinggemma-2",
            "--embedding-dimensions",
            "768",
        ],
        &env,
    )
    .await
    .unwrap();
    if c != 0 {
        eprintln!("[ingest] gbrain init exit {c}（非閘）：{err}");
    }
    let (c, _, err) = run_capture(
        &gbrain_exe,
        &["sources", "add", "ing", "--path", &notes_repo.to_string_lossy(), "--force"],
        &env,
    )
        .await
        .unwrap();
    assert_eq!(c, 0, "sources add：{err}");

    let cfg = ConvertConfig {
        mineru_command: Some(mineru.to_string_lossy().into_owned()),
        ..Default::default()
    }; // policy=Auto：真實分流（S3 訊號 → MinerU）
    // ── 第一次入庫（端到端：MinerU 解析→筆記→sync→sidecar→向量）──
    eprintln!("[ingest] 第一次入庫（MinerU basic，CPU 約 10 秒/頁）…");
    let r1 = ingest_pdf(
        pdf,
        &cfg,
        &figures_db,
        &notes_repo,
        "ing",
        &gbrain_exe,
        Some(&home_s),
    )
    .await
    .expect("第一次入庫應成功");
    eprintln!(
        "[ingest] #1 notes={} figures={} carried={} embedded={} synced={} warnings={:?}",
        r1.notes_written, r1.figure_rows, r1.vectors_carried, r1.vectors_embedded, r1.synced, r1.warnings
    );
    assert_eq!(r1.notes_written, 36, "27 文字＋9 圖筆記");
    assert_eq!(r1.figure_rows, 9);
    assert!(r1.synced, "sync 應成功：{:?}", r1.warnings);
    assert!(r1.vectors_embedded >= 9, "首輪應實嵌全部圖列");
    assert!(notes_repo.join("mueller2016").join("fig8-p6.md").exists());

    // 檢索冒煙：gbrain 文字面應可命中（sync＋embed 已跑）。
    let (qc, qout, qerr) = run_capture(
        &gbrain_exe,
        &[
            "query",
            "task: search result | query: efficiency versus number of phases for conversions",
            "--limit",
            "5",
            "--source",
            "ing",
            "--no-expand",
            "--json",
        ],
        &env,
    )
    .await
    .unwrap();
    assert_eq!(qc, 0, "query：{qerr}");
    assert!(qout.contains("mueller2016"), "應命中入庫的筆記：{qout}");

    // ── 第二次入庫：冪等——列數不變、向量攜帶（dedup）、零重嵌 ──
    let r2 = ingest_pdf(
        pdf,
        &cfg,
        &figures_db,
        &notes_repo,
        "ing",
        &gbrain_exe,
        Some(&home_s),
    )
    .await
    .expect("第二次入庫應成功");
    eprintln!(
        "[ingest] #2 notes={} figures={} carried={} embedded={} synced={}",
        r2.notes_written, r2.figure_rows, r2.vectors_carried, r2.vectors_embedded, r2.synced
    );
    assert_eq!(r2.figure_rows, 9, "冪等：列數不變");
    // 8 張帶 md5 的圖列：向量攜帶（零重嵌）；fig3（caption-only、無 md5）：
    // caption 可能變更 → 如設計地重嵌文字向量（成本可忽略）。
    assert_eq!(r2.vectors_carried, 8, "md5 攜帶：圖列向量全數繼承");
    assert_eq!(r2.vectors_embedded, 1, "caption-only 列重嵌文字向量");
    assert!(r2.synced);

    // sidecar 的向量確實在（供 K4 融合）。
    let sc = Sidecar::open(&figures_db).unwrap();
    let (rows, with_vec) = sc.doc_stats("mueller2016").unwrap();
    assert_eq!((rows, with_vec), (9, 9));

    std::fs::remove_dir_all(&dir).ok();
}

/// 編譯期守門：ingest 政策面（ConvertConfig 預設＝Auto 真實分流）。
#[test]
fn policy_default_is_auto() {
    assert_eq!(ConvertConfig::default().policy, ConvertPolicy::Auto);
}

/// 拖拽流驗證（#[ignore]）：工廠 run_core 全 PDF 批次 → 知識管線自動全做
/// （K1 轉換→筆記寫入→sidecar→向量），不需要單獨按鈕。
#[ignore = "真實環境相依：需 MinerU venv、llama-server（mmproj）"]
#[tokio::test]
async fn real_factory_pdf_drag_flow() {
    let pdf = Path::new(PDF);
    let mineru = Path::new(MINERU);
    for p in [pdf, mineru] {
        assert!(p.exists(), "缺少 {}", p.display());
    }
    let dir = std::env::temp_dir().join(format!(
        "drag-real-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let notes = dir.join("notes");
    let figures_db = dir.join("notes.figures.sqlite");
    let mut cfg = crate::app_config::AppConfig::default();
    cfg.notes_repo_path = notes.to_string_lossy().into_owned();
    cfg.mineru_command = Some(mineru.to_string_lossy().into_owned());
    // figures_db_path 未設 → 工廠用 notes repo 伴隨檔（預設鏈）。

    let result = crate::factories::run_core(&cfg, "pdf", &[PDF.to_string()], None)
        .await
        .expect("拖 PDF 進工廠應成功");
    eprintln!(
        "[drag] total={} pages={} written={} errors={:?}",
        result.total,
        result.sample.len(),
        result.written.len(),
        result.errors
    );
    assert_eq!(result.total, 36, "27 文字＋9 圖筆記");
    assert_eq!(result.factory, "pdf-knowledge");
    assert!(notes.join("mueller2016").join("fig8-p6.md").exists());
    let sc = Sidecar::open(&figures_db).unwrap();
    let (rows, with_vec) = sc.doc_stats("mueller2016").unwrap();
    assert_eq!((rows, with_vec), (9, 9), "sidecar 9 列全向量");

    std::fs::remove_dir_all(&dir).ok();
}
