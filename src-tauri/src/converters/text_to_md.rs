//! 非結構化文字 → gbrain-legal page（companies / meeting 工廠）。
//!
//! 流程：抽文字 → LLM 結構化成 JSON（title/type/tags/body/timeline/mentioned_names）
//! → Rust 後處理：title→slug、mentioned_names→dir-qualified wikilink、組裝合規 md。
//! slug 與 wikilink 一律由 Rust 用 slug::slugify 計算，不交給 LLM。

use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;

use super::{frontmatter, slug, wikilink};
use crate::config::{gbrain_config::LlmEndpoint, AppConfig, FactoryTargets};
use crate::llm;

/// LLM 回傳的結構化頁面。
#[derive(Debug, Clone, Deserialize)]
pub struct StructuredPage {
    pub title: String,
    #[serde(rename = "type")]
    pub page_type: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub body_markdown: String,
    #[serde(default)]
    pub timeline: Vec<TimelineEntry>,
    /// 文中出現的人物名（Rust 會轉成 [[people/slug|Name]]）。
    #[serde(default)]
    pub mentioned_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimelineEntry {
    pub date: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
}

/// 產出：slug + 合規 markdown。
pub fn render(factory: &str, sp: &StructuredPage, targets: &FactoryTargets) -> (String, String) {
    let (dir, default_type, default_tags): (&str, &str, Vec<&str>) = match factory {
        "companies" => (&targets.companies, "company", vec!["companies", "contact"]),
        "meeting" => (&targets.meetings, "meeting", vec!["meeting"]),
        "people" => (&targets.people, "person", vec!["people", "contact"]),
        _ => ("concepts", "concept", vec![]),
    };

    let title = if sp.title.trim().is_empty() {
        "未命名".to_string()
    } else {
        sp.title.trim().to_string()
    };
    let slug = slug::slugify(&title, &format!("{dir}-untitled"));

    let ptype = if sp.page_type.trim().is_empty() {
        default_type.to_string()
    } else {
        sp.page_type.trim().to_string()
    };
    let tags: Vec<String> = if sp.tags.is_empty() {
        default_tags.iter().map(|s| s.to_string()).collect()
    } else {
        sp.tags.clone()
    };

    let mut b: Vec<String> = vec![format!("# {title}"), String::new()];
    let body = sp.body_markdown.trim();
    if !body.is_empty() {
        b.push(body.to_string());
        b.push(String::new());
    }

    // mentioned_names → people wikilink bullets（公司=聯絡人 / 會議=與會者）
    if !sp.mentioned_names.is_empty() {
        let label = match factory {
            "companies" => "## 聯絡人",
            "meeting" => "## 與會者",
            _ => "## 相關人物",
        };
        b.push(label.to_string());
        b.push(String::new());
        for n in &sp.mentioned_names {
            let nm = n.trim();
            if !nm.is_empty() {
                b.push(format!("- {}", wikilink::qualified("people", nm)));
            }
        }
        b.push(String::new());
    }

    // timeline（用 <!-- timeline --> sentinel；gbrain 才會視為 timeline）
    if !sp.timeline.is_empty() {
        b.push("<!-- timeline -->".to_string());
        b.push(String::new());
        for t in &sp.timeline {
            let d = t.date.trim();
            let ttl = t.title.trim();
            b.push(format!("### {d} — {ttl}"));
            b.push(String::new());
            let det = t.detail.trim();
            if !det.is_empty() {
                b.push(det.to_string());
                b.push(String::new());
            }
        }
    }

    let fm = frontmatter::build(&[
        ("type", ptype),
        ("title", frontmatter::yaml_single_quote(&title)),
        ("tags", format!("[{}]", tags.join(", "))),
    ]);
    let joined = b.join("\n").trim_end().to_string();
    (slug, format!("{fm}{joined}\n"))
}

/// 把 LLM 回應去掉 ```json / ``` 圍欄，取出 JSON 物件。
fn strip_fence(resp: &str) -> String {
    let t = resp.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t).trim();
    // 取第一個 { 到最後一個 }（容忍前後廢話）
    if let (Some(start), Some(end)) = (t.find('{'), t.rfind('}')) {
        if end > start {
            return t[start..=end].to_string();
        }
    }
    t.to_string()
}

