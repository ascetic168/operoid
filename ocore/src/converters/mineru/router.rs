//! 複雜度分流（K1）——pdf_extract 的毫秒級訊號決定「快速路徑 vs MinerU」。
//!
//! 訊號（計畫 §K1；實測校準 2026-10-09，pdf-extract 0.7.12，語料
//! `pdf2zh/embed-exp/`——詳見計畫檔「實測證據」段）：
//! - **S1 無文字層**：每頁字元數過低（<500）→ 掃描/無文字層 → MinerU（需 OCR）。
//!   pdf_extract 輸出**無分頁字元**（實測 formfeed=0），頁數以原始位元組掃
//!   `/Type /Page` 估計；掃不出（壓縮 xref）時退為「總字元數極低」偵測。
//! - **S2 字形殘片**：`(cid:` 殘片（缺字形特徵）或「Fig. 後無編號」的數字缺失模式
//!   （.tf 表格式數字字形不支援的特徵）→ MinerU。
//! - **S3 圖表引用密度**：Figure/Table/圖/表 引用次數 ≥ max(4, 頁數) → 文件有
//!   視覺內容 → MinerU（sidecar 需要圖）。
//! - **S4 行長碎片化**：短行占比過高或行長變異過低 → 多欄位交錯疑慮 → MinerU。
//! - 無訊號 → 快速路徑（pdf_extract，毫秒級）。**曖昧個案預設走 MinerU**（品質優先）：
//!   硬錯誤（extract 例外）一律升級 MinerU。
//!
//! 門檻以實驗語料校正：mueller2016（IEEE 雙欄）必須被 S3 攔到；純文字文件必須全數
//! 通過走快速路徑。判定結果與訊號存進轉換 metadata（conversion-report.json，可稽核）。

use std::path::Path;

/// 路由政策（`pdf_convert_policy` 設定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertPolicy {
    /// 預設：S1–S4 任一命中即升級 MinerU。
    Auto,
    /// 永遠 pdf_extract 快速路徑（簡單文件、追求毫秒級）。
    AlwaysFast,
    /// 永遠 MinerU（品質優先）。
    AlwaysMineru,
}

impl ConvertPolicy {
    pub fn parse(s: &str) -> Self {
        match s {
            "always_fast" => Self::AlwaysFast,
            "always_mineru" => Self::AlwaysMineru,
            _ => Self::Auto,
        }
    }
    /// 報告用字串（與 operoid.toml 的設定值一致）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::AlwaysFast => "always_fast",
            Self::AlwaysMineru => "always_mineru",
        }
    }
}

/// S1–S4 訊號（可稽核；全量存進 conversion-report.json）。
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Signals {
    /// 估計頁數（原始位元組掃描；0 = 掃不出）。
    pub pages: usize,
    /// 每頁平均字元數（pages 未知時為 None）。
    pub chars_per_page: Option<usize>,
    /// `(cid:` 殘片次數。
    pub cid_fragments: usize,
    /// 「Fig.」後無編號的次數（數字字形缺失模式）。
    pub fig_without_number: usize,
    /// 圖/表引用總次數。
    pub figure_table_refs: usize,
    /// 非空行中短行（<25 字元）占比。
    pub short_line_ratio: f64,
    /// 總字元數（S1 頁數未知時的退路偵測用）。
    pub total_chars: usize,
}

impl Signals {
    /// 逐項判定（名稱 → 命中）；報告與測試共用。
    pub fn hits(&self) -> Vec<(&'static str, String)> {
        let mut v = Vec::new();
        if self.no_text_layer() {
            v.push(("S1", format!("每頁字元數過低（{:?}）→ 可能無文字層", self.chars_per_page)));
        }
        if self.glyph_fragments() {
            v.push(("S2", format!("字形殘片：cid={} fig 無編號={}", self.cid_fragments, self.fig_without_number)));
        }
        if self.visual_density() {
            v.push(("S3", format!("圖表引用 {} 次 ≥ 門檻", self.figure_table_refs)));
        }
        if self.line_fragmentation() {
            v.push(("S4", format!("短行占比 {:.2} → 多欄疑慮", self.short_line_ratio)));
        }
        v
    }

