//! 工廠核心（P1c 起居於 ocore）——拖放 → 轉換 → 立即寫入 → 預覽（可改可覆蓋）。
//!
//! `run_core` 一口氣轉換並寫入到 notes/<白名單目錄>/<slug>.md，回傳預覽。
//! people=CSV 純解析；companies/meeting=LLM 結構化；inbox=gbrain capture。
//! cfg 由呼叫端載入傳入；事件 emit 經 `Option<&AppState>`（殼層持有）。
//! `#[tauri::command]` 層與 factory_open_dir（open crate／VS Code）在桌面殼。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::agent_state::{AppState, EventKind, InboundEvent};
use crate::app_config::AppConfig;
use crate::converters::{csv_people, extract_companies, pdf_text, text_to_md};
use crate::gbrain_config;
use crate::i18n::{AppError, L10n};
use crate::proc::no_console;

/// 單檔轉換/寫入失敗的在地化訊息（code=factory.fileError，含 file+detail）。
fn file_err(file: impl ToString, detail: impl ToString) -> L10n {
    L10n::new("factory.fileError").p("file", file).p("detail", detail)
}

/// 要覆蓋寫入的單一頁(使用者編輯後用)。
#[derive(Debug, Clone, Deserialize)]
pub struct WritePage {
    pub slug: String,
    pub target_dir: String,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewPage {
    pub slug: String,
    pub target_dir: String,
    pub name: String,
    pub markdown: String,
}

/// 一個輸入檔的處理結果(檔案層級)。前端 >1 檔時顯示清單。
#[derive(Debug, Serialize)]
pub struct ProcessedFile {
    pub path: String,
    pub ok: bool,
    pub message: Option<L10n>,
    pub pages: Vec<PreviewPage>,
}

#[derive(Debug, Serialize)]
pub struct PreviewResult {
    pub factory: String,
    pub summary: L10n,
    pub sample: Vec<PreviewPage>,
    pub total: usize,
    /// 已立即寫入的檔案路徑。
    pub written: Vec<String>,
    pub errors: Vec<L10n>,
    /// 檔案層級結果(逐輸入檔)。前端 >1 檔時顯示清單;空 = 舊路徑(inbox)。
    #[serde(default)]
    pub files: Vec<ProcessedFile>,
}

#[derive(Debug, Serialize)]
pub struct WriteResult {
    pub written: Vec<String>,
    pub errors: Vec<L10n>,
    pub note: Option<L10n>,
}

#[derive(Debug, Serialize)]
pub struct AuthoredResult {
    pub slug: String,
    pub target_dir: String,
    pub path: String,
    pub used_fallback: bool,
    /// 實際寫入的內容(經 wikilink 補全)。
    pub enriched_markdown: String,
    /// LLM 抓到的人名+公司名數量。
    pub names_count: usize,
    /// 是否成功跑過 LLM 補全。
    pub enriched: bool,
}

fn read_text(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)?;
    let bytes = bytes
        .strip_prefix(b"\xef\xbb\xbf")
        .map(|b| b.to_vec())
        .unwrap_or(bytes); // 去 UTF-8 BOM
    Ok(String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

fn write_page(notes: &Path, target_dir: &str, slug: &str, markdown: &str) -> std::io::Result<PathBuf> {
    let dir = notes.join(target_dir);
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("{slug}.md"));
    std::fs::write(&file, markdown)?;
    Ok(file)
}

/// 從 markdown frontmatter 抽出 title(已產品化頁面的檔名來源)。
fn extract_title(markdown: &str) -> String {
    let (fm, _) = crate::converters::frontmatter::split(markdown);
    crate::converters::frontmatter::get(fm, "title").unwrap_or_default()
}

/// 工廠名 → 白名單目錄。
pub fn target_dir_of(factory: &str) -> Result<String, AppError> {
    match factory {
        "people" => Ok("people".into()),
        "companies" => Ok("companies".into()),
        "meeting" => Ok("meetings".into()),
        "inbox" => Ok("inbox".into()),
        "concepts" => Ok("concepts".into()),
        "projects" => Ok("projects".into()),
        other => Err(AppError::new("factory.unknown").p("factory", other)),
    }
}

/// 手寫編輯器存檔核心:首次(未命名)以 title 內容為檔名;之後覆蓋同檔。
/// 存檔前會先請 LLM 把文中人名/公司名補成 wikilink(best-effort)。
/// `state` 有提供且 `event_review_enabled` 時 emit FactoryWritten 事件（best-effort）。
pub async fn save_authored_core(
    cfg: &AppConfig,
    state: Option<&AppState>,
    factory: &str,
    markdown: &str,
    existing_slug: Option<&str>,
    target_repo: Option<&str>,
) -> Result<AuthoredResult, AppError> {
    let notes = PathBuf::from(target_repo.unwrap_or(&cfg.notes_repo_path));
    let target_dir = target_dir_of(factory)?;

    let title = extract_title(markdown);
    let own_dir = match factory {
        "people" => "people",
        "companies" => "companies",
        "meeting" => "meetings",
        "concepts" => "concepts",
        "projects" => "projects",
        _ => "",
    };
    let own_slug = crate::converters::slug::slugify(&title, "");

    // LLM 補全 wikilink(best-effort:失敗就寫原文) — 讀「作用中腦」的 config
    let (to_write, names_count, enriched) =
        match gbrain_config::load_for(cfg.active_env_home()).ok().and_then(|l| {
            gbrain_config::resolve_endpoint(&l.config).ok()
        }) {
            Some(endpoint) => {
                match text_to_md::enrich_wikilinks(markdown, own_dir, &own_slug, cfg, &endpoint)
                    .await
                {
                    Ok((m, c)) => (m, c, true),
                    Err(_) => (markdown.to_string(), 0, false),
                }
            }
            None => (markdown.to_string(), 0, false),
        };

    // 已命名 → 沿用;否則用 title 內容 slugify 作檔名
    let (slug, used_fallback) = match existing_slug.filter(|s| !s.is_empty()) {
        Some(s) => (s.to_string(), false),
        None => {
            let fallback = format!("untitled-{}", target_dir.trim_end_matches('/'));
            let s = crate::converters::slug::slugify(&title, &fallback);
            (s, title.trim().is_empty())
        }
    };

    let file = write_page(&notes, &target_dir, &slug, &to_write).map_err(|e| e.to_string())?;
    // 事件匯流排（Phase 7c）：手寫存檔後 emit 給腦匹配的員工 review（best-effort）。
    if cfg.event_review_enabled {
        if let Some(state) = state {
            state.emit(InboundEvent {
                kind: EventKind::FactoryWritten,
                source: "factory".into(),
                brain_id: cfg.active_brain_id.clone(),
                employee_id: None,
                title: slug.clone(),
                content: to_write.chars().take(800).collect(),
                external_ref: None,
                occurred_at: None,
                reply_to: None,
                category: Some(target_dir.clone()),
            });
        }
    }
    Ok(AuthoredResult {
        slug,
        target_dir,
        path: file.to_string_lossy().into_owned(),
        used_fallback,
        enriched_markdown: to_write,
        names_count,
        enriched,
    })
}

/// 主流程核心:轉換 + 立即寫入 + 回傳預覽。
pub async fn run_core(
    cfg: &AppConfig,
    factory: &str,
    paths: &[String],
    target_repo: Option<&str>,
) -> Result<PreviewResult, AppError> {
    let notes = PathBuf::from(target_repo.unwrap_or(&cfg.notes_repo_path));

    match factory {
        "people" => run_people(cfg, &notes, paths).await,
        "companies" | "meeting" | "concepts" | "projects" => {
            run_textual(factory, cfg, &notes, paths).await
        }
        "inbox" => run_inbox(cfg, &notes, paths),
        other => Err(AppError::new("factory.unknown").p("factory", other)),
    }
}

async fn run_people(
    cfg: &AppConfig,
    notes: &Path,
    paths: &[String],
) -> Result<PreviewResult, AppError> {
    // 僅當含 txt/md 才載 LLM endpoint(純 CSV 批次不要求 API key)。
    let has_text = paths.iter().any(|p| {
        Path::new(p)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("txt") || e.eq_ignore_ascii_case("md"))
            .unwrap_or(false)
    });
    let endpoint = if has_text {
        let loaded = gbrain_config::load_for(cfg.active_env_home())?;
        let ep = gbrain_config::resolve_endpoint(&loaded.config)?;
        if !ep.has_api_key && ep.provider != "ollama" {
            return Err(AppError::new("llm.noApiKey")
                .p("provider", &ep.provider)
                .p("envKey", gbrain_config::env_key(&ep.provider).unwrap_or("?")));
        }
        Some(ep)
    } else {
        None
    };

