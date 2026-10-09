//! MinerU v4 `structured_content.json` 解析（K1）——`pages[].blocks[]` → 統一區塊流。
//!
//! v4（MinerU 4.x；對照舊 `content_list.json`）關鍵差異：pages 巢狀、bbox 正規化、
//! block 類型更細（`chart` 獨立、`equation` 帶 LaTeX、表格直接是 markdown pipe table）。
//! 實測（mueller2016 全文 v4 basic 檔位，2026-10-09）兩個必須處理的形狀：
//! - 圖被分成 `image`（有完整 caption）與 `chart`（caption 常為 `(a)(b)(c)` 面板標記
//!   或空）；Figure 7–9 屬 chart 類——**有 `Figure N.` caption 的 chart 也是圖**。
//! - 圖說可能落在**相鄰區塊**：Figure 3 的 caption 掛在 TABLE I 的 `captions[]` 裡
//!   （同頁的 chart 裁圖反而無 caption）——table block 的 captions 掃到 `Figure N.`
//!   時拾回為圖說（note 有 caption 可檢；sidecar 列 image_path 記 NULL）。
//!
//! 章節路徑由 `paragraph_title` 內容重建（實測 v4 的 level 全為 2 不可信，
//! 以實驗校正的 regex 分類：`I.`→頂層、`A.`→次層、`1)`→三層）。

use serde::Deserialize;

/// 統一區塊（切塊器 chunk.rs 的輸入；語意對齊實驗 run_experiment.py 的 Block）。
#[derive(Debug, Clone)]
pub struct Block {
    pub kind: BlockKind,
    /// 文字內容；figure＝caption（不含「Figure N.」前綴）；table＝caption＋換行＋pipe table。
    pub text: String,
    pub fig: Option<u32>,
    pub table: Option<String>,
    /// 章節路徑（label, name）——如 [("III","BUCK CONVERTER DESIGN"),("B","Efficiency Modeling")]。
    pub section: Vec<(String, String)>,
    /// 0-based 頁碼（page_idx）。
    pub page: i64,
    /// 圖檔來源（figure；相對 MinerU 輸出目錄，如 `images/page_1_image_7.jpg`）。
    pub image_source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Text,
    Equation,
    Figure,
    Table,
}

/// 解析產物：標題＋區塊流＋部分文件警示。
#[derive(Debug, Clone)]
pub struct Parsed {
    pub title: String,
    pub blocks: Vec<Block>,
    pub is_full_document: bool,
}

// ── serde 模型（寬容：缺欄位一律 default，上游加欄位不破） ─────────────────

#[derive(Deserialize)]
pub struct StructuredContent {
    #[serde(default)]
    pub pages: Vec<Page>,
    #[serde(default)]
    pub is_full_document: bool,
}

#[derive(Deserialize)]
pub struct Page {
    #[serde(default, rename = "page_idx")]
    pub page_idx: i64,
    #[serde(default)]
    pub blocks: Vec<RawBlock>,
}

#[derive(Deserialize)]
pub struct RawBlock {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub captions: Vec<Caption>,
    #[serde(default, rename = "image_source")]
    pub image_source: Option<String>,
}

#[derive(Deserialize)]
pub struct Caption {
    #[serde(default)]
    pub content: String,
}

