//! K7 端到端評測（#[ignore] real test——比照 `knowledge/tests_real.rs` 慣例，手動跑）。
//!
//! 跑法：`cargo test -p ocore real_k7 -- --ignored --nocapture`
//!
//! 全鏈：mueller2016.pdf → K1 `convert()`（真實分流＋MinerU spawn）→ 筆記 →
//! 臨時 gbrain 腦（init/sources add/sync `--no-extract`/embed）→ 12 查詢（K2 前綴＋
//! `--no-expand`）→ hit@5／MRR 對照實驗 ground truth。
//!
//! **P0 驗收線**：hit@5 **12/12**、MRR **≥ 0.90**（實驗基準 0.917；v4 語料原型 0.958）。
//! 語料：`pdf2zh/mueller2016.pdf`＋MinerU 取用（絕對路徑 venv launcher）；
//! 三文件歸因測試屬第五輪實驗證據（歸因錯誤 0），P1 sidecar 落地時一併移植。

use std::path::{Path, PathBuf};

use crate::converters::mineru::{self, ConvertConfig};
use crate::gbrain_cli::run_capture;
use crate::proc::env_for_brain;

/// 12 查詢＋ground truth（移植自實驗 `run_experiment.py` 的 QUERIES——逐字）。
/// "fig:N" 對 chunk 的 figs、"table:X" 對 tables、"sec:X" 對 section 前綴。
fn queries() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "How does the two-chip system-in-package IVR solution work? A buck plus an LDO.",
            vec!["fig:1", "sec:I"],
        ),
        (
            "Four-phase buck converter architecture with PWM controller and error amplifier",
            vec!["fig:2", "sec:I", "sec:III.A"],
        ),
        (
            "Magnetic field distribution in the cross section of the six-winding inductor",
            vec!["fig:5"],
        ),
        (
            "Equivalent circuit pi-model used to extract inductance and resistance from S-parameters",
            vec!["fig:6", "sec:IV.B"],
        ),
        (
            "Extracted effective inductance and resistance versus frequency of the modelled inductor",
            vec!["fig:7"],
        ),
        (
            "Efficiency versus number of phases for the 5V:1V, 3V:1V and 1.7V:1.05V conversions",
            vec!["fig:8", "table:II", "sec:V.B"],
        ),
        (
            "Loss breakdown of power loss, FET losses dominate",
            vec!["fig:9", "sec:V.B"],
        ),
        (
            "Comparison with commercial Coilcraft SMT inductors, size and DC resistance",
            vec!["table:IV", "sec:VI.A"],
        ),
        (
            "Peak efficiency of 91.1 percent for the 1.7V to 1.05V conversion",
            vec!["table:II", "table:V", "sec:VI.B"],
        ),
        (
            "Equation for the conduction loss of the power switches",
            vec!["sec:III.B"],
        ),
        (
            "為什麼選擇螺線管式 solenoidal 電感，而不是 spiral 螺旋形電感？",
            vec!["fig:4", "sec:IV.A"],
        ),
        (
            "Area reduction of the embedded magnetic core inductor compared to air core",
            vec!["table:III", "sec:VI.A", "sec:I"],
        ),
    ]
}

/// 評測歸因：筆記屬性（slug → figs/tables/section）——對應實驗 note-mapping.json。
fn matches(attrs: &(Vec<u32>, Vec<String>, String), target: &str) -> bool {
    let (figs, tables, section) = attrs;
    let Some((kind, val)) = target.split_once(':') else {
        return false;
    };
    match kind {
        "fig" => figs.iter().any(|f| f.to_string() == val),
        "table" => tables.iter().any(|t| t == val),
        "sec" => section == val || section.starts_with(&format!("{val}.")),
        _ => false,
    }
}

