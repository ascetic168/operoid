//! PDF 知識入庫閉環——把 K1～K4 的管線接成產品路徑：
//! `PDF →（K1 轉換）→ notes 入聯邦 repo（圖檔＋註解頁發布）→ git commit
//!  → gbrain sync --no-extract（圖片經 gbrain 原生 image import 取得多模態
//!  向量與 image_of 連結）`。
//!
//! 冪等性（重複入庫安全）：
//! - notes：同檔名覆寫（K1 產出檔名確定：`sNN.md`／`figN-pP.md`）。
//! - 圖檔發布：`影像檔：`／`原論文 PDF：` 行與匯入錨點冪等（已存在不重寫）；
//!   同名圖檔已存在則略過。
//!
//! 降級紀律：sync 失敗只記警告不讓入庫失敗——資料已落盤，同步事後可補跑
//! （圖檔發布後的第二次 sync 為 image_of 連結保險，冪等 best-effort）。

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::converters::mineru::{self, ConvertConfig};

/// 入庫報告。
#[derive(Debug, Clone, Serialize)]
pub struct IngestReport {
    pub doc_id: String,
    /// 拷貝進 notes repo 的筆記檔數（含發布的圖檔）。
    pub notes_written: usize,
    /// 本輪發布的圖檔數（gbrain 原生 image import 軌道）。
    pub figures_published: usize,
    /// gbrain sync 是否成功（false＝notes 已入庫、同步待事後補跑）。
    pub synced: bool,
    pub warnings: Vec<String>,
}

/// 反斜線字元（避免在字面中書寫）。
fn chr_backslash() -> char {
    char::from_u32(0x5C).unwrap()
}

/// `gbrain sources list --json` → `(id, local_path)` 列（解析失敗＝Err）。
pub async fn list_sources(gbrain_exe: &str, home: Option<&str>) -> Result<Vec<(String, String)>> {
    let env = crate::proc::env_for_brain(home);
    let (code, out, err) = crate::gbrain_cli::run_capture(
        gbrain_exe,
        &["sources", "list", "--json"],
        &env,
    )
    .await
    .context("sources list spawn 失敗")?;
    if code != 0 {
        return Err(anyhow::anyhow!("sources list 失敗：{}", err.trim()));
    }
    let span = crate::knowledge::fusion::find_json_array(&out)
        .context("sources list 非 JSON 陣列")?;
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(span).context("sources list 解析失敗")?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let id = r["id"].as_str()?.to_string();
            let lp = r["local_path"].as_str()?.to_string();
            Some((id, lp))
        })
        .collect())
}

/// 解析 notes repo 對應的已註冊 source id（K4 授權歸檔用）——比對
/// `gbrain sources list --json` 的 `local_path`（分隔符／大小寫寬容比對）。
pub async fn resolve_source_id(
    gbrain_exe: &str,
    home: Option<&str>,
    notes_repo: &Path,
) -> Result<String> {
    let rows = list_sources(gbrain_exe, home).await?;
    let norm = |s: &str| s.replace(chr_backslash(), "/").trim_end_matches('/').to_ascii_lowercase();
    let want = norm(&notes_repo.to_string_lossy());
    for (id, lp) in &rows {
        if norm(lp) == want {
            return Ok(id.clone());
        }
    }
    Err(anyhow::anyhow!(
        "notes repo 未註冊為 source（{}）——請先 gbrain sources add <id> --path {}",
        notes_repo.display(),
        notes_repo.display()
    ))
}

/// doc_id → 來源 PDF 路徑（W2 sidecar 退役後的出處鏈）：讀 doc 目錄頁 frontmatter
/// 的 `source_pdf`（K1 轉換時寫入）。根集合＝gbrain 登記來源的 local_path
/// （best-effort：CLI 失敗／doc 不在任何來源 repo → 該 doc 不進表，與 sidecar
/// 缺檔同特性——`meta.source_docs` 留空即可）。
pub async fn collect_source_docs(
    gbrain_exe: &str,
    gbrain_home: Option<&str>,
    docs: &[String],
) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    if docs.is_empty() {
        return out;
    }
    let Ok(sources) = list_sources(gbrain_exe, gbrain_home).await else {
        return out;
    };
    for doc in docs {
        for (_, lp) in &sources {
            let dir = Path::new(lp).join(doc);
            if !dir.is_dir() {
                continue;
            }
            if let Some(pdf) = read_doc_source_pdf(&dir) {
                out.insert(doc.clone(), serde_json::json!(pdf));
            }
            break;
        }
    }
    out
}

/// 讀 doc 目錄頁 frontmatter 的 `source_pdf`（依檔名排序掃描，找到即回）。
pub fn read_doc_source_pdf(doc_dir: &Path) -> Option<String> {
    let mut pages: Vec<std::path::PathBuf> = std::fs::read_dir(doc_dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("md"))
        .collect();
    pages.sort();
    for p in pages {
        let text = std::fs::read_to_string(&p).ok()?;
        let (fm, _) = crate::converters::frontmatter::split(&text);
        if let Some(v) = crate::converters::frontmatter::get(fm, "source_pdf") {
            return Some(v);
        }
    }
    None
}

