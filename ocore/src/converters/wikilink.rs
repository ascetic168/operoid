//! Dir-qualified wikilink — `[[dir/slug|Name]]`。
//!
//! gbrain 只認 dir-qualified wikilink 為邊（純文字提及/frontmatter 陣列不產生邊）。
//! slug 一律由 slug::slugify 計算（與既有語料一致），不交給 LLM 造。

use super::slug;

/// 產生 `[[{dir}/{slug}|{name}]]`。name 為空時 slug 也空 → 退化為 `[[{dir}/]]`。
pub fn qualified(dir: &str, name: &str) -> String {
    let s = slug::slugify(name, "");
    if s.is_empty() {
        format!("[[{dir}/]]")
    } else {
        format!("[[{dir}/{s}|{name}]]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_basic() {
        assert_eq!(qualified("people", "Aaron Miletich"), "[[people/aaron-miletich|Aaron Miletich]]");
        assert_eq!(qualified("people", "陳昌瑞"), "[[people/陳昌瑞|陳昌瑞]]");
    }
}
