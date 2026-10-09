//! K4 三路融合——gbrain 文字路（路1）＋sidecar 圖向量路（路2）＋metadata 附帶（路3）。
//!
//! 融合＝**RRF（k=60）**：各路各自排序，每項得 Σ 1/(k + rank)，跨路分數可加——
//! 不需把 cosine 分數與 gbrain 混合分數放在同一把尺上（實驗 §7 的歸因鐵律：
//! 圖文配對只用 metadata，融合層不二次向量配對）。
//!
//! 契約：`BackendQuery` 不變；融合屬實作細節（`KnowledgeService` 內部）。輸出仍為
//! 人類可讀文字（員工具/操作台直接消費），每項自帶 doc/page/圖檔路徑——生成端
//! （K5）據此讀原圖或指向圖。

use serde::Deserialize;

use super::figures::FigureHit;

/// RRF 常數（計畫 §K4；標準值）。
pub const RRF_K: f64 = 60.0;

/// 路1 項：gbrain 命中（`query --json` 列的子集）。
#[derive(Debug, Clone, PartialEq)]
pub struct TextHit {
    pub source_id: String,
    pub slug: String,
    pub title: String,
    pub chunk_text: String,
    /// gbrain 混合分數（RRF 不吃此值，僅偕同顯示）。
    pub cosine: Option<f64>,
    /// MCP 傳輸退路：整段文字無 slug——以 source 為單位參與融合。
    pub opaque: bool,
}

/// 融合項（路1/路2 統一形狀；路3 以 `FigureHit::attached` 標記）。
#[derive(Debug, Clone, PartialEq)]
pub enum FusionItem {
    Text(TextHit),
    Figure(FigureHit),
}

impl FusionItem {
    /// 去重／歸因鍵：文字用 slug、圖用 doc+page+figure_no。
    pub fn key(&self) -> String {
        match self {
            FusionItem::Text(t) => format!("t:{}", t.slug),
            FusionItem::Figure(f) => {
                format!("f:{}/{}/{}", f.doc_id, f.page, f.figure_no.unwrap_or(0))
            }
        }
    }

    /// 所屬文件（路3 附帶的歸屬依據；slug 無目錄前綴時無法歸屬 → None）。
    pub fn doc_id(&self) -> Option<String> {
        match self {
            FusionItem::Text(t) => t
                .slug
                .split('/')
                .next()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty() && s != &t.slug),
            FusionItem::Figure(f) => Some(f.doc_id.clone()),
        }
    }

    /// 人類可讀區塊（員工具／操作台消費；每項自帶出處）。
    pub fn render(&self) -> String {
        match self {
            FusionItem::Text(t) => {
                if t.opaque {
                    format!("── {} ──\n{}", t.source_id, t.chunk_text.trim())
                } else {
                    let score = t
                        .cosine
                        .map(|c| format!(" · cos {c:.3}"))
                        .unwrap_or_default();
                    format!(
                        "── {} · {}{score} ──\n{}",
                        t.source_id,
                        t.slug,
                        t.chunk_text.trim()
                    )
                }
            }
            FusionItem::Figure(f) => {
                let label = match f.figure_no {
                    Some(n) => format!("Figure {n}"),
                    None => "Figure".to_string(),
                };
                let attached = if f.attached { "（結構性附帶）" } else { "" };
                let image = f
                    .image_path
                    .as_deref()
                    .map(|p| format!("\n圖檔：{p}"))
                    .unwrap_or_default();
                format!(
                    "── 圖片{attached} · {}/{}（p{}）[{}] ──\n{}{image}",
                    f.doc_id, label, f.page, f.section, f.caption.trim()
                )
            }
        }
    }
}

/// `gbrain query --json` 列（實測欄位集；寬容缺項）。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct GbrainRow {
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, rename = "chunk_text")]
    pub chunk_text: String,
    #[serde(default)]
    pub cosine: Option<f64>,
    #[serde(default, rename = "source_id")]
    pub source_id: Option<String>,
}

/// 解析 gbrain stdout（可能帶前置雜訊如升級提示）——取第一個 `[` 到最後一個 `]`。
pub fn parse_gbrain_hits(stdout: &str) -> Option<Vec<GbrainRow>> {
    let start = stdout.find('[')?;
    let end = stdout.rfind(']')?;
    serde_json::from_str(&stdout[start..=end]).ok()
}