pub fn parse(json: &str) -> anyhow::Result<Parsed> {
    let d: StructuredContent =
        serde_json::from_str(json).map_err(|e| anyhow::anyhow!("structured_content.json 解析失敗：{e}"))?;
    let mut title = String::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut section: Vec<(String, String)> = Vec::new();
    let mut skip_zone = false;

    for page in &d.pages {
        for raw in &page.blocks {
            match raw.kind.as_str() {
                // 版面雜訊：頁碼/頁首/頁尾不入索引。
                "page_number" | "header" | "footer" => {}
                "doc_title" => {
                    if title.is_empty() {
                        title = raw.content.trim().to_string();
                    }
                }
                "paragraph_title" => {
                    let name = raw.content.trim();
                    if SKIP_SECTIONS.contains(&name.to_ascii_uppercase().as_str()) {
                        skip_zone = true;
                        continue;
                    }
                    apply_heading(&mut section, name);
                    skip_zone = false;
                }
                "text" | "ref_text" => {
                    if skip_zone {
                        continue;
                    }
                    let t = raw.content.trim();
                    if !t.is_empty() {
                        blocks.push(block(BlockKind::Text, t, &section, page.page_idx, None));
                    }
                }
                "equation" => {
                    if skip_zone {
                        continue;
                    }
                    let t = raw.content.trim();
                    if !t.is_empty() {
                        // 公式保留 LaTeX（實驗語意：$$ 區塊原樣入文）。
                        blocks.push(block(BlockKind::Equation, &format!("$$ {t} $$"), &section, page.page_idx, None));
                    }
                }
                "image" | "chart" => {
                    if skip_zone {
                        continue;
                    }
                    if let Some(b) = figure_block(raw, &section, page.page_idx) {
                        blocks.push(b);
                    }
                    // 無 Figure caption 的裁圖（面板/無說明 chart）不入文字流——
                    // sidecar 列由 mod.rs 依 image_source 另行記錄（K3 多模態向量用）。
                }
                "table" => {
                    if skip_zone {
                        continue;
                    }
                    let mut table_caption = String::new();
                    let mut table_num: Option<String> = None;
                    let mut strays: Vec<Block> = Vec::new();
                    for c in &raw.captions {
                        let t = c.content.trim();
                        if t.is_empty() {
                            continue;
                        }
                        if let Some(m) = TABLE_CAP.captures(t) {
                            if table_num.is_none() {
                                table_num = Some(m[1].to_string());
                                table_caption = t.to_string();
                                continue;
                            }
                        }
                        if let Some(m) = FIG_CAP.captures(t) {
                            // 圖說落在 table block（實測 Figure 3 掛在 TABLE I 上）——
                            // 拾回為圖 block（緊隨表格，閱讀順序與版面一致）。
                            let mut b = block(
                                BlockKind::Figure,
                                m[2].trim(),
                                &section,
                                page.page_idx,
                                Some(m[1].parse::<u32>().unwrap_or(0)),
                            );
                            b.image_source = None; // caption-only：同頁裁圖歸屬不猜（P2 面板歸組）
                            strays.push(b);
                            continue;
                        }
                        // 其他 caption（罕見）併入表格說明。
                        if !table_caption.is_empty() {
                            table_caption.push(' ');
                        }
                        table_caption.push_str(t);
                    }
                    let content = raw.content.trim();
                    let text = if table_caption.is_empty() {
                        content.to_string()
                    } else {
                        format!("{table_caption}\n{content}")
                    };
                    let mut b = block(BlockKind::Table, &text, &section, page.page_idx, None);
                    b.table = table_num;
                    blocks.push(b);
                    blocks.extend(strays);
                }
                _ => {} // 未知類型：忽略（向前相容）
            }
        }
    }

    Ok(Parsed {
        title,
        blocks,
        is_full_document: d.is_full_document,
    })
}

fn block(kind: BlockKind, text: &str, section: &[(String, String)], page: i64, fig: Option<u32>) -> Block {
    Block {
        kind,
        text: text.to_string(),
        fig,
        table: None,
        section: section.to_vec(),
        page,
        image_source: None,
    }
}

/// image/chart block → 有 `Figure N.` caption 時回圖 block；否則 None。
fn figure_block(raw: &RawBlock, section: &[(String, String)], page: i64) -> Option<Block> {
    let mut fig: Option<u32> = None;
    let mut cap = String::new();
    for c in &raw.captions {
        let t = c.content.trim();
        if t.is_empty() {
            continue;
        }
        if fig.is_none() {
            if let Some(m) = FIG_CAP.captures(t) {
                fig = m[1].parse::<u32>().ok();
                cap = m[2].trim().to_string();
                continue;
            }
        }
        // 面板標記 ((a)/(b)/(c)) 或補充說明：不併入主 caption（實驗語意：caption 單一來源）。
    }
    fig.map(|n| {
        let mut b = block(BlockKind::Figure, &cap, section, page, Some(n));
        b.image_source = raw.image_source.clone();
        b
    })
}