    let mut files: Vec<ProcessedFile> = Vec::new();
    let mut all_pages: Vec<PreviewPage> = Vec::new();
    let mut written: Vec<String> = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    let mut rows = 0usize;
    let mut merged = 0usize;

    for p in paths {
        let path = Path::new(p);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mut pf = ProcessedFile {
            path: p.clone(),
            ok: true,
            message: None,
            pages: vec![],
        };

        // 逐檔依副檔名分流:csv→結構化解析(一檔多人);txt/md→LLM 結構化(一檔一人)。
        let parsed: Result<Vec<PreviewPage>, L10n> = if ext.eq_ignore_ascii_case("csv") {
            match read_text(path) {
                Ok(text) => match csv_people::parse(&text, true) {
                    Ok(imp) => {
                        rows += imp.rows_read;
                        merged += imp.groups_merged;
                        Ok(imp
                            .pages
                            .iter()
                            .map(|pg| PreviewPage {
                                slug: pg.slug.clone(),
                                target_dir: "people".into(),
                                name: pg.name.clone(),
                                markdown: pg.markdown.clone(),
                            })
                            .collect())
                    }
                    Err(e) => Err(file_err(p, e)),
                },
                Err(e) => Err(file_err(p, e)),
            }
        } else if ext.eq_ignore_ascii_case("txt") || ext.eq_ignore_ascii_case("md") {
            let ep = endpoint.as_ref().expect("has_text ⇒ endpoint loaded");
            match read_text(path) {
                Ok(raw) => match text_to_md::text_to_page("people", &raw, cfg, ep).await {
                    Ok(sp) => {
                        let (slug, markdown) = text_to_md::render("people", &sp);
                        Ok(vec![PreviewPage {
                            slug,
                            target_dir: "people".into(),
                            name: sp.title,
                            markdown,
                        }])
                    }
                    Err(e) => Err(file_err(p, e)),
                },
                Err(e) => Err(file_err(p, e)),
            }
        } else {
            Err(L10n::new("factory.csvOnly").p("file", p))
        };

        match parsed {
            Ok(pages) => {
                for page in &pages {
                    match write_page(notes, &page.target_dir, &page.slug, &page.markdown) {
                        Ok(f) => written.push(f.to_string_lossy().into_owned()),
                        Err(e) => errors.push(file_err(format!("{}/{}", page.target_dir, page.slug), e)),
                    }
                }
                all_pages.extend(pages.iter().cloned());
                pf.pages = pages;
            }
            Err(m) => {
                pf.ok = false;
                pf.message = Some(m.clone());
                errors.push(m);
            }
        }
        files.push(pf);
    }

