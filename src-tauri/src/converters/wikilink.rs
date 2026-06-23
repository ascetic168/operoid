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

/// bullets：把一組 (name) 轉成 wikilink bullet 清單（每行 `- [[dir/slug|name]]`）。
pub fn bullet_lines(dir: &str, names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|n| format!("- {}", qualified(dir, n)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_basic() {
        assert_eq!(qualified("people", "Aaron Miletich"), "[[people/aaron-miletich|Aaron Miletich]]");
        assert_eq!(qualified("people", "陳昌瑞"), "[[people/陳昌瑞|陳昌瑞]]");
    }

    #[test]
    fn bullet_lines_format() {
        let lines = bullet_lines("people", &["Bob Su".to_string(), "晶盛".to_string()]);
        assert_eq!(lines[0], "- [[people/bob-su|Bob Su]]");
        assert_eq!(lines[1], "- [[people/晶盛|晶盛]]");
    }
}
