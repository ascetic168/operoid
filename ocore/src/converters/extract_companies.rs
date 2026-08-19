//! people/*.md → companies/*.md —— 忠實移植自 `extract_companies.py`。
//!
//! 掃描每個 people 頁的 `- 公司/組織:` bullet，依公司分組，產出精瘦的公司頁
//! （identity + 成員 dir-qualified wikilink）。那個 wikilink 就是 gbrain 邊的來源。
//! 別名合併（company_aliases.json）+ enriched-page 保護（凍結手編頁）皆移植。
//! 產出未寫檔；由 factory/command 決定寫入（並跳過 enriched 頁）。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;

use super::{frontmatter, slug};

#[derive(Debug, Clone, Serialize)]
pub struct CompanyPage {
    pub slug: String,
    pub name: String,
    pub markdown: String,
}

#[derive(Debug, Serialize)]
pub struct CompaniesImport {
    pub people_read: usize,
    pub people_with_org: usize,
    pub distinct: usize,
    pub total_links: usize,
    pub pages: Vec<CompanyPage>,
}

/// 從 people 目錄建構公司頁。`aliases`：variant→canonical（可空）。
pub fn build(people_dir: &Path, aliases: &HashMap<String, String>) -> Result<CompaniesImport> {
    let org_re = Regex::new(r"(?m)^- 公司/組織:\s*(.+?)\s*$").unwrap();
    let title_re = Regex::new(r"(?m)^title:\s*(.+?)\s*$").unwrap();
    let h1_re = Regex::new(r"(?m)^#\s+(.+?)\s*$").unwrap();

    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(people_dir)
        .with_context(|| format!("讀取 people 目錄失敗：{}", people_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("md"))
        .collect();
    files.sort();

    // key -> (display, members: Vec<(slug,name)>, variants: Set)
    let mut order: Vec<String> = Vec::new();
    let mut display_map: HashMap<String, String> = HashMap::new();
    let mut members_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut variants_map: HashMap<String, HashSet<String>> = HashMap::new();

    let mut people_read = 0usize;
    let mut people_with_org = 0usize;
    let mut total_links = 0usize;

    for path in &files {
        people_read += 1;
        let text = std::fs::read_to_string(path)?;
        let slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        // name：frontmatter title（去單引號）→ H1 → slug
        let name = title_re
            .captures(&text)
            .map(|c| {
                let v = c.get(1).unwrap().as_str().trim();
                unquote_yaml(v)
            })
            .filter(|v| !v.is_empty())
            .or_else(|| h1_re.captures(&text).map(|c| c.get(1).unwrap().as_str().trim().to_string()))
            .unwrap_or_else(|| slug.clone());

        let orgs: Vec<String> = org_re
            .captures_iter(&text)
            .map(|c| c.get(1).unwrap().as_str().trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if orgs.is_empty() {
            continue;
        }
        people_with_org += 1;

        for org in orgs {
            let canon = aliases.get(&org).cloned().unwrap_or_else(|| org.clone());
            let key = norm_key(&canon);
            if key.is_empty() {
                continue;
            }
            if !display_map.contains_key(&key) {
                order.push(key.clone());
                display_map.insert(key.clone(), canon.clone());
            }
            members_map.entry(key.clone()).or_default().push((slug.clone(), name.clone()));
            total_links += 1;
            if org != canon {
                variants_map.entry(key).or_default().insert(org);
            }
        }
    }

    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut pages = Vec::new();
    for key in &order {
        let display = display_map[key].clone();
        let members = members_map[key].clone();
        // 成員去重（同一人可能列同一公司兩次）— 以 slug 為準，保留首次
        let mut uniq: Vec<(String, String)> = Vec::new();
        let mut seen_slugs: HashSet<String> = HashSet::new();
        for (s, n) in &members {
            if seen_slugs.insert(s.clone()) {
                uniq.push((s.clone(), n.clone()));
            }
        }
        let mut sorted: Vec<(String, String)> = uniq.clone();
        sorted.sort_by(|a, b| a.1.cmp(&b.1));

        let base = slug::slugify(&display, "company");
        let slug = slug::unique_slug(base, &mut seen);

        let variants: Vec<String> = {
            let mut v: Vec<String> = variants_map.get(key).cloned().unwrap_or_default().into_iter().collect();
            v.sort();
            v
        };
        pages.push(CompanyPage {
            slug,
            name: display.clone(),
            markdown: build_page(&display, &sorted, &variants),
        });
    }

    Ok(CompaniesImport {
        people_read,
        people_with_org,
        distinct: pages.len(),
        total_links,
        pages,
    })
}

fn norm_key(s: &str) -> String {
    let re = Regex::new(r"\s+").unwrap();
    re.replace_all(s.trim(), " ").into_owned()
}

fn unquote_yaml(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        v[1..v.len() - 1].replace("''", "'")
    } else {
        v.to_string()
    }
}

/// 與 extract_companies.py 的 build_page 一致。
fn build_page(name: &str, members: &[(String, String)], aliases: &[String]) -> String {
    let mut lines: Vec<String> = vec![
        "---".into(),
        "type: company".into(),
        format!("title: {}", frontmatter::yaml_single_quote(name)),
        "tags: [companies, contact]".into(),
        "---".into(),
        String::new(),
        format!("# {name}"),
        String::new(),
        format!("{name}，通訊錄中有 {} 位聯絡人。", members.len()),
        String::new(),
    ];
    if !aliases.is_empty() {
        lines.push(format!("別名：{}", aliases.join("、")));
        lines.push(String::new());
    }
    lines.push("## 聯絡人".into());
    lines.push(String::new());
    for (s, n) in members {
        lines.push(format!("- [[people/{s}|{n}]]"));
    }
    lines.push(String::new());
    lines.join("\n").trim_end().to_string() + "\n"
}

/// enriched-page 保護：frontmatter `enriched: true` 或 `<!-- enriched -->` 註解 → 凍結。
pub fn is_enriched(text: &str) -> bool {
    let re = Regex::new(r"(?m)^enriched:\s*true").unwrap();
    re.is_match(text) || text.contains("<!-- enriched")
}

/// 載入 company_aliases.json（{ "aliases": { variant: canonical } }）；不存在回空。
pub fn load_aliases(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    #[derive(serde::Deserialize)]
    struct Wrap {
        #[serde(default)]
        aliases: HashMap<String, String>,
    }
    let text = std::fs::read_to_string(path)?;
    let w: Wrap = serde_json::from_str(&text).context("company_aliases.json 解析失敗")?;
    Ok(w.aliases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn builds_companies_from_people() {
        let dir = std::env::temp_dir().join("gbs_extr_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("jane.md"),
            "---\ntype: person\ntitle: 'Jane Doe'\n---\n# Jane Doe\n\n- 公司/組織: Acme\n",
        )
        .unwrap();
        fs::write(
            dir.join("bob.md"),
            "---\ntype: person\ntitle: 'Bob'\n---\n# Bob\n\n- 公司/組織: Acme\n- 公司/組織: Beta\n",
        )
        .unwrap();
        let aliases = HashMap::new();
        let imp = build(&dir, &aliases).unwrap();
        assert_eq!(imp.people_read, 2);
        assert_eq!(imp.people_with_org, 2);
        assert_eq!(imp.distinct, 2); // Acme, Beta
        assert_eq!(imp.total_links, 3);

        let acme = imp.pages.iter().find(|p| p.slug == "acme").unwrap();
        assert!(acme.markdown.contains("- [[people/bob|Bob]]"));
        assert!(acme.markdown.contains("- [[people/jane|Jane Doe]]"));
        assert!(acme.markdown.contains("通訊錄中有 2 位聯絡人"));
        let beta = imp.pages.iter().find(|p| p.slug == "beta").unwrap();
        assert!(beta.markdown.contains("- [[people/bob|Bob]]"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn enriched_detection() {
        assert!(is_enriched("---\ntype: company\nenriched: true\n---\n"));
        assert!(is_enriched("# X\n\n<!-- enriched -->\n"));
        assert!(!is_enriched("---\ntype: company\n---\n# Plain\n"));
    }
}