    let total = all_pages.len();
    let sample: Vec<PreviewPage> = all_pages.iter().take(10).cloned().collect();
    let summary = if rows > 0 {
        L10n::new("factory.peopleSummary")
            .p("rows", rows)
            .p("merged", merged)
            .p("written", written.len())
    } else {
        L10n::new("factory.writtenN").p("factory", "people").p("n", written.len())
    };

    Ok(PreviewResult {
        factory: "people".into(),
        summary,
        sample,
        total,
        written,
        errors,
        files,
    })
}

async fn run_textual(
    factory: &str,
    cfg: &AppConfig,
    notes: &Path,
    paths: &[String],
) -> Result<PreviewResult, AppError> {
    let loaded = gbrain_config::load_for(cfg.active_env_home())?;
    let endpoint = gbrain_config::resolve_endpoint(&loaded.config)?;
    if !endpoint.has_api_key && endpoint.provider != "ollama" {
        return Err(AppError::new("llm.noApiKey")
            .p("provider", &endpoint.provider)
            .p("envKey", gbrain_config::env_key(&endpoint.provider).unwrap_or("?")));
    }
    let target_dir = match factory {
        "companies" => "companies".to_string(),
        "meeting" => "meetings".to_string(),
        "concepts" => "concepts".to_string(),
        "projects" => "projects".to_string(),
        _ => "concepts".to_string(),
    };

    let mut files: Vec<ProcessedFile> = Vec::new();
    let mut written = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    for p in paths {
        let path = Path::new(p);
        let mut pf = ProcessedFile {
            path: p.clone(),
            ok: true,
            message: None,
            pages: vec![],
        };
        let raw = match extract_raw(path) {
            Ok(t) => t,
            Err(e) => {
                let m = file_err(p, e);
                pf.ok = false;
                pf.message = Some(m.clone());
                errors.push(m);
                files.push(pf);
                continue;
            }
        };
        match text_to_md::text_to_page(factory, &raw, cfg, &endpoint).await {
            Ok(sp) => {
                let (slug, markdown) = text_to_md::render(factory, &sp);
                match write_page(notes, &target_dir, &slug, &markdown) {
                    Ok(f) => written.push(f.to_string_lossy().into_owned()),
                    Err(e) => {
                        let m = file_err(format!("{target_dir}/{slug}"), e);
                        pf.ok = false;
                        pf.message = Some(m.clone());
                        errors.push(m);
                    }
                }
                pf.pages.push(PreviewPage {
                    slug,
                    target_dir: target_dir.clone(),
                    name: sp.title,
                    markdown,
                });
            }
            Err(e) => {
                let m = file_err(p, e);
                pf.ok = false;
                pf.message = Some(m.clone());
                errors.push(m);
            }
        }
        files.push(pf);
    }
    let sample: Vec<PreviewPage> = files.iter().flat_map(|f| f.pages.iter().cloned()).take(10).collect();
    let total: usize = files.iter().map(|f| f.pages.len()).sum();
    let summary = L10n::new("factory.writtenN").p("factory", factory).p("n", written.len());
    Ok(PreviewResult {
        factory: factory.into(),
        summary,
        sample,
        total,
        written,
        errors,
        files,
    })
}

