//! PDF 知識入庫閉環——把 K1～K4 的管線接成產品路徑：
//! `PDF →（K1 轉換）→ notes 入聯邦 repo → git commit → gbrain sync --no-extract
//!  → sidecar 合併（冪等）→ 向量回填（best-effort）`。
//!
//! 冪等性（重複入庫安全）：
//! - notes：同檔名覆寫（K1 產出檔名確定：`sNN.md`／`figN-pP.md`）。
//! - sidecar：**doc 級重建**——先刪目標庫中同 `doc_id` 的列再插入，插入時以
//!   `image_md5` 從舊列**攜帶既有向量**（重轉換不重嵌；升級 mmproj 只補新列）。
//!
//! 降級紀律（同 K4）：sync 失敗／嵌入不可達都只記警告，不讓入庫失敗——
//! notes 與 sidecar 資料已落盤，同步可事後補跑。

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use super::figures::{self, Sidecar, SidecarMode};
use crate::converters::mineru::{self, ConvertConfig};

/// 入庫報告。
#[derive(Debug, Clone, Serialize)]
pub struct IngestReport {
    pub doc_id: String,
    /// 拷貝進 notes repo 的筆記檔數。
    pub notes_written: usize,
    /// 合併後該文件在生產 sidecar 的列數。
    pub figure_rows: usize,
    /// 攜帶既有向量的列數（重複入庫時 >0＝冪等生效）。
    pub vectors_carried: usize,
    /// 本輪新嵌入的列數（回填 best-effort；失敗記 warnings）。
    pub vectors_embedded: usize,
    /// gbrain sync 是否成功（false＝notes 已入庫、同步待事後補跑）。
    pub synced: bool,
    pub warnings: Vec<String>,
}

/// sidecar 合併：`from`（轉換產出）→ `to`（生產庫），doc 級冪等＋md5 攜帶向量。
/// 回傳（該文件合併後列數、攜帶向量數）。
pub fn merge_sidecar(
    from: &Path,
    to: &Path,
    doc_id: &str,
    source_id: &str,
) -> Result<(usize, usize)> {
    Sidecar::open(to)?.merge_from(from, doc_id, source_id)
}

/// PDF → 知識庫入庫閉環：
/// `PDF →（K1 轉換）→ notes 拷貝進聯邦 repo → sidecar doc 級合併 → git commit
///  → gbrain sync --no-extract → 向量回填（best-effort）`。
///
/// 冪等：重複入庫安全（notes 覆寫、sidecar doc 級重建＋md5 攜帶向量）。
/// 降級：sync 失敗／嵌入不可達只記警告不讓入庫失敗——資料已落盤，同步事後可補。
///
/// - `figures_db`：生產 sidecar 路徑（不存在則建立）。
/// - `notes_repo`：既有工廠 notes repo（`source_id` 須已 `gbrain sources add`
///   指向此 repo——入庫只 commit＋sync，不代註冊）。
#[allow(clippy::too_many_arguments)]
pub async fn ingest_pdf(
    pdf: &Path,
    cfg: &ConvertConfig,
    figures_db: &Path,
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

    // 2) notes 拷貝進聯邦 repo（冪等覆寫）。
    let src_notes = work.join("notes").join(&doc_id);
    let dst_notes = notes_repo.join(&doc_id);
    let mut notes_written = 0usize;
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
    }

    // 3) sidecar 合併（doc 級冪等＋md5 攜帶向量）。
    if let Some(parent) = figures_db.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let work_db = work.join("figures.sqlite");
    let (figure_rows, vectors_carried) = if work_db.exists() {
        merge_sidecar(&work_db, figures_db, &doc_id, source_id)?
    } else {
        (0, 0)
    };

    // 4) git commit＋gbrain sync --no-extract（best-effort——失敗記警告）。
    let mut warnings = Vec::new();
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
        Ok((code, _, err)) if code == 0 => synced = true,
        Ok((code, _, err)) => warnings.push(format!(
            "gbrain sync --source {source_id} 失敗（exit {code}）：{}——notes 已入庫，請確認 source 已註冊後補跑同步",
            err.trim().chars().take(200).collect::<String>()
        )),
        Err(e) => warnings.push(format!("gbrain sync spawn 失敗：{e}")),
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

    // 5) 向量回填（best-effort；失敗只記警告——事後重跑即可）。
    let stats = match figures::embed_pending(
        figures_db,
        figures::DEFAULT_EMBEDDING_BASE,
        None,
        SidecarMode::Multimodal,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            warnings.push(format!("向量回填失敗（略過；事後重跑 embed_pending 即可）：{e}"));
            figures::EmbedStats {
                mode: SidecarMode::Multimodal,
                embedded: 0,
                deduped: 0,
                skipped: 0,
                warnings: vec![e.to_string()],
            }
        }
    };

    std::fs::remove_dir_all(&work).ok();

    Ok(IngestReport {
        doc_id,
        notes_written,
        figure_rows,
        vectors_carried,
        vectors_embedded: stats.embedded + stats.deduped,
        synced,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        doc: &str,
        page: i64,
        fig: u32,
        caption: &str,
        md5: Option<&str>,
        source: Option<&str>,
    ) -> crate::converters::mineru::FigureRow {
        crate::converters::mineru::FigureRow {
            doc_id: doc.into(),
            page,
            image_path: None,
            caption: caption.into(),
            section: "I".into(),
            figure_no: Some(fig),
            image_md5: md5.map(|s| s.into()),
            source_id: source.map(|s| s.into()),
        }
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ingest-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 合併：doc 級冪等（重複合併列數不變）＋md5 攜帶向量＋source_id 歸檔。
    #[test]
    fn merge_sidecar_is_idempotent_and_carries_vectors() {
        let dir = temp_dir("merge");
        let from = dir.join("from.sqlite");
        let to = dir.join("to.sqlite");
        {
            let s = Sidecar::open(&from).unwrap();
            s.insert_row(
                &row("doc-a", 1, 1, "A chart.", Some("md5-a1"), None),
                None,
            )
            .unwrap();
        }
        {
            let s = Sidecar::open(&to).unwrap();
            // 舊列帶向量（模擬已回填）＋同文件雜列（應被 doc 級重建清掉）。
            s.insert_row(
                &row("doc-a", 1, 1, "OLD A chart.", Some("md5-a1"), Some("old-src")),
                Some(&[1.0, 0.0]),
            )
            .unwrap();
            s.insert_row(
                &row("doc-a", 9, 9, "stale row", Some("md5-stale"), Some("old-src")),
                Some(&[0.5]),
            )
            .unwrap();
            s.insert_row(&row("doc-b", 1, 1, "Other doc.", None, Some("k7")), None)
            .unwrap();
        }
        let (rows, carried) = merge_sidecar(&from, &to, "doc-a", "k7").unwrap();
        assert_eq!(rows, 1, "doc 級重建後只剩來源的列");
        assert_eq!(carried, 1, "md5 相同 → 攜帶舊向量");

        let s = Sidecar::open(&to).unwrap();
        let (rows, with_vec) = s.doc_stats("doc-a").unwrap();
        assert_eq!(rows, 1, "同文件雜列被清除");
        assert_eq!(with_vec, 1, "向量應被攜帶");
        let hits = s.figures_for_docs(&["doc-a".into()], None, 10).unwrap();
        assert_eq!(hits[0].source_id.as_deref(), Some("k7"), "歸檔新 source");
        assert_eq!(hits[0].figure_no, Some(1));
        // 他文件不受影響。
        assert_eq!(s.doc_stats("doc-b").unwrap(), (1, 0));
        std::fs::remove_dir_all(&dir).ok();
    }
}