    /// S1：無文字層疑慮。頁數未知時以「總字元 <2000」近似（掃描件抽不出字）。
    fn no_text_layer(&self) -> bool {
        match self.chars_per_page {
            Some(cpp) => cpp < 500,
            None => self.total_chars < 2000,
        }
    }

    /// S2：字形殘片（pdf-extract 的字形警告印在 stdout、in-process 難歸因——
    /// 以文字側特徵偵測）。
    fn glyph_fragments(&self) -> bool {
        self.cid_fragments >= 3 || self.fig_without_number >= 3
    }

    /// S3：文件有視覺內容（sidecar 需要圖）。
    fn visual_density(&self) -> bool {
        let pages = if self.pages == 0 { 4 } else { self.pages };
        self.figure_table_refs >= pages.max(4)
    }

    /// S4：行長碎片化（多欄位交錯疑慮）。
    fn line_fragmentation(&self) -> bool {
        self.short_line_ratio > 0.55
    }

    /// 任一訊號命中 → MinerU。
    pub fn any_hit(&self) -> bool {
        self.no_text_layer() || self.glyph_fragments() || self.visual_density() || self.line_fragmentation()
    }
}

/// 路由判定。
#[derive(Debug, Clone)]
pub struct RouteDecision {
    pub policy: ConvertPolicy,
    pub signals: Signals,
    /// true = 走 MinerU；false = 快速路徑。
    pub to_mineru: bool,
    /// 人類可讀理由（存報告）。
    pub reason: String,
}

/// 對「pdf 原始位元組＋pdf_extract 文字」跑訊號分流。
/// `extracted` 為 None 表示 pdf_extract 硬錯誤 → 一律升級 MinerU（例外路徑）。
pub fn route(pdf_bytes: &[u8], extracted: Option<&str>, policy: ConvertPolicy) -> RouteDecision {
    let mut d = RouteDecision {
        policy,
        signals: Signals::default(),
        to_mineru: true,
        reason: String::new(),
    };
    match policy {
        ConvertPolicy::AlwaysFast => {
            d.to_mineru = false;
            d.reason = "policy=always_fast".into();
            return d;
        }
        ConvertPolicy::AlwaysMineru => {
            d.to_mineru = true;
            d.reason = "policy=always_mineru".into();
            return d;
        }
        ConvertPolicy::Auto => {}
    }
    let Some(text) = extracted else {
        d.reason = "pdf_extract 硬錯誤 → 例外路徑升級 MinerU".into();
        return d;
    };
    let signals = analyze(pdf_bytes, text);
    let hits = signals.hits();
    d.signals = signals.clone();
    d.to_mineru = signals.any_hit();
    d.reason = if hits.is_empty() {
        "無訊號 → 快速路徑".into()
    } else {
        hits.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join("；")
    };
    d
}

/// 純訊號計算（測試直接餵文字）。
pub fn analyze(pdf_bytes: &[u8], text: &str) -> Signals {
    let pages = count_pages(pdf_bytes);
    let total = text.chars().count();
    let chars_per_page = if pages > 0 { Some(total / pages) } else { None };

    let cid_fragments = text.matches("(cid:").count();
    let fig_without_number = FIG_NO_NUM.find_iter(text).count();
    let figure_table_refs = FIG_TABLE_REF.find_iter(text).count();

    let lines: Vec<usize> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.chars().count())
        .collect();
    let short_line_ratio = if lines.is_empty() {
        0.0
    } else {
        lines.iter().filter(|&&n| n < 25).count() as f64 / lines.len() as f64
    };

    Signals {
        pages,
        chars_per_page,
        cid_fragments,
        fig_without_number,
        figure_table_refs,
        short_line_ratio,
        total_chars: total,
    }
}

/// 原始位元組掃 `/Type /Page`（排除 `/Pages`）估頁數；壓縮 xref 掃不出 → 0。
pub fn count_pages(pdf_bytes: &[u8]) -> usize {
    let mut n = 0usize;
    let mut rest = pdf_bytes;
    while let Some(i) = find(rest, b"/Type /Page") {
        let after = &rest[i + 11..];
        if !after.starts_with(b"s") {
            n += 1;
        }
        rest = after;
    }
    n
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

// Rust regex 無 lookahead——以「Fig. 後第一個非空白字元非數字/羅馬字（或行尾）」表達缺號。
static FIG_NO_NUM: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?i)\bFig\.\s*(?:[^0-9IVX\s]|$)").expect("static regex")
});
static FIG_TABLE_REF: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?i)\b(?:Figure|Fig\.|Table|圖|表)\s*[0-9]*[IVXivx]*").expect("static regex")
});

