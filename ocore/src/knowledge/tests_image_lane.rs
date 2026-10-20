//! 圖 lane 回歸閘（#[ignore] real test）——sidecar 退役後，檢索圖路只剩
//! **gbrain 原生 `--cross-modal image`** 一條；本測試以 2026-10-17 試點的同一組
//! 帶圖查詢（11 條，逐字移植）常駐驗證兩個視圖：
//!
//! - **IMG**：圖 arm 單臂（`embedding_image` 純圖向量）。
//! - **PROD**：圖 arm×文字路 RRF（k=60）——即 `service.rs` route 2 的產品視圖
//!   （圖頁與同名註解頁在 repo 內並存，caption 訊號走文字 lane）。
//!
//! 指標：hit@5（目標圖進前 5）＋MRR。主裁決子集＝實驗 IMG_QUERIES 定義的 6 條
//! 圖片主目標＋3 條 pixel-only 純視覺查詢（9 條）。
//!
//! **硬斷言**：主裁決子集 hit@5 = 9/9（2026-10-17 四 lane 試點中 A′/C 全 lane
//! 持平的底線；退役裁決見 TODO 項目 3）。MRR 印出供人讀（試點值 C=0.909；
//! #1 vs #2/#3 的波動屬 LLM/嵌入非確定性，不硬斷言）。
//!
//! 環境：本機 llama-server（mmproj 已掛）＋ gbrain 預設腦（demo 腦已同步 8 圖）。
//! 來源 id 可用 `FIGURES_LANE_SOURCE` 覆寫（預設 `gbrain-demo`）。

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::gbrain_cli::run_capture;
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

#[ignore = "真實環境相依：需本機 llama-server（mmproj）與 demo 腦已同步 8 圖"]
#[tokio::test]
async fn real_image_lane_regression() {
    let queries = figure_queries();
    let source = std::env::var("FIGURES_LANE_SOURCE").unwrap_or_else(|_| "gbrain-demo".into());
    let gbrain_exe = dirs::home_dir()
        .map(|h| h.join(".bun").join("bin").join("gbrain.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("gbrain"));
    let exe_s = gbrain_exe.to_string_lossy().into_owned();
    let env = env_for_brain(None);

    // slug → figs（文字面歸因用：sNN chunk 引用的圖號；figN-pP 即自身）——
    // 以 K1 純 JSON 解析取得（不入庫、不嵌向量）。
    let zip_dir = PathBuf::from(
        r"C:\Users\charl\Documents\python\pdf2zh\embed-exp\zip-full\mueller2016",
    );
    assert!(
        zip_dir.join("structured_content.json").exists(),
        "缺少語料：{}",
        zip_dir.display()
    );
    let dir = std::env::temp_dir().join(format!(
        "image-lane-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let outcome =
        crate::converters::mineru::convert_structured(&zip_dir, &dir, Some("mueller2016"), None)
            .expect("K1 轉換應成功");
    let attrs_by_slug: BTreeMap<String, Vec<u32>> = outcome
        .notes
        .iter()
        .map(|n| (n.slug.clone(), n.figs.clone()))
        .collect();

    // ── 逐查詢：文字 arm＋圖 arm（gbrain）→ 兩視圖計位 ───────────────────
    let mut lane_img: Vec<Option<usize>> = Vec::new();
    let mut lane_prod: Vec<Option<usize>> = Vec::new();
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
        let text_items: Vec<(String, bool)> = parse_query_json(&stdout)
            .expect("query --json 應可解析")
            .iter()
            .filter_map(|r| {
                let slug = r["slug"].as_str()?.to_string();
                let figs = attrs_by_slug.get(&slug)?;
                Some((format!("slug:{slug}"), figs.contains(target)))
            })
            .collect();

        // 圖 arm（embedding_image 純圖向量）。
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
        let image_items: Vec<(String, bool)> = parse_query_json(&stdout)
            .expect("query --json 應可解析")
            .iter()
            .map(|r| {
                let f = slug_fig(r);
                (item_key(f), f == Some(*target))
            })
            .collect();

        lane_img.push(image_items.iter().position(|(_, h)| *h).map(|i| i + 1));
        lane_prod.push(rrf_first_hit(&[&image_items, &text_items]));
    }
    std::fs::remove_dir_all(&dir).ok();

    // ── 對照表與指標 ────────────────────────────────────────────────────
    eprintln!(
        "\n[image-lane] ── 對照（IMG=圖 arm 單臂 / PROD=圖 arm×文字 RRF 產品視圖；top-5）──"
    );
    for (i, ((q, target, primary), (img, prod))) in
        queries.iter().zip(lane_img.iter().zip(&lane_prod)).enumerate()
    {
        let label: String = q.chars().take(48).collect();
        eprintln!(
            "Q{:>2}{} {label:<48} fig{:<2}  IMG={:>4} PROD={:>4}",
            i + 1,
            if *primary { "*" } else { " " },
            target,
            fmt_rank(img),
            fmt_rank(prod)
        );
    }
    let idx: Vec<usize> = queries
        .iter()
        .enumerate()
        .filter(|(_, (_, _, p))| *p)
        .map(|(i, _)| i)
        .collect();
    let n = idx.len();
    let (img_hits, img_mrr) = metrics(&idx, &lane_img);
    let (prod_hits, prod_mrr) = metrics(&idx, &lane_prod);
    eprintln!(
        "[image-lane] 主裁決子集（IMG_QUERIES＋pixel，{n} 條）:\n  \
         單臂  IMG hit@5 {img_hits}/{n} MRR {img_mrr:.3}\n  \
         產品  PROD hit@5 {prod_hits}/{n} MRR {prod_mrr:.3}"
    );
    assert_eq!(
        prod_hits, n,
        "產品視圖 hit@5 必須全中（退役裁決底線；目標圖必進前 5）"
    );
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

/// gbrain `query --json` 輸出解析：共用平衡括號掃描（容忍前置雜訊與尾隨附加文字）。
fn parse_query_json(stdout: &str) -> Option<Vec<serde_json::Value>> {
    serde_json::from_str(crate::knowledge::fusion::find_json_array(stdout)?).ok()
}
