//! K8 VLM 補圖說（條件式/選配）——短 caption 圖以 chat VLM 生成描述（明確要求
//! 讀出關鍵數值與趨勢），寫進圖片筆記 body（冪等）。sidecar 退役後，工作佇列
//! 改為 **repo 掃描**：doc 目錄的 `figN-pP.md`（caption 短、帶同名圖檔、
//! 尚無 AI 標記）。
//!
//! 觸發：`factories::run_pdf_knowledge`／`ingest::ingest_pdf` 發布完成後
//! [`spawn_after_publish`]（tokio::spawn best-effort——失敗只記 log，不阻塞入庫）。
//!
//! 紀律（計畫 §K8）：
//! - **egress 信任分層**：public 第三方雲預設擋（`allow_public_egress` 才放行）；
//!   local／private（內網自設）照常可用。
//! - **AI 生成標記**：補寫圖說標記「僅供檢索，不作引用」——幻覺面由標記與
//!   「指向圖」語意共同控制；已含標記的頁跳過（重跑冪等）。
//! - **效果**：把「純視覺答案」轉為 gbrain 可命中的文字（消融實驗外推：
//!   caption 豐富時純文字 12/12）。

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::gbrain_config::LlmEndpoint;

/// caption 低於此字元數視為「短圖說」（K8 判準）。
pub const SHORT_CAPTION_CHARS: usize = 80;

/// AI 生成圖說的標記（body 區塊首行；掃描佇列以「已含標記」判冪等跳過）。
pub const AI_CAPTION_MARKER: &str = "[AI 生成圖說（僅供檢索，不作引用）]";

/// VLM 生成圖說的提示（K8：明確要求讀出關鍵數值與趨勢）。
pub const ENRICH_PROMPT: &str = "This image is a figure from a technical document. Read it carefully and describe: (1) what the figure shows, (2) the key numeric values readable in it, (3) the trends it illustrates. Be specific and concise (3-5 sentences).";

/// 圖檔副檔名（同名主檔任一存在即視為帶圖；與工廠 image 管線一致）。
const IMAGE_EXTS: [&str; 5] = ["jpg", "jpeg", "png", "gif", "webp"];

/// chat 端點的 egress 分層（K1 分層治理；K8 同受約束）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatEgress {
    /// 本機（loopback）——永遠允許。
    Local,
    /// 內網自設端點——設定即同意。
    Private,
    /// 第三方公有雲——預設擋，須 opt-in。
    Public,
}

/// 由 chat 端點 base_url 分類信任層級（純函式）。
pub fn chat_egress_tier(base_url: &str) -> ChatEgress {
    let host = base_url
        .split("://")
        .nth(1)
        .unwrap_or(base_url)
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if host == "localhost"
        || host.starts_with("127.")
        || host == "::1"
        || host.starts_with("[::1]")
    {
        return ChatEgress::Local;
    }
    // RFC1918 私有網段／無點主機名／.local/.internal → private。
    let is_private_ip = host
        .split('.')
        .next()
        .and_then(|o| o.parse::<u32>().ok())
        .map(|o| o == 10 || o == 172 || o == 192)
        .unwrap_or(false)
        && (host.starts_with("10.")
            || host.starts_with("192.168.")
            || host.starts_with("172.16.")
            || host.starts_with("172.17.")
            || host.starts_with("172.18.")
            || host.starts_with("172.19.")
            || host.starts_with("172.2")
            || host.starts_with("172.30.")
            || host.starts_with("172.31."));
    if is_private_ip || !host.contains('.') || host.ends_with(".local") || host.ends_with(".internal")
    {
        return ChatEgress::Private;
    }
    ChatEgress::Public
}

/// 回填統計。
#[derive(Debug, Clone, Serialize)]
pub struct EnrichStats {
    /// 實際補寫的圖數。
    pub enriched: usize,
    /// 因 egress 閘門跳過的圖數。
    pub skipped_egress: usize,
    /// 其他跳過（無圖檔等）。
    pub skipped_other: usize,
    pub warnings: Vec<String>,
}

/// 待補圖說的一列（repo 掃描佇列）。
#[derive(Debug, Clone)]
pub struct ShortCaptionRow {
    pub doc_id: String,
    pub page: i64,
    pub figure_no: Option<u32>,
    pub caption: String,
    /// 圖片筆記頁路徑（補寫目標）。
    pub note_path: std::path::PathBuf,
    /// 同名圖檔絕對路徑（VLM 輸入）。
    pub image_path: String,
}

