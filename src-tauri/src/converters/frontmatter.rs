//! Frontmatter — YAML frontmatter 的產生與最小解析。
//!
//! 採用 Python 腳本的單引號 scalar 慣例（`yq`）：把值用 `'…'` 包起，內部 `'` → `''`。
//! gbrain 的 schema pack 只認 type/title/tags（+選用 slug）產生連結；其餘鍵可存在但不產生邊。

/// YAML 單引號 scalar（對應 csv_to_people.py 的 yq）。
pub fn yaml_single_quote(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        if c == '\'' {
            out.push_str("''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// 由 (key, raw_value) 序列組出 frontmatter 區塊，結尾含換行。
/// 呼叫端決定 value 是否要先過 yaml_single_quote（title 要；type/tags 通常原樣）。
pub fn build(entries: &[(&str, String)]) -> String {
    let mut s = String::from("---\n");
    for (k, v) in entries {
        s.push_str(k);
        s.push_str(": ");
        s.push_str(v);
        s.push('\n');
    }
    s.push_str("---\n");
    s
}

/// 把文件切成 (frontmatter 文字, body)；frontmatter 不存在時回 (None, 原文)。
pub fn split(text: &str) -> (Option<&str>, &str) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if let Some(rest) = text.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let fm = &rest[..end];
            let body = &rest[end + "\n---\n".len()..];
            return (Some(fm), body);
        }
        // frontmatter 在檔尾以 \n--- 結束（body 為空）
        if let Some(end) = rest.find("\n---") {
            let body_start = end + "\n---".len();
            let body = rest[body_start..].trim_start_matches('\n');
            return (Some(&rest[..end]), body);
        }
    }
    (None, text)
}

/// 從 frontmatter 取出某 key 的值（第一個 `key: value`）；處理單引號 scalar。
pub fn get(fm: Option<&str>, key: &str) -> Option<String> {
    let fm = fm?;
    let prefix = format!("{key}:");
    for line in fm.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(&prefix) {
            let v = rest.trim();
            return Some(unquote_scalar(v));
        }
    }
    None
}

/// 反解單引號 scalar（`'…''…'` → 原值）；非引號包夾則原樣回傳。
fn unquote_scalar(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        let inner = &v[1..v.len() - 1];
        inner.replace("''", "'")
    } else {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_quote_escapes() {
        assert_eq!(yaml_single_quote("O'Brien"), "'O''Brien'");
        assert_eq!(yaml_single_quote("晶盛"), "'晶盛'");
    }

    #[test]
    fn build_block() {
        let s = build(&[
            ("type", "person".into()),
            ("title", yaml_single_quote("Aaron")),
            ("tags", "[people, contact]".into()),
        ]);
        assert!(s.starts_with("---\ntype: person\ntitle: 'Aaron'\ntags: [people, contact]\n---\n"));
    }

    #[test]
    fn split_and_get() {
        let md = "---\ntype: company\ntitle: '一品光學'\ntags: [companies, contact]\n---\n# 一品光學\n";
        let (fm, body) = split(md);
        assert_eq!(get(fm, "type"), Some("company".into()));
        assert_eq!(get(fm, "title"), Some("一品光學".into()));
        assert!(body.starts_with("# 一品光學"));
    }

    #[test]
    fn no_frontmatter() {
        let (fm, body) = split("# just a heading\n");
        assert!(fm.is_none());
        assert!(body.contains("just a heading"));
    }
}
