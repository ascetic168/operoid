//! Slug — 忠實移植自 csv_to_people.py / extract_companies.py 的 `slugify`。
//!
//! 這是本腦的「事實 slug 函式」：既有的 1963 個 people/ 檔名與 companies/ 裡的
//! `[[people/slug|Name]]` 連結都用它產生。保留 CJK/kana/hangul（is_alphanumeric 為真），
//! 小寫 ASCII、把分隔符/非法字元摺成 `-`、收斂連續 `-`、裁切至 80 字元。
//! 與既有語料一致 → 新產生的 wikilink 才會解析得到。

use std::collections::HashMap;

/// Windows 非法檔名字元 + 純空白控制字元 → 一律當分隔符。
const ILLEGAL: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|', '\t', '\n', '\r'];

/// 名稱 → 檔案系統安全的 slug。`fallback` 用於名稱為空時。
pub fn slugify(s: &str, fallback: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return fallback.to_string();
    }
    // 小寫（含 unicode；與 Python str.lower() 在 CJK/ASCII 上一致）
    let lower: String = s.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut out = String::with_capacity(lower.len());
    for c in lower.chars() {
        if ILLEGAL.contains(&c) {
            out.push('-');
        } else if c == '-' || c.is_alphanumeric() {
            out.push(c);
        } else {
            out.push('-');
        }
    }
    collapse_hyphens(&mut out);
    let trimmed = out.trim_matches('-');
    // 裁切 80 字元（以 char 計，避免切斷多位元組 CJK），結尾再去 `-`
    let mut result: String = trimmed.chars().take(80).collect();
    while result.ends_with('-') {
        result.pop();
    }
    if result.is_empty() {
        fallback.to_string()
    } else {
        result
    }
}

/// 收斂連續 `-` 為單一 `-`（對應 Python re.sub(r'-+', '-', ...)）。
fn collapse_hyphens(s: &mut String) {
    let mut buf = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c == '-' {
            if !prev_dash {
                buf.push('-');
            }
            prev_dash = true;
        } else {
            buf.push(c);
            prev_dash = false;
        }
    }
    *s = buf;
}

/// 批次 slug 去重：首次出現保留 base，第二次 `-2`、第三次 `-3` …
/// （對應 csv_to_people.py 的 `seen` 字典邏輯）。
pub fn unique_slug(base: String, seen: &mut HashMap<String, u32>) -> String {
    let count = seen.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}-{count}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_lowercase_and_separators() {
        assert_eq!(slugify("Aaron Miletich", "x"), "aaron-miletich");
        assert_eq!(slugify("  Bob  Su ", "x"), "bob-su");
    }

    #[test]
    fn preserves_cjk_and_mixed() {
        // 對應既有檔名 alex陳昌瑞 / 晶盛 / 5號海鮮熱炒
        assert_eq!(slugify("Alex陳昌瑞", "x"), "alex陳昌瑞");
        assert_eq!(slugify("晶盛", "x"), "晶盛");
        assert_eq!(slugify("5號海鮮熱炒", "x"), "5號海鮮熱炒");
    }

    #[test]
    fn illegal_chars_become_separators() {
        assert_eq!(slugify("a/b:c?d", "x"), "a-b-c-d");
    }

    #[test]
    fn collapses_and_trims_hyphens() {
        assert_eq!(slugify("---A---B---", "x"), "a-b");
    }

    #[test]
    fn empty_uses_fallback() {
        assert_eq!(slugify("", "person-1"), "person-1");
        assert_eq!(slugify("   ", "person-1"), "person-1");
    }

    #[test]
    fn leading_digit_not_stripped() {
        assert_eq!(slugify("1foo", "x"), "1foo");
        assert_ne!(slugify("1foo", "x"), slugify("foo", "x"));
    }

    #[test]
    fn truncates_to_80_chars() {
        let long = "a".repeat(120);
        let s = slugify(&long, "x");
        assert_eq!(s.chars().count(), 80);
    }

    #[test]
    fn unique_slug_suffix() {
        let mut seen = HashMap::new();
        assert_eq!(unique_slug("foo".into(), &mut seen), "foo");
        assert_eq!(unique_slug("foo".into(), &mut seen), "foo-2");
        assert_eq!(unique_slug("foo".into(), &mut seen), "foo-3");
        assert_eq!(unique_slug("bar".into(), &mut seen), "bar");
    }
}