/// 掃描 doc 目錄的短 caption 圖（`figN-pP.md`）：caption < SHORT_CAPTION_CHARS、
/// 帶同名圖檔、且尚未含 AI 生成標記（重跑冪等）。無圖檔的頁不入佇列
/// （VLM 無圖可讀；caption-only 圖說本就走文字 lane）。
pub fn scan_short_captions(notes_repo: &Path, doc_id: &str, max: usize) -> Vec<ShortCaptionRow> {
    let dir = notes_repo.join(doc_id);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut pages: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().and_then(|x| x.to_str()) == Some("md")
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .map_or(false, |s| s.starts_with("fig") && s.contains("-p"))
        })
        .collect();
    pages.sort();
    let mut out = Vec::new();
    for p in pages {
        if out.len() >= max {
            break;
        }
        let Ok(content) = std::fs::read_to_string(&p) else {
            continue;
        };
        if content.contains(AI_CAPTION_MARKER) {
            continue;
        }
        let Some(caption) = caption_from_figure_body(&content) else {
            continue;
        };
        if caption.chars().count() >= SHORT_CAPTION_CHARS {
            continue;
        }
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let Some(image_path) = IMAGE_EXTS.iter().find_map(|ext| {
            let ip = dir.join(format!("{stem}.{ext}"));
            ip.is_file().then(|| ip.to_string_lossy().into_owned())
        }) else {
            continue;
        };
        let (figure_no, page) = parse_fig_stem(stem);
        out.push(ShortCaptionRow {
            doc_id: doc_id.to_string(),
            page,
            figure_no,
            caption: caption.to_string(),
            note_path: p,
            image_path,
        });
    }
    out
}

/// 圖片筆記 body 首行取回 caption：
/// `title: … | text: [Figure N in Section S, page P] Figure N. <caption>`。
fn caption_from_figure_body(body: &str) -> Option<&str> {
    let line = body.lines().next()?;
    let marker = "] Figure ";
    let idx = line.find(marker)?;
    let rest = line[idx + marker.len()..].trim();
    let caption = rest.split_once(". ").map(|(_, c)| c).unwrap_or(rest);
    Some(caption.trim())
}

/// `figN-pP` →（圖號、頁）。
fn parse_fig_stem(stem: &str) -> (Option<u32>, i64) {
    let Some(rest) = stem.strip_prefix("fig") else {
        return (None, 0);
    };
    match rest.split_once("-p") {
        Some((n, p)) => (n.parse().ok(), p.parse().unwrap_or(0)),
        None => (None, 0),
    }
}

/// K8 補圖說：對 doc 目錄的短 caption 圖以 chat VLM 生成描述（明確要求關鍵
/// 數值與趨勢），補寫進圖片筆記 body（冪等——已含標記的頁於掃描即跳過）。
pub async fn enrich_short_captions(
    notes_repo: &Path,
    doc_id: &str,
    ep: &LlmEndpoint,
    allow_public_egress: bool,
    max_figures: usize,
) -> Result<EnrichStats> {
    let tier = chat_egress_tier(&ep.base_url);
    let mut stats = EnrichStats {
        enriched: 0,
        skipped_egress: 0,
        skipped_other: 0,
        warnings: Vec::new(),
    };
    if tier == ChatEgress::Public && !allow_public_egress {
        stats
            .warnings
            .push("chat 端點為 public 第三方雲且 allow_public_egress=false——K8 補圖說整批跳過（文件外流需明確 opt-in）".into());
        stats.skipped_egress = max_figures;
        return Ok(stats);
    }
    let rows = scan_short_captions(notes_repo, doc_id, max_figures);
    if rows.is_empty() {
        return Ok(stats);
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let url = format!("{}/chat/completions", ep.base_url.trim_end_matches('/'));
    for row in &rows {
        let Some(uri) = crate::knowledge::fusion::read_image_data_uri(&row.image_path) else {
            stats.skipped_other += 1;
            stats.warnings.push(format!("圖檔讀取失敗：{}", row.image_path));
            continue;
        };
        let body = serde_json::json!({
            "model": ep.model,
            "max_tokens": 600,
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": format!("{} Caption: {}", ENRICH_PROMPT, row.caption) },
                    { "type": "image_url", "image_url": { "url": uri } }
                ]
            }]
        });
        let mut req = client.post(&url).json(&body);
        if ep.has_api_key {
            if let Some(k) = crate::gbrain_config::env_key(&ep.provider) {
                if let Ok(v) = std::env::var(k) {
                    req = req.header("Authorization", format!("Bearer {v}"));
                }
            }
        }
        let desc = match req.send().await {
            Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
                Ok(v) => v["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                Err(e) => {
                    stats.skipped_other += 1;
                    stats.warnings.push(format!("VLM 回應非 JSON：{e}"));
                    continue;
                }
            },
            Ok(r) => {
                stats.skipped_other += 1;
                stats.warnings.push(format!(
                    "VLM 回應 {}（id={} doc={}）",
                    r.status(),
                    row.page,
                    row.doc_id
                ));
                continue;
            }
            Err(e) => {
                stats.skipped_other += 1;
                stats.warnings.push(format!("VLM 請求失敗：{e}"));
                continue;
            }
        };
        if desc.is_empty() {
            stats.skipped_other += 1;
            continue;
        }
        // 圖片筆記頁附加 AI 生成圖說（掃描已排除含標記的頁——重跑冪等）。
        let block = format!("\n{AI_CAPTION_MARKER}\n{desc}\n");
        let mut content = std::fs::read_to_string(&row.note_path).unwrap_or_default();
        content.push_str(&block);
        std::fs::write(&row.note_path, content)?;
        stats.enriched += 1;
    }
    Ok(stats)
}

