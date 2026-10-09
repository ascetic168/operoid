//! K8 VLM 補圖說（條件式/選配）——短 caption 圖以 chat VLM 生成描述（明確要求
//! 讀出關鍵數值與趨勢），寫進圖片筆記 body 並更新 sidecar caption（清向量待重嵌）。
//!
//! 紀律（計畫 §K8）：
//! - **egress 信任分層**：public 第三方雲預設擋（`allow_public` 才放行）；
//!   local／private（內網自設）照常可用。
//! - **AI 生成標記**：補寫圖說標記「僅供檢索，不作引用」——幻覺面由標記與
//!   「指向圖」語意共同控制。
//! - **效果**：把「純視覺答案」轉為 gbrain 可命中的文字（消融實驗外推：
//!   caption 豐富時純文字 12/12）；更新後清向量，`embed_pending` 重嵌即可。

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::gbrain_config::LlmEndpoint;
pub use crate::knowledge::figures::ShortCaptionRow;
use crate::knowledge::figures::Sidecar;

/// caption 低於此字元數視為「短圖說」（K8 判準）。
pub const SHORT_CAPTION_CHARS: usize = 80;

/// VLM 生成圖說的提示（K8：明確要求讀出關鍵數值與趨勢）。
pub const ENRICH_PROMPT: &str = "This image is a figure from a technical document. Read it carefully and describe: (1) what the figure shows, (2) the key numeric values readable in it, (3) the trends it illustrates. Be specific and concise (3-5 sentences).";

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



/// K8 補圖說：對短 caption 圖以 chat VLM 生成描述（明確要求關鍵數值與趨勢），
/// 更新圖片筆記檔與 sidecar caption（清向量待重嵌）。
/// `notes_repo` 為筆記所在聯邦 repo（圖片筆記檔案路徑可導出）。
pub async fn enrich_short_captions(
    db: &Path,
    notes_repo: &Path,
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
        return Ok(stats);
    }
    let sc = Sidecar::open(db)?;
    let rows = sc.short_captions(SHORT_CAPTION_CHARS, max_figures)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let url = format!("{}/chat/completions", ep.base_url.trim_end_matches('/'));
    for row in &rows {
        let Some(image_path) = row.image_path.as_deref() else {
            stats.skipped_other += 1;
            continue;
        };
        let Some(uri) = crate::knowledge::figures::read_image_data_uri(image_path) else {
            stats.skipped_other += 1;
            stats.warnings.push(format!("圖檔讀取失敗：{image_path}"));
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
        // 圖片筆記檔（K1 命名：fig{N}-p{P}.md）附加 AI 生成圖說。
        let note_path = notes_repo
            .join(&row.doc_id)
            .join(format!("fig{}-p{}.md", row.figure_no.unwrap_or(0), row.page));
        if note_path.exists() {
            let block = format!(
                "\n[AI 生成圖說（僅供檢索，不作引用）]\n{desc}\n"
            );
            let mut content = std::fs::read_to_string(&note_path).unwrap_or_default();
            content.push_str(&block);
            std::fs::write(&note_path, content)?;
        }
        sc.apply_enrichment(&row.doc_id, row.page, &desc)?;
        stats.enriched += 1;
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::figures::Sidecar;

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

    /// K8 real（#[ignore]：需 zhipu key 於環境；手動跑）——直接回答計畫中的
    /// 「glm-5.3-flash 讀圖待驗證」：以真實 fig8 圖檔＋短 caption 走補圖說。
    /// 結果印出 VLM 描述；zhipu 不支援讀圖時 skipped_other=1（降級紀律）。
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
        std::fs::create_dir_all(dir.join("mueller2016")).unwrap();
        let db = dir.join("figures.sqlite");
        {
            let sc = Sidecar::open(&db).unwrap();
            sc.insert_row(
                &crate::converters::mineru::FigureRow {
                    doc_id: "mueller2016".into(),
                    page: 6,
                    image_path: Some(img.to_string_lossy().into_owned()),
                    caption: "Efficiency versus number of phases.".into(),
                    section: "V.B".into(),
                    figure_no: Some(8),
                    image_md5: None,
                    source_id: Some("ing".into()),
                },
                None,
            )
            .unwrap();
        }
        let stats = enrich_short_captions(&db, &dir, &ep, true, 5)
            .await
            .expect("enrich 不得 Err（降級記錄）");
        eprintln!(
            "[k8] enriched={} skipped_other={} warnings={:?}",
            stats.enriched, stats.skipped_other, stats.warnings
        );
        assert_eq!(stats.enriched + stats.skipped_other, 1);
        let sc = Sidecar::open(&db).unwrap();
        let hits = sc.figures_for_docs(&["mueller2016".into()], None, 5).unwrap();
        eprintln!("[k8] caption after: {:?}", hits[0].caption);
        if stats.enriched == 1 {
            assert!(hits[0].caption.contains("[AI 生成圖說]"));
            let (_, with_vec) = sc.doc_stats("mueller2016").unwrap();
            assert_eq!(with_vec, 0, "更新後向量應清空待重嵌");
            eprintln!("[k8] glm-5.3-flash 可讀圖——待驗證結案：支援");
        } else {
            eprintln!("[k8] glm-5.3-flash 不支援讀圖——定位者模式為正確降級");
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