/// RRF 融合：`lists` 為各路已排序的項；回傳依融合分數降序（同分依首次出現序）。
pub fn rrf_merge(lists: Vec<Vec<FusionItem>>, k: f64) -> Vec<(FusionItem, f64)> {
    let mut order: Vec<String> = Vec::new();
    let mut by_key: std::collections::HashMap<String, (FusionItem, f64)> =
        std::collections::HashMap::new();
    for list in lists {
        for (rank, item) in list.into_iter().enumerate() {
            let key = item.key();
            let contribution = 1.0 / (k + rank as f64 + 1.0);
            match by_key.get_mut(&key) {
                Some((_, score)) => *score += contribution,
                None => {
                    order.push(key.clone());
                    by_key.insert(key, (item, contribution));
                }
            }
        }
    }
    let mut merged: Vec<(FusionItem, f64)> =
        order.into_iter().filter_map(|key| by_key.remove(&key)).collect();
    merged.sort_by(|a, b| b.1.total_cmp(&a.1));
    merged
}

/// 路3 附帶：`related`＝命中文件集合的同文件圖（metadata 綁定——不需向量、
/// 不做任何二次向量配對）。已在融合結果中的圖不重複；附帶總量上限 `max`，
/// 以 `attached = true` 標記並排在結果尾部（生成端知道這是「帶出來看的」）。
pub fn attach_related_figures(
    merged: Vec<(FusionItem, f64)>,
    related: Vec<FigureHit>,
    max: usize,
) -> Vec<(FusionItem, f64)> {
    let mut existing: std::collections::HashSet<String> =
        merged.iter().map(|(it, _)| it.key()).collect();
    let mut out = merged;
    let mut attached = 0usize;
    for mut f in related {
        if attached >= max {
            break;
        }
        f.attached = true;
        let item = FusionItem::Figure(f);
        if existing.contains(&item.key()) {
            continue;
        }
        out.push((item.clone(), 0.0));
        existing.insert(item.key());
        attached += 1;
    }
    out
}