#[ignore = "真實環境相依：需 mueller2016 語料、MinerU venv、本機 gbrain＋llama-server；手動跑"]
#[tokio::test]
async fn real_k7_end_to_end_mueller2016() {
    // ── 語料與工具存在性（缺即明確指引） ─────────────────────────────────
    let pdf = PathBuf::from(r"C:\Users\charl\Documents\python\pdf2zh\mueller2016.pdf");
    let mineru_exe = PathBuf::from(r"C:\Users\charl\Documents\python\.mineru\Scripts\mineru-kit.exe");
    let gbrain_exe = dirs::home_dir()
        .map(|h| h.join(".bun").join("bin").join("gbrain.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("gbrain"));
    for (p, hint) in [
        (&pdf, "獨立語料：C:\\Users\\charl\\Documents\\python\\pdf2zh\\mueller2016.pdf"),
        (&mineru_exe, "MinerU venv launcher（階梯 1 設定覆寫用）"),
    ] {
        assert!(p.exists(), "缺少語料/工具：{}（{hint}）", p.display());
    }

    // ── K1 端到端：真實分流（S3 應命中）＋ MinerU spawn ───────────────────
    let dir = temp_dir("k7");
    let out = dir.join("out");
    let mut cfg = ConvertConfig {
        mineru_command: Some(mineru_exe.to_string_lossy().into_owned()),
        ..Default::default()
    };
    cfg.policy = crate::converters::mineru::router::ConvertPolicy::Auto;
    eprintln!("[k7] convert() 開始（MinerU basic，CPU 約 10 秒/頁）…");
    let outcome = mineru::convert(&pdf, &out, &cfg)
        .await
        .expect("K1 convert() 應成功");
    assert!(outcome.report.route.to_mineru, "mueller2016 應被訊號攔到 MinerU：{}", outcome.report.route.reason);
    assert_eq!(outcome.report.ladder.tier.as_deref(), Some("local"), "階梯 1（設定覆寫）應命中");
    eprintln!(
        "[k7] route: {}；ladder: {} 版本未知 tier=local；text={} fig={}",
        outcome.report.route.reason,
        outcome.report.ladder.program.as_deref().unwrap_or(""),
        outcome.report.text_notes,
        outcome.report.figure_notes,
    );

    // 轉換驗收：文字 chunk＋9 圖（Fig 1–9）；表 I–V 由 chunk attrs 覆蓋。
    assert_eq!(outcome.report.figure_notes, 9, "9 圖筆記（Fig 3 靠 table caption 拾回）");
    let mut figs: Vec<u32> = outcome
        .notes
        .iter()
        .flat_map(|n| n.figs.iter().copied())
        .collect();
    figs.sort_unstable();
    figs.dedup();
    assert_eq!(figs, vec![1, 2, 3, 4, 5, 6, 7, 8, 9], "圖 1–9 全覆蓋");
    let tables: std::collections::BTreeSet<String> = outcome
        .notes
        .iter()
        .flat_map(|n| n.tables.iter().cloned())
        .collect();
    assert_eq!(
        tables,
        ["I", "II", "III", "IV", "V"].iter().map(|s| s.to_string()).collect(),
        "表 I–V 全覆蓋"
    );
    assert!(out.join("conversion-report.json").exists());
    assert!(out.join("figures.sqlite").exists());
    assert_eq!(outcome.figure_rows.len(), 9, "sidecar 每圖一列");

    // ── 臨時腦：git → init → sources add → sync（--no-extract）→ embed ────
    let home_s = dir.join("home").to_string_lossy().into_owned();
    let notes = out.join("notes");
    let notes_s = notes.to_string_lossy().into_owned();
    let exe_s = gbrain_exe.to_string_lossy().into_owned();
    let env = env_for_brain(Some(&home_s));
    git_commit_all(&notes);

    let (c, _, err) = g_run(&exe_s, &env, &[
        "init",
        "--embedding-model",
        "llama-server:embeddinggemma-2",
        "--embedding-dimensions",
        "768",
    ])
    .await
    .expect("spawn gbrain");
    if c != 0 {
        let _ = g_run(&exe_s, &env, &["init"]).await;
        eprintln!("[k7] gbrain init exit {c}（skill publication 等，非閘）：{err}");
    }
    let (c, _, err) = g_run(&exe_s, &env, &["sources", "add", "k7", "--path", &notes_s, "--force"]).await.unwrap();
    assert_eq!(c, 0, "sources add 失敗：{err}");
    let (c, _, err) = g_run(&exe_s, &env, &["sync", "--source", "k7", "--no-pull", "--yes", "--no-hard-deadline", "--no-extract"])
        .await
        .unwrap();
    assert_eq!(c, 0, "sync 失敗：{err}");
    // embed 兩輪：大批次可能 defer（冪等、安全）。
    for i in 0..2 {
        let _ = g_run(&exe_s, &env, &["embed", "--stale"]).await;
        eprintln!("[k7] embed pass {}", i + 1);
    }

    // ── 12 查詢評測（K2 前綴＋--no-expand，--json 解析 slug） ─────────────
    let attrs_by_slug: std::collections::HashMap<String, (Vec<u32>, Vec<String>, String)> = outcome
        .notes
        .iter()
        .map(|n| {
            (
                n.slug.clone(),
                (n.figs.clone(), n.tables.clone(), n.section_id.clone()),
            )
        })
        .collect();
    let exe_s = gbrain_exe.to_string_lossy().into_owned();
    let mut first_hits: Vec<Option<usize>> = Vec::new();
    eprintln!("\n[k7] ── 12 查詢評測（prefix+no-expand, limit 5, source=k7）──");
    for (qi, (q, targets)) in queries().into_iter().enumerate() {
        let prefixed = crate::knowledge::service::retrieval_query(q);
        let limit = "5";
        let args: Vec<String> = vec![
            "query".into(),
            prefixed,
            "--limit".into(),
            limit.into(),
            "--source".into(),
            "k7".into(),
            "--no-expand".into(),
            "--json".into(),
        ];
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (code, stdout, stderr) = g_run(&exe_s, &env, &refs).await.unwrap();
        assert_eq!(code, 0, "query 失敗：{stderr}");
        let rows = parse_query_json(&stdout).expect("query --json 應可解析");
        let mut line = Vec::new();
        let mut first = None;
        for (rn, row) in rows.iter().enumerate() {
            let slug = row["slug"].as_str().unwrap_or_default();
            let empty = (Vec::new(), Vec::new(), String::new());
            let attrs = attrs_by_slug.get(slug).unwrap_or(&empty);
            let hit = targets.iter().any(|t| matches(attrs, t));
            if hit && first.is_none() {
                first = Some(rn + 1);
            }
            line.push(format!("{slug}:{}", if hit { "HIT" } else { "." }));
        }
        first_hits.push(first);
        let fh = first.map(|n| n.to_string()).unwrap_or_else(|| "--".into());
        let qlabel: String = q.chars().take(48).collect();
        eprintln!("Q{:>2} {qlabel:<48} first-hit {fh:>3}  {}", qi + 1, line.join(" "));
    }

    // ── P0 驗收線 ────────────────────────────────────────────────────────
    let hits5 = first_hits.iter().filter(|h| h.is_some_and(|n| n <= 5)).count();
    let mrr: f64 = first_hits
        .iter()
        .filter_map(|h| h.map(|n| 1.0 / n as f64))
        .sum::<f64>()
        / first_hits.len() as f64;
    eprintln!("\n[k7] K7 real pipeline: hit@5 {hits5}/{}  MRR {mrr:.3}", first_hits.len());
    assert_eq!(hits5, 12, "P0 驗收線：hit@5 12/12");
    assert!(
        mrr >= 0.90,
        "P0 驗收線：MRR ≥ 0.90（實測 {mrr:.3}；實驗基準 0.917）"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// gbrain 子命令包裝（帶 GBRAIN_HOME 環境；exit code 由呼叫端斷言）。
async fn g_run(
    exe: &str,
    env: &[(&'static str, std::ffi::OsString)],
    args: &[&str],
) -> std::io::Result<(i32, String, String)> {
    run_capture(exe, args, env).await
}

/// gbrain `query --json` 輸出解析：stdout 可能帶前置雜訊（升級提示等）——取第一個 `[` 起。
fn parse_query_json(stdout: &str) -> Option<Vec<serde_json::Value>> {
    let start = stdout.find('[')?;
    let end = stdout.rfind(']')?;
    serde_json::from_str(&stdout[start..=end]).ok()
}

fn git_commit_all(repo: &Path) {
    for args in [
        vec!["init", "-q"],
        vec!["add", "-A"],
        vec!["-c", "user.email=k7@test", "-c", "user.name=k7", "commit", "-qm", "seed"],
    ] {
        let st = std::process::Command::new("git")
            .current_dir(repo)
            .args(&args)
            .status()
            .expect("git 可用");
        // commit 在無變更時非零——不視為錯誤（add -A 後必有變更，防呆而已）。
        assert!(st.success() || args[0] == "commit", "git {args:?} 失敗");
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "operoid-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[cfg(test)]
mod unit {
    use super::*;

    /// 歸因匹配：fig/table/sec 三種 ground truth 語意（對齊實驗 chunk_matches）。
    #[test]
    fn k7_attr_matching() {
        let attrs = (vec![8u32], vec!["II".to_string()], "V.B".to_string());
        assert!(matches(&attrs, "fig:8"));
        assert!(!matches(&attrs, "fig:9"));
        assert!(matches(&attrs, "table:II"));
        assert!(matches(&attrs, "sec:V.B"));
        assert!(matches(&attrs, "sec:V"), "前綴匹配（sec:V 涵蓋 V.B）");
        assert!(!matches(&attrs, "sec:VI"));
        assert!(!matches(&attrs, "bogus:x"));
    }

    /// query --json 解析：容忍前置雜訊（升級提示）。
    #[test]
    fn k7_parse_query_json_tolerates_noise() {
        let s = "UPGRADE_AVAILABLE 0.60.105.0 0.60.120.0\n[{\"slug\":\"a/s01\",\"cosine\":0.9}]";
        let rows = parse_query_json(s).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["slug"], "a/s01");
    }
}
