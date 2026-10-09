//! 章節切塊（K1）——語意忠實對齊實驗 `run_experiment.py` 的 `pack_chunks`＋V3-t 混合式
//! （docs/Operoid-研究-嵌入檔索實驗報告.md §3/§8.2：hit@5 12/12、gbrain 管線 MRR 0.917）。
//!
//! - 區塊依文件順序累積；**章節邊界必切**、超過 [`MAX_CHUNK_CHARS`] 即切（可略超——
//!   與實驗一致：先附加後檢查，單一長 block 不再切）。
//! - 文字 chunk body＝`title: {論文標題} | text: [{章節標籤}] {內容}`；
//!   圖 block 內嵌為 `[Figure N. {caption}]`（caption 內嵌，C2 的 V1 語意）。
//! - 圖獨立檢索項 body＝`title: {論文標題} | text: [Figure N in Section {id}, page {P}] {caption}`
//!   （實驗 V2 語意；文件側前綴由 gbrain 逐字通過——第十章攔截驗證）。

use super::structured::{join_labels, Block, BlockKind};

/// 切塊上限（實驗 MAX_CHUNK_CHARS——約 450–600 tokens，遠低於 llama-server 8192 批次上限）。
pub const MAX_CHUNK_CHARS: usize = 1800;

/// 一個文字 chunk（對應實驗 V1 的 notes；attrs 供評測歸因）。
#[derive(Debug, Clone)]
pub struct Chunk {
    /// 章節 id（如 "III.B"；front matter → "front"）。
    pub section_id: String,
    /// 章節標籤（如 "III BUCK CONVERTER DESIGN > B Efficiency Modeling"）。
    pub section_label: String,
    /// chunk 內引用的圖號。
    pub figs: Vec<u32>,
    /// chunk 內的表號（羅馬數字字串）。
    pub tables: Vec<String>,
    /// 內文（不含文件前綴；block 以空行相接，圖為 `[Figure N. …]`）。
    pub content: String,
}

/// 章節邊界切塊（pack_chunks 的 Rust 移植）。
pub fn pack_chunks(blocks: &[Block]) -> Vec<Chunk> {
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut cur: Vec<&Block> = Vec::new();
    let mut cur_section: Vec<(String, String)> = Vec::new();
    let mut started = false;

    macro_rules! flush {
        () => {
            if !cur.is_empty() {
                let sid = join_labels(&cur_section, ".");
                let label = section_label_of(&cur_section);
                let figs: Vec<u32> = {
                    let mut v: Vec<u32> = cur.iter().filter_map(|b| b.fig).collect();
                    v.sort_unstable();
                    v.dedup();
                    v
                };
                let tables: Vec<String> = {
                    let mut v: Vec<String> =
                        cur.iter().filter_map(|b| b.table.clone()).collect();
                    v.sort();
                    v.dedup();
                    v
                };
                let parts: Vec<String> = cur
                    .iter()
                    .map(|b| match b.kind {
                        BlockKind::Figure => match b.fig {
                            Some(n) => format!("[Figure {n}. {}]", b.text),
                            None => format!("[{}]", b.text),
                        },
                        _ => b.text.clone(),
                    })
                    .collect();
                chunks.push(Chunk {
                    section_id: sid,
                    section_label: label,
                    figs,
                    tables,
                    content: parts.join("\n\n"),
                });
                cur.clear();
            }
        };
    }

    for b in blocks {
        if started && b.section != cur_section && !cur.is_empty() {
            flush!();
        }
        started = true;
        cur_section = b.section.clone();
        cur.push(b);
        let total: usize = cur.iter().map(|x| x.text.chars().count()).sum();
        if total > MAX_CHUNK_CHARS {
            flush!();
        }
    }
    flush!();
    chunks
}

