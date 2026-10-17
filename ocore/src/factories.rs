//! 工廠核心（P1c 起居於 ocore）——拖放 → 轉換 → 立即寫入 → 預覽（可改可覆蓋）。
//!
//! `run_core` 一口氣轉換並寫入到 notes/<白名單目錄>/<slug>.md，回傳預覽。
//! people=CSV 純解析；companies/meeting=LLM 結構化；inbox=gbrain capture。
//! cfg 由呼叫端載入傳入；事件 emit 經 `Option<&AppState>`（殼層持有）。
//! `#[tauri::command]` 層與 factory_open_dir（open crate／VS Code）在桌面殼。

use std::future::Future;
use std::path::{Path, PathBuf};

use chrono::Local;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::agent_state::{AppState, EventKind, InboundEvent};
use crate::app_config::AppConfig;
use crate::converters::{csv_people, extract_companies, pdf_text, text_to_md};
use crate::factory_sched::FactoryGate;
use crate::factory_types::{self, FactoryTypeSpec, Pack, Pipeline};
use crate::gbrain_config;
use crate::i18n::{AppError, L10n};

/// 企業版公平調度閘門的引用形：`Some((閘門, 使用者 id))`＝逐檔過閘（oserver——
/// 全域 ≤ `factory_concurrency` 名額、飽和時每位遠端使用者 1 線程）；
/// `None`＝本地並發（桌面／個人版——單一批次內 `factory_concurrency` 檔並發）。
pub type Gate<'a> = Option<(&'a FactoryGate, &'a str)>;

/// 逐檔並發上限（防呆：0 視為 1）。
fn file_concurrency(cfg: &AppConfig) -> usize {
    cfg.factory_concurrency.max(1)
}

/// 逐檔驅動器（多工核心）：把 `paths` 逐檔丟給 `f` 並發執行、按輸入序收集結果。
/// - `gate=None`：本地 `buffered(n)`——批次內 n 檔並發（個人版形）。
/// - `gate=Some`：每檔包進 `FactoryGate::run`——並發由閘門全域裁定（企業版形，
///   跨請求公平調度）；本地不再限流。
async fn run_all<T, FU, F>(paths: Vec<String>, gate: Gate<'_>, local_max: usize, f: F) -> Vec<T>
where
    F: Fn(String) -> FU,
    FU: Future<Output = T> + Send,
    T: Send,
{
    match gate {
        Some((g, user)) => {
            futures::stream::iter(paths.into_iter().map(|p| g.run(user, local_max, f(p))))
                .buffered(usize::MAX)
                .collect::<Vec<T>>()
                .await
        }
        None => futures::stream::iter(paths.into_iter().map(f))
            .buffered(local_max)
            .collect::<Vec<T>>()
            .await,
    }
}

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

/// 批次工廠寫入（凍結保護）：目標已存在且 (a) 帶 enriched 凍結標記（frontmatter
/// `enriched: true` 或 `<!-- enriched -->`，同 companies 工廠），或 (b) frontmatter
/// `origin` ≠ `operoid-factory`（人手或其他工具建立的頁面）→ 跳過不覆寫，回
/// `Ok(None)`。避免批次重跑洗掉頁面上的非工廠內容（人工編輯、gbrain 寫回的
/// timeline 列）；工廠自產且未凍結的頁面照常覆寫（重跑＝以來源最新內容更新）。
/// 手寫編輯器存檔（`authored`）與預覽後確認寫回（`write_pages_core`）屬人類明確
/// 指令，不走此保護。
fn write_page_guarded(
    notes: &Path,
    target_dir: &str,
    slug: &str,
    markdown: &str,
) -> std::io::Result<Option<PathBuf>> {
    let file = notes.join(target_dir).join(format!("{slug}.md"));
    if file.exists() {
        if let Ok(existing) = std::fs::read_to_string(&file) {
            let factory_owned = crate::converters::frontmatter::split(&existing)
                .0
                .and_then(|fm| crate::converters::frontmatter::get(Some(fm), "origin"))
                .map(|o| o == "operoid-factory")
                .unwrap_or(false);
            if !factory_owned || extract_companies::is_enriched(&existing) {
                return Ok(None);
            }
        }
    }
    let dir = notes.join(target_dir);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&file, markdown)?;
    Ok(Some(file))
}

/// 從 markdown frontmatter 抽出 title(已產品化頁面的檔名來源)。
fn extract_title(markdown: &str) -> String {
    let (fm, _) = crate::converters::frontmatter::split(markdown);
    crate::converters::frontmatter::get(fm, "title").unwrap_or_default()
}

/// 作用中腦的 schema pack + 工廠 id → 規格（唯一查表入口；未知 id 報錯）。
pub fn spec_for(cfg: &AppConfig, factory: &str) -> Result<(&'static Pack, &'static FactoryTypeSpec), AppError> {
    let (pack, _) = factory_types::active_pack(cfg);
    Ok((pack, pack.spec(factory)?))
}

