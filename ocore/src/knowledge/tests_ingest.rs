//! PDF 知識入庫 real test（#[ignore]——需本機 MinerU、gbrain、llama-server；手動跑）。
//!
//! 跑法：`cargo test -p ocore real_ingest -- --ignored --nocapture`
//!
//! **生產閉環驗證**：mueller2016.pdf → ingest_pdf（K1 轉換→notes 入 repo→
//! 圖檔＋註解頁發布→git→sync）→ **二次入庫冪等**（筆記覆寫、圖檔不重寫、
//! 錨點不重複）。圖片的 gbrain 原生 import（多模態向量＋image_of）已在
//! demo 腦試點驗證（TODO 項目 3、`tests_figures_lane`）——臨時腦未配置
//! multimodal，sync 會跳過圖片，故本測試只驗證檔案面與文字面。

use crate::converters::mineru::ConvertConfig;
use crate::converters::mineru::router::ConvertPolicy;
use crate::knowledge::ingest::ingest_pdf;
use crate::gbrain_cli::run_capture;
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
    // ── 第一次入庫（端到端：MinerU 解析→筆記→圖檔發布→sync）──
    eprintln!("[ingest] 第一次入庫（MinerU basic，CPU 約 10 秒/頁）…");
    let r1 = ingest_pdf(pdf, &cfg, &notes_repo, "ing", &gbrain_exe, Some(&home_s))
        .await
        .expect("第一次入庫應成功");
    eprintln!(
        "[ingest] #1 notes={} figures_published={} synced={} warnings={:?}",
        r1.notes_written, r1.figures_published, r1.synced, r1.warnings
    );
    assert_eq!(r1.notes_written, 44, "27 文字＋9 圖筆記＋8 圖檔");
    assert_eq!(r1.figures_published, 8, "8 張帶檔圖（fig3 caption-only 無檔）");
    assert!(r1.synced, "sync 應成功：{:?}", r1.warnings);
    assert!(notes_repo.join("mueller2016").join("fig8-p6.md").exists());
    assert!(
        notes_repo.join("mueller2016").join("fig8-p6.jpg").exists(),
        "同名圖檔應發布至 doc 目錄"
    );
    // 註解頁補強：影像檔 wikilink＋原論文 PDF 出處＋匯入錨點。
    let fig_md = std::fs::read_to_string(notes_repo.join("mueller2016").join("fig8-p6.md")).unwrap();
    assert!(fig_md.contains("影像檔：[[mueller2016/fig8-p6.jpg"), "wikilink 應指向同名圖檔：{fig_md}");
    assert!(fig_md.contains("原論文 PDF："), "應保留來源 PDF 出處");
    assert!(fig_md.contains("— 匯入：Figure 8"), "應含匯入 timeline 錨點");
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

    // ── 第二次入庫：冪等——筆記覆寫、圖檔不重寫（0 新發布）──
    let r2 = ingest_pdf(pdf, &cfg, &notes_repo, "ing", &gbrain_exe, Some(&home_s))
        .await
        .expect("第二次入庫應成功");
    eprintln!(
        "[ingest] #2 notes={} figures_published={} synced={}",
        r2.notes_written, r2.figures_published, r2.synced
    );
    assert_eq!(r2.notes_written, 36, "md 重複製（補強行由發布步驟重加）；圖檔已存在不計");
    assert_eq!(r2.figures_published, 0, "冪等：同名圖檔已存在不重寫");
    assert!(r2.synced);

    std::fs::remove_dir_all(&dir).ok();
}

/// 編譯期守門：ingest 政策面（ConvertConfig 預設＝Auto 真實分流）。
#[test]
fn policy_default_is_auto() {
    assert_eq!(ConvertConfig::default().policy, ConvertPolicy::Auto);
}

/// W2 出處鏈：doc 目錄頁的 `source_pdf` frontmatter 可讀出（含路徑內空白——
/// 單引號 scalar）；無鍵 → None（fail-closed）。
#[test]
fn read_doc_source_pdf_reads_frontmatter() {
    let dir = std::env::temp_dir().join(format!("srcpdf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let doc = dir.join("mueller2016");
    std::fs::create_dir_all(&doc).unwrap();
    std::fs::write(
        doc.join("s00.md"),
        "---\norigin: operoid-employee\nsource_pdf: 'C:/papers/mueller 2016.pdf'\n---\nbody\n",
    )
    .unwrap();
    assert_eq!(
        crate::knowledge::ingest::read_doc_source_pdf(&doc).as_deref(),
        Some("C:/papers/mueller 2016.pdf")
    );
    let doc2 = dir.join("nokey");
    std::fs::create_dir_all(&doc2).unwrap();
    std::fs::write(doc2.join("s00.md"), "---\norigin: x\n---\nbody\n").unwrap();
    assert!(crate::knowledge::ingest::read_doc_source_pdf(&doc2).is_none());
    std::fs::remove_dir_all(&dir).ok();
}

/// W2 出處鏈：來源清單取得失敗（CLI 不存在）→ 空表——best-effort，
/// 檢索面不得因出處鏈失敗而失效。
#[tokio::test]
async fn collect_source_docs_fails_open_empty() {
    let out = crate::knowledge::ingest::collect_source_docs(
        "definitely-not-a-real-exe",
        None,
        &["mueller2016".to_string()],
    )
    .await;
    assert!(out.is_empty());
}

/// 拖拽流驗證（#[ignore]）：工廠 run_core 全 PDF 批次 → 知識管線自動全做
/// （K1 轉換→筆記寫入→圖檔＋註解頁發布），不需要單獨按鈕。
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
    let mut cfg = crate::app_config::AppConfig::default();
    cfg.notes_repo_path = notes.to_string_lossy().into_owned();
    cfg.mineru_command = Some(mineru.to_string_lossy().into_owned());

    let result = crate::factories::run_core(&cfg, "pdf", &[PDF.to_string()], None, None, None)
        .await
        .expect("拖 PDF 進工廠應成功");
    eprintln!(
        "[drag] total={} pages={} written={} errors={:?}",
        result.total,
        result.sample.len(),
        result.written.len(),
        result.errors
    );
    assert_eq!(result.total, 44, "27 文字＋9 圖筆記＋8 圖檔");
    assert_eq!(result.factory, "pdf-knowledge");
    assert!(notes.join("mueller2016").join("fig8-p6.md").exists());
    assert!(
        notes.join("mueller2016").join("fig8-p6.jpg").exists(),
        "同名圖檔應發布至 doc 目錄"
    );
    // 工廠不寫 sidecar（gbrain 原生 image import 軌道）。
    assert!(!notes.with_file_name("notes.figures.sqlite").exists());

    std::fs::remove_dir_all(&dir).ok();
}
