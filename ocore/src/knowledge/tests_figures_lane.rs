//! Figures lane 對照試點（#[ignore] real test——sidecar 聯合向量 vs gbrain 原生圖向量）。
//!
//! 跑法：`cargo test -p ocore real_figures_lane -- --ignored --nocapture`
//!
//! 實測 TODO 項目 3「方向決議候選」：同一組帶圖目標的查詢打四條路徑——
//! - **Lane A（現行，單臂）**：K3 sidecar（caption＋原圖聯合向量；brute-force
//!   cosine）。語料 `pdf2zh/embed-exp/zip-full/mueller2016` 純 JSON 轉換＋mmproj 回填。
//! - **Lane B（候選，單臂）**：gbrain 原生 `query --cross-modal image`
//!   （`embedding_image` 純圖向量；demo 腦已同步 8 張 MinerU 圖；OCR 未啟用）。
//! - **Lane A′（現行產品視圖）**：sidecar arm＋gbrain 文字查詢，RRF（k=60）融合、
//!   sidecar 列在前——即 `service.rs` route 2 融合的模擬。
//! - **Lane C（候選產品視圖）**：gbrain 圖 arm＋文字查詢，同款 RRF——改道後的
//!   產品視圖（圖頁與同名註解頁在 repo 內並存，caption 訊號走文字 lane）。
//!
//! 指標：hit@5（目標圖進前 5）＋MRR。主裁決子集＝實驗 IMG_QUERIES 定義的 6 條
//! 圖片主目標＋3 條 pixel-only 純視覺查詢；查詢 1/2 帶 fig 目標但主目標為章節，
//! 另列參考（四 lane 均以 fig 目標計位）。
//!
//! **決策閘**（人讀，不硬斷言）：lane C（候選產品視圖）hit@5 與 MRR 不劣於
//! lane A′（現行產品視圖）→ PDF figures 改道 gbrain 原生 image import（改寫
//! run_pdf_knowledge／ingest_pdf）、sidecar 退役遷移。
//!
//! 環境：本機 llama-server（mmproj 已掛）＋ gbrain 預設腦。來源 id 可用
//! `FIGURES_LANE_SOURCE` 覆寫（預設 `gbrain-demo`）。

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::gbrain_cli::run_capture;
use crate::knowledge::figures::{embed_pending, embed_query, Sidecar, SidecarMode};
use crate::proc::env_for_brain;

/// 帶圖目標的查詢集（`(query, 目標圖號, 主裁決子集)`）。
/// 1–8 逐字移植自實驗 `run_experiment.py` QUERIES（取其 fig 目標；IMG_QUERIES
/// 0-based {2,3,4,5,6,10} 對應此處 3–8 為主裁決）；9–11 為 K3 pixel-only 三題。
fn figure_queries() -> Vec<(&'static str, u32, bool)> {
    vec![
        (
            "How does the two-chip system-in-package IVR solution work? A buck plus an LDO.",
            1,
            false,
        ),
        (
            "Four-phase buck converter architecture with PWM controller and error amplifier",
            2,
            false,
        ),
        (
            "Magnetic field distribution in the cross section of the six-winding inductor",
            5,
            true,
        ),
        (
            "Equivalent circuit pi-model used to extract inductance and resistance from S-parameters",
            6,
            true,
        ),
        (
            "Extracted effective inductance and resistance versus frequency of the modelled inductor",
            7,
            true,
        ),
        (
            "Efficiency versus number of phases for the 5V:1V, 3V:1V and 1.7V:1.05V conversions",
            8,
            true,
        ),
        ("Loss breakdown of power loss, FET losses dominate", 9, true),
        (
            "為什麼選擇螺線管式 solenoidal 電感，而不是 spiral 螺旋形電感？",
            4,
            true,
        ),
        (
            "What is the efficiency of the 4-phase design at 60 MHz for the 1.7V:1.05V conversion?",
            8,
            true,
        ),
        (
            "Up to what frequency does the extracted effective inductance stay roughly constant before dropping?",
            7,
            true,
        ),
        (
            "In the loss breakdown chart, which loss component is the second largest at 100MHz for the 5:1 conversion?",
            9,
            true,
        ),
    ]
}

