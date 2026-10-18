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
    /// gbrain 原始 cosine（RRF 不吃此值，僅偕同顯示）。
    pub cosine: Option<f64>,
    /// gbrain 混合分數（同引擎跨來源可比——route 1 跨來源排序與門檻用）。
    pub score: Option<f64>,
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
                    let shown = t
                        .score
                        .or(t.cosine)
                        .map(|v| format!(" · score {v:.2}"))
                        .unwrap_or_default();
                    format!(
                        "── {} · {}{shown} ──\n{}",
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
    /// gbrain 混合分數（同引擎跨來源可比；融合排序用）。
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default, rename = "source_id")]
    pub source_id: Option<String>,
}

/// 掃描文字中第一個**平衡**的 `[...]`（字串內括號與跳脫不計深度），回傳切片。
/// gbrain stdout 帶前置雜訊（升級提示）與尾隨附加文字（`[gbrain dates] …`）——
/// 舊式「首 `[` 到末 `]`」會被附加文字搶走切片，共用此掃描。
pub fn find_json_array(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'[' {
            continue;
        }
        if let Some(end) = array_span(s, i) {
            return Some(&s[i..=end]);
        }
    }
    None
}

/// 解析 gbrain hits 列——stdout 可能帶前置雜訊（升級提示）與**尾隨附加文字**
/// （例如含日期欄位結果時附加的 `[gbrain dates] …` 建議行）。舊實作「首 `[` 到
/// 末 `]`」會被附加文字裡的 `]` 搶走切片（2026-10-17 知識庫頁 raw JSON 回歸），
/// 改為平衡括號掃描：取第一個能成功解析為列陣列的 `[...]`。
pub fn parse_gbrain_hits(stdout: &str) -> Option<Vec<GbrainRow>> {
    for cand in scan_array_candidates(stdout) {
        if let Ok(rows) = serde_json::from_str::<Vec<GbrainRow>>(cand) {
            return Some(rows);
        }
    }
    None
}

/// 依序產出所有平衡 `[...]` 候選切片。
fn scan_array_candidates(s: &str) -> impl Iterator<Item = &str> {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    std::iter::from_fn(move || {
        while i < bytes.len() {
            if bytes[i] != b'[' {
                i += 1;
                continue;
            }
            match array_span(s, i) {
                Some(end) => {
                    let span = &s[i..=end];
                    i = end + 1;
                    return Some(span);
                }
                None => {
                    i += 1;
                }
            }
        }
        None
    })
}

