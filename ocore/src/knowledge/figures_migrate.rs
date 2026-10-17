//! figures sidecar 退役遷移（2026-10-17 試點裁決，TODO 項目 3）：把 sidecar 的
//! **已授權**圖列搬進其 source repo（gbrain 原生 image import 軌道：同名圖檔＋
//! 註解頁），之後 sidecar 只封存不刪除。
//!
//! 授權語意（fail-closed 保留）：`source_id` 未歸檔的列**不遷**——它們從未對
//! 任何授權集可見，搬進 repo 等於授權外流；未遷列計入報告 `figures_skipped_unauthorized`。
//! 圖檔來源是 sidecar 生產庫（`figures.assets/<doc>/<md5>.<ext>`，即列上的
//! `image_path`）；註解頁缺漏時由列 metadata 重建（caption／section／頁號），
//! 既有 K8 補圖說（`[AI 生成圖說]`）已寫回 caption 欄，隨遷移保留。
//!
//! 封存：`figures.sqlite` → `figures.sqlite.retired-<date>`（wal/shm 一併）；
//! `figures.assets/` 原地保留，驗證後由使用者手動刪。遷移後查詢端由
//! `sidecar_config` 的存在性檢查導向圖 arm（service route 2）。

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use super::figures::Sidecar;

/// 退役遷移報告。
#[derive(Debug, Clone, Serialize)]
pub struct MigrationReport {
    pub sidecar_db: String,
    /// sidecar 內總列數（含未授權）。
    pub figures_total: usize,
    /// 已搬進 repo 的圖檔數。
    pub figures_migrated: usize,
    /// 未授權（`source_id` 空）而略過的列數（留在封存檔，不可見語意不變）。
    pub figures_skipped_unauthorized: usize,
    /// 發布失敗的列數（明細在 warnings）。
    pub figures_failed: usize,
    pub docs_total: usize,
    pub docs_migrated: usize,
    /// 逐 source repo 的遷移摘要。
    pub repos: Vec<RepoMigration>,
    /// 封存位置（None＝無 db 可封存或改失敗——見 warnings）。
    pub archived_to: Option<String>,
    pub warnings: Vec<String>,
}

/// 單一 source repo 的遷移摘要。
#[derive(Debug, Clone, Serialize)]
pub struct RepoMigration {
    pub source_id: String,
    pub repo: String,
    pub docs: Vec<String>,
}