#[ignore = "真實環境相依：需本機 llama-server（mmproj）、embed-exp/zip-full 語料與 demo 腦已同步 8 圖"]
#[tokio::test]
async fn real_figures_lane_comparison() {
    let queries = figure_queries();
    let source = std::env::var("FIGURES_LANE_SOURCE").unwrap_or_else(|_| "gbrain-demo".into());
    let gbrain_exe = dirs::home_dir()
        .map(|h| h.join(".bun").join("bin").join("gbrain.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("gbrain"));
    let exe_s = gbrain_exe.to_string_lossy().into_owned();
    let env = env_for_brain(None);

    // ── Lane A 語料：K1 轉換（純 JSON 解析）＋sidecar 多模態回填 ──────────
    let zip_dir = PathBuf::from(
        r"C:\Users\charl\Documents\python\pdf2zh\embed-exp\zip-full\mueller2016",
    );
    assert!(
        zip_dir.join("structured_content.json").exists(),
        "缺少語料：{}",
        zip_dir.display()
    );
    let dir = std::env::temp_dir().join(format!(
        "figures-lane-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let outcome =
        crate::converters::mineru::convert_structured(&zip_dir, &dir, Some("mueller2016"))
            .expect("K1 轉換應成功");
    assert_eq!(outcome.report.figure_rows, 9);
    // slug → figs（文字面歸因用：sNN chunk 引用的圖號；figN-pP 即自身）。
    let attrs_by_slug: BTreeMap<String, Vec<u32>> = outcome
        .notes
        .iter()
        .map(|n| (n.slug.clone(), n.figs.clone()))
        .collect();

    let db = dir.join("figures.sqlite");
    {
        let sc = Sidecar::open(&db).unwrap();
        assert_eq!(sc.tag_source("mueller2016", "k7").unwrap(), 9);
    }
    let base = crate::knowledge::figures::DEFAULT_EMBEDDING_BASE;
    let stats = embed_pending(&db, base, None, SidecarMode::Multimodal)
        .await
        .expect("多模態回填應成功");
    assert_eq!(
        stats.embedded + stats.deduped,
        9,
        "9 列全數有向量：{:?}",
        stats.warnings
    );
    let sidecar = Sidecar::open(&db).unwrap();
    let authorized = ["k7".to_string()];

    // ── 逐查詢：文字 arm＋圖 arm（gbrain）＋sidecar arm → 四 lane 計位 ────
    let mut lane_a: Vec<Option<usize>> = Vec::new();
    let mut lane_ap: Vec<Option<usize>> = Vec::new();
    let mut lane_b: Vec<Option<usize>> = Vec::new();
    let mut lane_c: Vec<Option<usize>> = Vec::new();
    for (q, target, _primary) in &queries {
        let prefixed = crate::knowledge::service::retrieval_query(q);

        // 文字 arm（route 1 語意；slug 歸因）。
        let t_args = [
            "query",
            prefixed.as_str(),
            "--limit",
            "5",
            "--source",
            source.as_str(),
            "--no-expand",
            "--json",
        ];
        let (code, stdout, stderr) = run_capture(&exe_s, &t_args, &env).await.expect("gbrain spawn");
        assert_eq!(code, 0, "gbrain query（text）失敗：{stderr}");
        let text_rows: Vec<(String, bool)> = parse_query_json(&stdout)
            .expect("query --json 應可解析")
            .iter()
            .filter_map(|r| {
                let slug = r["slug"].as_str()?.to_string();
                let figs = attrs_by_slug.get(&slug)?;
                Some((slug, figs.contains(target)))
            })
            .collect();

        // 圖 arm（候選路徑；embedding_image 純圖向量）。
        let i_args = [
            "query",
            prefixed.as_str(),
            "--limit",
            "5",
            "--source",
            source.as_str(),
            "--no-expand",
            "--cross-modal",
            "image",
            "--json",
        ];
        let (code, stdout, stderr) = run_capture(&exe_s, &i_args, &env).await.expect("gbrain spawn");
        assert_eq!(code, 0, "gbrain query（image）失敗：{stderr}");
        let image_figs: Vec<Option<u32>> = parse_query_json(&stdout)
            .expect("query --json 應可解析")
            .iter()
            .map(slug_fig)
            .collect();

        // sidecar arm（現行路徑；聯合向量 brute-force）。
        let qv = embed_query(base, Some("embeddinggemma-2"), q).await.unwrap();
        let hits = sidecar.search(&qv, Some(&authorized), 5).unwrap();
        let sidecar_figs: Vec<Option<u32>> = hits.iter().map(|h| h.figure_no).collect();

        let to_items = |figs: &[Option<u32>]| -> Vec<(String, bool)> {
            figs.iter()
                .map(|f| (item_key(*f), *f == Some(*target)))
                .collect()
        };
        let sidecar_items = to_items(&sidecar_figs);
        let image_items = to_items(&image_figs);
        let text_items: Vec<(String, bool)> = text_rows
            .iter()
            .map(|(slug, hit)| (format!("slug:{slug}"), *hit))
            .collect();

        lane_a.push(sidecar_items.iter().position(|(_, h)| *h).map(|i| i + 1));
        lane_b.push(image_items.iter().position(|(_, h)| *h).map(|i| i + 1));
        lane_ap.push(rrf_first_hit(&[&sidecar_items, &text_items]));
        lane_c.push(rrf_first_hit(&[&image_items, &text_items]));
    }
    std::fs::remove_dir_all(&dir).ok();

    // ── 對照表與指標 ────────────────────────────────────────────────────
    eprintln!(
        "\n[figures-lane] ── 對照（A=sidecar 單臂 / B=gbrain 圖單臂 / A′=現行產品融合 / C=候選產品融合；top-5）──"
    );
    for (i, ((q, target, primary), (((a, ap), b), c))) in queries
        .iter()
        .zip(lane_a.iter().zip(&lane_ap).zip(&lane_b).zip(&lane_c))
        .enumerate()
    {
        let label: String = q.chars().take(48).collect();
        eprintln!(
            "Q{:>2}{} {label:<48} fig{:<2}  A={:>4} B={:>4}  A′={:>4} C={:>4}",
            i + 1,
            if *primary { "*" } else { " " },
            target,
            fmt_rank(a),
            fmt_rank(b),
            fmt_rank(ap),
            fmt_rank(c)
        );
    }
    for (name, subset_only) in [
        ("主裁決子集（IMG_QUERIES＋pixel，9 條）", true),
        ("全部帶圖查詢（11 條）", false),
    ] {
        let idx: Vec<usize> = queries
            .iter()
            .enumerate()
            .filter(|(_, (_, _, p))| !subset_only || *p)
            .map(|(i, _)| i)
            .collect();
        let n = idx.len();
        let (a_hits, a_mrr) = metrics(&idx, &lane_a);
        let (b_hits, b_mrr) = metrics(&idx, &lane_b);
        let (ap_hits, ap_mrr) = metrics(&idx, &lane_ap);
        let (c_hits, c_mrr) = metrics(&idx, &lane_c);
        eprintln!(
            "[figures-lane] {name}:\n  \
             單臂  A hit@5 {a_hits}/{n} MRR {a_mrr:.3}  |  B hit@5 {b_hits}/{n} MRR {b_mrr:.3}\n  \
             產品  A′ hit@5 {ap_hits}/{n} MRR {ap_mrr:.3} |  C hit@5 {c_hits}/{n} MRR {c_mrr:.3}"
        );
    }
    let idx: Vec<usize> = queries
        .iter()
        .enumerate()
        .filter(|(_, (_, _, p))| *p)
        .map(|(i, _)| i)
        .collect();
    let (ap_hits, ap_mrr) = metrics(&idx, &lane_ap);
    let (c_hits, c_mrr) = metrics(&idx, &lane_c);
    let verdict = if c_hits >= ap_hits && c_mrr >= ap_mrr {
        "lane C（候選產品視圖）不劣於 A′（現行）——支持改道＋sidecar 退役"
    } else {
        "lane C 劣於 A′——暫緩退役，人工複核"
    };
    eprintln!("[figures-lane] 決策閘：{verdict}");
}

fn item_key(fig: Option<u32>) -> String {
    format!("fig:{}", fig.unwrap_or(0))
}

/// RRF（k=60）融合多列表（key 去重累分、分高者前），回傳第一個命中列的名次（1-based）。
/// 模擬 `service.rs` route 2 融合（圖列在前、文字列在後——僅影響同分排序）。
fn rrf_first_hit(lists: &[&[(String, bool)]]) -> Option<usize> {
    const RRF_K: f64 = 60.0;
    let mut scores: BTreeMap<&str, f64> = BTreeMap::new();
    let mut hit_by_key: BTreeMap<&str, bool> = BTreeMap::new();
    for list in lists {
        for (i, (key, hit)) in list.iter().enumerate() {
            *scores.entry(key.as_str()).or_insert(0.0) += 1.0 / (RRF_K + i as f64 + 1.0);
            *hit_by_key.entry(key.as_str()).or_insert(false) |= *hit;
        }
    }
    let mut ranked: Vec<(&str, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked
        .iter()
        .position(|(k, _)| hit_by_key.get(k).copied().unwrap_or(false))
        .map(|i| i + 1)
}

fn fmt_rank(r: &Option<usize>) -> String {
    r.map(|n| format!("#{n}")).unwrap_or_else(|| "--".into())
}

/// hit@5（有排名即前 5 內）與 MRR（缺席計 0）。
fn metrics(idx: &[usize], lane: &[Option<usize>]) -> (usize, f64) {
    let hits = idx.iter().filter(|&&i| lane[i].is_some()).count();
    let mrr = idx
        .iter()
        .map(|&i| lane[i].map(|r| 1.0 / r as f64).unwrap_or(0.0))
        .sum::<f64>()
        / idx.len().max(1) as f64;
    (hits, mrr)
}

/// gbrain `--json` 列的 slug → 圖號（`mueller2016/fig5-p5.jpg` → 5）；非圖頁回 None。
fn slug_fig(row: &serde_json::Value) -> Option<u32> {
    let slug = row["slug"].as_str()?;
    let stem = slug.rsplit('/').next()?.strip_suffix(".jpg")?;
    let fig = stem.strip_prefix("fig")?;
    let num: String = fig.chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse().ok()
}

/// gbrain `query --json` 輸出解析：stdout 可能帶前置雜訊（升級提示）——取首 `[` 起。
fn parse_query_json(stdout: &str) -> Option<Vec<serde_json::Value>> {
    let start = stdout.find('[')?;
    let end = stdout.rfind(']')?;
    serde_json::from_str(&stdout[start..=end]).ok()
}