/// 由 `open`（`[`）找配對的 `]`：字串內的括號與跳脫不計深度。
fn array_span(s: &str, open: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' => depth += 1,
            b']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// 圖 arm 命中的 slug → `FigureHit`：`<doc_id>/figN-pP.<ext>` 解析圖號與頁；
/// `image_path` 以 source 的 local_path 解析（圖在 repo 內＝授權語意隨 source 走）。
/// 無 `figN` 慣例的圖頁（使用者拖放圖等）仍保留——`figure_no` 未知、頁 0、
/// doc 以目錄前綴歸屬（注意：同 doc 的多張此類圖在融合 key 上會併項）。
/// 無目錄前綴的 slug 無法歸屬 → None。
pub fn figure_hit_from_image_slug(
    slug: &str,
    source_id: &str,
    local_path: Option<&str>,
) -> Option<FigureHit> {
    let (doc_id, stem_full) = slug.split_once('/')?;
    let stem = stem_full.split_once('.').map(|(s, _)| s).unwrap_or(stem_full);
    let (figure_no, page) = parse_fig_stem(stem).unwrap_or((None, 0));
    let rel: std::path::PathBuf = slug.split('/').collect();
    let image_path = local_path.map(|lp| {
        std::path::PathBuf::from(lp)
            .join(rel)
            .to_string_lossy()
            .into_owned()
    });
    Some(FigureHit {
        doc_id: doc_id.to_string(),
        page,
        figure_no,
        caption: String::new(),
        section: String::new(),
        image_path,
        source_id: Some(source_id.to_string()),
        score: 0.0,
        attached: false,
    })
}

/// `figN-pP` →（圖號、頁）。
fn parse_fig_stem(stem: &str) -> Option<(Option<u32>, i64)> {
    let rest = stem.strip_prefix("fig")?;
    let (num, tail) = rest.split_once('-')?;
    let fig: u32 = num.parse().ok()?;
    let page: i64 = tail.strip_prefix('p')?.parse().ok()?;
    Some((Some(fig), page))
}

/// route 1 跨來源合併（同引擎分數可比）：攤平 → 丟掉低於 min_score 的弱相關項
/// （無分數者保留）→ 依分數降序（stable）。**取代跨來源 RRF**：RRF 只看各來
/// 源內部名次，弱相關來源的第 1 名會壓過強相關來源的第 2 名（2026-10-18
/// 知識庫雜訊回歸：default 的 dedup_review 壓過 demo 答案）。
pub fn merge_by_score(lists: Vec<Vec<TextHit>>, min_score: f64) -> Vec<TextHit> {
    let mut flat: Vec<TextHit> = lists.into_iter().flatten().collect();
    flat.retain(|h| h.score.is_none_or(|s| s >= min_score));
    flat.sort_by(|a, b| {
        b.score
            .unwrap_or(f64::NEG_INFINITY)
            .total_cmp(&a.score.unwrap_or(f64::NEG_INFINITY))
    });
    flat
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

    /// merge_by_score：跨來源按分數排序＋門檻丟棄（無分數者保留）。
    #[test]
    fn merge_by_score_floors_and_sorts() {
        let h = |slug: &str, score: Option<f64>| TextHit {
            source_id: "s".into(),
            slug: slug.into(),
            title: String::new(),
            chunk_text: String::new(),
            cosine: None,
            score,
            opaque: false,
        };
        let lists = vec![
            vec![h("a-top", Some(0.55)), h("a-2", Some(0.40))],
            vec![h("b-top", Some(0.95)), h("b-2", Some(0.70))],
            vec![h("noscore", None)],
        ];
        let merged = merge_by_score(lists, 0.6);
        let slugs: Vec<&str> = merged.iter().map(|h| h.slug.as_str()).collect();
        assert_eq!(slugs, vec!["b-top", "b-2", "noscore"], "弱分數丟棄、跨來源按分數排序、無分數保留");
    }

    #[test]
    fn image_slug_parses_fig_page_and_resolves_local_path() {
        let h = figure_hit_from_image_slug(
            "mueller2016/fig5-p5.jpg",
            "gbrain-demo",
            Some(r"C:\repo\demo"),
        )
        .expect("figN slug 應可解析");
        assert_eq!(h.doc_id, "mueller2016");
        assert_eq!(h.figure_no, Some(5));
        assert_eq!(h.page, 5);
        assert_eq!(h.source_id.as_deref(), Some("gbrain-demo"));
        assert_eq!(h.image_path.as_deref(), Some(r"C:\repo\demo\mueller2016\fig5-p5.jpg"));
        assert!(!h.attached);
    }

    #[test]
    fn image_slug_without_fig_convention_keeps_hit_with_unknown_figure() {
        let h = figure_hit_from_image_slug("media/shot.png", "s1", None).expect("圖頁應保留");
        assert_eq!(h.doc_id, "media");
        assert_eq!(h.figure_no, None);
        assert_eq!(h.page, 0);
        assert_eq!(h.image_path, None);
    }

    #[test]
    fn image_slug_without_doc_prefix_is_dropped() {
        assert!(figure_hit_from_image_slug("fig1-p2.jpg", "s1", None).is_none());
    }

    /// 回歸（2026-10-17 知識庫頁 raw JSON）：尾隨 `[gbrain dates] …` 附加文字的
    /// `]` 不得搶走陣列切片——舊式「首 `[` 到末 `]`」在此輸入上解析失敗，
    /// MCP 路徑退化為 opaque 模式把原文塞給使用者。
    #[test]
    fn parses_rows_despite_trailing_date_advisory() {
        let s = concat!(
            r##"[{"slug":"companies/光寶科技","title":"光寶科技","chunk_text":"# 光寶\r\n- Louis","cosine":0.9457}]"##,
            "\n[gbrain dates] effective_date by effective_date_source: fallback = fallback (page created/imported, not from its text). Never a contract's effective or validity date (read the text).",
        );
        let rows = parse_gbrain_hits(s).expect("附加文字不得破壞解析");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].slug, "companies/光寶科技");
    }

    /// chunk_text 內的方括號（如 `[Figure 5]`）不影響平衡掃描；前置雜訊容忍。
    #[test]
    fn parses_rows_with_brackets_inside_strings_and_leading_noise() {
        let s = concat!(
            "UPGRADE_AVAILABLE 0.60.262.0 0.60.271.0\n",
            r#"[{"slug":"a/s01","chunk_text":"see [Figure 5] and [b","cosine":0.9}]"#,
        );
        let rows = parse_gbrain_hits(s).expect("應可解析");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].chunk_text.contains("[Figure 5]"));
    }

    /// 錯誤物件（message 內含方括號）→ None（不得把 `[text, image]` 當列陣列）。
    #[test]
    fn error_object_yields_none() {
        assert!(parse_gbrain_hits(
            r#"{"error":"invalid_params","message":"--cross-modal must be one of: text, image, both, auto; got 'true'.","suggestion":"Pass one of the declared values, e.g. gbrain query --cross-modal text."}"#
        )
        .is_none());
    }

    fn text(slug: &str, source: &str) -> FusionItem {
        FusionItem::Text(TextHit {
            source_id: source.into(),
            slug: slug.into(),
            title: String::new(),
            chunk_text: format!("content of {slug}"),
            cosine: Some(0.9),
            score: Some(0.9),
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
            score: None,
            opaque: true,
        });
        assert_eq!(op.render(), "── k7 ──\nraw text");
    }
}
