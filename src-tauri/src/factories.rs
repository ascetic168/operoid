//! 工廠指令 — 拖放 → 轉換 → **立即寫入** → 預覽(可改可覆蓋)。
//!
//! 流程:`factory_run` 一口氣轉換並寫入到 notes/<白名單目錄>/<slug>.md,回傳預覽。
//! 使用者預覽時若要修改,直接編輯後用 `factory_write_pages` 覆蓋同一檔。
//! people=CSV 純解析;companies/meeting=LLM 結構化;inbox=gbrain capture。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::config;
use crate::converters::{csv_people, extract_companies, pdf_text, text_to_md};
use crate::gbrain_cli::no_console;
use crate::i18n::{AppError, L10n};

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

fn read_text(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)?;
    let bytes = bytes
        .strip_prefix(b"\xef\xbb\xbf")
        .map(|b| b.to_vec())
        .unwrap_or(bytes); // 去 UTF-8 BOM
    Ok(String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

fn notes_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let cfg = config::app_config::load(app).map_err(|e| e.to_string())?;
    Ok(PathBuf::from(&cfg.notes_repo_path))
}

fn app_cfg<R: Runtime>(app: &AppHandle<R>) -> Result<config::AppConfig, String> {
    config::app_config::load(app).map_err(|e| e.to_string())
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

/// 手寫編輯器存檔:首次(未命名)以 title 內容為檔名;之後覆蓋同檔。
/// 存檔前會先請 LLM 把文中人名/公司名補成 wikilink(best-effort)。
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

#[tauri::command]
pub async fn factory_save_authored<R: Runtime>(
    app: AppHandle<R>,
    factory: String,
    markdown: String,
    existing_slug: Option<String>,
    target_repo: Option<String>,
) -> Result<AuthoredResult, AppError> {
    let cfg = app_cfg(&app)?;
    let notes = PathBuf::from(target_repo.unwrap_or_else(|| cfg.notes_repo_path.clone()));
    let target_dir = match factory.as_str() {
        "people" => cfg.factory_targets.people.clone(),
        "companies" => cfg.factory_targets.companies.clone(),
        "meeting" => cfg.factory_targets.meetings.clone(),
        "inbox" => "inbox".to_string(),
        "concepts" => "concepts".to_string(),
        "projects" => "projects".to_string(),
        other => return Err(AppError::new("factory.unknown").p("factory", other)),
    };

    let title = extract_title(&markdown);
    let own_dir = match factory.as_str() {
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
        match config::gbrain_config::load_for(cfg.active_env_home()).ok().and_then(|l| {
            config::gbrain_config::resolve_endpoint(&l.config).ok()
        }) {
            Some(endpoint) => {
                match text_to_md::enrich_wikilinks(&markdown, own_dir, &own_slug, &cfg, &endpoint)
                    .await
                {
                    Ok((m, c)) => (m, c, true),
                    Err(_) => (markdown.clone(), 0, false),
                }
            }
            None => (markdown.clone(), 0, false),
        };

    // 已命名 → 沿用;否則用 title 內容 slugify 作檔名
    let (slug, used_fallback) = match existing_slug.filter(|s| !s.is_empty()) {
        Some(s) => (s, false),
        None => {
            let fallback = format!("untitled-{}", target_dir.trim_end_matches('/'));
            let s = crate::converters::slug::slugify(&title, &fallback);
            (s, title.trim().is_empty())
        }
    };

    let file = write_page(&notes, &target_dir, &slug, &to_write).map_err(|e| e.to_string())?;
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

/// 主指令:轉換 + 立即寫入 + 回傳預覽。
#[tauri::command]
pub async fn factory_run<R: Runtime>(
    app: AppHandle<R>,
    factory: String,
    paths: Vec<String>,
    target_repo: Option<String>,
) -> Result<PreviewResult, AppError> {
    let cfg = app_cfg(&app)?;
    let notes = PathBuf::from(target_repo.unwrap_or_else(|| cfg.notes_repo_path.clone()));

    match factory.as_str() {
        "people" => run_people(&cfg, &notes, &paths).await,
        "companies" | "meeting" | "concepts" | "projects" => {
            run_textual(&factory, &cfg, &notes, &paths).await
        }
        "inbox" => run_inbox(&cfg, &notes, &paths),
        other => Err(AppError::new("factory.unknown").p("factory", other)),
    }
}

async fn run_people(
    cfg: &config::AppConfig,
    notes: &Path,
    paths: &[String],
) -> Result<PreviewResult, AppError> {
    let target = cfg.factory_targets.people.clone();
    let targets = cfg.factory_targets.clone();

    // 僅當含 txt/md 才載 LLM endpoint(純 CSV 批次不要求 API key)。
    let has_text = paths.iter().any(|p| {
        Path::new(p)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("txt") || e.eq_ignore_ascii_case("md"))
            .unwrap_or(false)
    });
    let endpoint = if has_text {
        let loaded = config::gbrain_config::load_for(cfg.active_env_home())?;
        let ep = config::gbrain_config::resolve_endpoint(&loaded.config)?;
        if !ep.has_api_key && ep.provider != "ollama" {
            return Err(AppError::new("llm.noApiKey")
                .p("provider", &ep.provider)
                .p("envKey", config::gbrain_config::env_key(&ep.provider).unwrap_or("?")));
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
                                target_dir: target.clone(),
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
                        let (slug, markdown) = text_to_md::render("people", &sp, &targets);
                        Ok(vec![PreviewPage {
                            slug,
                            target_dir: target.clone(),
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
    cfg: &config::AppConfig,
    notes: &Path,
    paths: &[String],
) -> Result<PreviewResult, AppError> {
    let loaded = config::gbrain_config::load_for(cfg.active_env_home())?;
    let endpoint = config::gbrain_config::resolve_endpoint(&loaded.config)?;
    if !endpoint.has_api_key && endpoint.provider != "ollama" {
        return Err(AppError::new("llm.noApiKey")
            .p("provider", &endpoint.provider)
            .p("envKey", config::gbrain_config::env_key(&endpoint.provider).unwrap_or("?")));
    }
    let targets = cfg.factory_targets.clone();
    let target_dir = match factory {
        "companies" => targets.companies.clone(),
        "meeting" => targets.meetings.clone(),
        "concepts" => "concepts".into(),
        "projects" => "projects".into(),
        _ => "concepts".into(),
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
                let (slug, markdown) = text_to_md::render(factory, &sp, &targets);
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
    cfg: &config::AppConfig,
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
        other => Err(anyhow::anyhow!("不支援的副檔名：{other}（people=csv；companies=txt,pdf；meeting=txt,md,pdf）")),
    }
}

/// 覆蓋寫入(使用者預覽後編輯過的頁面)。
#[tauri::command]
pub fn factory_write_pages<R: Runtime>(
    app: AppHandle<R>,
    pages: Vec<WritePage>,
    target_repo: Option<String>,
) -> Result<WriteResult, AppError> {
    let notes = match target_repo {
        Some(t) => PathBuf::from(t),
        None => notes_dir(&app)?,
    };
    let mut written = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    for pg in pages {
        match write_page(&notes, &pg.target_dir, &pg.slug, &pg.markdown) {
            Ok(f) => written.push(f.to_string_lossy().into_owned()),
            Err(e) => errors.push(file_err(format!("{}/{}", pg.target_dir, pg.slug), e)),
        }
    }
    Ok(WriteResult { written, errors, note: None })
}

/// 重建 companies:掃描 people/ 的 `公司/組織:` bullet → companies/*.md。
/// enriched 頁(`enriched: true` 或 `<!-- enriched -->`)凍結不覆蓋。
#[tauri::command]
pub fn extract_companies_run<R: Runtime>(
    app: AppHandle<R>,
    clean: bool,
    target_repo: Option<String>,
) -> Result<WriteResult, AppError> {
    let cfg = app_cfg(&app)?;
    let notes = PathBuf::from(target_repo.unwrap_or_else(|| cfg.notes_repo_path.clone()));
    let people_dir = notes.join(&cfg.factory_targets.people);
    let companies_dir = notes.join(&cfg.factory_targets.companies);
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
