//! K1 MinerU converter——複雜 PDF → gbrain-legal 知識管線（計畫 §K1）。
//!
//! ```text
//! PDF ──分流（router：S1–S4 訊號）──► MinerU（ladder 階梯取用；zip 輸出）
//!                                       ├─ structured_content.json 解析（structured）
//!                                       ├─ 章節切塊（chunk；實驗 V3-t 語意）
//!                                       └─ 產出：文字 chunk 筆記＋圖片筆記
//!                                              ＋figures.sqlite（sidecar 來源資料，K3 消費）
//!                                              ＋conversion-report.json（路由/階梯/egress 稽核）
//! ```
//!
//! 設計鐵律（皆由實驗背書）：
//! - **gbrain 零修改**——筆記 body 自帶 `title: … | text: …` 文件前綴（C5/第十章攔截驗證）、
//!   slug 編碼 `doc/sNN`、`doc/figN-pP` 歸因結構（C4）；查詢前綴在呼叫端（K2）。
//! - **egress 信任分層**（ladder）：local 永遠允許；private 自設端點設定即同意；
//!   public 第三方雲（mineru.net）預設擋（`allow_public_egress=false`）。
//! - 轉換 metadata 記錄路由訊號與每次 egress 的層級/端點（可稽核）。
//!
//! 產出布局（`out_dir`）：
//! ```text
//! notes/<doc_slug>/s00.md … fig1-p2.md …   ← gbrain source 目錄（呼叫端 git init＋sources add）
//! figures.sqlite                            ← 每圖一列：doc_id/page/image_path/caption/section/md5
//! conversion-report.json                    ← 路由＋階梯＋egress 稽核＋警告
//! mineru/                                   ← MinerU 原始 zip＋解壓（圖檔是 K3 嵌入來源，不清）
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

pub mod chunk;
pub mod ladder;
pub mod router;
pub mod structured;

use ladder::LadderOutcome;
use router::{ConvertPolicy, RouteDecision};
use structured::{Block, BlockKind, Parsed};

/// 轉換設定（AppConfig 對應欄位見 `app_config.rs`；operoid.toml `[knowledge]`）。
#[derive(Debug, Clone)]
pub struct ConvertConfig {
    /// 路由政策：auto（預設）／always_fast／always_mineru。
    pub policy: ConvertPolicy,
    /// 階梯 1：完整可執行路徑或命令模板（venv launcher exe、`conda run …`）。
    pub mineru_command: Option<String>,
    /// MinerU 檔位：flash／basic（預設）／standard。
    pub mineru_tier: String,
    /// 階梯 3（private）：自架 mineru-kit api-server URL——設定即同意。
    pub mineru_remote_url: Option<String>,
    /// 階梯 3（public）：mineru.net API key——須 `allow_public_egress` 才生效。
    pub mineru_api_key: Option<String>,
    /// public 第三方雲 egress 閘門（**預設 false**）。
    pub allow_public_egress: bool,
}

impl Default for ConvertConfig {
    fn default() -> Self {
        Self {
            policy: ConvertPolicy::Auto,
            mineru_command: None,
            mineru_tier: "basic".into(),
            mineru_remote_url: None,
            mineru_api_key: None,
            allow_public_egress: false,
        }
    }
}

/// 一則筆記（含評測歸因屬性——K7 逐項對映用）。
#[derive(Debug, Clone, Serialize)]
pub struct NoteRecord {
    /// gbrain slug（`<doc_slug>/s00`、`<doc_slug>/fig1-p2`）。
    pub slug: String,
    /// 寫出的檔案絕對路徑。
    pub path: PathBuf,
    pub kind: NoteKind,
    pub section_id: String,
    pub figs: Vec<u32>,
    pub tables: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    Text,
    Figure,
}

/// figures.sqlite 一列（sidecar 來源資料；`vec` 由 K3 回填）。
#[derive(Debug, Clone, Serialize)]
pub struct FigureRow {
    pub doc_id: String,
    /// 1-based 頁碼。
    pub page: i64,
    /// 圖檔絕對路徑；caption-only 拾回（無對應裁圖）為 None。
    pub image_path: Option<String>,
    pub caption: String,
    pub section: String,
    /// 圖號（歸因標籤；K4 輸出「Figure N」用）。
    pub figure_no: Option<u32>,
    /// 圖檔內容 md5（K3 去重鍵——重轉換不重嵌）。
    pub image_md5: Option<String>,
    /// 授權過濾用（K4：source_ids 鐵律；轉換時未知，工廠歸檔時補 tag）。
    pub source_id: Option<String>,
}