/// 遷移檔案面（無 gbrain 依賴——可單測）：掃 sidecar → 逐授權 doc 把圖檔＋
/// 註解頁發布進 source repo。回傳報告（`archived_to` 恆 None——封存由
/// [`migrate_figures_sidecar`] 統一做，測試可單獨驗證檔案面）。
pub fn migrate_figures_sidecar_files(
    figures_db: &Path,
    sources: &[(String, String)],
    date: &str,
) -> Result<MigrationReport> {
    let sidecar = Sidecar::open_existing(figures_db)
        .context("figures sidecar 不存在——無可遷移資料")?;
    let mut report = MigrationReport {
        sidecar_db: figures_db.to_string_lossy().into_owned(),
        figures_total: 0,
        figures_migrated: 0,
        figures_skipped_unauthorized: 0,
        figures_failed: 0,
        docs_total: 0,
        docs_migrated: 0,
        repos: Vec::new(),
        archived_to: None,
        warnings: Vec::new(),
    };

    let docs = sidecar.authorized_docs()?;
    let by_source: BTreeMap<&str, Vec<&str>> = docs.iter().fold(BTreeMap::new(), |mut m, (d, s)| {
        m.entry(s.as_str()).or_default().push(d.as_str());
        m
    });

    for (source_id, doc_ids) in &by_source {
        let Some((_, repo)) = sources.iter().find(|(id, _)| id == source_id) else {
            report
                .warnings
                .push(format!("source {source_id} 不在 sources 清單——{} 文件略過", doc_ids.len()));
            continue;
        };
        let mut migrated_docs = Vec::new();
        for doc_id in doc_ids {
            report.docs_total += 1;
            let rows = sidecar.rows_for_doc(doc_id)?;
            report.figures_total += rows.len();
            let authorized: usize = rows.iter().filter(|r| r.source_id.as_deref().is_some_and(|s| !s.is_empty())).count();
            report.figures_skipped_unauthorized += rows.len() - authorized;
            let usable: Vec<_> = rows
                .into_iter()
                .filter(|r| r.source_id.as_deref().is_some_and(|s| !s.is_empty()))
                .collect();
            if usable.is_empty() {
                continue;
            }
            let doc_dir = Path::new(repo).join(doc_id);
            // 註解頁缺漏時由列 metadata 重建（publish_pdf_figures 需既有頁）。
            for r in &usable {
                let Some(fig) = r.figure_no else { continue };
                let stem = format!("fig{fig}-p{}", r.page);
                let md_path = doc_dir.join(format!("{stem}.md"));
                if !md_path.is_file() {
                    let _ = std::fs::create_dir_all(&doc_dir);
                    let title = format!("{} · Figure {fig} (p{})", doc_id, r.page);
                    let body = format!(
                        "title: {doc_id} | text: [Figure {fig} in Section {}, page {}] Figure {fig}. {}",
                        r.section, r.page, r.caption
                    );
                    std::fs::write(
                        &md_path,
                        format!(
                            "{}\n{body}\n",
                            crate::converters::frontmatter::build(&[
                                ("origin", "operoid-employee".into()),
                                ("produced_by", "figures-migration".into()),
                                ("created_at", date.into()),
                                ("title", crate::converters::frontmatter::yaml_single_quote(&title)),
                            ])
                        ),
                    )?;
                }
            }
            let pdf_path = sidecar
                .pdf_paths_for_docs(std::slice::from_ref(&doc_id.to_string()))
                .ok()
                .and_then(|ps| {
                    ps.iter()
                        .find(|(d, _)| d == doc_id)
                        .map(|(_, p)| p.clone())
                })
                .unwrap_or_default();
            let (_enriched, imgs, errors) =
                crate::factories::publish_pdf_figures(&doc_dir, &usable, &pdf_path, date);
            report.figures_migrated += imgs.len();
            report.figures_failed += errors.len();
            for e in errors {
                report.warnings.push(format!("{doc_id}: {}：{}", e.file, e.detail));
            }
            migrated_docs.push(doc_id.to_string());
        }
        if !migrated_docs.is_empty() {
            report.docs_migrated += migrated_docs.len();
            report.repos.push(RepoMigration {
                source_id: source_id.to_string(),
                repo: repo.clone(),
                docs: migrated_docs,
            });
        }
    }
    Ok(report)
}

/// 完整遷移：檔案面 → 逐 repo git commit → 逐 source sync（×2，image_of 保險）
/// → `embed --stale` → 封存 sidecar。git/sync 失敗只記 warnings——檔案已落盤。
pub async fn migrate_figures_sidecar(
    figures_db: &Path,
    sources: &[(String, String)],
    gbrain_exe: &str,
    gbrain_home: Option<&str>,
) -> Result<MigrationReport> {
    let date = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
    let mut report = migrate_figures_sidecar_files(figures_db, sources, &date)?;

    let env = crate::proc::env_for_brain(gbrain_home);
    let sink = crate::gbrain_cli::noop_sink();
    let mut repos: Vec<&str> = report.repos.iter().map(|r| r.repo.as_str()).collect();
    repos.sort();
    repos.dedup();
    for repo in &repos {
        if let Err(e) = crate::gbrain_cli::git_add_commit(&sink, Path::new(repo)).await {
            report.warnings.push(format!("repo {repo} git commit 失敗：{e}"));
        }
    }
    let sync_args = [
        "sync",
        "--source",
        "", // 逐 source 代換
        "--no-pull",
        "--yes",
        "--no-hard-deadline",
        "--no-extract",
    ];
    for rm in &report.repos {
        let mut args = sync_args.to_vec();
        args[2] = rm.source_id.as_str();
        for round in 1..=2 {
            match crate::gbrain_cli::run_capture(gbrain_exe, &args, &env).await {
                Ok((0, _, _)) => {}
                Ok((code, _, err)) => {
                    report.warnings.push(format!(
                        "gbrain sync --source {}（第 {round} 輪）失敗（exit {code}）：{}",
                        rm.source_id,
                        err.trim().chars().take(160).collect::<String>()
                    ));
                    break;
                }
                Err(e) => {
                    report.warnings.push(format!("gbrain sync spawn 失敗：{e}"));
                    break;
                }
            }
        }
    }
    if !report.repos.is_empty() {
        if let Ok((code, _, err)) = crate::gbrain_cli::run_capture(gbrain_exe, &["embed", "--stale"], &env).await {
            if code != 0 {
                report.warnings.push(format!("gbrain embed --stale 失敗（exit {code}）：{err}"));
            }
        }
    }

    // 封存：db（＋wal/shm）改名 .retired-<date>；figures.assets 原地保留。
    match archive_sidecar(figures_db, &date) {
        Ok(Some(archived)) => report.archived_to = Some(archived),
        Ok(None) => {}
        Err(e) => report
            .warnings
            .push(format!("sidecar 封存失敗（保留原檔，請手動改名）：{e}")),
    }
    Ok(report)
}