/// `paragraph_title` 內容 → 遞增章節路徑（實驗 parse_md 語意）：
/// `I.` 頂層重設；`A.` = 頂層＋本段；`1)` = 前兩層＋本段；無編號標題（ABSTRACT 型）記 `("?", name)`。
pub fn apply_heading(section: &mut Vec<(String, String)>, name: &str) {
    let seg = |m: &regex::Captures| (m[1].to_string(), m[2].trim().to_string());
    if let Some(m) = TOP_RX.captures(name) {
        *section = vec![seg(&m)];
    } else if let Some(m) = SUB_RX.captures(name) {
        section.truncate(1);
        section.push(seg(&m));
    } else if let Some(m) = SUBSUB_RX.captures(name) {
        section.truncate(2);
        section.push(seg(&m));
    } else {
        *section = vec![("?".into(), name.to_string())];
    }
}

static TOP_RX: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^([IVX]+)\.\s+(.+)$").expect("static regex"));
static SUB_RX: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^([A-Z])\.\s+(.+)$").expect("static regex"));
static SUBSUB_RX: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^(\d+)\)\s+(.+)$").expect("static regex"));

impl Block {
    /// 章節 id（實驗 section_id：`"III.B"`；空 → `"front"`）。佔位段不輸出。
    pub fn section_id(&self) -> String {
        join_labels(&self.section, ".")
    }
    /// 章節標籤（實驗 section_label：`"III BUCK CONVERTER DESIGN > B Efficiency Modeling"`）。
    pub fn section_label(&self) -> String {
        let segs: Vec<String> = self
            .section
            .iter()
            .filter(|(l, _)| l != "?")
            .map(|(l, n)| format!("{l} {n}"))
            .collect();
        if segs.is_empty() {
            "Front matter".into()
        } else {
            segs.join(" > ")
        }
    }
}

/// 過濾佔位段後以 sep 串接 label（section_id 用）。
pub fn join_labels(section: &[(String, String)], sep: &str) -> String {
    let v: Vec<&str> = section
        .iter()
        .filter(|(l, _)| l != "?")
        .map(|(l, _)| l.as_str())
        .collect();
    if v.is_empty() {
        "front".into()
    } else {
        v.join(sep)
    }
}

static FIG_CAP: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)^Figure (\d+)\.\s*(.*)$").expect("static regex"));
static TABLE_CAP: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^TABLE ([IVX]+)\.\s*(.*)$").expect("static regex"));

const SKIP_SECTIONS: &[&str] = &["REFERENCES", "ACKNOWLEDGMENT"];

#[cfg(test)]
mod tests {
    use super::*;

