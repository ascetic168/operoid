//! 點擊 wikilink → 把該筆記 `.md` 轉成自足 HTML，用系統預設瀏覽器開啟。
//!
//! 觸發點在前端 console：`ask`/`think` 回覆裡的 `[[dir/slug]]` 標籤被點擊。
//! 檔案位置是**來源感知**的——在「作用中腦」的作用中來源 repo 找（與 factories 一致），
//! 找不到再退其他來源、最後退 `notes_repo_path`。

use std::path::{Path, PathBuf};

use pulldown_cmark::{html::push_html, Options, Parser};
use regex::Regex;
use serde::Serialize;
use tauri::{AppHandle, Runtime};

use crate::config;
use crate::converters::{frontmatter, slug};
use crate::i18n::AppError;
use crate::{brains};

/// 開啟結果（前端用 title 推一行成功訊息）。
#[derive(Serialize)]
pub struct NoteViewResult {
    pub title: String,
}

/// 把 wikilink 內文解析成 `(dir, stem)`：`[[people/JLin|J林]]`、`[people/JLin]` 或
/// `people/JLin`（前端已去括號）皆 → `("people","JLin")`。dir 可能為空（裸標籤）；
/// stem 為空（如 `people/`）→ None。
fn parse_target(target: &str) -> Option<(&str, &str)> {
    let inner = target.trim().trim_matches(|c| c == '[' || c == ']').trim();
    let path_part = inner.split('|').next().unwrap_or(inner).trim();
    if path_part.is_empty() {
        return None;
    }
    let (dir, stem) = match path_part.split_once('/') {
        Some((d, s)) => (d.trim(), s.trim()),
        None => ("", path_part),
    };
    if stem.is_empty() {
        return None;
    }
    Some((dir, stem))
}

/// 候選「檔案系統子目錄」：factory_targets 三類 + inbox/concepts。用來在 wikilink 的 dir
/// 與實際目錄（單複數、未知 dir）對不上時，仍能以 stem 在已知目錄裡找到檔。
fn candidate_dirs(cfg: &config::AppConfig) -> Vec<String> {
    let mut v: Vec<String> = vec![
        cfg.factory_targets.people.clone(),
        cfg.factory_targets.companies.clone(),
        cfg.factory_targets.meetings.clone(),
        "inbox".into(),
        "concepts".into(),
    ];
    v.sort();
    v.dedup();
    v
}

/// 在單一 root 下找 `.md`：先試 wikilink 的 dir，再掃已知目錄，最後大小寫寬容比對。
fn resolve(root: &Path, dir: &str, stem: &str, candidate_dirs: &[String]) -> Option<PathBuf> {
    let fname = format!("{stem}.md");
    // 1. wikilink 指明的 dir（可能是單數 meeting / 未知目錄）
    if !dir.is_empty() {
        let cand = root.join(dir).join(&fname);
        if cand.is_file() {
            return Some(cand);
        }
    }
    // 2. 在已知目錄裡找（處理 meeting↔meetings 之類差異）
    for d in candidate_dirs {
        let cand = root.join(d).join(&fname);
        if cand.is_file() {
            return Some(cand);
        }
    }
    // 3. 大小寫寬容掃描（JLin vs jlin）
    let mut scan_dirs: Vec<PathBuf> = candidate_dirs.iter().map(|d| root.join(d)).collect();
    if !dir.is_empty() {
        scan_dirs.insert(0, root.join(dir));
    }
    for sd in scan_dirs {
        if let Some(f) = scan_dir_ci(&sd, stem) {
            return Some(f);
        }
    }
    None
}

/// 掃目錄，回傳檔名莖（去 .md）大小寫一致的首個 `.md`。
fn scan_dir_ci(dir: &Path, stem: &str) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let lower = stem.to_lowercase();
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("md")) != Some(true) {
            continue;
        }
        if let Some(s) = p.file_stem().and_then(|s| s.to_str()) {
            if s.to_lowercase() == lower {
                return Some(p);
            }
        }
    }
    None
}