/// 封存：`figures.sqlite` → `figures.sqlite.retired-<date>`（wal/shm 一併改名）。
/// 回傳封存路徑（None＝db 不存在）。`figures.assets/` 不動（驗證後手動刪）。
pub fn archive_sidecar(figures_db: &Path, date: &str) -> Result<Option<String>> {
    if !figures_db.is_file() {
        return Ok(None);
    }
    let archive = |suffix: &str| -> Result<String> {
        let p = figures_db.to_string_lossy().into_owned();
        let target = format!("{p}.retired-{date}{suffix}");
        std::fs::rename(format!("{p}{suffix}"), &target)
            .with_context(|| format!("rename {} → {target}", figures_db.display()))?;
        Ok(target)
    };
    let main = archive("")?;
    for suffix in ["-wal", "-shm"] {
        let p = format!("{}{suffix}", figures_db.display());
        if Path::new(&p).is_file() {
            let _ = archive(suffix);
        }
    }
    Ok(Some(main))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::mineru::FigureRow;

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "operoid-figmigrate-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn row(
        doc: &str,
        fig: u32,
        page: i64,
        image_path: Option<String>,
        source_id: Option<&str>,
    ) -> FigureRow {
        FigureRow {
            doc_id: doc.into(),
            page,
            image_path,
            caption: "Loss breakdown of the modelled converter".into(),
            section: "V.B".into(),
            figure_no: Some(fig),
            image_md5: None,
            source_id: source_id.map(Into::into),
        }
    }

    #[test]
    fn migration_moves_assets_into_repo_and_skips_unauthorized() {
        let dir = tmpdir("files");
        let db = dir.join("figures.sqlite");
        let repo = dir.join("repo");
        std::fs::create_dir_all(&repo).unwrap();

        // 已授權列：asset 檔存在（sidecar 生產庫）；既有筆記頁（legacy repo 形態）。
        let asset_dir = dir.join("figures.assets").join("paper2020");
        std::fs::create_dir_all(&asset_dir).unwrap();
        let asset = asset_dir.join("abc123.jpg");
        std::fs::write(&asset, b"jpeg-bytes").unwrap();
        // 未授權列：無 asset、無頁（從未入 repo——不得因遷移而出現）。
        let sc = Sidecar::open(&db).unwrap();
        sc.insert_row(&row("paper2020", 2, 3, Some(asset.to_string_lossy().into_owned()), Some("k7")), None)
            .unwrap();
        sc.insert_row(&row("paper2020", 4, 5, None, None), None).unwrap();
        drop(sc);
        let doc_dir = repo.join("paper2020");
        std::fs::create_dir_all(&doc_dir).unwrap();
        std::fs::write(
            doc_dir.join("fig2-p3.md"),
            "---\norigin: operoid-employee\nproduced_by: mineru-converter\ncreated_at: 2025-01-01\ntitle: 'paper2020 · Figure 2 (p3)'\n---\n\ntitle: paper2020 | text: [Figure 2 in Section V.B, page 3] Figure 2. Loss breakdown.\n",
        )
        .unwrap();

        let report =
            migrate_figures_sidecar_files(&db, &[("k7".into(), repo.to_string_lossy().into_owned())], "2026-10-17")
                .unwrap();
        assert_eq!(report.figures_migrated, 1, "warnings={:?}", report.warnings);
        assert_eq!(report.figures_skipped_unauthorized, 1);
        assert_eq!(report.docs_migrated, 1);
        assert_eq!(report.repos.len(), 1);
        assert_eq!(report.repos[0].source_id, "k7");
        assert!(doc_dir.join("fig2-p3.jpg").exists(), "asset 應搬進 repo");
        let md = std::fs::read_to_string(doc_dir.join("fig2-p3.md")).unwrap();
        assert!(md.contains("影像檔：[[paper2020/fig2-p3.jpg"));
        assert!(md.contains("— 匯入：Figure 2（p3）："));
        assert!(!doc_dir.join("fig4-p5.md").exists(), "未授權列不得入 repo");

        // 封存：db 改名 .retired-<date>；open_existing 之後為 None（查詢端降級）。
        let archived = archive_sidecar(&db, "2026-10-17").unwrap();
        assert_eq!(
            archived.as_deref(),
            Some(dir.join("figures.sqlite.retired-2026-10-17").to_string_lossy().as_ref())
        );
        assert!(!db.exists());
        assert!(dir.join("figures.sqlite.retired-2026-10-17").exists());
        assert!(Sidecar::open_existing(&db).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migration_rebuilds_missing_annotation_page_from_row_metadata() {
        let dir = tmpdir("rebuild");
        let db = dir.join("figures.sqlite");
        let repo = dir.join("repo");
        let asset_dir = dir.join("figures.assets").join("docX");
        std::fs::create_dir_all(&asset_dir).unwrap();
        let asset = asset_dir.join("deadbeef.png");
        std::fs::write(&asset, b"png").unwrap();
        {
            let sc = Sidecar::open(&db).unwrap();
            sc.tag_doc_source("docX", r"C:\papers\docX.pdf").unwrap();
            sc.insert_row(&row("docX", 7, 12, Some(asset.to_string_lossy().into_owned()), Some("s1")), None)
                .unwrap();
        }
        let report = migrate_figures_sidecar_files(
            &db,
            &[("s1".into(), repo.to_string_lossy().into_owned())],
            "2026-10-17",
        )
        .unwrap();
        assert_eq!(report.figures_migrated, 1, "warnings={:?}", report.warnings);
        let md = std::fs::read_to_string(repo.join("docX").join("fig7-p12.md")).unwrap();
        assert!(md.contains("title: docX | text: [Figure 7 in Section V.B, page 12] Figure 7."));
        assert!(md.contains(r"原論文 PDF：C:\papers\docX.pdf"), "figure_docs 出處應隨遷移");
        assert!(repo.join("docX").join("fig7-p12.png").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// 本機退役遷移 runner（#[ignore]；非破壞——sidecar 封存 `.retired-<date>`，
/// 圖檔已存在則略過、註解頁補強為冪等）。
/// 跑法：`MIGRATE_FIGURES_DB=<figures.sqlite> cargo test -p ocore real_migrate_local -- --ignored --nocapture`
/// （未設 env 即無 op 返回——CI 安全。）
#[ignore = "真實遷移：需 MIGRATE_FIGURES_DB 指向生產 sidecar；會 commit/sync 目標 repo 並封存 sidecar"]
#[tokio::test]
async fn real_migrate_local_sidecar() {
    let Ok(db) = std::env::var("MIGRATE_FIGURES_DB") else {
        eprintln!("[migrate] MIGRATE_FIGURES_DB 未設——略過");
        return;
    };
    let gbrain_exe = dirs::home_dir()
        .map(|h| h.join(".bun").join("bin").join("gbrain.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from("gbrain"));
    let exe_s = gbrain_exe.to_string_lossy().into_owned();
    let sources = super::ingest::list_sources(&exe_s, None)
        .await
        .expect("sources list 應成功");
    eprintln!("[migrate] sources: {sources:?}");
    let report = migrate_figures_sidecar(std::path::Path::new(&db), &sources, &exe_s, None)
        .await
        .expect("遷移應成功");
    eprintln!(
        "[migrate] {}",
        serde_json::to_string_pretty(&report).expect("report 序列化")
    );
}