    /// 合成 v4 文件：涵蓋章節重建、doc_title、image caption、chart 的 Figure caption、
    /// table 的 stray figure caption（Figure 3 型）、equation、REFERENCES skip zone。
    const SAMPLE: &str = r#"{
      "pages": [
        {"page_idx": 0, "blocks": [
          {"type": "header", "content": "IEEE 2016"},
          {"type": "doc_title", "level": 1, "content": "Paper Title Here"},
          {"type": "text", "content": "Abstract paragraph one."},
          {"type": "paragraph_title", "level": 2, "content": "I. INTRODUCTION"},
          {"type": "text", "content": "Intro body sentence."},
          {"type": "image", "bbox": [0,0,1,1], "content": "",
           "captions": [{"content": "Figure 1. Two-chip solution for an IVR."}],
           "image_source": "images/page_1_image_7.jpg"},
          {"type": "paragraph_title", "level": 2, "content": "II. DESIGN"},
          {"type": "paragraph_title", "level": 2, "content": "A. Details"},
          {"type": "text", "content": "Detail body."},
          {"type": "equation", "content": "P = R I ^ { 2 } \\tag{1}"},
          {"type": "table", "content": "| A | B |\n| --- | --- |\n| 1 | 2 |",
           "captions": [{"content": "TABLE I. PARAMETERS"},
                        {"content": "Figure 2. Comparison of measured loss."}],
           "image_source": "images/page_1_table_0.jpg"},
          {"type": "chart", "content": "", "captions": [], "image_source": "images/page_1_chart_0.jpg"},
          {"type": "chart", "content": "", "captions": [{"content": "(a)"}, {"content": "Figure 3. Efficiency versus phases."}], "image_source": "images/page_1_chart_1.jpg"},
          {"type": "paragraph_title", "level": 2, "content": "REFERENCES"},
          {"type": "ref_text", "content": "[1] Someone. Title."}
        ]},
        {"page_idx": 1, "blocks": [
          {"type": "page_number", "content": "2"}
        ]}
      ],
      "is_full_document": true
    }"#;

    #[test]
    fn parses_blocks_sections_and_skips_references() {
        let p = parse(SAMPLE).unwrap();
        assert_eq!(p.title, "Paper Title Here");
        assert!(p.is_full_document);
        let kinds: Vec<_> = p.blocks.iter().map(|b| b.kind).collect();
        // header/page_number 不入；REFERENCES 之後的 ref_text 不入。
        assert_eq!(kinds.len(), 8, "blocks: {p:#?}");
        assert_eq!(p.blocks[0].kind, BlockKind::Text);
        assert_eq!(p.blocks[0].section_id(), "front");
        assert_eq!(p.blocks[1].section_id(), "I");

        // 圖 block：caption 剝掉「Figure 1.」前綴、帶 fig 與 image_source。
        let fig1 = &p.blocks[2];
        assert_eq!(fig1.kind, BlockKind::Figure);
        assert_eq!(fig1.fig, Some(1));
        assert_eq!(fig1.text, "Two-chip solution for an IVR.");
        assert_eq!(fig1.image_source.as_deref(), Some("images/page_1_image_7.jpg"));
        assert_eq!(fig1.section_id(), "I");

        // 章節重建：II → II.A（regex 分類，非 level）。
        let detail = &p.blocks[3];
        assert_eq!(detail.section_id(), "II.A");
        assert_eq!(detail.section_label(), "II DESIGN > A Details");

        // equation 保留 LaTeX。
        assert!(p.blocks[4].text.starts_with("$$ P ="));

        // table：caption＋pipe table 原子、table_num 帶出；stray 圖說拾回為圖 block。
        let table = &p.blocks[5];
        assert_eq!(table.kind, BlockKind::Table);
        assert_eq!(table.table.as_deref(), Some("I"));
        assert!(table.text.starts_with("TABLE I. PARAMETERS"));
        assert!(table.text.contains("| 1 | 2 |"));
        let fig2 = &p.blocks[6];
        assert_eq!(fig2.fig, Some(2));
        assert_eq!(fig2.section_id(), "II.A");
        assert_eq!(fig2.image_source, None, "caption-only 拾回不虛構圖檔");
        // chart 帶 Figure caption → 也是圖；面板標記 (a) 不入 caption。
        let fig3 = &p.blocks[7];
        assert_eq!(fig3.fig, Some(3));
        assert_eq!(fig3.text, "Efficiency versus phases.");
    }

    /// 頁數未知/空文件不炸。
    #[test]
    fn empty_and_malformed() {
        let p = parse(r#"{"pages": [], "is_full_document": false}"#).unwrap();
        assert!(p.blocks.is_empty());
        assert!(!p.is_full_document);
        assert!(parse(r#"{"pages": [{"page_idx": 0}]}"#).unwrap().blocks.is_empty());
        assert!(parse("not json").is_err());
    }

    /// heading 分類：頂層重設、次層附加、三層附加（實驗遞增語意）。
    #[test]
    fn heading_classification() {
        let mut s: Vec<(String, String)> = Vec::new();
        apply_heading(&mut s, "III. BUCK CONVERTER DESIGN");
        assert_eq!(s.len(), 1);
        apply_heading(&mut s, "B. Efficiency Modeling");
        assert_eq!(s.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), ["III", "B"]);
        apply_heading(&mut s, "2) Inductor");
        assert_eq!(s.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), ["III", "B", "2"]);
        apply_heading(&mut s, "IV. NEXT");
        assert_eq!(s.len(), 1);
        apply_heading(&mut s, "ABSTRACT");
        assert_eq!(s[0].0, "?");
    }
}