fn section_label_of(section: &[(String, String)]) -> String {
    let segs: Vec<String> = section
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

/// 文字 chunk 筆記 body（實驗 V1：文件前綴 `[章節]` 內嵌）。
pub fn text_body(title: &str, ch: &Chunk) -> String {
    format!("title: {title} | text: [{}] {}", ch.section_label, ch.content)
}

/// 圖片筆記 body（實驗 V2：定位器＋完整 caption）。
pub fn figure_body(title: &str, fig: u32, section_id: &str, page_1based: i64, caption: &str) -> String {
    format!("title: {title} | text: [Figure {fig} in Section {section_id}, page {page_1based}] Figure {fig}. {caption}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(kind: BlockKind, text: &str, sec: &[(&str, &str)]) -> Block {
        Block {
            kind,
            text: text.into(),
            fig: None,
            table: None,
            section: sec
                .iter()
                .map(|(l, n)| (l.to_string(), n.to_string()))
                .collect(),
            page: 0,
            image_source: None,
        }
    }

    /// 章節邊界必切；同章節超長才切；chunk 不跨章節。
    #[test]
    fn packs_by_section_boundary() {
        let long = "x".repeat(1500);
        let blocks = vec![
            b(BlockKind::Text, &long, &[("I", "INTRO")]),
            b(BlockKind::Text, "second", &[("I", "INTRO")]),
            b(BlockKind::Text, "next section", &[("II", "DESIGN")]),
        ];
        let chunks = pack_chunks(&blocks);
        assert_eq!(chunks.len(), 2, "同章節 1500+6 併一塊；II 另起");
        assert_eq!(chunks[0].section_id, "I");
        assert!(chunks[0].content.contains("second"));
        assert_eq!(chunks[1].section_id, "II");
        assert_eq!(chunks[1].figs, Vec::<u32>::new());
    }

    /// 超 1800 即切；溢出的 block 併入當前 chunk（實驗語意：先附加後檢查、block 原子）。
    #[test]
    fn splits_over_limit_but_keeps_block_atomic() {
        let a = "a".repeat(1000);
        let big = "b".repeat(2000);
        let blocks = vec![
            b(BlockKind::Text, &a, &[("I", "S")]),
            b(BlockKind::Text, &big, &[("I", "S")]),
            b(BlockKind::Text, "tail", &[("I", "S")]),
        ];
        let chunks = pack_chunks(&blocks);
        assert_eq!(chunks.len(), 2);
        // 加入 big 後 3002（1000+2000+分隔）> 1800 → 連同 big 一起切出（塊不拆）。
        assert_eq!(chunks[0].content.chars().count(), 3002);
        assert_eq!(chunks[1].content, "tail");
    }

    /// 圖 block 內嵌 `[Figure N. caption]`；attrs 帶出 figs/tables。
    #[test]
    fn figures_inline_and_attrs() {
        let mut f = b(BlockKind::Figure, "Two-chip solution.", &[("I", "S")]);
        f.fig = Some(1);
        let mut t = b(BlockKind::Table, "TABLE I. P\n| a |", &[("I", "S")]);
        t.table = Some("I".into());
        let chunks = pack_chunks(&[f, t]);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("[Figure 1. Two-chip solution.]"));
        assert!(chunks[0].content.contains("TABLE I. P"));
        assert_eq!(chunks[0].figs, vec![1]);
        assert_eq!(chunks[0].tables, vec!["I"]);
    }

    /// body 格式逐字對齊實驗（文件前綴＋章節標籤＋內容）。
    #[test]
    fn body_formats_match_experiment() {
        let mut f = b(BlockKind::Figure, "Two-chip solution.", &[("III", "BUCK"), ("A", "Silicon")]);
        f.fig = Some(1);
        let chunks = pack_chunks(&[f]);
        let body = text_body("Paper Title", &chunks[0]);
        assert_eq!(
            body,
            "title: Paper Title | text: [III BUCK > A Silicon] [Figure 1. Two-chip solution.]"
        );
        let fig_note = figure_body("Paper Title", 1, "III.A", 2, "Two-chip solution.");
        assert_eq!(
            fig_note,
            "title: Paper Title | text: [Figure 1 in Section III.A, page 2] Figure 1. Two-chip solution."
        );
    }
}