/// C4 歸因（純函式）：以文字路首位命中的文件為**錨**——同文件圖留在排序面，
/// 他文件圖降入附帶池（metadata 綁定，絕不靠向量做跨文件歸因）。錨未知（裸 slug）
/// 時全部保留。回傳（排序面、附帶池）。
pub fn split_by_anchor(
    hits: Vec<FigureHit>,
    anchor_doc: Option<&str>,
) -> (Vec<FigureHit>, Vec<FigureHit>) {
    let Some(anchor) = anchor_doc else {
        return (hits, Vec::new());
    };
    let (same, other) = hits
        .into_iter()
        .partition(|h| h.doc_id == anchor);
    (same, other)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(slug: &str, source: &str) -> FusionItem {
        FusionItem::Text(TextHit {
            source_id: source.into(),
            slug: slug.into(),
            title: String::new(),
            chunk_text: format!("content of {slug}"),
            cosine: Some(0.9),
            opaque: false,
        })
    }

    fn fig(doc: &str, page: i64, no: u32) -> FusionItem {
        FusionItem::Figure(FigureHit {
            doc_id: doc.into(),
            page,
            figure_no: Some(no),
            caption: format!("{doc} fig{no}"),
            section: "I".into(),
            image_path: None,
            source_id: Some("s".into()),
            score: 0.8,
            attached: false,
        })
    }

    fn fhit(doc: &str, page: i64, no: u32) -> FigureHit {
        FigureHit {
            doc_id: doc.into(),
            page,
            figure_no: Some(no),
            caption: format!("{doc} fig{no}"),
            section: "I".into(),
            image_path: None,
            source_id: Some("s".into()),
            score: 0.8,
            attached: false,
        }
    }

    /// RRF：兩路各排第一名者勝出；跨路重複項分數累加。
    #[test]
    fn rrf_merges_and_dedups() {
        let gbrain = vec![text("mueller2016/s05", "k7"), text("mueller2016/s20", "k7")];
        let sidecar = vec![fig("mueller2016", 6, 8), fig("mueller2016", 3, 3)];
        let merged = rrf_merge(vec![gbrain, sidecar], RRF_K);
        let keys: Vec<String> = merged.iter().map(|(it, _)| it.key()).collect();
        // 四項各 1/(60+rank)——路1 首名與路2 首名同分並列最前。
        assert_eq!(keys.len(), 4);
        assert!((merged[0].1 - merged[1].1).abs() < 1e-12);
        assert!((merged[0].1 - 1.0 / 61.0).abs() < 1e-12);
        // 重複項：同 slug 出現兩路 → 只留一項、分數加倍。
        let merged2 = rrf_merge(
            vec![vec![text("a", "s")], vec![text("a", "s"), text("b", "s")]],
            RRF_K,
        );
        assert_eq!(merged2.len(), 2);
        assert!(merged2[0].1 > merged2[1].1, "跨路累加者應領先");
    }

    /// 路3 附帶：同文件、未重複、上限；attached 標記；doc 歸屬。
    #[test]
    fn attach_related_respects_limits_and_marks() {
        let merged = vec![
            (text("mueller2016/s20", "k7"), 0.03),
            (fig("mueller2016", 6, 8), 0.028),
        ];
        let related = vec![
            FigureHit {
                doc_id: "mueller2016".into(),
                page: 7,
                figure_no: Some(9),
                caption: "Loss breakdown.".into(),
                section: "V.B".into(),
                image_path: Some("img".into()),
                source_id: Some("k7".into()),
                score: 0.0,
                attached: false,
            },
            FigureHit {
                doc_id: "mueller2016".into(),
                page: 6,
                figure_no: Some(8),
                caption: "mueller2016 fig8".into(),
                section: "I".into(),
                image_path: None,
                source_id: Some("s".into()),
                score: 0.0,
                attached: false,
            }, // 已在融合結果——不重複
        ];
        let out = attach_related_figures(merged, related, 2);
        assert_eq!(out.len(), 3, "只附帶未重複的那張");
        match &out[2].0 {
            FusionItem::Figure(f) => assert!(f.attached, "附帶項必須標記"),
            other => panic!("unexpected {other:?}"),
        }
        // 上限：max=0 不附帶。
        let dup = FigureHit {
            doc_id: "mueller2016".into(),
            page: 4,
            figure_no: Some(5),
            caption: "extra".into(),
            section: "I".into(),
            image_path: None,
            source_id: Some("s".into()),
            score: 0.0,
            attached: false,
        };
        let out2 = attach_related_figures(out, vec![dup], 0);
        assert_eq!(out2.len(), 3, "max=0 不附帶");
    }

    /// doc 歸屬：有目錄前綴的 slug 歸其文件；裸 slug 不歸屬。
    #[test]
    fn doc_id_attribution() {
        assert_eq!(text("mueller2016/s05", "k7").doc_id().as_deref(), Some("mueller2016"));
        assert_eq!(text("alpha", "k4").doc_id(), None, "裸 slug 無法歸屬文件");
        assert_eq!(fig("doc-b", 1, 1).doc_id().as_deref(), Some("doc-b"));
    }

    /// C4 錨點分流：同文件圖留排序面、他文件圖降附帶池；錨未知全保留。
    #[test]
    fn anchor_split_demotes_other_docs() {
        let hits = vec![fhit("doc-a", 1, 1), fhit("doc-b", 2, 2), fhit("doc-a", 3, 3)];
        let (same, other) = split_by_anchor(hits, Some("doc-a"));
        assert_eq!(same.len(), 2);
        assert!(same.iter().all(|h| h.doc_id == "doc-a"));
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].doc_id, "doc-b");
        let (same, other) = split_by_anchor(vec![fhit("doc-a", 1, 1)], None);
        assert_eq!(same.len(), 1);
        assert!(other.is_empty());
    }

    /// gbrain --json 解析：容忍前置雜訊、缺欄位寬容。
    #[test]
    fn parses_gbrain_rows() {
        let s = "UPGRADE_AVAILABLE x y\n[{\"slug\":\"a/s01\",\"chunk_text\":\"txt\",\"cosine\":0.81,\"source_id\":\"k7\"}]";
        let rows = parse_gbrain_hits(s).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].slug, "a/s01");
        assert_eq!(rows[0].chunk_text, "txt");
        assert_eq!(rows[0].cosine, Some(0.81));
        assert!(parse_gbrain_hits("no json here").is_none());
    }

    /// 輸出格式：文字項帶來源與 slug；圖片項帶 doc/Figure/頁/圖檔路徑（K5 消費面）。
    #[test]
    fn render_includes_citations() {
        let t = text("mueller2016/s20", "k7");
        assert!(t.render().contains("── k7 · mueller2016/s20"));
        assert!(t.render().contains("content of mueller2016/s20"));
        let f = fig("mueller2016", 6, 8);
        let r = f.render();
        assert!(r.contains("── 圖片 · mueller2016/Figure 8（p6）"));
        assert!(r.contains("mueller2016 fig8"));
        // MCP 退路：opaque 項保持舊格式（── source ──）。
        let op = FusionItem::Text(TextHit {
            source_id: "k7".into(),
            slug: String::new(),
            title: String::new(),
            chunk_text: "raw text".into(),
            cosine: None,
            opaque: true,
        });
        assert_eq!(op.render(), "── k7 ──\nraw text");
    }
}