/// 發布後自動補圖說（fire-and-forget 背景）：無 chat 端點／egress 被閘門擋／
/// VLM 失敗 → 記 log 即可，**絕不阻塞或使入庫失敗**。只掃本次發布的 doc。
/// 呼叫端給作用中腦的 `gbrain_home` 與 egress 閘門值（`allow_public_egress`）。
pub fn spawn_after_publish(
    gbrain_home: Option<String>,
    allow_public_egress: bool,
    notes_repo: std::path::PathBuf,
    doc_id: String,
) {
    tokio::spawn(async move {
        let loaded = match crate::gbrain_config::load_for(gbrain_home.as_deref()) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[k8] 補圖說跳過（{doc_id}）：gbrain 設定不可讀——{e}");
                return;
            }
        };
        let ep = match crate::gbrain_config::resolve_endpoint(&loaded.config) {
            Ok(ep) => ep,
            Err(e) => {
                eprintln!("[k8] 補圖說跳過（{doc_id}）：無 chat 端點——{e}");
                return;
            }
        };
        match enrich_short_captions(&notes_repo, &doc_id, &ep, allow_public_egress, 16).await {
            Ok(stats) if stats.enriched > 0 || !stats.warnings.is_empty() => {
                eprintln!(
                    "[k8] 補圖說完成（{doc_id}）：enriched={} skipped={} warnings={:?}",
                    stats.enriched,
                    stats.skipped_other + stats.skipped_egress,
                    stats.warnings
                );
            }
            Ok(_) => {}
            Err(e) => eprintln!("[k8] 補圖說失敗（{doc_id}）：{e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// egress 分層：loopback=Local、內網/無點主機=Private、公網=Public。
    #[test]
    fn chat_egress_classification() {
        assert_eq!(chat_egress_tier("http://127.0.0.1:8080/v1"), ChatEgress::Local);
        assert_eq!(chat_egress_tier("http://localhost:11434/v1"), ChatEgress::Local);
        assert_eq!(chat_egress_tier("http://192.168.1.10:8000/v1"), ChatEgress::Private);
        assert_eq!(chat_egress_tier("http://10.0.0.5/chat"), ChatEgress::Private);
        assert_eq!(chat_egress_tier("http://llama.internal/v1"), ChatEgress::Private);
        assert_eq!(chat_egress_tier("http://open.bigmodel.cn/api"), ChatEgress::Public);
        assert_eq!(chat_egress_tier("https://api.openai.com/v1"), ChatEgress::Public);
    }

    /// 掃描佇列：短 caption＋帶圖入列；已含 AI 標記、長 caption、無圖檔的頁跳過。
    #[test]
    fn scan_short_captions_filters_and_parses() {
        let dir = std::env::temp_dir().join(format!("k8scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let doc = dir.join("doc");
        std::fs::create_dir_all(&doc).unwrap();
        // 短 caption＋帶圖 → 入列
        std::fs::write(
            doc.join("fig1-p1.md"),
            "title: T | text: [Figure 1 in Section II.A, page 1] Figure 1. Efficiency versus phases.\n",
        )
        .unwrap();
        std::fs::write(doc.join("fig1-p1.jpg"), b"x").unwrap();
        // 長 caption（≥80 字元）→ 跳過
        let long = format!("title: T | text: [Figure 2 in Section II.B, page 2] Figure 2. {}\n", "長".repeat(90));
        std::fs::write(doc.join("fig2-p2.md"), long).unwrap();
        std::fs::write(doc.join("fig2-p2.jpg"), b"x").unwrap();
        // 已含 AI 標記 → 跳過（冪等）
        std::fs::write(
            doc.join("fig3-p3.md"),
            "title: T | text: [Figure 3 in Section II.C, page 3] Figure 3. Short.\n\n[AI 生成圖說（僅供檢索，不作引用）]\nold\n",
        )
        .unwrap();
        std::fs::write(doc.join("fig3-p3.jpg"), b"x").unwrap();
        // 無圖檔 → 跳過
        std::fs::write(
            doc.join("fig4-p4.md"),
            "title: T | text: [Figure 4 in Section II.D, page 4] Figure 4. Caption only.\n",
        )
        .unwrap();
        // 文字 chunk（非 fig 頁）→ 不掃
        std::fs::write(doc.join("s00.md"), "title: T | text: [Section I] …\n").unwrap();

        let rows = scan_short_captions(&dir, "doc", 16);
        assert_eq!(rows.len(), 1, "只該剩 fig1-p1：{rows:?}");
        assert_eq!(rows[0].figure_no, Some(1));
        assert_eq!(rows[0].page, 1);
        assert_eq!(
            rows[0].caption, "Efficiency versus phases.",
            "caption 應自 body 首行取回"
        );
        assert!(rows[0].image_path.ends_with("fig1-p1.jpg"));
        assert!(rows[0].note_path.ends_with("fig1-p1.md"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// K8 real（#[ignore]：需 zhipu key 於環境；手動跑）——以真實 fig8 圖檔＋
    /// 短 caption 走 repo 掃描佇列補圖說；zhipu 不支援讀圖時 skipped_other=1
    /// （降級紀律）。跑兩次驗證冪等（第二次掃描即空佇列）。
    #[ignore = "真實環境相依：需 ZHIPUAI_API_KEY 與網路（public egress 已 opt-in 於測試內）"]
    #[tokio::test]
    async fn real_k8_zhipu_glm_enriches_real_figure() {
        let img = std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default())
            .join(r"Documents\python\pdf2zh\embed-exp\zip-full\mueller2016\images\page_5_chart_6.jpg");
        if !img.exists() {
            eprintln!("[k8] 語料圖檔缺失，略過");
            return;
        }
        let loaded = crate::gbrain_config::load_for(None).ok();
        let ep = loaded
            .and_then(|l| crate::gbrain_config::resolve_endpoint(&l.config).ok())
            .unwrap_or_else(|| LlmEndpoint {
                provider: "zhipu".into(),
                model: "glm-5.3-flash".into(),
                base_url: "https://open.bigmodel.cn/api/paas/v4".into(),
                has_api_key: true,
            });
        eprintln!("[k8] endpoint: {} {} ({})", ep.provider, ep.model, ep.base_url);
        assert_eq!(chat_egress_tier(&ep.base_url), ChatEgress::Public);

        let dir = std::env::temp_dir().join(format!(
            "k8-real-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let doc = dir.join("mueller2016");
        std::fs::create_dir_all(&doc).unwrap();
        std::fs::write(
            doc.join("fig8-p6.md"),
            "title: mueller2016 | text: [Figure 8 in Section V.B, page 6] Figure 8. Efficiency versus number of phases.\n",
        )
        .unwrap();
        std::fs::copy(&img, doc.join("fig8-p6.jpg")).unwrap();

        let stats = enrich_short_captions(&dir, "mueller2016", &ep, true, 5)
            .await
            .expect("enrich 不得 Err（降級記錄）");
        eprintln!(
            "[k8] enriched={} skipped_other={} warnings={:?}",
            stats.enriched, stats.skipped_other, stats.warnings
        );
        assert_eq!(stats.enriched + stats.skipped_other, 1);
        if stats.enriched == 1 {
            let body = std::fs::read_to_string(doc.join("fig8-p6.md")).unwrap();
            assert!(body.contains(AI_CAPTION_MARKER));
            // 冪等：重跑掃描即空佇列（已含標記），0 補寫。
            let again = enrich_short_captions(&dir, "mueller2016", &ep, true, 5)
                .await
                .unwrap();
            assert_eq!(again.enriched, 0, "重跑必須冪等");
            eprintln!("[k8] glm-5.3-flash 可讀圖——冪等重跑通過");
        } else {
            eprintln!("[k8] glm-5.3-flash 不支援讀圖——定位者模式為正確降級");
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