/// 轉換報告（conversion-report.json；可稽核）。
#[derive(Debug, Clone, Serialize)]
pub struct ConversionReport {
    pub doc_id: String,
    pub title: String,
    pub route: RouteReport,
    pub ladder: LadderReport,
    pub text_notes: usize,
    pub figure_notes: usize,
    pub figure_rows: usize,
    /// 每次 egress 的層級與端點（K1 分層稽核）。
    pub egress: Vec<EgressEvent>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteReport {
    pub policy: String,
    pub to_mineru: bool,
    pub reason: String,
    pub signals: router::Signals,
}

#[derive(Debug, Clone, Serialize)]
pub struct LadderReport {
    pub resolved: bool,
    pub tier: Option<String>,
    pub program: Option<String>,
    pub endpoint: Option<String>,
    pub attempts: Vec<ladder::LadderAttempt>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EgressEvent {
    pub tier: String,
    pub endpoint: Option<String>,
}

/// 轉換結果：報告＋筆記清單（測試/工廠/評測消費）。
#[derive(Debug, Clone)]
pub struct ConversionOutcome {
    pub report: ConversionReport,
    pub notes: Vec<NoteRecord>,
    pub figure_rows: Vec<FigureRow>,
}

/// 端到端入口：PDF → 分流 → MinerU（或快速路徑）→ 筆記＋sidecar＋報告。
pub async fn convert(pdf: &Path, out_dir: &Path, cfg: &ConvertConfig) -> Result<ConversionOutcome> {
    std::fs::create_dir_all(out_dir)?;
    let doc_id = crate::slug::slugify(
        pdf.file_stem().and_then(|s| s.to_str()).unwrap_or("document"),
        "document",
    );

    // 分流：先 pdf_extract 抽訊號（毫秒級）；硬錯誤 → 例外路徑升級 MinerU。
    let extracted = router::extract_text(pdf).ok();
    let route = router::route(&std::fs::read(pdf)?, extracted.as_deref(), cfg.policy);

    let mut warnings: Vec<String> = Vec::new();
    if route.to_mineru {
        let ladder_out = ladder::resolve(cfg);
        if ladder_out.resolved() {
            let work = out_dir.join("mineru");
            match ladder::parse_zip(&ladder_out, cfg, pdf, &work).await {
                Ok(sc_path) => {
                    let zip_dir = sc_path
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| out_dir.to_path_buf());
                    let mut outcome = convert_structured(&zip_dir, out_dir, Some(&doc_id))?;
                    outcome.report.route = route_report(&route);
                    merge_ladder(&mut outcome.report, &ladder_out, &warnings);
                    write_report(out_dir, &outcome.report)?;
                    return Ok(outcome);
                }
                Err(e) => {
                    warnings.push(format!("MinerU 解析失敗，降級 pdf_extract：{e}"));
                }
            }
        } else {
            warnings.push(
                "MinerU 取用階梯全級落空（設定覆寫/PATH/遠端皆不可用）——降級 pdf_extract \
                 （無圖、品質降）。安裝建議：`uv tool install mineru`、或設定 \
                 `[knowledge] mineru_command` 指 venv launcher exe。"
                    .into(),
            );
        }
        warnings.extend(ladder_preview_warnings(cfg));
    }

    // 快速路徑（或降級）：naive 文字抽取 → 段落切塊；無圖、品質降。
    let text = extracted.ok_or_else(|| {
        anyhow::anyhow!("pdf_extract 失敗且 MinerU 不可用（見 conversion-report.json 的警告）")
    })?;
    let mut outcome = convert_fast(&text, out_dir, &doc_id)?;
    outcome.report.route = route_report(&route);
    outcome.report.warnings.extend(warnings);
    write_report(out_dir, &outcome.report)?;
    Ok(outcome)
}

/// 階梯未解析時的 egress 設定預警（public key 存在但被閘門擋住的提示已在 ladder）。
fn ladder_preview_warnings(_cfg: &ConvertConfig) -> Vec<String> {
    Vec::new()
}

/// 從既有 MinerU 解析目錄（unzipped：structured_content.json＋images/）產出筆記。
/// MinerU 已跑過（重跑/批次場景）時跳過分流與階梯。
pub fn convert_structured(
    zip_dir: &Path,
    out_dir: &Path,
    doc_id_hint: Option<&str>,
) -> Result<ConversionOutcome> {
    let sc = zip_dir.join("structured_content.json");
    let json = std::fs::read_to_string(&sc)
        .with_context(|| format!("讀取 {} 失敗", sc.display()))?;
    let parsed = structured::parse(&json)?;
    let doc_id = doc_id_hint
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::slug::slugify(&zip_dir.file_name().and_then(|s| s.to_str()).unwrap_or("document"), "document"));
    let outcome = emit(&doc_id, &parsed, Some(zip_dir), out_dir)?;
    write_report(out_dir, &outcome.report)?;
    Ok(outcome)
}