/// PDF → 知識庫入庫閉環：
/// `PDF →（K1 轉換）→ notes 拷貝進聯邦 repo（圖檔＋註解頁發布）→ git commit
///  → gbrain sync --no-extract（圖檔發布後補第二次 sync＝image_of 保險）`。
///
/// 冪等：重複入庫安全（notes 覆寫、發布行/錨點/圖檔冪等）。
/// 降級：sync 失敗只記警告不讓入庫失敗——資料已落盤，同步事後可補。
///
/// - `notes_repo`：既有工廠 notes repo（`source_id` 須已 `gbrain sources add`
///   指向此 repo——入庫只 commit＋sync，不代註冊）。
pub async fn ingest_pdf(
    pdf: &Path,
    cfg: &ConvertConfig,
    notes_repo: &Path,
    source_id: &str,
    gbrain_exe: &str,
    gbrain_home: Option<&str>,
) -> Result<IngestReport> {
    // 1) K1 轉換（工作目錄暫存；結束時清除）。
    let work = std::env::temp_dir().join(format!(
        "ingest-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&work)?;
    let outcome = mineru::convert(pdf, &work, cfg)
        .await
        .context("K1 轉換失敗")?;
    let doc_id = outcome.report.doc_id.clone();

    // 2) notes 拷貝進聯邦 repo（冪等覆寫）＋圖檔發布（gbrain 原生 image import）。
    let src_notes = work.join("notes").join(&doc_id);
    let dst_notes = notes_repo.join(&doc_id);
    let mut notes_written = 0usize;
    let mut figures_published = 0usize;
    let mut publish_warnings: Vec<String> = Vec::new();
    if src_notes.exists() {
        std::fs::create_dir_all(&dst_notes)?;
        for entry in std::fs::read_dir(&src_notes)?.flatten() {
            let p = entry.path();
            if p.is_file() {
                let name = entry.file_name();
                std::fs::copy(&p, dst_notes.join(&name))?;
                notes_written += 1;
            }
        }
        let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
        let (_enriched, imgs, errors) =
            crate::factories::publish_pdf_figures(&dst_notes, &outcome.figure_rows, &pdf.to_string_lossy(), &today);
        figures_published = imgs.len();
        notes_written += imgs.len();
        for e in errors {
            publish_warnings.push(format!("圖檔發布失敗（略過）：{}：{}", e.file, e.detail));
        }
        // K8：發布後背景補圖說（best-effort——egress 閘門照舊，失敗只記 log）。
        crate::knowledge::enrich::spawn_after_publish(
            gbrain_home.map(str::to_string),
            cfg.allow_public_egress,
            notes_repo.to_path_buf(),
            doc_id.clone(),
        );
    }

    // 3) git commit＋gbrain sync --no-extract（best-effort——失敗記警告）。
    let mut warnings = publish_warnings;
    let mut synced = false;
    let env = crate::proc::env_for_brain(gbrain_home);
    let sink = crate::gbrain_cli::noop_sink();
    if let Err(e) = crate::gbrain_cli::git_add_commit(&sink, notes_repo).await {
        warnings.push(format!("notes repo git commit 失敗：{e}"));
    }
    let sync_args = [
        "sync",
        "--source",
        source_id,
        "--no-pull",
        "--yes",
        "--no-hard-deadline",
        "--no-extract",
    ];
    match crate::gbrain_cli::run_capture(gbrain_exe, &sync_args, &env).await {
        Ok((code, _, _err)) if code == 0 => synced = true,
        Ok((code, _, err)) => warnings.push(format!(
            "gbrain sync --source {source_id} 失敗（exit {code}）：{}——notes 已入庫，請確認 source 已註冊後補跑同步",
            err.trim().chars().take(200).collect::<String>()
        )),
        Err(e) => warnings.push(format!("gbrain sync spawn 失敗：{e}")),
    }
    if synced && figures_published > 0 {
        // image_of 保險：首輪 sync 若「圖先於註解頁」處理會靜默跳過連結
        // （TODO 項目 3）——此時 .md 已在磁碟，補一輪冪等 sync 完成連結。
        if let Ok((code, _, err)) = crate::gbrain_cli::run_capture(gbrain_exe, &sync_args, &env).await {
            if code != 0 {
                warnings.push(format!("gbrain sync（image_of 保險輪）失敗（exit {code}）：{err}"));
            }
        }
    }
    if synced {
        // sync 會defer大批次嵌入——embed --stale 冪等補齊（gbrain 文字面的向量）。
        if let Ok((code, _, err)) = crate::gbrain_cli::run_capture(
            gbrain_exe,
            &["embed", "--stale"],
            &env,
        )
        .await
        {
            if code != 0 {
                warnings.push(format!("gbrain embed --stale 失敗（exit {code}）：{err}"));
            }
        }
    }

    std::fs::remove_dir_all(&work).ok();

    Ok(IngestReport {
        doc_id,
        notes_written,
        figures_published,
        synced,
        warnings,
    })
}