/// 舊介面相容：工廠名 → 白名單目錄（以作用中 pack 查表）。
pub fn target_dir_of(cfg: &AppConfig, factory: &str) -> Result<String, AppError> {
    Ok(spec_for(cfg, factory)?.1.dir.to_string())
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
    let (pack, spec) = spec_for(cfg, factory)?;
    let target_dir = spec.dir.to_string();

    let title = extract_title(markdown);
    // capture 型無自有目錄（筆記由 gbrain capture 寫入），wikilink 補全範圍為空。
    let own_dir = if spec.is_capture() { "" } else { spec.dir };
    let person_dir = pack.id_of_kind("people").map(|id| pack.spec(id).unwrap().dir).unwrap_or("person");
    let company_dir = pack.id_of_kind("company").map(|id| pack.spec(id).unwrap().dir).unwrap_or("company");
    let own_slug = crate::converters::slug::slugify(&title, "");

    // LLM 補全 wikilink(best-effort:失敗就寫原文) — 讀「作用中腦」的 config
    let (to_write, names_count, enriched) =
        match gbrain_config::load_for(cfg.active_env_home()).ok().and_then(|l| {
            gbrain_config::resolve_endpoint(&l.config).ok()
        }) {
            Some(endpoint) => {
                match text_to_md::enrich_wikilinks(
                    markdown, person_dir, company_dir, own_dir, &own_slug, cfg, &endpoint,
                )
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
    annotations: Option<&[String]>,
    import_annotation: Option<&str>,
    target_repo: Option<&str>,
) -> Result<PreviewResult, AppError> {
    run_core_sched(cfg, factory, paths, target_repo, None, annotations, import_annotation).await
}

/// [`run_core`] 的多工版：`gate=Some` 時逐檔經企業公平閘門（oserver 接線）；
/// `None` 與 `run_core` 完全等價（本地 `factory_concurrency` 檔並發）。
/// `annotations`：image 工廠的逐檔註解（與 `paths` 平行；強制非空），
/// 其餘工廠忽略。
pub async fn run_core_sched(
    cfg: &AppConfig,
    factory: &str,
    paths: &[String],
    target_repo: Option<&str>,
    gate: Gate<'_>,
    annotations: Option<&[String]>,
    import_annotation: Option<&str>,
) -> Result<PreviewResult, AppError> {
    let notes = PathBuf::from(target_repo.unwrap_or(&cfg.notes_repo_path));
    let max = file_concurrency(cfg);

    // 多模態檢索升級（K1～K5）：PDF 逐檔分流——PDF 進知識管線（複雜度分流 →
    // MinerU/快速路徑 → 章節切塊筆記＋圖片筆記 → sidecar → 向量回填），其餘檔案
    // 走原工廠分流。拖 PDF 進工廠即自動完成，混合批次也逐檔正確。
    let (pdfs, others): (Vec<String>, Vec<String>) = paths
        .iter()
        .cloned()
        .partition(|p| {
            Path::new(p)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
        });

    let mut results: Vec<PreviewResult> = Vec::new();
    if !pdfs.is_empty() {
        results.push(run_pdf_knowledge(cfg, &notes, &pdfs, gate, max).await?);
    }
    if !others.is_empty() {
        let (pack, spec) = spec_for(cfg, factory)?;
        results.push(match spec.pipeline {
            Pipeline::People => {
                run_people(pack, spec, cfg, &notes, &others, gate, max, import_annotation).await?
            }
            Pipeline::Textual => {
                run_textual(pack, spec, cfg, &notes, &others, gate, max, import_annotation).await?
            }
            Pipeline::Image => run_images(spec, &notes, &others, annotations)?,
            Pipeline::Capture => run_inbox(spec, cfg, &others, gate, max).await?,
        });
    }
    Ok(merge_previews(results))
}

/// 合併多個分流結果（逐檔分流的總合）：清單延伸、計數累加；單一結果原樣返回。
fn merge_previews(mut results: Vec<PreviewResult>) -> PreviewResult {
    if results.len() == 1 {
        return results.pop().expect("len checked");
    }
    let mut it = results.into_iter();
    let mut first = it.next().expect("len checked");
    for r in it {
        first.total += r.total;
        first.written.extend(r.written);
        first.errors.extend(r.errors);
        first.files.extend(r.files);
        first.sample.extend(r.sample);
    }
    first
}

/// PDF 知識管線：每個 PDF 經複雜度分流轉換為知識筆記（章節切塊＋圖片筆記），
/// 寫入 notes repo（同 textual 語意）。圖片走 **gbrain 原生 image import 軌道**
/// （2026-10-17 試點裁決，TODO 項目 3）：每圖發布同名圖檔＋補強註解頁至 doc
/// 目錄（`figN-pP.jpg`／`figN-pP.md`，同目錄同名觸發 sync 的 `image_of` 自動
/// 連結與多模態向量），不再寫 figures sidecar——舊庫由查詢端 K4 融合續讀
/// （`figures_db_path` 接線不變），退役遷移見 `knowledge::figures_migrate`。
///
/// 多工切分：**轉換**（MinerU/快速路徑子行程——重活）逐檔並發（本地上限或公平
/// 閘門）；**圖檔發布／notes 複製**共享覆寫語意，由轉換完成後在父端按輸入序
/// 循序做（便宜步驟，序列化換正確性）。
async fn run_pdf_knowledge(
    cfg: &AppConfig,
    notes: &Path,
    paths: &[String],
    gate: Gate<'_>,
    max: usize,
) -> Result<PreviewResult, AppError> {
    let conv = cfg.convert_config();
    let conv = &conv; // 閉包以借用共享（async move 不得搬走——Fn 多次呼叫）

    // ── 階段一：逐檔並發轉換（重活；暫存目錄保留給階段二合併用）──
    let converted: Vec<(String, PathBuf, Result<crate::converters::mineru::ConversionOutcome, L10n>)> =
        run_all(paths.to_vec(), gate, max, |p| async move {
            let work = std::env::temp_dir().join(format!(
                "pdf-factory-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                PDF_WORK_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let _ = std::fs::create_dir_all(&work);
            let outcome = match crate::converters::mineru::convert(Path::new(&p), &work, &conv).await {
                Ok(o) => Ok(o),
                Err(e) => Err(L10n::new("factory.fileError").p("file", p.clone()).p("detail", e.to_string())),
            };
            (p, work, outcome)
        })
        .await;

    // ── 階段二：共享寫入（循序；與多工前逐檔行為一致）──
    let mut pages: Vec<PreviewPage> = Vec::new();
    let mut written: Vec<String> = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    let mut files: Vec<ProcessedFile> = Vec::new();
    let mut total_notes = 0usize;

    for (p, work, outcome) in converted {
        let mut pf = ProcessedFile {
            path: p.clone(),
            ok: true,
            message: None,
            pages: vec![],
        };
        let outcome = match outcome {
            Ok(o) => o,
            Err(m) => {
                pf.ok = false;
                pf.message = Some(m.clone());
                errors.push(m);
                files.push(pf);
                std::fs::remove_dir_all(&work).ok();
                continue;
            }
        };
        let doc_id = outcome.report.doc_id.clone();

        // 筆記寫入 notes repo（doc 子目錄；冪等覆寫）。
        let src_notes = work.join("notes").join(&doc_id);
        let dst_notes = notes.join(&doc_id);
        let mut doc_pages: Vec<PreviewPage> = Vec::new();
        if src_notes.exists() {
            let _ = std::fs::create_dir_all(&dst_notes);
            for entry in std::fs::read_dir(&src_notes).into_iter().flatten().flatten() {
                let from = entry.path();
                if from.is_file() {
                    let name = entry.file_name();
                    match std::fs::copy(&from, dst_notes.join(&name)) {
                        Ok(_) => {
                            total_notes += 1;
                            written.push(dst_notes.join(&name).to_string_lossy().into_owned());
                        }
                        Err(e) => {
                            let m = L10n::new("factory.fileError")
                                .p("file", name.to_string_lossy().into_owned())
                                .p("detail", e.to_string());
                            errors.push(m);
                        }
                    }
                }
            }
            // 圖檔發布（gbrain 原生 image import）：註解頁補強＋同名圖檔——
            // 先 .md 後 .jpg（image_of 順序鐵律）。
            let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
            let (_enriched, imgs, pub_errors) =
                publish_pdf_figures(&dst_notes, &outcome.figure_rows, &p, &today);
            written.extend(
                imgs.iter()
                    .map(|ip| ip.to_string_lossy().into_owned()),
            );
            total_notes += imgs.len();
            errors.extend(pub_errors.into_iter().map(|e| {
                L10n::new("factory.fileError").p("file", e.file).p("detail", e.detail)
            }));
        }
        // 預覽頁：每篇筆記一頁（doc 子目錄；前端可編輯後覆寫）。
        for entry in std::fs::read_dir(&dst_notes).into_iter().flatten().flatten() {
            let p2 = entry.path();
            if let Some(stem) = p2.file_stem().and_then(|s| s.to_str()) {
                if let Ok(md) = std::fs::read_to_string(&p2) {
                    doc_pages.push(PreviewPage {
                        slug: stem.into(),
                        target_dir: format!("{doc_id}/"),
                        name: stem.into(),
                        markdown: md,
                    });
                }
            }
        }
        pages.extend(doc_pages.iter().cloned());
        pf.pages = doc_pages;
        files.push(pf);
        std::fs::remove_dir_all(&work).ok();
    }

    let total = total_notes;
    let sample: Vec<PreviewPage> = pages.iter().take(6).cloned().collect();
    Ok(PreviewResult {
        factory: "pdf-knowledge".into(),
        summary: L10n::new("factory.pdfDone").p("n", total),
        sample,
        total,
        written,
        errors,
        files,
    })
}

/// 從頁面 markdown 抽出「匯入」錨點整塊（`### date — 匯入：…` 起，到下一個
/// `###`/`##` 標題或 EOF）。供重匯時保留舊錨點（TODO 項目 2 去重策略）。
fn extract_import_anchors(md: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut in_anchor = false;
    for line in md.lines() {
        let is_heading = line.starts_with("###") || line.starts_with("## ");
        if in_anchor && is_heading {
            let joined = cur.join("
").trim_end().to_string();
            if !joined.is_empty() {
                out.push(joined);
            }
            cur.clear();
            in_anchor = false;
        }
        if !in_anchor && line.starts_with("### ") && line.contains("— 匯入：") {
            in_anchor = true;
            cur.clear();
        }
        if in_anchor {
            cur.push(line.to_string());
        }
    }
    if in_anchor {
        let joined = cur.join("
").trim_end().to_string();
        if !joined.is_empty() {
            out.push(joined);
        }
    }
    out
}

/// 合併匯入錨點（TODO 項目 2，去重策略）：新頁 timeline 首位插入本批
/// `### {date} — 匯入：{annotation}` 錨點；舊頁的既有匯入錨點**保留**並追加在其後
/// （首行與本批相同者去重）。新頁無 timeline 區塊 → 頁尾附加 sentinel＋錨點。
fn merge_import_anchor(new_md: &str, old_md: Option<&str>, date: &str, annotation: &str) -> String {
    let ann_trim = annotation.trim();
    if ann_trim.is_empty() {
        return new_md.to_string();
    }
    let new_anchor_line = format!("### {date} — 匯入：{ann_trim}");
    let mut anchors: Vec<String> = vec![new_anchor_line.clone()];
    if let Some(old) = old_md {
        for a in extract_import_anchors(old) {
            if a.lines().next() != Some(new_anchor_line.as_str()) {
                anchors.push(a);
            }
        }
    }
    let block = anchors.join("

");
    match new_md.find("<!-- timeline -->") {
        Some(idx) => {
            let after = idx + "<!-- timeline -->".len();
            let (head, tail) = new_md.split_at(after);
            let tail_trimmed = tail.trim_start_matches(['\r', '\n']);
            format!("{head}

{block}

{tail_trimmed}")
        }
        None => {
            let trimmed = new_md.trim_end();
            format!("{trimmed}

<!-- timeline -->

{block}
")
        }
    }
}
/// 抽頁面的「備註」區段內文（`## 備註` 之後到下一個 `## ` 標題或 EOF）。
fn notes_section(md: &str) -> Option<String> {
    let mut buf: Vec<&str> = Vec::new();
    let mut in_sec = false;
    for line in md.lines() {
        if !in_sec {
            if line.trim_end() == "## 備註" {
                in_sec = true;
            }
            continue;
        }
        if line.starts_with("## ") {
            break;
        }
        buf.push(line);
    }
    let text = buf.join("\n").trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

/// 剝掉頁面開頭的 frontmatter（`---…---`），回傳 body。
fn strip_frontmatter(md: &str) -> String {
    if let Some(rest) = md.strip_prefix("---\n") {
        if let Some(idx) = rest.find("\n---\n") {
            return rest[idx + 5..].trim_start_matches('\n').to_string();
        }
    }
    md.to_string()
}

/// 取 frontmatter 的 title 值（去單引號）。
fn frontmatter_title_of(md: &str) -> Option<String> {
    for line in md.lines().skip(1) {
        if line.trim() == "---" {
            break;
        }
        if let Some(v) = line.strip_prefix("title: ") {
            return Some(v.trim_matches('\'').to_string());
        }
    }
    None
}

/// 釘回 frontmatter title（防 LLM 標題變異造成 slug/title 發散——只改 title 行）。
fn pin_frontmatter_title(md: &str, title: &str) -> String {
    let quoted = crate::converters::frontmatter::yaml_single_quote(title);
    if let Some(end) = md.find("\n---\n") {
        let head = &md[..=end];
        if let Some(t) = head.find("title: ") {
            let tline_end = md[t..].find('\n').map(|n| t + n).unwrap_or(md.len());
            let mut out = String::with_capacity(md.len() + quoted.len());
            out.push_str(&md[..t]);
            out.push_str("title: ");
            out.push_str(&quoted);
            out.push_str(&md[tline_end..]);
            return out;
        }
    }
    md.to_string()
}

/// 圖片工廠（`Pipeline::Image`）：圖片落 `<notes>/media/<名稱>.<副檔>`＋強制註解頁
/// 圖片工廠（`Pipeline::Image`）：圖片落 `<notes>/media/<名稱>.<副檔>`＋強制註解頁
/// 圖片工廠（`Pipeline::Image`）：圖片落 `<notes>/media/<名稱>.<副檔>`＋強制註解頁
/// `<notes>/media/<名稱>.md`（同目錄同名 → gbrain sync 自動 `image_of` 連結）。
/// 無 LLM；多模態向量由 gbrain sync 的 image import 產生（需 multimodal 設定，
/// 見 DEPLOYMENT.md「多模態（圖片嵌入）」）。同步寫入匯入 timeline 錨點
/// （TODO 項目 2 的圖片版）。
///
/// `annotations`：與 `paths` 平行的逐檔註解，**強制非空**（使用者決策：圖片無註解
/// 不入庫）。註解成為註解頁的 lead line 與匯入錨點內文。
fn run_images(
    spec: &'static FactoryTypeSpec,
    notes: &Path,
    paths: &[String],
    annotations: Option<&[String]>,
) -> Result<PreviewResult, AppError> {
    // 與 gbrain importImageFile 的 Voyage multimodal 上限一致。
    const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
    let anns = annotations.unwrap_or(&[]);
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();

    let media_dir = notes.join(spec.dir);
    let _ = std::fs::create_dir_all(&media_dir);

    let mut files: Vec<ProcessedFile> = Vec::new();
    let mut all_pages: Vec<PreviewPage> = Vec::new();
    let mut written: Vec<String> = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    let mut frozen = 0usize;
    let mut images_copied = 0usize;
    let mut images_reused = 0usize;

    for (i, p) in paths.iter().enumerate() {
        let mut pf = ProcessedFile { path: p.clone(), ok: true, message: None, pages: vec![] };
        let res: Result<PreviewPage, L10n> = (|| {
            let ext = path_ext(p);
            if !spec.pipeline.extensions().contains(&ext.as_str()) {
                return Err(L10n::new("factory.imageUnsupportedExt")
                    .p("file", p.clone())
                    .p("ext", ext.clone()));
            }
            let ann = anns
                .get(i)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| L10n::new("factory.imageAnnotationRequired").p("file", p.clone()))?;
            let bytes = std::fs::read(p).map_err(|e| file_err(p, e))?;
            if bytes.is_empty() {
                return Err(L10n::new("factory.imageEmpty").p("file", p.clone()));
            }
            if bytes.len() as u64 > MAX_IMAGE_BYTES {
                return Err(L10n::new("factory.imageTooLarge")
                    .p("file", p.clone())
                    .p("max", "20MB"));
            }
            let hash = format!("{:x}", Sha256::digest(&bytes))[..16].to_string();
            let name = Path::new(p)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .ok_or_else(|| file_err(p, "路徑沒有檔名"))?;
            let stem = Path::new(&name)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(String::from)
                .ok_or_else(|| file_err(&name, "無法解析主檔名"))?;
            let ext_full = if ext.is_empty() { String::new() } else { format!(".{ext}") };

            // 名稱碰撞：同內容 → 重用既有圖；不同內容 → stem-2/-3…
            let mut img_name = name.clone();
            let mut reused = false;
            // 順序：原名 → stem-2 → stem-3…（-2 起跳，與 csv_people 的碰撞慣例一致）
            let candidates =
                std::iter::once(name.clone()).chain((2..100).map(|s| format!("{stem}-{s}{ext_full}")));
            for cand in candidates {
                img_name = cand;
                match std::fs::read(media_dir.join(&img_name)) {
                    Ok(existing) => {
                        if format!("{:x}", Sha256::digest(&existing))[..16] == hash {
                            reused = true;
                            break;
                        }
                    }
                    Err(_) => break, // 不存在 → 此名可用
                }
            }
            let img_stem = img_name.strip_suffix(&ext_full).unwrap_or(&img_name).to_string();
            if !reused {
                let target = media_dir.join(&img_name);
                std::fs::write(&target, &bytes)
                    .map_err(|e| file_err(target.to_string_lossy().into_owned(), e))?;
                images_copied += 1;
            } else {
                images_reused += 1;
            }

            let markdown = build_image_annotation_page(&img_stem, &img_name, ann, &today);
            match write_page_guarded(notes, spec.dir, &img_stem, &markdown) {
                Ok(Some(f)) => written.push(f.to_string_lossy().into_owned()),
                Ok(None) => frozen += 1, // 註解頁被凍結（enriched／人手頁）——圖仍已複製
                Err(e) => errors.push(file_err(format!("{}/{}", spec.dir, img_stem), e)),
            }
            Ok(PreviewPage {
                slug: img_stem.clone(),
                target_dir: spec.dir.into(),
                name: img_stem,
                markdown,
            })
        })();
        match res {
            Ok(page) => {
                all_pages.push(page.clone());
                pf.pages = vec![page];
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
    let summary = L10n::new("factory.imageSummary")
        .p("images", images_copied)
        .p("reused", images_reused)
        .p("written", written.len())
        .p("frozen", frozen);
    Ok(PreviewResult {
        factory: spec.id.into(),
        summary,
        sample,
        total,
        written,
        errors,
        files,
    })
}

/// 圖片的註解頁：`type: note`＋強制註解作為 lead line＋匯入 timeline 錨點
/// ＋指向影像檔的 wikilink（sync 後 gbrain 以 `image_of` 反向串起）。
fn build_image_annotation_page(stem: &str, img_name: &str, annotation: &str, date: &str) -> String {
    let fm = crate::converters::frontmatter::build(&[
        ("type", "note".to_string()),
        ("origin", "operoid-factory".to_string()),
        ("title", crate::converters::frontmatter::yaml_single_quote(stem)),
        ("tags", "[note, image]".to_string()),
    ]);
    let first_line = annotation
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or(stem);
    format!(
        "{fm}# {stem}\n\n{annotation}\n\n影像檔：[[media/{img_name}|{img_name}]]\n\n\
         <!-- timeline -->\n### {date} — 匯入圖片\n\n{first_line}\n"
    )
}

fn path_ext(p: &str) -> String {
    Path::new(p)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
}

/// 圖檔發布的檔案層級錯誤（呼叫端包 L10n 或 warnings——工廠與 ingest 兩處消費）。
#[derive(Debug)]
pub(crate) struct FigurePublishError {
    pub file: String,
    pub detail: String,
}

/// 頁面追加一行匯入錨點（冪等增量版——與 `merge_import_anchor` 不同，本函式
/// 用於「頁面已存在且可能已含錨點」的補強：既有錨點原位保留，新錨插在
/// `<!-- timeline -->` 標記之後（新在上）；無標記則頁尾補 sentinel＋錨點）。
fn append_anchor_line(md: &str, anchor_line: &str) -> String {
    match md.find("<!-- timeline -->") {
        Some(idx) => {
            let after = idx + "<!-- timeline -->".len();
            let (head, tail) = md.split_at(after);
            let tail_trimmed = tail.trim_start_matches(['\r', '\n']);
            format!("{head}\n\n{anchor_line}\n\n{tail_trimmed}")
        }
        None => {
            let trimmed = md.trim_end();
            format!("{trimmed}\n\n<!-- timeline -->\n\n{anchor_line}\n")
        }
    }
}

/// PDF 圖檔發布（gbrain 原生 image import 軌道；TODO 項目 3）。對每個圖列：
/// 1. 補強已複製的 `figN-pP.md`：`影像檔：` wikilink＋`原論文 PDF：` 出處行
///    （保留原始 PDF 的授權／來源語意）＋匯入 timeline 錨點（冪等——既有錨點
///    首行相同即不再插入；`影像檔：`／`原論文 PDF：` 行已存在即不重寫）。
/// 2. MinerU 裁圖複製為同目錄同名 `figN-pP.<ext>`（內容已存在則略過——避免
///    mtime 攪動觸發無謂 resync）。**順序鐵律：先 .md 後 .jpg**——gbrain
///    `image_of` 在「圖先於註解頁」時靜默跳過（TODO 項目 3）。
///
/// caption-only 列（`image_path` 為 None）只補錨點。回傳（補強頁數、寫入的
/// 圖檔路徑、錯誤）。
pub(crate) fn publish_pdf_figures(
    doc_dir: &Path,
    rows: &[crate::converters::mineru::FigureRow],
    pdf_path: &str,
    date: &str,
) -> (usize, Vec<std::path::PathBuf>, Vec<FigurePublishError>) {
    let doc_id = doc_dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut enriched = 0usize;
    let mut images: Vec<std::path::PathBuf> = Vec::new();
    let mut errors: Vec<FigurePublishError> = Vec::new();
    for row in rows {
        let Some(fig) = row.figure_no else { continue };
        let stem = format!("fig{fig}-p{}", row.page);
        let md_path = doc_dir.join(format!("{stem}.md"));
        let md = match std::fs::read_to_string(&md_path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(FigurePublishError {
                    file: md_path.to_string_lossy().into_owned(),
                    detail: format!("圖片筆記不存在（轉換與複製不一致）：{e}"),
                });
                continue;
            }
        };

        // 1) 註解頁補強（冪等；additive——不動既有正文與錨點）。
        let img_src = row
            .image_path
            .as_deref()
            .filter(|p| Path::new(p).is_file());
        let img_ext = img_src.and_then(|p| Path::new(p).extension()).map(|e| {
            e.to_str().unwrap_or_default().to_ascii_lowercase()
        });
        let img_name =
            img_ext.clone().map(|ext| format!("{stem}.{ext}"));
        let mut next = md.clone();
        if let Some(name) = &img_name {
            if !next.contains("影像檔：") {
                let trimmed = next.trim_end();
                let provenance = if pdf_path.trim().is_empty() {
                    String::new()
                } else {
                    format!("原論文 PDF：{pdf_path}\n")
                };
                next = format!(
                    "{trimmed}\n\n影像檔：[[{doc_id}/{name}|{name}]]\n{provenance}"
                );
            }
        }
        let cap_head: String = {
            let mut s: String = row.caption.chars().take(60).collect();
            if row.caption.chars().count() > 60 {
                s.push('…');
            }
            s
        };
        let anchor_line = format!(
            "### {date} — 匯入：Figure {fig}（p{}）：{cap_head}",
            row.page
        );
        let next = if next.contains(&anchor_line) {
            next
        } else {
            append_anchor_line(&next, &anchor_line)
        };
        if next != md {
            if let Err(e) = std::fs::write(&md_path, &next) {
                errors.push(FigurePublishError {
                    file: md_path.to_string_lossy().into_owned(),
                    detail: e.to_string(),
                });
                continue;
            }
            enriched += 1;
        }

        // 2) 同名圖檔（在 .md 之後落地——image_of 順序鐵律）。
        if let (Some(src), Some(name)) = (img_src, &img_name) {
            let dst = doc_dir.join(name);
            if dst.exists() {
                continue;
            }
            match std::fs::copy(src, &dst) {
                Ok(_) => images.push(dst),
                Err(e) => errors.push(FigurePublishError {
                    file: dst.to_string_lossy().into_owned(),
                    detail: e.to_string(),
                }),
            }
        } else if row.image_path.is_some() && img_src.is_none() {
            errors.push(FigurePublishError {
                file: row.image_path.clone().unwrap_or_default(),
                detail: "MinerU 裁圖不存在（工作目錄不完整）——僅發布註解頁".into(),
            });
        }
    }
    (enriched, images, errors)
}

/// 並發轉換的暫存目錄序號（pid＋nanos 之外的去重——多工同拍建立不碰撞）。
static PDF_WORK_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

async fn run_people(
    pack: &'static Pack,
    spec: &'static FactoryTypeSpec,
    cfg: &AppConfig,
    notes: &Path,
    paths: &[String],
    gate: Gate<'_>,
    max: usize,
    import_annotation: Option<&str>,
) -> Result<PreviewResult, AppError> {
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
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
    // 閉包以借用共享 endpoint（Copy 的 Option<&_>——async move 不搬走原值）。
    let endpoint = endpoint.as_ref();
    let person_dir = pack.id_of_kind("people").map(|id| pack.spec(id).unwrap().dir).unwrap_or("person");

    // ── 階段一：逐檔並發解析/LLM 結構化（重活；不落盤）──
    // 回傳（路徑, 解析結果(頁面們, csv 列數, 合併數, 丟棄的非法日期事件數)）。
    let parsed: Vec<(String, Result<(Vec<PreviewPage>, usize, usize, usize), L10n>)> =
        run_all(paths.to_vec(), gate, max, |p| async move {
            let path = Path::new(&p);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let res: Result<(Vec<PreviewPage>, usize, usize, usize), L10n> = if ext.eq_ignore_ascii_case("csv") {
                // csv→結構化解析(一檔多人)。
                match read_text(path)
                    .map_err(|e| file_err(&p, e))
                    .and_then(|text| csv_people::parse(&text, true).map_err(|e| file_err(&p, e)))
                {
                    Ok(imp) => {
                        let pages = imp
                            .pages
                            .iter()
                            .map(|pg| PreviewPage {
                                slug: pg.slug.clone(),
                                target_dir: spec.dir.into(),
                                name: pg.name.clone(),
                                markdown: pg.markdown.clone(),
                            })
                            .collect();
                        Ok((pages, imp.rows_read, imp.groups_merged, 0))
                    }
                    Err(m) => Err(m),
                }
            } else if ext.eq_ignore_ascii_case("txt") || ext.eq_ignore_ascii_case("md") {
                // txt/md→LLM 結構化(一檔一人)。
                let ep = endpoint.as_ref().expect("has_text ⇒ endpoint loaded");
                match read_text(path).map_err(|e| file_err(&p, e)) {
                Ok(raw) => match text_to_md::text_to_page(spec, &raw, cfg, &ep).await {
                    Ok(sp) => {
                        let (slug, markdown, dropped) = text_to_md::render(spec, person_dir, &sp);
                        Ok((
                            vec![PreviewPage {
                                slug,
                                target_dir: spec.dir.into(),
                                name: sp.title,
                                markdown,
                            }],
                            0,
                            0,
                            dropped,
                        ))
                    }
                    Err(e) => Err(file_err(&p, e)),
                },
                    Err(m) => Err(m),
                }
            } else {
                Err(L10n::new("factory.csvOnly").p("file", &p))
            };
            (p, res)
        })
        .await;

    // ── 階段二：按輸入序循序寫入（覆寫語意與多工前一致；已凍結/非工廠頁跳過）──
    let mut files: Vec<ProcessedFile> = Vec::new();
    let mut all_pages: Vec<PreviewPage> = Vec::new();
    let mut written: Vec<String> = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    let mut rows = 0usize;
    let mut merged = 0usize;
    let mut frozen = 0usize;
    let mut dropped_timeline = 0usize;

    for (p, parsed) in parsed {
        let mut pf = ProcessedFile {
            path: p,
            ok: true,
            message: None,
            pages: vec![],
        };
        match parsed {
            Ok((pages, r, m, d)) => {
                rows += r;
                merged += m;
                dropped_timeline += d;
                for page in &pages {
                    let mut md = page.markdown.clone();
                    if let Some(ann) = import_annotation {
                        let old = std::fs::read_to_string(
                            notes.join(&page.target_dir).join(format!("{}.md", page.slug)),
                        )
                        .ok();
                        md = merge_import_anchor(&md, old.as_deref(), &today, ann);
                    }
                    match write_page_guarded(notes, &page.target_dir, &page.slug, &md) {
                        Ok(Some(f)) => written.push(f.to_string_lossy().into_owned()),
                        Ok(None) => frozen += 1,
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
    // ── 智慧 LLM 路由（TODO 項目 1）：備註含日期訊號的頁 → LLM 富集 timeline ──
    // 兩階段：純解析頁面已全部落盤；此處僅對「備註命中日期訊號」的頁跑 LLM 富集，
    // 富集後釘回原 slug/title 覆寫（頁面為本批剛寫入的 factory-owned 頁）。
    let mut enrich_hits = 0usize;
    let mut enrich_ok = 0usize;
    let mut enrich_failed = 0usize;
    let mut enrich_skipped = 0usize;
    let hit_files: Vec<&String> = written
        .iter()
        .filter(|f| {
            std::fs::read_to_string(f)
                .ok()
                .and_then(|md| notes_section(&md).map(|t| csv_people::has_date_signal(&t)))
                .unwrap_or(false)
        })
        .collect();
    enrich_hits = hit_files.len();
    if !hit_files.is_empty() {
        // 命中 > 0 才需要 LLM endpoint；無 key／載入失敗 → 跳過富集（匯入本身不受影響）
        let ep = gbrain_config::load_for(cfg.active_env_home())
            .ok()
            .and_then(|loaded| gbrain_config::resolve_endpoint(&loaded.config).ok())
            .filter(|e| e.has_api_key || e.provider == "ollama");
        match ep {
            Some(ep) => {
                for f in &hit_files {
                    let md = match std::fs::read_to_string(f) {
                        Ok(m) => m,
                        Err(_) => {
                            enrich_failed += 1;
                            continue;
                        }
                    };
                    let slug = Path::new(f)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(String::from)
                        .unwrap_or_default();
                    let title = frontmatter_title_of(&md).unwrap_or_else(|| slug.clone());
                    let raw = strip_frontmatter(&md);
                    match text_to_md::text_to_page(spec, &raw, cfg, &ep).await {
                        Ok(sp) => {
                            let (_, enriched_md, dropped) =
                                text_to_md::render(spec, &person_dir, &sp);
                            let pinned = pin_frontmatter_title(&enriched_md, &title);
                            dropped_timeline += dropped;
                            match write_page_guarded(notes, &person_dir, &slug, &pinned) {
                                Ok(_) => enrich_ok += 1,
                                Err(_) => enrich_failed += 1,
                            }
                        }
                        Err(_) => enrich_failed += 1,
                    }
                }
            }
            None => enrich_skipped = hit_files.len(),
        }
    }

    let summary = if rows > 0 {
        L10n::new("factory.peopleSummary")
            .p("rows", rows)
            .p("merged", merged)
            .p("written", written.len())
            .p("frozen", frozen)
            .p("dropped", dropped_timeline)
            .p("hits", enrich_hits)
            .p("enriched", enrich_ok)
            .p("failed", enrich_failed)
            .p("skipped", enrich_skipped)
    } else {
        L10n::new("factory.writtenN")
            .p("factory", spec.id)
            .p("n", written.len())
            .p("frozen", frozen)
            .p("dropped", dropped_timeline)
    };

    Ok(PreviewResult {
        factory: spec.id.into(),
        summary,
        sample,
        total,
        written,
        errors,
        files,
    })
}

async fn run_textual(
    pack: &'static Pack,
    spec: &'static FactoryTypeSpec,
    cfg: &AppConfig,
    notes: &Path,
    paths: &[String],
    gate: Gate<'_>,
    max: usize,
    import_annotation: Option<&str>,
) -> Result<PreviewResult, AppError> {
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let loaded = gbrain_config::load_for(cfg.active_env_home())?;
    let endpoint = gbrain_config::resolve_endpoint(&loaded.config)?;
    if !endpoint.has_api_key && endpoint.provider != "ollama" {
        return Err(AppError::new("llm.noApiKey")
            .p("provider", &endpoint.provider)
            .p("envKey", gbrain_config::env_key(&endpoint.provider).unwrap_or("?")));
    }
    let target_dir = spec.dir.to_string();
    let person_dir = pack.id_of_kind("people").map(|id| pack.spec(id).unwrap().dir).unwrap_or("person");
    // 閉包以借用共享 endpoint（Copy 的 &LlmEndpoint——async move 不搬走原值）。
    let endpoint = &endpoint;

    // ── 階段一：逐檔並發抽文字＋LLM 結構化（重活；不落盤）──
    // 回傳（路徑, 結果(slug, 標題, markdown, 丟棄的非法日期事件數)）。
    let converted: Vec<(String, Result<(String, String, String, usize), L10n>)> =
        run_all(paths.to_vec(), gate, max, |p| async move {
            let path = Path::new(&p);
            let res = match extract_raw(path).map_err(|e| file_err(&p, e)) {
                Ok(raw) => match text_to_md::text_to_page(spec, &raw, cfg, &endpoint).await {
                    Ok(sp) => {
                        let (slug, markdown, dropped) = text_to_md::render(spec, person_dir, &sp);
                        Ok((slug, sp.title, markdown, dropped))
                    }
                    Err(e) => Err(file_err(&p, e)),
                },
                Err(m) => Err(m),
            };
            (p, res)
        })
        .await;

    // ── 階段二：按輸入序循序寫入（覆寫語意與多工前一致；已凍結/非工廠頁跳過）──
    let mut files: Vec<ProcessedFile> = Vec::new();
    let mut written = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    let mut frozen = 0usize;
    let mut dropped_timeline = 0usize;
    for (p, converted) in converted {
        let mut pf = ProcessedFile {
            path: p,
            ok: true,
            message: None,
            pages: vec![],
        };
        match converted {
            Ok((slug, title, markdown, dropped)) => {
                dropped_timeline += dropped;
                let markdown = if let Some(ann) = import_annotation {
                    let old = std::fs::read_to_string(
                        notes.join(&target_dir).join(format!("{slug}.md")),
                    )
                    .ok();
                    merge_import_anchor(&markdown, old.as_deref(), &today, ann)
                } else {
                    markdown
                };
                match write_page_guarded(notes, &target_dir, &slug, &markdown) {
                    Ok(Some(f)) => written.push(f.to_string_lossy().into_owned()),
                    Ok(None) => frozen += 1,
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
                    name: title,
                    markdown,
                });
            }
            Err(m) => {
                pf.ok = false;
                pf.message = Some(m.clone());
                errors.push(m);
            }
        }
        files.push(pf);
    }
    let sample: Vec<PreviewPage> = files.iter().flat_map(|f| f.pages.iter().cloned()).take(10).collect();
    let total: usize = files.iter().map(|f| f.pages.len()).sum();
    let summary = L10n::new("factory.writtenN")
        .p("factory", spec.id)
        .p("n", written.len())
        .p("frozen", frozen)
        .p("dropped", dropped_timeline);
    Ok(PreviewResult {
        factory: spec.id.into(),
        summary,
        sample,
        total,
        written,
        errors,
        files,
    })
}

async fn run_inbox(
    spec: &'static FactoryTypeSpec,
    cfg: &AppConfig,
    paths: &[String],
    gate: Gate<'_>,
    max: usize,
) -> Result<PreviewResult, AppError> {
    // capture 型直接走 gbrain capture(寫入知識庫內部儲存),不走 notes repo。
    // 逐檔並發子行程（tokio::process；公平閘門照樣節流——capture 也是工作）。
    let results: Vec<(String, Result<(String, String), L10n>)> =
        run_all(paths.to_vec(), gate, max, |p| async move {
            let name = Path::new(&p).file_name().and_then(|s| s.to_str()).unwrap_or("note").to_string();
            let mut cmd = tokio::process::Command::new(&cfg.gbrain_exe_path);
            cmd.args(["capture", "--file", &p, "--type", "note", "--quiet"])
                .env("PYTHONUTF8", "1");
            crate::proc::no_console_async(&mut cmd);
            if let Some(h) = cfg.active_env_home() {
                cmd.env("GBRAIN_HOME", h);
            }
            let res = match cmd.output().await {
                Ok(o) if o.status.success() => {
                    let slug = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    Ok((if slug.is_empty() { p.clone() } else { slug }, name))
                }
                Ok(o) => Err(file_err(&p, String::from_utf8_lossy(&o.stderr).trim())),
                Err(e) => Err(file_err(&p, e)),
            };
            (p, res)
        })
        .await;

    let mut sample = Vec::new();
    let mut written = Vec::new();
    let mut errors: Vec<L10n> = Vec::new();
    for (_, r) in results {
        match r {
            Ok((slug, name)) => {
                written.push(slug.clone());
                sample.push(PreviewPage {
                    slug,
                    target_dir: format!("{}/", spec.dir),
                    name,
                    markdown: String::new(),
                });
            }
            Err(m) => errors.push(m),
        }
    }
    let total = written.len();
    Ok(PreviewResult {
        factory: spec.id.into(),
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
        other => Err(anyhow::anyhow!("不支援的副檔名：{other}（person/people=csv,txt,md；其餘文字類型=txt,md,pdf；capture 型=txt,md）")),
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

/// C13c（D-C13i）共用步驟①②：寫入端天花板（作者 clearance ≥ 目標等級，未賦＝Internal）
/// ＋自動供給 scope/source（慣例命名＋冪等）。回傳寫入目標。
async fn scope_target_checked(
    store: &crate::domain::SqliteStore,
    cfg: &AppConfig,
    kind: crate::knowledge::provision::CircleKind,
    circle: &str,
    level: crate::knowledge::types::SecurityLevel,
    owner_principal: &str,
) -> Result<crate::knowledge::provision::WriteTarget, AppError> {
    use crate::domain::Store as _;
    let owner = store
        .get_principal(owner_principal)
        .map_err(|e| AppError::new("knowledge.writeFailed").p("detail", e.to_string()))?
        .ok_or_else(|| AppError::new("knowledge.writeFailed").p("detail", format!("principal 不存在：{owner_principal}")))?;
    let clearance = owner.attrs.clearance.unwrap_or(crate::knowledge::types::SecurityLevel::Internal);
    if level > clearance {
        return Err(AppError::new("knowledge.writeAboveClearance")
            .p("level", format!("{level:?}").to_lowercase())
            .p("clearance", format!("{clearance:?}").to_lowercase()));
    }
    crate::knowledge::provision::resolve_write_target(store, cfg, kind, circle, level, owner_principal)
        .await
        .map_err(|e| AppError::new("knowledge.writeFailed").p("detail", e.to_string()))
}

/// C13c 共用步驟④：git commit＋`sync --source` 立即入圖（免等事件匯流排的全腦同步）。
async fn commit_and_sync_scope(
    cfg: &AppConfig,
    target: &crate::knowledge::provision::WriteTarget,
) -> Result<(), AppError> {
    crate::knowledge::provision::commit_dir(&target.dir)
        .map_err(|e| AppError::new("knowledge.writeFailed").p("detail", e.to_string()))?;
    let exe = cfg.gbrain_exe_path.clone();
    let env = crate::proc::env_for_brain(cfg.active_env_home());
    let (code, _, err) = crate::gbrain_cli::run_capture(
        &exe,
        &["sync", "--source", &target.source_id, "--no-pull", "--yes", "--no-hard-deadline"],
        &env,
    )
    .await
    .map_err(|e| AppError::new("knowledge.writeFailed").p("detail", e.to_string()))?;
    if code != 0 {
        return Err(AppError::new("knowledge.writeFailed")
            .p("detail", format!("sync {} 失敗：{}", target.source_id, err.trim())));
    }
    Ok(())
}

/// C13c（Q12 兌現，D-C13g/i）：撰寫器文章寫入**指定圈子×等級**——
/// ①寫入端天花板 ②自動供給 scope/source ③enriched 內容寫進供給目錄
/// （save_authored_core 的 target_repo 重定向）＋git commit ④sync 立即入圖。
/// 回傳（撰寫結果, 寫入目標）。
pub async fn authored_to_scope_core(
    cfg: &AppConfig,
    state: Option<&AppState>,
    store: &crate::domain::SqliteStore,
    factory: &str,
    markdown: &str,
    existing_slug: Option<&str>,
    kind: crate::knowledge::provision::CircleKind,
    circle: &str,
    level: crate::knowledge::types::SecurityLevel,
    owner_principal: &str,
) -> Result<(AuthoredResult, crate::knowledge::provision::WriteTarget), AppError> {
    let target = scope_target_checked(store, cfg, kind, circle, level, owner_principal).await?;
    let res = save_authored_core(
        cfg,
        state,
        factory,
        markdown,
        existing_slug,
        Some(target.dir.to_string_lossy().as_ref()),
    )
    .await?;
    commit_and_sync_scope(cfg, &target).await?;
    Ok((res, target))
}

/// 工廠批次轉換寫入**指定圈子×等級**（企業版 user 級工廠頁的轉換目標）——
/// 與 [`authored_to_scope_core`] 同一供給與入圖序列，寫入端換成 `run_core_sched`
/// （各工廠管線寫進供給目錄的 `<type>/<slug>.md`；`gate=Some` 時逐檔過企業
/// 公平閘門）。回傳（轉換預覽, 寫入目標）。
pub async fn run_to_scope_core(
    cfg: &AppConfig,
    store: &crate::domain::SqliteStore,
    factory: &str,
    paths: &[String],
    kind: crate::knowledge::provision::CircleKind,
    circle: &str,
    level: crate::knowledge::types::SecurityLevel,
    owner_principal: &str,
    gate: Gate<'_>,
) -> Result<(PreviewResult, crate::knowledge::provision::WriteTarget), AppError> {
    let target = scope_target_checked(store, cfg, kind, circle, level, owner_principal).await?;
    let preview =
        run_core_sched(cfg, factory, paths, Some(target.dir.to_string_lossy().as_ref()), gate, None, None).await?;
    commit_and_sync_scope(cfg, &target).await?;
    Ok((preview, target))
}

/// 覆蓋寫入（預覽後編輯過的頁）寫回**指定圈子×等級**的供給目錄——與轉換同一目標時
/// 冪等回既有 scope，頁面覆蓋原地。回傳（覆寫結果, 寫入目標）。
pub async fn write_pages_to_scope_core(
    cfg: &AppConfig,
    store: &crate::domain::SqliteStore,
    pages: &[WritePage],
    kind: crate::knowledge::provision::CircleKind,
    circle: &str,
    level: crate::knowledge::types::SecurityLevel,
    owner_principal: &str,
) -> Result<(WriteResult, crate::knowledge::provision::WriteTarget), AppError> {
    let target = scope_target_checked(store, cfg, kind, circle, level, owner_principal).await?;
    let res = write_pages_core(&target.dir, pages);
    commit_and_sync_scope(cfg, &target).await?;
    Ok((res, target))
}
#[cfg(test)]
mod write_guard_tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("operoid-factory-guard-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn guarded_overwrites_factory_owned_page() {
        let notes = tmpdir("overwrite");
        let first =
            write_page_guarded(&notes, "people", "jane", "---\ntype: person\norigin: operoid-factory\n---\n# Jane\n").unwrap();
        assert!(first.is_some());
        let again =
            write_page_guarded(&notes, "people", "jane", "---\ntype: person\norigin: operoid-factory\n---\n# Jane v2\n")
                .unwrap();
        assert!(again.is_some());
        let text = std::fs::read_to_string(notes.join("people").join("jane.md")).unwrap();
        assert!(text.contains("Jane v2"));
        let _ = std::fs::remove_dir_all(&notes);
    }

    #[test]
    fn guarded_freezes_enriched_page() {
        let notes = tmpdir("enriched");
        std::fs::create_dir_all(notes.join("people")).unwrap();
        std::fs::write(
            notes.join("people").join("jane.md"),
            "---\ntype: person\norigin: operoid-factory\nenriched: true\n---\n# Jane\n curated\n",
        )
        .unwrap();
        let r = write_page_guarded(&notes, "people", "jane", "---\ntype: person\n---\n# new\n").unwrap();
        assert!(r.is_none());
        let text = std::fs::read_to_string(notes.join("people").join("jane.md")).unwrap();
        assert!(text.contains("enriched: true"));
        assert!(text.contains("curated"));
        let _ = std::fs::remove_dir_all(&notes);
    }

    #[test]
    fn guarded_freezes_foreign_page() {
        // 人手建立的頁面（無 origin）——即使 slug 撞名也不得被批次工廠覆寫。
        let notes = tmpdir("foreign");
        std::fs::create_dir_all(notes.join("people")).unwrap();
        std::fs::write(
            notes.join("people").join("jane.md"),
            "---\ntype: person\ntitle: 'Jane'\n---\n# Jane\n手寫內容\n",
        )
        .unwrap();
        let r = write_page_guarded(&notes, "people", "jane", "---\ntype: person\n---\n# new\n").unwrap();
        assert!(r.is_none());
        let text = std::fs::read_to_string(notes.join("people").join("jane.md")).unwrap();
        assert!(text.contains("手寫內容"));
        let _ = std::fs::remove_dir_all(&notes);
    }

    #[test]
    fn guarded_writes_missing_page() {
        let notes = tmpdir("missing");
        let r = write_page_guarded(&notes, "meeting", "m1", "---\ntype: meeting\norigin: operoid-factory\n---\n# M1\n")
            .unwrap();
        assert!(r.is_some());
        assert!(notes.join("meeting").join("m1.md").exists());
        let _ = std::fs::remove_dir_all(&notes);
    }
}

#[cfg(test)]
mod image_run_tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("operoid-factory-image-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn image_spec() -> &'static FactoryTypeSpec {
        crate::factory_types::V2_PACK
            .types
            .iter()
            .find(|t| t.id == "image")
            .expect("v2 pack 應含 image 工廠")
    }

    fn drop_png(dir: &Path, name: &str, payload: &[u8]) -> String {
        let p = dir.join(name);
        std::fs::write(&p, payload).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn image_run_happy_path_copies_image_and_writes_annotation_page() {
        let notes = tmpdir("happy");
        let input = drop_png(&notes, "in-shot.png", b"PNG-fake-bytes-89");
        let notes_s = notes.to_string_lossy().into_owned();
        let r = run_images(image_spec(), Path::new(&notes_s), &[input], Some(&["產品外觀照".into()]))
            .unwrap();
        assert!(r.errors.is_empty(), "errors: {:?}", r.errors);
        assert_eq!(r.total, 1);

        let img = notes.join("media").join("in-shot.png");
        assert_eq!(std::fs::read(&img).unwrap(), b"PNG-fake-bytes-89");
        let page = notes.join("media").join("in-shot.md");
        let md = std::fs::read_to_string(&page).unwrap();
        assert!(md.contains("origin: operoid-factory"));
        assert!(md.contains("type: note"));
        assert!(md.contains("產品外觀照"));
        assert!(md.contains("[[media/in-shot.png|in-shot.png]]"));
        assert!(md.contains("<!-- timeline -->"));
        assert!(md.contains("— 匯入圖片"));
        assert_eq!(r.written.len(), 1);
        assert_eq!(r.summary.code, "factory.imageSummary");
        let _ = std::fs::remove_dir_all(&notes);
    }

    #[test]
    fn image_run_requires_annotation() {
        let notes = tmpdir("noann");
        let input = drop_png(&notes, "shot.png", b"abc");
        let notes_s = notes.to_string_lossy().into_owned();
        let r = run_images(image_spec(), Path::new(&notes_s), &[input], None).unwrap();
        assert_eq!(r.total, 0);
        assert!(r.errors.iter().any(|e| e.code == "factory.imageAnnotationRequired"));
        assert!(!notes.join("media").join("shot.png").exists());
        let _ = std::fs::remove_dir_all(&notes);
    }

    #[test]
    fn image_run_unsupported_ext_rejected() {
        let notes = tmpdir("badext");
        let input = drop_png(&notes, "doc.txt", b"plain");
        let notes_s = notes.to_string_lossy().into_owned();
        let r =
            run_images(image_spec(), Path::new(&notes_s), &[input], Some(&["描述".into()])).unwrap();
        assert!(r.errors.iter().any(|e| e.code == "factory.imageUnsupportedExt"));
        let _ = std::fs::remove_dir_all(&notes);
    }

    #[test]
    fn image_run_name_collision_different_content_gets_suffix() {
        let notes = tmpdir("collide");
        std::fs::create_dir_all(notes.join("a")).unwrap();
        std::fs::create_dir_all(notes.join("b")).unwrap();
        let a = drop_png(&notes.join("a"), "shot.png", b"content-A");
        let b = drop_png(&notes.join("b"), "shot.png", b"content-B");
        let notes_s = notes.to_string_lossy().into_owned();
        let r = run_images(
            image_spec(),
            Path::new(&notes_s),
            &[a, b],
            Some(&["第一張".into(), "第二張".into()]),
        )
        .unwrap();
        assert!(r.errors.is_empty(), "errors: {:?}", r.errors);
        assert!(notes.join("media").join("shot.png").exists());
        assert!(notes.join("media").join("shot-2.png").exists());
        assert_eq!(std::fs::read(notes.join("media").join("shot.png")).unwrap(), b"content-A");
        assert_eq!(std::fs::read(notes.join("media").join("shot-2.png")).unwrap(), b"content-B");
        assert!(notes.join("media").join("shot.md").exists());
        assert!(notes.join("media").join("shot-2.md").exists());
        assert_eq!(r.total, 2);
        let _ = std::fs::remove_dir_all(&notes);
    }

    #[test]
    fn image_run_same_content_reused() {
        let notes = tmpdir("reuse");
        std::fs::create_dir_all(notes.join("a")).unwrap();
        std::fs::create_dir_all(notes.join("b")).unwrap();
        let a = drop_png(&notes.join("a"), "shot.png", b"same-bytes");
        let b = drop_png(&notes.join("b"), "shot.png", b"same-bytes");
        let notes_s = notes.to_string_lossy().into_owned();
        let r = run_images(
            image_spec(),
            Path::new(&notes_s),
            &[a, b],
            Some(&["同一張".into(), "同一張".into()]),
        )
        .unwrap();
        assert!(r.errors.is_empty());
        // 同內容 → 第二張重用既有圖（不產生 -2）、頁面照常覆寫
        assert!(notes.join("media").join("shot.png").exists());
        assert!(!notes.join("media").join("shot-2.png").exists());
        assert_eq!(r.total, 2);
        assert_eq!(r.summary.params.get("reused").cloned().as_deref(), Some("1"));
        assert_eq!(r.summary.params.get("images").cloned().as_deref(), Some("1"));
        let _ = std::fs::remove_dir_all(&notes);
    }

    #[test]
    fn image_run_frozen_enriched_annotation_page() {
        let notes = tmpdir("frozen");
        std::fs::create_dir_all(notes.join("media")).unwrap();
        std::fs::write(
            notes.join("media").join("in-shot.md"),
            "---
type: note
origin: operoid-factory
enriched: true
---
# shot
人工整理內容
",
        )
        .unwrap();
        let input = drop_png(&notes, "in-shot.png", b"xyz");
        let notes_s = notes.to_string_lossy().into_owned();
        let r = run_images(image_spec(), Path::new(&notes_s), &[input], Some(&["描述".into()]))
            .unwrap();
        // 圖照拷、頁凍結
        assert!(notes.join("media").join("in-shot.png").exists());
        let md = std::fs::read_to_string(notes.join("media").join("in-shot.md")).unwrap();
        assert!(md.contains("人工整理內容"));
        assert!(!md.contains("<!-- timeline -->"));
        assert!(r.written.is_empty()); // 凍結頁不計入 written
        assert_eq!(r.summary.params.get("frozen").cloned().as_deref(), Some("1"));
        let _ = std::fs::remove_dir_all(&notes);
    }
}

#[cfg(test)]
mod enrich_helper_tests {
    use super::*;

    #[test]
    fn notes_section_extracts_between_headings() {
        let md = "---\ntype: person\n---\n# 王小明\n\n任職於甲公司。\n\n## 備註\n\n2008/2/4 拜訪客戶\n3月12日 電話聯絡\n\n## 其他\n\n不應包含\n";
        let sec = notes_section(md).unwrap();
        assert!(sec.contains("2008/2/4 拜訪客戶"));
        assert!(sec.contains("3月12日 電話聯絡"));
        assert!(!sec.contains("不應包含"));
        assert!(notes_section("# 無備註頁").is_none());
    }

    #[test]
    fn strip_frontmatter_removes_leading_block() {
        let md = "---\ntype: person\norigin: operoid-factory\n---\n# 王小明\n\n本文\n";
        let body = strip_frontmatter(md);
        assert!(body.starts_with("# 王小明"));
        assert!(!body.contains("type: person"));
    }

    #[test]
    fn pin_frontmatter_title_forces_title_line() {
        let md = "---\ntype: person\norigin: operoid-factory\ntitle: 'LLM 幻覺標題'\ntags: [people]\n---\n# body\n";
        let pinned = pin_frontmatter_title(md, "王小明");
        assert!(pinned.contains("title: '王小明'"));
        assert!(!pinned.contains("LLM 幻覺標題"));
        assert!(pinned.ends_with("# body\n"));
    }

    #[test]
    fn frontmatter_title_of_reads_quoted_value() {
        let md = "---\ntype: person\ntitle: '王小明'\n---\n";
        assert_eq!(frontmatter_title_of(md).unwrap(), "王小明");
    }
}

#[cfg(test)]
mod pdf_publish_tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "operoid-factory-publish-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 轉換器風格的圖片筆記（frontmatter＋`title: | text:` 正文）。
    fn drop_fig_note(dir: &Path, stem: &str) {
        std::fs::write(
            dir.join(format!("{stem}.md")),
            format!(
                "---\norigin: operoid-employee\nproduced_by: mineru-converter\ncreated_at: 2026-10-17\ntitle: 'Test Paper · Figure 1 (p2)'\n---\n\ntitle: Test Paper | text: [Figure 1 in Section II, page 2] Figure 1. The system architecture of the two-chip IVR.\n"
            ),
        )
        .unwrap();
    }

    fn row(fig: u32, page: i64, image_path: Option<String>) -> crate::converters::mineru::FigureRow {
        crate::converters::mineru::FigureRow {
            doc_id: "doc2026".into(),
            page,
            image_path,
            caption: "The system architecture of the two-chip IVR solution".into(),
            section: "II".into(),
            figure_no: Some(fig),
            image_md5: None,
            source_id: None,
        }
    }

    #[test]
    fn publish_enriches_note_then_copies_image_and_is_idempotent() {
        let dir = tmpdir("happy");
        let doc_dir = dir.join("doc2026");
        std::fs::create_dir_all(&doc_dir).unwrap();
        drop_fig_note(&doc_dir, "fig1-p2");
        let work_img = dir.join("page_2_image_1.jpg");
        std::fs::write(&work_img, b"fake-jpeg-bytes").unwrap();

        let rows = vec![row(1, 2, Some(work_img.to_string_lossy().into_owned()))];
        let (enriched, imgs, errors) =
            publish_pdf_figures(&doc_dir, &rows, r"C:\papers\test.pdf", "2026-10-17");
        assert!(errors.is_empty(), "errors: {errors:?}");
        assert_eq!(enriched, 1);
        assert_eq!(imgs, vec![doc_dir.join("fig1-p2.jpg")]);
        assert!(doc_dir.join("fig1-p2.jpg").exists());

        let md = std::fs::read_to_string(doc_dir.join("fig1-p2.md")).unwrap();
        assert!(md.contains("影像檔：[[doc2026/fig1-p2.jpg|fig1-p2.jpg]]"));
        assert!(md.contains(r"原論文 PDF：C:\papers\test.pdf"));
        assert!(md.contains("### 2026-10-17 — 匯入：Figure 1（p2）："));
        assert!(md.contains("The system architecture"), "正文保留");

        // 冪等：再跑一次——錨點首行相同去重、行不重寫、圖不重複製。
        let before = md.clone();
        let (enriched2, imgs2, errors2) =
            publish_pdf_figures(&doc_dir, &rows, r"C:\papers\test.pdf", "2026-10-17");
        assert!(errors2.is_empty());
        assert_eq!(enriched2, 0, "第二次不應有變更");
        assert!(imgs2.is_empty(), "同名圖檔已存在不重寫");
        let after = std::fs::read_to_string(doc_dir.join("fig1-p2.md")).unwrap();
        assert_eq!(before, after);
        assert_eq!(
            after.matches("影像檔：").count(),
            1,
            "影像檔行不重複"
        );
        assert_eq!(after.matches("— 匯入：Figure 1").count(), 1, "錨點不重複");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn publish_caption_only_gets_anchor_without_image_line() {
        let dir = tmpdir("caption-only");
        let doc_dir = dir.join("doc2026");
        std::fs::create_dir_all(&doc_dir).unwrap();
        drop_fig_note(&doc_dir, "fig3-p3");
        let rows = vec![row(3, 3, None)];
        let (_enriched, imgs, errors) =
            publish_pdf_figures(&doc_dir, &rows, r"C:\papers\test.pdf", "2026-10-17");
        assert!(errors.is_empty());
        assert!(imgs.is_empty(), "caption-only 不發布圖檔");
        let md = std::fs::read_to_string(doc_dir.join("fig3-p3.md")).unwrap();
        assert!(md.contains("— 匯入：Figure 3（p3）："));
        assert!(!md.contains("影像檔："), "無圖檔不加影像行");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn publish_missing_note_and_missing_crop_report_errors() {
        let dir = tmpdir("errors");
        let doc_dir = dir.join("doc2026");
        std::fs::create_dir_all(&doc_dir).unwrap();
        // fig5：md 存在但裁圖遺失；fig6：md 缺失。
        drop_fig_note(&doc_dir, "fig5-p4");
        let rows = vec![
            row(5, 4, Some(dir.join("gone.jpg").to_string_lossy().into_owned())),
            row(6, 5, Some(dir.join("x.jpg").to_string_lossy().into_owned())),
        ];
        let (_enriched, imgs, errors) =
            publish_pdf_figures(&doc_dir, &rows, r"C:\papers\test.pdf", "2026-10-17");
        assert!(imgs.is_empty());
        assert_eq!(errors.len(), 2, "裁圖遺失＋筆記缺失各一：{errors:?}");
        assert!(errors.iter().any(|e| e.file.ends_with("fig6-p5.md")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