/// 快速路徑來源（保留 pdf_text 為 naive fallback 的既定地位）。
pub fn extract_text(pdf: &Path) -> anyhow::Result<String> {
    super::super::pdf_text::extract(pdf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// mueller2016 型（雙欄論文）必須被 S3 攔到——即使欄序正確、無字形殘片。
    #[test]
    fn academic_paper_hits_s3() {
        let body = "Deep silicon integration drives integrated voltage regulators [1]. ".repeat(80);
        let refs = "As shown in Figure 1 and Table II, Figure 2 compares. Figure 3 lists. \
                    Table III gives Figure 4 results, Table IV and Figure 5 follow. Fig. 6 too.";
        let text = format!("{body}\n{refs}");
        let s = analyze(b"", &text);
        assert!(s.visual_density(), "refs={} pages={}", s.figure_table_refs, s.pages);
        assert!(!s.line_fragmentation());
        let d = route(b"", Some(&text), ConvertPolicy::Auto);
        assert!(d.to_mineru, "reason: {}", d.reason);
        assert!(d.reason.contains("S3"));
    }

    /// 純文字文件（書信/備忘）必須全數通過 → 快速路徑。
    #[test]
    fn plain_text_goes_fast() {
        let para = "This is a regular paragraph line with plenty of characters in it, ";
        let text: String = std::iter::repeat_n(format!("{para}\n\n"), 40).collect::<Vec<_>>().concat();
        let d = route(b"", Some(&text), ConvertPolicy::Auto);
        assert!(!d.to_mineru, "reason: {}", d.reason);
        assert_eq!(d.reason, "無訊號 → 快速路徑");
    }

    /// S1：掃描件（近乎無文字）→ MinerU；頁數掃不出時退總字元偵測。
    #[test]
    fn scanned_pdf_hits_s1() {
        let d = route(b"", Some("scant text"), ConvertPolicy::Auto);
        assert!(d.to_mineru);
        assert!(d.reason.contains("S1"));
    }

    /// S2：(cid: 殘片與「Fig.」後無編號都是字形缺失特徵。
    #[test]
    fn glyph_fragments_hit_s2() {
        let text = "(cid:12) some (cid:3) text (cid:77) with fragments and also Fig. without a number, Fig. again, Fig. thrice.";
        let d = route(b"", Some(text), ConvertPolicy::Auto);
        assert!(d.to_mineru);
        assert!(d.reason.contains("S2"));
    }

    /// S4：多欄碎片化（大量超短行）→ MinerU。
    #[test]
    fn fragmented_lines_hit_s4() {
        let text = "ab cd\nef gh\nij kl\nmn op\nqr st\nuv wx\n".repeat(30);
        let d = route(b"", Some(&text), ConvertPolicy::Auto);
        assert!(d.to_mineru);
        assert!(d.reason.contains("S4"));
    }

    /// 政策覆寫：always_fast／always_mineru 不看訊號。
    #[test]
    fn policy_overrides_signals() {
        let refs = "Figure 1 Table II Figure 2 Table III Figure 3 Table IV Figure 4";
        assert!(!route(b"", Some(refs), ConvertPolicy::AlwaysFast).to_mineru);
        assert!(route(b"", Some("plain"), ConvertPolicy::AlwaysMineru).to_mineru);
    }

    /// pdf_extract 硬錯誤（加密 PDF 等）→ 例外路徑一律升級。
    #[test]
    fn extract_error_goes_mineru() {
        let d = route(b"", None, ConvertPolicy::Auto);
        assert!(d.to_mineru);
        assert!(d.reason.contains("硬錯誤"));
    }

    /// 頁數掃描：`/Type /Page` 計數、排除 `/Pages`。
    #[test]
    fn count_pages_scans_raw_bytes() {
        let raw = b"xx /Type /Pages /Kids [ /Type /Page ] /Type /Page yy /Type /Pages";
        assert_eq!(count_pages(raw), 2);
        assert_eq!(count_pages(b"nothing here"), 0);
    }
}