fn run_inbox(
    cfg: &AppConfig,
    _notes: &Path,
    paths: &[String],
) -> Result<PreviewResult, AppError> {
    // inbox 直接走 gbrain capture(寫 inbox/),不走 notes repo。
    let mut sample = Vec::new();
    let mut written = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    for p in paths {
        let path = Path::new(p);
        let mut cmd = std::process::Command::new(&cfg.gbrain_exe_path);
        cmd.args(["capture", "--file", p, "--type", "note", "--quiet"])
            .env("PYTHONUTF8", "1");
        no_console(&mut cmd);
        if let Some(h) = cfg.active_env_home() {
            cmd.env("GBRAIN_HOME", h);
        }
        let out = cmd.output();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("note").to_string();
        match out {
            Ok(o) if o.status.success() => {
                let slug = String::from_utf8_lossy(&o.stdout).trim().to_string();
                written.push(if slug.is_empty() { p.clone() } else { slug.clone() });
                sample.push(PreviewPage {
                    slug,
                    target_dir: "inbox/".into(),
                    name,
                    markdown: String::new(),
                });
            }
            Ok(o) => errors.push(file_err(p, String::from_utf8_lossy(&o.stderr).trim())),
            Err(e) => errors.push(file_err(p, e)),
        }
    }
    let total = written.len();
    Ok(PreviewResult {
        factory: "inbox".into(),
        summary: L10n::new("factory.inboxCaptured").p("n", total),
        sample,
        total,
        written,
        errors,
        files: vec![],
    })
}

/// 依副檔名抽出純文字。
fn extract_raw(path: &Path) -> anyhow::Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "txt" | "md" | "markdown" => read_text(path),
        "pdf" => pdf_text::extract(path),
        other => Err(anyhow::anyhow!("不支援的副檔名：{other}（people=csv；companies/meeting/projects/concepts=txt,md,pdf）")),
    }
}

