//! K1 端到端層級測試（合成 v4 語料；真實語料的實機測試見 `converters/mineru_real_tests.rs`）。

use super::*;
use std::path::PathBuf;

/// 合成 MinerU unzipped 輸出（structured_content.json＋images/）。
fn sample_zip_dir(dir: &Path) -> PathBuf {
    let zip_dir = dir.join("mueller2016");
    let images = zip_dir.join("images");
    std::fs::create_dir_all(&images).unwrap();
    std::fs::write(
        zip_dir.join("structured_content.json"),
        r#"{
          "pages": [
            {"page_idx": 0, "blocks": [
              {"type": "doc_title", "level": 1, "content": "Design of a Test Document"},
              {"type": "text", "content": "Abstract text of the paper."},
              {"type": "paragraph_title", "level": 2, "content": "I. INTRODUCTION"},
              {"type": "text", "content": "Intro paragraph one with some length. Intro paragraph two."},
              {"type": "image", "content": "", "captions": [{"content": "Figure 1. Two-chip solution for a system-in-package IVR."}], "image_source": "images/page_1_image_7.jpg"},
              {"type": "paragraph_title", "level": 2, "content": "II. DESIGN"},
              {"type": "paragraph_title", "level": 2, "content": "A. Details"},
              {"type": "table", "content": "| L | C |\n| --- | --- |\n| 100nH | 100nF |",
               "captions": [{"content": "TABLE I. PARAMETERS USED"}, {"content": "Figure 2. Loss comparison breakdown."}], "image_source": "images/page_1_table_0.jpg"},
              {"type": "chart", "content": "", "captions": [{"content": "Figure 3. Efficiency numbers for phases."}], "image_source": "images/page_1_chart_1.jpg"}
            ]},
            {"page_idx": 1, "blocks": [
              {"type": "paragraph_title", "level": 2, "content": "REFERENCES"},
              {"type": "ref_text", "content": "[1] Ref one."}
            ]}
          ],
          "is_full_document": true
        }"#,
    )
    .unwrap();
    // 兩張真實圖檔（驗 md5/image_path；table crop 也留著但不入 sidecar——P0 邊界）。
    std::fs::write(images.join("page_1_image_7.jpg"), b"jpeg-bytes-fig1").unwrap();
    std::fs::write(images.join("page_1_chart_1.jpg"), b"jpeg-bytes-fig3").unwrap();
    std::fs::write(images.join("page_1_table_0.jpg"), b"jpeg-bytes-table").unwrap();
    zip_dir
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "mineru-k1-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// convert_structured：筆記數量/slug/body、sidecar 列（md5/路徑）、conversion-report。
#[test]
fn structured_to_notes_and_sidecar() {
    let dir = temp_dir("conv");
    let zip_dir = sample_zip_dir(&dir);
    let out = dir.join("out");
    let outcome = convert_structured(&zip_dir, &out, Some("mueller2016")).unwrap();

    // 文字 chunk：front＋I＋II.A＝3；圖筆記：fig1/2/3＝3（Figure 2 由 table caption 拾回）。
    assert_eq!(outcome.report.text_notes, 3, "notes: {:#?}", outcome.notes);
    assert_eq!(outcome.report.figure_notes, 3);
    let by_slug = |s: &str| outcome.notes.iter().find(|n| n.slug == s).unwrap();
    let s00 = by_slug("mueller2016/s00");
    assert_eq!(s00.kind, NoteKind::Text);
    assert_eq!(s00.section_id, "front");
    assert!(std::fs::read_to_string(&s00.path).unwrap().contains("title: Design of a Test Document | text: [Front matter] Abstract text of the paper."));

    let fig1 = by_slug("mueller2016/fig1-p1");
    assert_eq!(fig1.figs, vec![1]);
    let body = std::fs::read_to_string(&fig1.path).unwrap();
    assert!(body.contains("text: [Figure 1 in Section I, page 1] Figure 1. Two-chip solution"), "{body}");

    // 章節邊界：s01=I（含圖 caption 內嵌）、s02=II.A（表格原子＋拾回圖）。
    let s01 = by_slug("mueller2016/s01");
    assert_eq!(s01.section_id, "I");
    assert_eq!(s01.figs, vec![1]);
    let s02 = by_slug("mueller2016/s02");
    assert_eq!(s02.section_id, "II.A");
    assert_eq!(s02.tables, vec!["I"]);
    assert_eq!(s02.figs, vec![2, 3], "表格原子＋拾回圖＋chart 圖同章節");
    let body2 = std::fs::read_to_string(&s02.path).unwrap();
    assert!(body2.contains("TABLE I. PARAMETERS USED"));
    assert!(body2.contains("| 100nH | 100nF |"));
    assert!(body2.contains("[Figure 2. Loss comparison breakdown.]"));

    // REFERENCES skip：ref_text 不入任何筆記。
    assert!(!outcome.notes.iter().any(|n| {
        std::fs::read_to_string(&n.path).unwrap().contains("Ref one.")
    }));

    // figures.sqlite：3 列（fig1/3 有圖＋md5；fig2 caption-only 無圖）；vec 為 NULL。
    let db = rusqlite::Connection::open(out.join("figures.sqlite")).unwrap();
    let n: i64 = db.query_row("SELECT COUNT(*) FROM figures", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 3);
    let expect_md5 = md5_file(&zip_dir.join("images/page_1_image_7.jpg")).unwrap();
    let (path, md5): (Option<String>, Option<String>) = db
        .query_row(
            "SELECT image_path, image_md5 FROM figures WHERE caption LIKE 'Two-chip%'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(path.unwrap().ends_with("page_1_image_7.jpg"));
    assert_eq!(md5.as_deref(), Some(expect_md5.as_str()));
    let vec_null: Option<Vec<u8>> = db
        .query_row("SELECT vec FROM figures WHERE caption LIKE 'Loss%'", [], |r| r.get(0))
        .unwrap();
    assert!(vec_null.is_none(), "K3 之前 vec 必為 NULL");
    let tbl_path: Option<String> = db
        .query_row("SELECT image_path FROM figures WHERE caption LIKE 'Efficiency%'", [], |r| r.get(0))
        .unwrap();
    assert!(tbl_path.unwrap().ends_with("page_1_chart_1.jpg"));

    // conversion-report.json 存在且可解析（本入口不帶 route/ladder——呼叫端補）。
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("conversion-report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["doc_id"], "mueller2016");
    assert_eq!(report["text_notes"], 3);
    assert_eq!(report["figure_rows"], 3);
    std::fs::remove_dir_all(&dir).ok();
}

/// 快速路徑：純文字 → 段落切塊 → 純文字筆記、無 sidecar 列、帶品質警告。
#[test]
fn fast_path_plain_text() {
    let dir = temp_dir("fast");
    let out = dir.join("out");
    let para = "A plain paragraph with ordinary length. ";
    let text = format!(
        "Memo Title Here\n\n{}\n\n{}\n\n{}\n\n{}\n",
        para.repeat(4),
        para.repeat(4),
        para.repeat(40),
        para.repeat(4)
    );
    let outcome = convert_fast(&text, &out, "memo").unwrap();
    assert!(outcome.report.text_notes >= 2, "長段落應切塊");
    assert_eq!(outcome.report.figure_notes, 0);
    assert!(outcome.figure_rows.is_empty());
    assert!(outcome.report.warnings.iter().any(|w| w.contains("快速路徑")));
    // 切塊不跨 1800：檢查每篇 body content 長度。
    for n in &outcome.notes {
        let body = std::fs::read_to_string(&n.path).unwrap();
        let content = body.split("\n\n").collect::<Vec<_>>()[1..].join("\n\n");
        assert!(content.chars().count() <= 1800 + 200, "chunk 過長：{}", content.chars().count());
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// 端到端 convert()：fast 政策下不碰 MinerU；損壞 PDF＋無文字 → 明確報錯（不靜默空產出）。
#[tokio::test]
async fn convert_always_fast_end_to_end() {
    let dir = temp_dir("e2e");
    let pdf = dir.join("plain.pdf");
    std::fs::write(&pdf, b"%PDF-1.4 broken").unwrap();
    let mut cfg = ConvertConfig::default();
    cfg.policy = ConvertPolicy::AlwaysFast;
    let r = convert(&pdf, &dir.join("out"), &cfg).await;
    assert!(r.is_err(), "損壞 PDF＋fast 政策＋MinerU 不可用 → 明確報錯");
    std::fs::remove_dir_all(&dir).ok();
}