/// 讀 `.md`：去 UTF-8 BOM、lossy 解碼（仿 factories::read_text）。
fn read_note(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let bytes = bytes
        .strip_prefix(b"\xef\xbb\xbf")
        .map(|b| b.to_vec())
        .unwrap_or(bytes);
    Ok(String::from_utf8(bytes)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

/// `.md` → 自足 HTML 文件（含內嵌 CSS；頁內 `[[...]]` 只變色不連結）。
fn render_html(md: &str) -> String {
    let (fm, body) = frontmatter::split(md);
    let ptype = frontmatter::get(fm, "type").unwrap_or_default();
    let tags = frontmatter::get(fm, "tags").unwrap_or_default();

    // `<!-- timeline -->` sentinel → 可見小標；其下 `### 日期 — 標題` 由 markdown 原生渲染
    let body: String = body
        .lines()
        .map(|l| if l.trim() == "<!-- timeline -->" { "## 時間線" } else { l })
        .collect::<Vec<_>>()
        .join("\n");

    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let parser = Parser::new_ext(&body, opts);
    let mut body_html = String::new();
    push_html(&mut body_html, parser);

    // 頁內 wikilink → 藍色 span（display 已經被 pulldown escape 過，不再二次 escape）
    let body_html = inline_wikilinks(&body_html);

    assemble_html(&ptype, &tags, &body_html)
}

/// 把頁內 wikilink 換成藍色 span：`[[dir/slug|name]]`/`[[dir/slug]]`（雙括）或
/// `[dir/slug]`（單括）。輸入是 pulldown-cmark 產出（已 escape、且 `[text](url)`
/// 已被轉成 `<a>`），故 name 原樣回放、不需 lookahead。
fn inline_wikilinks(html: &str) -> String {
    let re = Regex::new(r"\[\[([^\]]+)\]\]|\[([^\]\[\s|/]+/[^\]\[\s|/]+)\]").unwrap();
    re.replace_all(html, |caps: &regex::Captures| {
        let (path, display) = if let Some(inner) = caps.get(1) {
            let inner = inner.as_str();
            (inner.split('|').next().unwrap_or(inner), inner.split('|').nth(1).unwrap_or(""))
        } else {
            (caps.get(2).unwrap().as_str(), "")
        };
        let label = if display.trim().is_empty() {
            path.rsplit_once('/').map(|(_, s)| s).unwrap_or(path)
        } else {
            display
        };
        format!("<span class=\"glink\">{}</span>", label.trim())
    })
    .into_owned()
}

fn assemble_html(ptype: &str, tags: &str, body_html: &str) -> String {
    let mut meta = String::new();
    if !ptype.is_empty() || !tags.is_empty() {
        meta.push_str("<div class=\"meta\">");
        if !ptype.is_empty() {
            meta.push_str(&format!("<span class=\"badge\">{}</span>", html_escape(ptype)));
        }
        if !tags.is_empty() {
            meta.push_str(&format!("<span class=\"tags\">{}</span>", html_escape(tags)));
        }
        meta.push_str("</div>\n");
    }

    let mut s = String::new();
    s.push_str("<!doctype html>\n<html lang=\"zh-Hant\">\n<head>\n<meta charset=\"utf-8\">\n");
    s.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    s.push_str("<title>GBrain Note</title>\n<style>\n");
    s.push_str(CSS);
    s.push_str("</style>\n</head>\n<body>\n<div class=\"paper\">\n");
    s.push_str(&meta);
    s.push_str(body_html);
    s.push_str("\n</div>\n</body>\n</html>\n");
    s
}

/// HTML 文字 escape（用於 frontmatter 值；body 由 pulldown 自行 escape）。
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 來源感知：收集候選 root。作用中來源優先 → 其他來源 → `notes_repo_path` 兜底。
async fn collect_roots<R: Runtime>(app: &AppHandle<R>, cfg: &config::AppConfig) -> Vec<PathBuf> {
    let notes_fallback = PathBuf::from(&cfg.notes_repo_path);
    let brain_id = match cfg.active_brain_id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return vec![notes_fallback],
    };
    let sources = match brains::list_sources(app, &brain_id).await {
        Ok(s) => s,
        Err(_) => return vec![notes_fallback], // exe 缺/解析失敗 → 靜默退 fallback
    };

    let mut roots: Vec<PathBuf> = Vec::new();
    let push = |v: &mut Vec<PathBuf>, p: PathBuf| {
        if !v.contains(&p) {
            v.push(p);
        }
    };
    // 作用中來源優先
    if let Some(active) = cfg.active_source_id.as_deref() {
        if let Some(s) = sources.iter().find(|x| x.id == active) {
            push(&mut roots, PathBuf::from(&s.local_path));
        }
    }
    // 其餘來源
    for s in &sources {
        push(&mut roots, PathBuf::from(&s.local_path));
    }
    // 最終兜底
    push(&mut roots, notes_fallback);
    if roots.is_empty() {
        vec![PathBuf::from(&cfg.notes_repo_path)]
    } else {
        roots
    }
}

/// 點擊 wikilink：解析→讀檔→轉 HTML→寫 temp→系統預設瀏覽器開啟。
#[tauri::command]
pub async fn open_note<R: Runtime>(
    app: AppHandle<R>,
    target: String,
) -> Result<NoteViewResult, AppError> {
    let cfg = config::app_config::load(&app).map_err(|e| e.to_string())?;
    let (dir, stem) = parse_target(&target)
        .ok_or_else(|| AppError::new("note.notFound").p("target", &target))?;
    let dirs = candidate_dirs(&cfg);

    let roots = collect_roots(&app, &cfg).await;
    let file = {
        let mut found = None;
        for root in &roots {
            if let Some(f) = resolve(root, dir, stem, &dirs) {
                found = Some(f);
                break;
            }
        }
        found
    }
    .ok_or_else(|| AppError::new("note.notFound").p("target", &target))?;

    let md = read_note(&file).map_err(|e| e.to_string())?;
    let (fm, _) = frontmatter::split(&md);
    let title = frontmatter::get(fm, "title").unwrap_or_else(|| {
        file.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("note")
            .to_string()
    });
    let html = render_html(&md);

    // 寫 temp（檔名用 slug，ASCII/CJK 安全）
    let temp_dir = std::env::temp_dir().join("gbrain-studio");
    std::fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;
    let dir_seg = if dir.is_empty() {
        "note".to_string()
    } else {
        slug::slugify(dir, "dir")
    };
    let name_seg = slug::slugify(stem, "note");
    let out = temp_dir.join(format!("gbrain-{dir_seg}-{name_seg}.html"));
    std::fs::write(&out, html).map_err(|e| e.to_string())?;

    // 系統預設程式開啟（.html → 預設瀏覽器）
    open::that(&out).map_err(|e| e.to_string())?;

    Ok(NoteViewResult { title })
}

/// 自足 HTML 的內嵌樣式（CJK 友善、可獨立閱讀）。
const CSS: &str = r#"
:root { color-scheme: light; }
* { box-sizing: border-box; }
body {
  margin: 0; background: #f5f6f8; color: #1b1b1b;
  font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI",
    "Microsoft JhengHei", "PingFang TC", "Noto Sans CJK TC", sans-serif;
  font-size: 16px; line-height: 1.75;
}
.paper {
  max-width: 780px; margin: 0 auto; padding: 42px 30px 90px;
  background: #fff; min-height: 100vh; box-shadow: 0 0 0 1px #eceeef;
}
h1 { font-size: 1.9em; margin: 0 0 .4em; line-height: 1.3; }
h2 { font-size: 1.35em; margin: 1.7em 0 .5em; border-bottom: 1px solid #ececec; padding-bottom: .25em; }
h3 { font-size: 1.12em; margin: 1.3em 0 .4em; }
p, ul, ol { margin: .6em 0; }
ul, ol { padding-left: 1.5em; }
li { margin: .2em 0; }
code {
  font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
  background: #f1f3f5; padding: .12em .4em; border-radius: 4px; font-size: .9em;
}
pre { background: #f6f8fa; padding: 14px 16px; border-radius: 8px; overflow-x: auto; }
pre code { background: none; padding: 0; }
blockquote {
  border-left: 4px solid #cfe3ff; margin: .8em 0; padding: .2em 1em;
  color: #555; background: #f7faff; border-radius: 0 4px 4px 0;
}
table { border-collapse: collapse; width: 100%; margin: 1em 0; }
th, td { border: 1px solid #ddd; padding: .4em .7em; text-align: left; }
th { background: #f3f4f6; }
hr { border: none; border-top: 1px solid #e5e7eb; margin: 1.6em 0; }
.meta { margin: .2em 0 1.4em; display: flex; gap: .5em; flex-wrap: wrap; align-items: center; }
.badge {
  background: #e0f2fe; color: #0369a1; border: 1px solid #bae6fd;
  padding: .1em .6em; border-radius: 999px; font-size: .8em; font-weight: 600;
}
.tags { color: #6b7280; font-size: .82em; font-family: ui-monospace, Consolas, monospace; }
.glink { color: #0284c7; font-weight: 500; } /* sky-600，頁內 wikilink 變色 */
a { color: #0284c7; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dir_qualified() {
        assert_eq!(parse_target("[[people/JLin]]"), Some(("people", "JLin")));
        assert_eq!(parse_target("[people/JLin]"), Some(("people", "JLin")));
        assert_eq!(parse_target("people/JLin|JLin"), Some(("people", "JLin")));
        assert_eq!(parse_target("people/JLin"), Some(("people", "JLin")));
        assert_eq!(
            parse_target("[[meeting/e-07-良率降低緊急會議|良率會議]]"),
            Some(("meeting", "e-07-良率降低緊急會議"))
        );
        // 裸標籤
        assert_eq!(parse_target("[[JLin]]"), Some(("", "JLin")));
        // stem 空
        assert_eq!(parse_target("[[people/]]"), None);
        assert_eq!(parse_target(""), None);
    }

    #[test]
    fn inline_wikilinks_become_spans() {
        // 雙括 + 單括混合
        let h = inline_wikilinks("找 [[people/jlin|林家豪]] 與 [companies/晶瀚半導體] 討論。");
        assert!(h.contains("<span class=\"glink\">林家豪</span>"));
        assert!(h.contains("<span class=\"glink\">晶瀚半導體</span>"));
        assert!(!h.contains("[["));
        assert!(!h.contains("[companies"));
    }

    #[test]
    fn render_html_basic() {
        let md = "---\ntype: person\ntitle: '林家豪'\ntags: [people, contact]\n---\n\n# 林家豪\n\n蝕刻設備工程師。\n\n與 [[people/陳志遠|陳志遠]] 共同處理 E-07。\n";
        let html = render_html(md);
        assert!(html.contains("<h1>林家豪</h1>"));
        assert!(html.contains("蝕刻設備工程師"));
        assert!(html.contains("<span class=\"glink\">陳志遠</span>"));
        assert!(html.contains("class=\"badge\""));
    }
}