fn system_prompt(factory: &str) -> String {
    let role = match factory {
        "companies" => "公司（company）",
        "meeting" => "會議（meeting）",
        "people" => "人物（person）",
        _ => "主題（concept）",
    };
    format!(
        "你是知識圖譜頁面抽取器。把使用者给的文件轉成一個 gbrain 合規的 {role} 頁面。\n\
         只回傳一個 JSON 物件，不要任何說明文字。schema：\n\
         {{\n  \"title\": \"顯示名稱\",\n  \"type\": \"{ptype}\",\n  \"tags\": [\"...\"],\n  \
         \"body_markdown\": \"compiled_truth：最重要的散文摘要（prose 優於表格，重要事實置頂）\",\n  \
         \"timeline\": [{{\"date\": \"YYYY-MM-DD\", \"title\": \"事件標題\", \"detail\": \"細節散文\"}}],\n  \
         \"mentioned_names\": [\"文中出現的人物全名\"]\n}}\n\
         規則：\n\
         - title 為簡潔專有名稱（公司名／會議主題）。\n\
         - body_markdown 用中文散文概述，不要含 wikilink 語法（連結由系統自動產生）。\n\
         - mentioned_names 列出實際出現的人物全名（若文件提到該人在腦中有 people 頁才會解析成邊）。\n\
         - timeline 的 date 必須是 YYYY-MM-DD；沒有明確日期的事件不要放 timeline。\n\
         - 沒有的欄位給空字串或空陣列。",
        ptype = match factory {
            "companies" => "company",
            "meeting" => "meeting",
            "people" => "person",
            _ => "concept",
        }
    )
}

/// 文字 → 結構化頁面（呼叫 LLM）。`raw` 為已抽出的純文字。
pub async fn text_to_page(
    factory: &str,
    raw: &str,
    cfg: &AppConfig,
    endpoint: &LlmEndpoint,
) -> Result<StructuredPage> {
    let system = system_prompt(factory);
    // 限制輸入長度，避免超出 context
    let max_in = 24_000;
    let trimmed = if raw.chars().count() > max_in {
        let cut: String = raw.chars().take(max_in).collect();
        format!("{cut}\n\n[...文件已截斷...]")
    } else {
        raw.to_string()
    };
    let user = format!("來源類型：{factory}\n\n文件內容：\n{trimmed}\n\n請只回傳 JSON 物件。");
    let resp = llm::complete(endpoint, cfg, &system, &user).await?;
    let json = strip_fence(&resp);
    let sp: StructuredPage =
        serde_json::from_str(&json).context(format!("LLM JSON 解析失敗；原始回應：{resp}"))?;
    Ok(sp)
}

// ── wikilink 補全(給「+」手寫編輯器用)──────────────────────────────────
// 使用者只會寫「林家豪」,不會寫 [[people/林家豪]]。這裡由 LLM 抓出文中的人名/
// 公司名,Rust 負責把它們包成 dir-qualified wikilink(slug 由 slug::slugify 算,
// 與既有語料一致)。已在 [[..]] 內的不重包;頁面自身名稱不自我連結。

#[derive(Deserialize)]
struct NameLists {
    #[serde(default)]
    people: Vec<String>,
    #[serde(default)]
    companies: Vec<String>,
}

/// LLM 從文字中抽取人名/公司名 JSON。
async fn extract_names(body: &str, cfg: &AppConfig, endpoint: &LlmEndpoint) -> Result<NameLists> {
    let system = "你是實體辨識器。從使用者文字中找出明確的「人物全名」與「公司/組織全名」，\
以 JSON 回傳：{\"people\":[...],\"companies\":[...]}。\
只列專有名詞（可作為實體的人名/公司名）；不要職稱、一般名詞、代名詞、日期、數字。\
不要修改原文。沒有就給空陣列。";
    let user = format!("文字：\n{body}\n\n請只回傳 JSON 物件。");
    let resp = llm::complete(endpoint, cfg, system, &user).await?;
    let json = strip_fence(&resp);
    let names: NameLists =
        serde_json::from_str(&json).context(format!("名字 JSON 解析失敗；原始：{resp}"))?;
    Ok(names)
}

/// 把 body 裡的人名/公司名包成 `[[dir/slug|name]]`；已在 `[[..]]` 內的不重包；
/// 頁面自身（own_dir + own_slug）不自我連結。純字串處理，可單測。
pub fn wrap_names(
    body: &str,
    people: &[String],
    companies: &[String],
    own_dir: &str,
    own_slug: &str,
) -> String {
    let mut items: Vec<(String, &'static str)> = Vec::new();
    for n in people {
        let n = n.trim();
        if n.is_empty() {
            continue;
        }
        if own_dir == "people" && slug::slugify(n, "") == own_slug {
            continue; // 不自我連結
        }
        items.push((n.to_string(), "people"));
    }
    for n in companies {
        let n = n.trim();
        if n.is_empty() {
            continue;
        }
        if own_dir == "companies" && slug::slugify(n, "") == own_slug {
            continue;
        }
        items.push((n.to_string(), "companies"));
    }
    // 去重 + 長度遞減（避免短名吃到長名的子字串）
    let mut seen = std::collections::HashSet::new();
    items.retain(|(n, d)| seen.insert((n.clone(), *d)));
    items.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()));
    if items.is_empty() {
        return body.to_string();
    }

    // alternation:`[[..]]` 先吃(保留),其餘為名稱(替換)
    let mut pattern = String::from(r"\[\[[^\]]*\]\]");
    for (n, _) in &items {
        pattern.push('|');
        pattern.push_str(&regex::escape(n));
    }
    let re = Regex::new(&pattern).unwrap();
    re.replace_all(body, |caps: &regex::Captures| {
        let m = caps[0].to_string();
        if m.starts_with("[[") {
            return m; // 既有 wikilink，原樣保留
        }
        let (dir, s) = items
            .iter()
            .find(|(n, _)| *n == m)
            .map(|(n, d)| (*d, slug::slugify(n, "")))
            .unwrap();
        format!("[[{dir}/{s}|{m}]]")
    })
    .into_owned()
}