/// 覆蓋寫入核心(使用者預覽後編輯過的頁面)。
pub fn write_pages_core(notes: &Path, pages: &[WritePage]) -> WriteResult {
    let mut written = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    for pg in pages {
        match write_page(notes, &pg.target_dir, &pg.slug, &pg.markdown) {
            Ok(f) => written.push(f.to_string_lossy().into_owned()),
            Err(e) => errors.push(file_err(format!("{}/{}", pg.target_dir, pg.slug), e)),
        }
    }
    WriteResult { written, errors, note: None }
}

/// 寫入完成後 emit FactoryWritten 事件（Phase 7c；best-effort；同步 try_send）。
/// active_brain_id 為路由錨點——喚醒共用此腦的全部員工。content 帶 800 字預覽，
/// reasoner 不靠圖譜同步也能審閱（gbrain 索引的 race 由預覽雙保險兜底；E8 的
/// fire-and-forget sync 為後續）。
pub fn emit_factory_events(state: &AppState, cfg: &AppConfig, pages: &[WritePage]) {
    if !cfg.event_review_enabled {
        return;
    }
    for pg in pages {
        state.emit(InboundEvent {
            kind: EventKind::FactoryWritten,
            source: "factory".into(),
            brain_id: cfg.active_brain_id.clone(),
            employee_id: None,
            title: pg.slug.clone(),
            content: pg.markdown.chars().take(800).collect(),
            external_ref: None,
            occurred_at: None,
            reply_to: None,
            category: Some(pg.target_dir.clone()),
        });
    }
}

/// 重建 companies 核心:掃描 people/ 的 `公司/組織:` bullet → companies/*.md。
/// enriched 頁(`enriched: true` 或 `<!-- enriched -->`)凍結不覆蓋。
pub fn extract_companies_core(
    cfg: &AppConfig,
    clean: bool,
    target_repo: Option<&str>,
) -> Result<WriteResult, AppError> {
    let notes = PathBuf::from(target_repo.unwrap_or(&cfg.notes_repo_path));
    let people_dir = notes.join("people");
    let companies_dir = notes.join("companies");
    std::fs::create_dir_all(&companies_dir).map_err(|e| e.to_string())?;

    let aliases =
        extract_companies::load_aliases(&notes.join("company_aliases.json")).map_err(|e| e.to_string())?;
    let imp = extract_companies::build(&people_dir, &aliases).map_err(|e| e.to_string())?;

    let mut written = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    let mut frozen = 0usize;
    let mut generated_slugs: std::collections::HashSet<String> = std::collections::HashSet::new();
    for p in &imp.pages {
        generated_slugs.insert(p.slug.clone());
        let file = companies_dir.join(format!("{}.md", p.slug));
        if file.exists() {
            if let Ok(text) = std::fs::read_to_string(&file) {
                if extract_companies::is_enriched(&text) {
                    frozen += 1;
                    continue;
                }
            }
        }
        match std::fs::write(&file, &p.markdown) {
            Ok(_) => written.push(file.to_string_lossy().into_owned()),
            Err(e) => errors.push(file_err(file.display(), e)),
        }
    }

    let mut removed = 0usize;
    if clean {
        if let Ok(rd) = std::fs::read_dir(&companies_dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.extension().and_then(|x| x.to_str()) != Some("md") {
                    continue;
                }
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                if generated_slugs.contains(&stem) {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if extract_companies::is_enriched(&text) {
                        continue;
                    }
                }
                if std::fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }
    }

    let note = if clean {
        L10n::new("factory.companiesRebuiltClean")
            .p("people", imp.people_read)
            .p("distinct", imp.distinct)
            .p("links", imp.total_links)
            .p("frozen", frozen)
            .p("removed", removed)
    } else {
        L10n::new("factory.companiesRebuilt")
            .p("people", imp.people_read)
            .p("distinct", imp.distinct)
            .p("links", imp.total_links)
            .p("frozen", frozen)
    };

    Ok(WriteResult {
        written,
        errors,
        note: Some(note),
    })
}
