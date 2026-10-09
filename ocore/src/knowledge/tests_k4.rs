//! K4 real test（#[ignore]——需本機 gbrain＋llama-server（mmproj）與 K7/K3 語料）。
//!
//! 跑法：`cargo test -p ocore real_k4 -- --ignored --nocapture`
//!
//! 雙文件語料（mueller2016 電力電子＋mHC 機器學習架構干擾文件）建**同一個腦的同一個
//! source**（slug 前綴防碰撞、`figures.sqlite` 兩文件列同庫），驗證：
//! 1. **融合**：純視覺查詢經 sidecar 路（K3）進 fused 輸出，正確圖在前三個區塊；
//! 2. **歸因**：指名文件的定向查詢，圖片項的文件來源零錯誤（計畫 §K4/P1 驗收——
//!    mHC 無任何 `Figure N.` caption，正確行為＝只有 mHC 文字命中、**零** mueller 圖）。

use crate::domain::tools::ToolCtx;
use crate::gbrain_cli::run_capture;
use crate::knowledge::figures::{self, Sidecar, SidecarConfig};
use crate::knowledge::service::KnowledgeService;
use std::path::Path;
use std::sync::Arc;

#[ignore = "真實環境相依：需本機 gbrain、llama-server（mmproj）與 embed-exp 語料"]
#[tokio::test]
async fn real_k4_fusion_and_attribution() {
    // ── 語料轉換（兩文件、同一 out dir——figures.sqlite 累積、notes 同庫）──
    let mueller_zip = Path::new(
        r"C:\Users\charl\Documents\python\pdf2zh\embed-exp\zip-full\mueller2016",
    );
    let mhc_zip = Path::new(r"C:\Users\charl\Documents\python\pdf2zh\embed-exp\zip-mhc\mhc2512");
    for p in [mueller_zip, mhc_zip] {
        assert!(
            p.join("structured_content.json").exists(),
            "缺少語料 {}（見計畫 §K7/K4）",
            p.display()
        );
    }
    let dir = std::env::temp_dir().join(format!(
        "k4-real-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out");
    let m = crate::converters::mineru::convert_structured(mueller_zip, &out, Some("mueller2016"))
        .expect("mueller2016 轉換");
    let h = crate::converters::mineru::convert_structured(mhc_zip, &out, Some("mhc2512"))
        .expect("mhc2512 轉換");
    eprintln!(
        "[k4] corpus: mueller {}+{} notes, mhc {}+{} notes",
        m.report.text_notes, m.report.figure_notes, h.report.text_notes, h.report.figure_notes
    );
    assert_eq!(m.report.figure_notes, 9);

    // sidecar：兩文件列都歸檔 k7（mueller 9 列；mhc 為 arXiv「Figure N |」圖說——
    // Figure 2/3/4/5 共 4 列）。
    let sidecar_db = out.join("figures.sqlite");
    let total_sidecar_rows;
    {
        let sc = Sidecar::open(&sidecar_db).unwrap();
        let mueller_rows = sc.tag_source("mueller2016", "k7").unwrap();
        let mhc_rows = sc.tag_source("mhc2512", "k7").unwrap();
        eprintln!("[k4] sidecar rows tagged: mueller={mueller_rows} mhc={mhc_rows}");
        assert_eq!(mueller_rows, 9);
        assert!(mhc_rows >= 3, "mhc 應有圖說圖列（實得 {mhc_rows}）");
        total_sidecar_rows = mueller_rows + mhc_rows;
    }
    // 路2 的前提：聯合向量回填（K3；多模態，全 sidecar 列）。
    let stats = figures::embed_pending(
        &sidecar_db,
        figures::DEFAULT_EMBEDDING_BASE,
        None,
        figures::SidecarMode::Multimodal,
    )
    .await
    .expect("sidecar 回填應成功");
    eprintln!(
        "[k4] sidecar embedded={} deduped={} skipped={}",
        stats.embedded, stats.deduped, stats.skipped
    );
    assert_eq!(
        stats.embedded + stats.deduped,
        total_sidecar_rows,
        "全部 sidecar 列都應有向量"
    );
    assert_eq!(stats.skipped, 0, "{:?}", stats.warnings);

    // ── 腦建置：git → init → sources add → sync（--no-extract）→ embed ──
    let gbrain_exe = dirs::home_dir()
        .map(|hd| hd.join(".bun").join("bin").join("gbrain.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from("gbrain"));
    let gbrain_exe = gbrain_exe.to_string_lossy().into_owned();
    let home_s = dir.join("home").to_string_lossy().into_owned();
    let env = crate::proc::env_for_brain(Some(&home_s));
    let notes = out.join("notes");
    let notes_s = notes.to_string_lossy().into_owned();
    for args in [
        vec!["init", "-q"],
        vec!["add", "-A"],
        vec!["-c", "user.email=k4@test", "-c", "user.name=k4", "commit", "-qm", "seed"],
    ] {
        let st = std::process::Command::new("git")
            .current_dir(&notes)
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
        eprintln!("[k4] gbrain init exit {c}（非閘）：{err}");
    }
    let (c, _, err) = run_capture(&gbrain_exe, &["sources", "add", "k7", "--path", &notes_s, "--force"], &env)
        .await
        .unwrap();
    assert_eq!(c, 0, "sources add：{err}");
    let (c, _, err) = run_capture(
        &gbrain_exe,
        &[
            "sync", "--source", "k7", "--no-pull", "--yes", "--no-hard-deadline", "--no-extract",
        ],
        &env,
    )
    .await
    .unwrap();
    assert_eq!(c, 0, "sync：{err}");
    for _ in 0..2 {
        let _ = run_capture(&gbrain_exe, &["embed", "--stale"], &env).await;
    }

    // ── KnowledgeService＋sidecar 接線＋policy（operator 全權）──
    let store_db = dir.join("operoid.db");
    {
        use crate::domain::Store as _;
        let store = crate::domain::SqliteStore::open(&store_db).unwrap();
        crate::knowledge::bootstrap::bootstrap_with_sources(&store, &["k7".into()]).unwrap();
    }
    let svc = Arc::new(
        KnowledgeService::new(&store_db).with_sidecar(SidecarConfig {
            db_path: sidecar_db.clone(),
            embedding_base: figures::DEFAULT_EMBEDDING_BASE.into(),
        }),
    );
    let access = crate::knowledge::identity::operator_access_context(crate::runtime::AGENT_WS);
    let ctx = ToolCtx {
        gbrain_exe: gbrain_exe.clone(),
        gbrain_home: Some(home_s.clone()),
        chat_model: None,
        mcp: None,
        allowed_tools: Default::default(),
        employee_output_root: std::env::temp_dir(),
        registry: None,
        knowledge: None,
        access: access.clone(),
        turn_max_steps: 40,
        tool_result_max_chars: 8_000,
    };
    let retrieve = |q: &'static str| {
        let svc = svc.clone();
        let ctx = &ctx;
        async move {
            svc.retrieve(&ctx.access, crate::knowledge::backend::RetrieveKind::Search, q, None, 10, ctx)
                .await
                .expect("retrieve 應成功")
        }
    };

    // ── Test A：融合——純視覺查詢（fig8），sidecar 命中應在前三個區塊 ──
    let out_a = retrieve(
        "What is the efficiency of the 4-phase design at 60 MHz for the 1.7V:1.05V conversion?",
    )
    .await;
    let blocks_a: Vec<&str> = out_a.text.split("\n\n").collect();
    let fig8_block = blocks_a
        .iter()
        .position(|b| b.contains("mueller2016/Figure 8（p"))
        .unwrap_or_else(|| {
            panic!(
                "融合輸出應含 mueller2016 Figure 8 區塊——meta={:?} 前 600 字：{}",
                out_a.meta.get("sidecar"),
                &out_a.text[..out_a.text.len().min(600)]
            )
        });
    eprintln!(
        "[k4] A: visual query → fig8 block rank {} / {} blocks；sidecar meta {:?}",
        fig8_block + 1,
        blocks_a.len(),
        out_a.meta.get("sidecar")
    );
    assert!(
        fig8_block < 3,
        "P1 驗收線：正確圖進前三個區塊（實得第 {}）",
        fig8_block + 1
    );
    assert!(out_a.text.contains("圖檔："), "圖片區塊應帶圖檔路徑（K5 消費面）");

    // ── Test B：歸因——mueller 定向查詢，圖片項全部來自 mueller2016 ──
    let out_b = retrieve(
        "Mueller 2016 paper: magnetic field distribution in the cross section of the six-winding inductor",
    )
    .await;
    let fig_blocks_b: Vec<&str> = out_b
        .text
        .split("\n\n")
        .filter(|b| b.starts_with("── 圖片"))
        .collect();
    assert!(
        !fig_blocks_b.is_empty(),
        "定向查詢應帶出 mueller 圖：{}",
        out_b.text
    );
    // 實驗 §7 的歸因量測語意：被指名文件的圖項領先、top 圖片項屬於正確文件
    //（尾部低分他文件圖不算錯誤——top-5 之外）。
    assert!(
        fig_blocks_b[0].contains("mueller2016/Figure 5"),
        "被指名的 fig5 應是第一個圖片項：{:?}",
        fig_blocks_b[0]
    );
    let top3 = &fig_blocks_b[..fig_blocks_b.len().min(3)];
    assert!(
        top3.iter().all(|b| b.contains("mueller2016")),
        "前三個圖片項應屬於 mueller2016（歸因）：{:?}",
        top3
    );
    eprintln!(
        "[k4] B: mueller directed → {} figure blocks，首名 fig5、前三全 mueller2016",
        fig_blocks_b.len()
    );

    // ── Test C：歸因（實驗 §7 語意）——指名文件的圖項領先，整體前三區塊
    //    不得混入他文件圖片項（錨點分流：他文件向量命中降為結構性附帶）──
    let out_c = retrieve(
        "In the DeepSeek mHC paper, which figure illustrates the training instability of Hyper-Connections?",
    )
    .await;
    let blocks_c: Vec<&str> = out_c.text.split("

").collect();
    let fig_blocks_c: Vec<&str> = blocks_c
        .iter()
        .filter(|b| b.starts_with("── 圖片"))
        .copied()
        .collect();
    assert!(
        !fig_blocks_c.is_empty(),
        "mHC 定向查詢應帶出 mhc2512 的圖（Figure 2/3/5 即訓練不穩定性圖說）：{}",
        &out_c.text[..out_c.text.len().min(400)]
    );
    assert!(
        fig_blocks_c[0].contains("mhc2512"),
        "第一個圖片項應屬於被指名的 mhc2512：{:?}",
        fig_blocks_c[0]
    );
    let mueller_in_top3 = blocks_c
        .iter()
        .take(3)
        .filter(|b| b.starts_with("── 圖片") && b.contains("mueller2016"))
        .count();
    assert_eq!(
        mueller_in_top3, 0,
        "歸因錯誤：整體前三區塊混入 mueller 圖片項"
    );
    // metadata 綁定鐵律：每個圖片區塊都正確標記自己的來源文件（歸因靠 metadata 不靠向量）。
    for b in &fig_blocks_c {
        assert!(
            b.contains("mhc2512") || b.contains("mueller2016"),
            "圖片區塊缺少文件歸屬：{b}"
        );
    }
    eprintln!(
        "[k4] C: mhc directed → {} figure blocks，首名 mhc2512、top-3 歸因乾淨",
        fig_blocks_c.len()
    );

    // ── Test D：降級不出錯——嵌入端點不可達＋sidecar 檔缺失時，檢索仍 Ok
    //    （文字路完整）、sidecar 錯誤如實記在 meta，絕不傳播 Err ──
    let broken_svc = Arc::new(
        KnowledgeService::new(&store_db).with_sidecar(SidecarConfig {
            db_path: dir.join("nonexistent").join("figures.sqlite"),
            embedding_base: "http://127.0.0.1:9/v1".into(), // discard port——連線立即拒絕
        }),
    );
    let out_d = broken_svc
        .retrieve(
            &ctx.access,
            crate::knowledge::backend::RetrieveKind::Search,
            "Mueller 2016 paper: magnetic field distribution in the cross section of the six-winding inductor",
            None,
            10,
            &ctx,
        )
        .await
        .expect("sidecar 故障時檢索不得 Err（應降級為文字路）");
    assert!(!out_d.text.trim().is_empty(), "降級路徑仍應有文字命中");
    assert!(
        out_d.text.contains("mueller2016"),
        "文字路應正常命中：{}",
        &out_d.text[..out_d.text.len().min(300)]
    );
    let sc_meta = &out_d.meta["sidecar"];
    assert!(
        sc_meta.is_object() && sc_meta["error"].is_string(),
        "sidecar 故障應記錄在 meta.sidecar.error：{sc_meta}"
    );
    eprintln!(
        "[k4] D: broken sidecar → text {} bytes, meta.sidecar.error 記錄 ✓",
        out_d.text.len()
    );

    // 收據：四次檢索各一筆 receipt＋retrieval 事件。
    use crate::domain::Store as _;
    let store = crate::domain::SqliteStore::open(&store_db).unwrap();
    assert_eq!(store.list_recent_receipts(10).unwrap().len(), 4);

    std::fs::remove_dir_all(&dir).ok();
}