/// 快速路徑：naive 文字 → 段落切塊 → 純文字筆記（無圖）。
pub fn convert_fast(text: &str, out_dir: &Path, doc_id: &str) -> Result<ConversionOutcome> {
    let title = text
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or(doc_id)
        .chars()
        .take(120)
        .collect::<String>();
    let blocks: Vec<Block> = paragraphs(text)
        .into_iter()
        .map(|p| Block {
            kind: BlockKind::Text,
            text: p,
            fig: None,
            table: None,
            section: Vec::new(),
            page: 0,
            image_source: None,
        })
        .collect();
    let parsed = Parsed {
        title,
        is_full_document: true,
        blocks,
    };
    let mut outcome = emit(doc_id, &parsed, None, out_dir)?;
    outcome
        .report
        .warnings
        .push("快速路徑（pdf_extract）：無圖片側資料——sidecar 為空；複雜排版（雙欄/掃描）品質降。".into());
    write_report(out_dir, &outcome.report)?;
    Ok(outcome)
}

/// 段落切分（空行為界；連續非空行併一段）。
fn paragraphs(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            if !cur.is_empty() {
                out.push(cur.join(" "));
                cur.clear();
            }
        } else {
            cur.push(t.to_string());
        }
    }
    if !cur.is_empty() {
        out.push(cur.join(" "));
    }
    out
}

/// 共用產出層：切塊 → 筆記檔＋figures.sqlite＋報告（不含 route/ladder——呼叫端補）。
fn emit(doc_id: &str, parsed: &Parsed, image_root: Option<&Path>, out_dir: &Path) -> Result<ConversionOutcome> {
    let title = if parsed.title.is_empty() { doc_id.to_string() } else { parsed.title.clone() };
    let chunks = chunk::pack_chunks(&parsed.blocks);
    let notes_dir = out_dir.join("notes").join(doc_id);
    std::fs::create_dir_all(&notes_dir)?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let mut notes: Vec<NoteRecord> = Vec::new();
    let mut figure_rows: Vec<FigureRow> = Vec::new();

    // 文字 chunk 筆記（一 chunk 一檔；slug `<doc>/sNN`）。
    for (i, ch) in chunks.iter().enumerate() {
        let slug = format!("{doc_id}/s{i:02}");
        let body = chunk::text_body(&title, ch);
        let path = write_note(&notes_dir, &format!("s{i:02}"), &title_note_title(&title, &ch.section_label), &body, &today)?;
        notes.push(NoteRecord {
            slug,
            path,
            kind: NoteKind::Text,
            section_id: ch.section_id.clone(),
            figs: ch.figs.clone(),
            tables: ch.tables.clone(),
        });
    }

    // 圖片筆記（一圖一檔；slug `<doc>/figN-pP`）＋sidecar 列。
    for b in &parsed.blocks {
        if b.kind != BlockKind::Figure {
            continue;
        }
        let Some(fig) = b.fig else { continue };
        let page = b.page + 1;
        let fid = format!("fig{fig}-p{page}");
        let slug = format!("{doc_id}/{fid}");
        let body = chunk::figure_body(&title, fig, &b.section_id(), page, &b.text);
        let path = write_note(
            &notes_dir,
            &fid,
            &format!("{} · Figure {fig} (p{page})", title),
            &body,
            &today,
        )?;
        notes.push(NoteRecord {
            slug,
            path,
            kind: NoteKind::Figure,
            section_id: b.section_id(),
            figs: vec![fig],
            tables: Vec::new(),
        });
        // sidecar 列（K3 消費；vec 欄 P1 回填）。
        let (img_path, md5) = match (&b.image_source, image_root) {
            (Some(rel), Some(root)) => {
                let abs = root.join(rel);
                if abs.is_file() {
                    let digest = md5_file(&abs)?;
                    (Some(abs.to_string_lossy().into_owned()), Some(digest))
                } else {
                    (None, None)
                }
            }
            _ => (None, None),
        };
        figure_rows.push(FigureRow {
            doc_id: doc_id.to_string(),
            page,
            image_path: img_path,
            caption: b.text.clone(),
            section: b.section_id(),
            figure_no: Some(fig),
            image_md5: md5,
            source_id: None,
        });
    }

    // 設計邊界紀錄：無 Figure caption 的視覺裁圖（chart 面板等）不入文字流也不入
    // sidecar——structured 解析時已丟棄（避免虛構歸屬）；面板歸組屬 P2。P0 sidecar
    // 僅收「有 Figure caption」的列（含 table block 拾回的 caption-only 圖說）。

    let warnings = if parsed.is_full_document {
        Vec::new()
    } else {
        vec!["MinerU 報告 is_full_document=false——非全文解析（部分頁）".to_string()]
    };
    let report = ConversionReport {
        doc_id: doc_id.to_string(),
        title,
        route: RouteReport {
            policy: "external".into(),
            to_mineru: false,
            reason: String::new(),
            signals: Default::default(),
        },
        ladder: LadderReport {
            resolved: false,
            tier: None,
            program: None,
            endpoint: None,
            attempts: Vec::new(),
        },
        text_notes: chunks.len(),
        figure_notes: notes.len() - chunks.len(),
        figure_rows: figure_rows.len(),
        egress: Vec::new(),
        warnings,
    };

    // figures.sqlite（rusqlite bundled；K3 以 image_md5 去重回填 vec）。
    write_figures_sqlite(out_dir, &figure_rows)?;

    Ok(ConversionOutcome {
        report,
        notes,
        figure_rows,
    })
}