/// 對整份 markdown 補 wikilink：frontmatter 不動，只 enrich body。回傳(enriched, 名字數)。
pub async fn enrich_wikilinks(
    markdown: &str,
    own_dir: &str,
    own_slug: &str,
    cfg: &AppConfig,
    endpoint: &LlmEndpoint,
) -> Result<(String, usize)> {
    let (fm, body) = frontmatter::split(markdown);
    let names = extract_names(body, cfg, endpoint).await?;
    let count = names.people.len() + names.companies.len();
    let enriched_body = wrap_names(body, &names.people, &names.companies, own_dir, own_slug);
    let head = match fm {
        Some(f) => format!("---\n{f}\n---\n"),
        None => String::new(),
    };
    Ok((format!("{head}{enriched_body}"), count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_company_page() {
        let sp = StructuredPage {
            title: "Acme Corp".into(),
            page_type: "".into(),
            tags: vec![],
            body_markdown: "Acme 是一家做 widget 的公司。".into(),
            timeline: vec![],
            mentioned_names: vec!["Jane Doe".into()],
        };
        let targets = FactoryTargets::default();
        let (slug, md) = render("companies", &sp, &targets);
        assert_eq!(slug, "acme-corp");
        assert!(md.starts_with("---\ntype: company\ntitle: 'Acme Corp'\ntags: [companies, contact]\n---"));
        assert!(md.contains("# Acme Corp"));
        assert!(md.contains("[[people/jane-doe|Jane Doe]]"));
    }

    #[test]
    fn renders_meeting_with_timeline() {
        let sp = StructuredPage {
            title: "Q3 Review".into(),
            page_type: "meeting".into(),
            tags: vec!["meeting".into()],
            body_markdown: "檢視 Q3 進度。".into(),
            timeline: vec![TimelineEntry {
                date: "2026-06-15".into(),
                title: "Q3 Review".into(),
                detail: "Jane 報告營收。".into(),
            }],
            mentioned_names: vec!["Jane Doe".into()],
        };
        let targets = FactoryTargets::default();
        let (slug, md) = render("meeting", &sp, &targets);
        assert_eq!(slug, "q3-review");
        assert!(md.contains("<!-- timeline -->"));
        assert!(md.contains("### 2026-06-15 — Q3 Review"));
        assert!(md.contains("## 與會者"));
    }

    #[test]
    fn strips_code_fence() {
        let r = "```json\n{\"title\":\"X\"}\n```";
        assert_eq!(strip_fence(r), "{\"title\":\"X\"}");
        let r2 = "前綴 {\"a\":1} 後綴";
        assert_eq!(strip_fence(r2), "{\"a\":1}");
    }

    #[test]
    fn wrap_names_people_and_companies() {
        let body = "與會者：趙建宏、林家豪。會中提到晶瀚半導體的 E-07。";
        let out = wrap_names(
            body,
            &["趙建宏".into(), "林家豪".into()],
            &["晶瀚半導體".into()],
            "meetings",
            "some-meeting",
        );
        assert!(out.contains("[[people/趙建宏|趙建宏]]"));
        assert!(out.contains("[[people/林家豪|林家豪]]"));
        assert!(out.contains("[[companies/晶瀚半導體|晶瀚半導體]]"));
    }

    #[test]
    fn wrap_names_skips_self() {
        // 林家豪自己的頁面，body H1 與內文出現自己 → 不自我連結；其他人物仍包。
        let body = "# 林家豪\n\n與陳志遠共同處理 E-07。";
        let out = wrap_names(body, &["林家豪".into(), "陳志遠".into()], &[], "people", "林家豪");
        assert!(!out.contains("[[people/林家豪"));
        assert!(out.contains("[[people/陳志遠|陳志遠]]"));
        assert!(out.contains("# 林家豪")); // H1 保持原樣
    }

    #[test]
    fn wrap_names_no_double_wrap() {
        let body = "找 [[people/林家豪|林家豪]] 與 陳志遠 討論。";
        let out = wrap_names(body, &["林家豪".into(), "陳志遠".into()], &[], "meetings", "m");
        // 已在 [[..]] 內的林家豪不重包；陳志遠包起來
        assert_eq!(out, "找 [[people/林家豪|林家豪]] 與 [[people/陳志遠|陳志遠]] 討論。");
    }

}
