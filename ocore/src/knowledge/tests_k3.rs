//! K3 real test（#[ignore]——需本機 llama-server（mmproj 已掛）與 K7 語料；手動跑）。
//!
//! 跑法：`cargo test -p ocore real_k3 -- --ignored --nocapture`
//!
//! **P1 驗收線（K3 部分）**：純視覺查詢（pixel_only_probe 三題——答案只在圖的
//! 像素裡、圖說不含答案）經 sidecar 聯合向量檢索，正確圖進 **top-3**；
//! 純文字模式（TextOnly）下 caption 查詢可定位。

use crate::knowledge::figures::{embed_pending, embed_query, Sidecar, SidecarMode};

/// pixel_only_probe 的三題（逐字移植；目標圖號）。
fn pixel_queries() -> Vec<(&'static str, u32)> {
    vec![
        (
            "What is the efficiency of the 4-phase design at 60 MHz for the 1.7V:1.05V conversion?",
            8,
        ),
        (
            "Up to what frequency does the extracted effective inductance stay roughly constant before dropping?",
            7,
        ),
        (
            "In the loss breakdown chart, which loss component is the second largest at 100MHz for the 5:1 conversion?",
            9,
        ),
    ]
}

#[ignore = "真實環境相依：需本機 llama-server（mmproj）與 embed-exp/zip-full 語料"]
#[tokio::test]
async fn real_k3_sidecar_multimodal_visual_queries() {
    // ── 語料：K1 轉換（純 JSON 解析，秒級）──
    let zip_dir = std::path::Path::new(
        r"C:\Users\charl\Documents\python\pdf2zh\embed-exp\zip-full\mueller2016",
    );
    assert!(
        zip_dir.join("structured_content.json").exists(),
        "缺少語料：{}（K7 語料，見計畫 §K7）",
        zip_dir.display()
    );
    let dir = std::env::temp_dir().join(format!(
        "k3-real-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let outcome = crate::converters::mineru::convert_structured(zip_dir, &dir, Some("mueller2016"))
        .expect("K1 轉換應成功");
    assert_eq!(outcome.report.figure_rows, 9);

    let db = dir.join("figures.sqlite");
    let sc = Sidecar::open(&db).unwrap();
    assert_eq!(
        sc.tag_source("mueller2016", "k7").unwrap(),
        9,
        "9 列歸檔到授權 source"
    );
    drop(sc);

    // ── 多模態回填（llama-server mmproj；9 圖 3 批）──
    let base = crate::knowledge::figures::DEFAULT_EMBEDDING_BASE;
    let stats = embed_pending(&db, base, None, SidecarMode::Multimodal)
        .await
        .expect("多模態回填應成功");
    eprintln!(
        "[k3] multimodal: embedded={} deduped={} skipped={} warnings={:?}",
        stats.embedded, stats.deduped, stats.skipped, stats.warnings
    );
    assert_eq!(stats.embedded + stats.deduped, 9, "9 列全數有向量");
    assert_eq!(stats.skipped, 0, "不應有跳過：{:?}", stats.warnings);

    // ── 純視覺查詢：答案在像素裡，聯合向量應把正確圖拉進 top-3 ──
    let sidecar = Sidecar::open(&db).unwrap();
    let authorized = ["k7".to_string()];
    eprintln!("\n[k3] ── 純視覺查詢（multimodal sidecar, top-5）──");
    let mut top3_hits = 0usize;
    for (q, target_fig) in pixel_queries() {
        let qv = embed_query(base, "embeddinggemma-2", q).await.unwrap();
        let hits = sidecar.search(&qv, Some(&authorized), 5).unwrap();
        let rank = hits
            .iter()
            .position(|h| h.figure_no == Some(target_fig))
            .map(|i| i + 1);
        eprintln!(
            "Q {:<52} → fig{} rank {:?}  top5: {}",
            &q[..q.len().min(52)],
            target_fig,
            rank,
            hits.iter()
                .map(|h| format!("fig{}:{:.3}", h.figure_no.unwrap_or(0), h.score))
                .collect::<Vec<_>>()
                .join(" ")
        );
        assert!(rank.is_some(), "fig{target_fig} 應在 top-5：{hits:#?}");
        assert!(rank.unwrap() <= 3, "P1 驗收線：正確圖進 top-3（實得 {:?}）", rank);
        top3_hits += 1;
    }
    assert_eq!(top3_hits, 3);

    // ── 對照：純文字模式（TextOnly）下 caption 查詢可定位（第一輪 V2 語意）──
    let db2 = dir.join("figures-textonly.sqlite");
    std::fs::copy(&db, &db2).unwrap();
    {
        // 清掉向量，重跑 TextOnly（避免 md5 去重吃掉回填）。
        let sc = Sidecar::open(&db2).unwrap();
        assert_eq!(sc.clear_vectors().unwrap(), 9);
    }
    let stats2 = embed_pending(&db2, base, None, SidecarMode::TextOnly)
        .await
        .expect("文字回填應成功");
    assert_eq!(stats2.embedded, 9, "文字模式 9 列全嵌");
    let sidecar2 = Sidecar::open(&db2).unwrap();
    let qv = embed_query(
        base,
        "embeddinggemma-2",
        "Extracted effective inductance and resistance versus frequency of the modelled inductor",
    )
    .await
    .unwrap();
    let hits = sidecar2.search(&qv, Some(&authorized), 5).unwrap();
    let rank7 = hits
        .iter()
        .position(|h| h.figure_no == Some(7))
        .map(|i| i + 1);
    eprintln!("\n[k3] text-only caption query → fig7 rank {rank7:?}（實驗 V2：rank 1）");
    assert!(rank7.is_some_and(|r| r <= 3), "TextOnly 模式 caption 查詢應可定位 fig7（實得 {rank7:?}）");

    std::fs::remove_dir_all(&dir).ok();
}