fn title_note_title(title: &str, section_label: &str) -> String {
    format!("{title} · {section_label}")
}

/// 寫一篇筆記（frontmatter 對齊實驗 corpus／gbrain schema pack 慣例）。
fn write_note(dir: &Path, stem: &str, fm_title: &str, body: &str, created_at: &str) -> Result<PathBuf> {
    let fm = crate::converters::frontmatter::build(&[
        ("origin", "operoid-employee".into()),
        ("produced_by", "mineru-converter".into()),
        ("created_at", created_at.into()),
        (
            "title",
            crate::converters::frontmatter::yaml_single_quote(fm_title),
        ),
    ]);
    let path = dir.join(format!("{stem}.md"));
    std::fs::write(&path, format!("{fm}\n{body}\n"))?;
    Ok(path)
}

/// figures.sqlite：每圖一列（vec BLOB 留 NULL——K3 以 caption＋原圖嵌入後回填）。
fn write_figures_sqlite(out_dir: &Path, rows: &[FigureRow]) -> Result<()> {
    let db = rusqlite::Connection::open(out_dir.join("figures.sqlite"))?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS figures (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             doc_id TEXT NOT NULL,
             page INTEGER NOT NULL,
             image_path TEXT,
             caption TEXT NOT NULL DEFAULT '',
             section TEXT NOT NULL DEFAULT '',
             figure_no INTEGER,
             image_md5 TEXT,
             source_id TEXT,
             vec BLOB
         );
         CREATE INDEX IF NOT EXISTS idx_figures_doc ON figures(doc_id);",
    )?;
    for r in rows {
        db.execute(
            "INSERT INTO figures (doc_id, page, image_path, caption, section, figure_no, image_md5, source_id, vec)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
            rusqlite::params![
                r.doc_id,
                r.page,
                r.image_path,
                r.caption,
                r.section,
                r.figure_no,
                r.image_md5,
                r.source_id
            ],
        )?;
    }
    Ok(())
}

fn md5_file(path: &Path) -> Result<String> {
    use md5::Digest;
    let bytes = std::fs::read(path)?;
    Ok(hex(&md5::Md5::digest(&bytes)))
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn route_report(route: &RouteDecision) -> RouteReport {
    RouteReport {
        policy: route.policy.as_str().into(),
        to_mineru: route.to_mineru,
        reason: route.reason.clone(),
        signals: route.signals.clone(),
    }
}

fn merge_ladder(report: &mut ConversionReport, outcome: &LadderOutcome, extra: &[String]) {
    report.ladder = LadderReport {
        resolved: outcome.resolved(),
        tier: outcome.tier.map(|t| t.as_str().to_string()),
        program: outcome.program.clone(),
        endpoint: outcome.endpoint.clone(),
        attempts: outcome.attempts.clone(),
    };
    if let (Some(tier), ep) = (outcome.tier, outcome.endpoint.as_deref()) {
        report.egress.push(EgressEvent {
            tier: tier.as_str().into(),
            endpoint: ep.map(|s| s.to_string()),
        });
    }
    report.warnings.extend(extra.iter().cloned());
    report.warnings.extend(outcome.warnings.iter().cloned());
}

fn write_report(out_dir: &Path, report: &ConversionReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(out_dir.join("conversion-report.json"), json)?;
    Ok(())
}

#[cfg(test)]
mod tests;
