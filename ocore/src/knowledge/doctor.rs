//! K6 知識管線健康檢查（doctor 類）——部署紀律的自動化臉。
//!
//! 三項檢查（計畫 §K6）：
//! 1. **嵌入端點**：`/v1/models` 的 `input_modalities` 應含 vision（多模態嵌入可用）；
//!    **長輸入探測**（約 2,200 tokens > 預設批次 512）抓「llama-server 未帶
//!    `-b/-ub 8192` → 長 chunk 被拒」的回歸（C8 服務紀律——部署最常踩的雷）。
//! 2. **chat 端點 VLM 能力**：1 圖試探（1×1 PNG），結果**按端點快取**——
//!    決定 K5 走「讀圖作答」或「定位者」模式。
//! 3. **MinerU 探測**：跑取用階梯（ladder）＋ `<program> --version`，回報命中的
//!    階梯與安裝建議（未找到時給 `uv tool install mineru`／設定覆寫／遠端三選）。
//!
//! 網路探測皆為顯式 async（不掛啟動關鍵路徑——與 prereq 的快速路徑紀律一致）。

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;

use crate::converters::mineru::{ladder, ConvertConfig};

/// 預設本地 llama-server（與 gbrain provider 慣例一致）。
pub const DEFAULT_EMBEDDING_BASE: &str = "http://127.0.0.1:8080/v1";

/// 長輸入探測文（重複到約 9,000 字元 ≈ 2,200 tokens——`-ub 512` 必拒、`-ub 8192` 必收）。
pub const LONG_PROBE_UNIT: &str = "operoid embedding batch probe sentence for physical batch size discipline. ";

/// 1×1 PNG（紅點）——VLM 試探的最小合法圖。
pub const TINY_PNG_DATA_URI: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

// ── 報告模型 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingHealth {
    pub base_url: String,
    pub reachable: bool,
    pub model: Option<String>,
    /// `/v1/models` 的 input_modalities（llama-server：text/image/audio/video…）。
    pub input_modalities: Vec<String>,
    /// 多模態嵌入可用（modalities 含 image）——K3 sidecar 的滿血條件。
    pub vision: bool,
    pub context_tokens: Option<u64>,
    /// 長輸入（>512 tokens）可嵌入——抓 `-b/-ub` 旗標回歸。
    pub long_input_ok: bool,
    pub dimensions: Option<usize>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VlmHealth {
    pub base_url: String,
    pub model: String,
    /// Some(true)=可讀圖作答；Some(false)=不可（K5 定位者模式）；None=未探測（無端點）。
    pub capable: Option<bool>,
    /// 命中按端點快取的結果。
    pub cached: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MineruHealth {
    pub resolved: bool,
    /// 命中的階梯層級（egress tier）。
    pub tier: Option<String>,
    pub program: Option<String>,
    /// `<program> --version`（本機階梯才探；遠端 None）。
    pub version: Option<String>,
    /// 未安裝時的安裝建議。
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeHealth {
    pub embedding: EmbeddingHealth,
    pub vlm: VlmHealth,
    pub mineru: MineruHealth,
}

/// 使用者層級的**精簡能力狀態**（企業版 user 前端的上傳面提示用）。
/// 只回答使用者能感知的兩件事：「複雜 PDF 能不能完整轉換」「圖片索引有沒有多模態」——
/// 不含內部路徑／端點 URL／版本等管理資訊。
#[derive(Debug, Clone, Serialize)]
pub struct QuickStatus {
    /// false＝MinerU 未偵測到，PDF 走快速路徑（僅非常簡單的 PDF 有完整品質）。
    pub mineru_resolved: bool,
    pub embedding_reachable: bool,
    /// false＝圖片僅以文字說明索引（caption＋章節＋頁碼）。
    pub embedding_vision: bool,
}

impl QuickStatus {
    /// 上傳面是否該提示（任一降級成立）。
    pub fn degraded(&self) -> bool {
        !self.mineru_resolved || (self.embedding_reachable && !self.embedding_vision)
    }
}

/// 精簡探測——刻意便宜：MinerU 只做檔案存在性（不 spawn、不打遠端）；
/// 嵌入只 GET `/v1/models`（不做長輸入實測）。適合每次上傳面載入呼叫。
pub async fn quick_status(embedding_base: &str, mineru_cfg: &ConvertConfig) -> QuickStatus {
    let mineru_resolved = ladder::resolve(mineru_cfg).resolved();
    let models_url = format!("{}/models", embedding_base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok();
    let (embedding_reachable, embedding_vision) = match client
        .map(|c| c.get(&models_url).send())
    {
        Some(fut) => match fut.await {
            Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
                Ok(v) => {
                    let (_, mods, _) = parse_models(&v);
                    (true, mods.iter().any(|m| m.eq_ignore_ascii_case("image")))
                }
                Err(_) => (true, false),
            },
            Ok(_) => (true, false),
            Err(_) => (false, false),
        },
        None => (false, false),
    };
    QuickStatus {
        mineru_resolved,
        embedding_reachable,
        embedding_vision,
    }
}

// ── 探測 ─────────────────────────────────────────────────────────────────

/// 完整健康檢查。`chat`為 Some 時才探 VLM（無 chat 端點的環境跳過——K5 略過讀圖）。
pub async fn check(
    embedding_base: &str,
    chat: Option<(&str, &str)>,
    mineru_cfg: &ConvertConfig,
) -> KnowledgeHealth {
    let embedding = probe_embedding(embedding_base).await;
    let vlm = match chat {
        Some((base, model)) => probe_chat_vlm(base, model).await,
        None => VlmHealth {
            base_url: String::new(),
            model: String::new(),
            capable: None,
            cached: false,
            error: None,
        },
    };
    let mineru = probe_mineru(mineru_cfg);
    KnowledgeHealth {
        embedding,
        vlm,
        mineru,
    }
}

/// 嵌入端點探測（models 中繼資料＋長輸入實測）。
pub async fn probe_embedding(base_url: &str) -> EmbeddingHealth {
    let mut h = EmbeddingHealth {
        base_url: base_url.to_string(),
        reachable: false,
        model: None,
        input_modalities: Vec::new(),
        vision: false,
        context_tokens: None,
        long_input_ok: false,
        dimensions: None,
        error: None,
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            h.error = Some(format!("HTTP client 建構失敗：{e}"));
            return h;
        }
    };
    let models_url = format!("{}/models", base_url.trim_end_matches('/'));
    let models: serde_json::Value = match client.get(&models_url).send().await {
        Ok(r) if r.status().is_success() => match r.json().await {
            Ok(v) => v,
            Err(e) => {
                h.error = Some(format!("models 回應非 JSON：{e}"));
                return h;
            }
        },
        Ok(r) => {
            h.error = Some(format!("GET {models_url} → {}", r.status()));
            return h;
        }
        Err(e) => {
            h.error = Some(format!("嵌入服務不可達（{models_url}）：{e}"));
            return h;
        }
    };
    h.reachable = true;
    let (model, modalities, n_ctx) = parse_models(&models);
    h.model = Some(model.clone());
    h.input_modalities = modalities.clone();
    h.vision = modalities.iter().any(|m| m.eq_ignore_ascii_case("image"));
    h.context_tokens = n_ctx;

    // 長輸入探測：`-ub 512` 預設下 >512 tokens 的 chunk 被拒——這是部署紀律的回歸點。
    let long_text: String = LONG_PROBE_UNIT.repeat(220); // ≈9,020 字元 ≈ 2,250 tokens
    let embed_url = format!("{}/embeddings", base_url.trim_end_matches('/'));
    let body = serde_json::json!({ "model": model, "input": [long_text] });
    match client.post(&embed_url).json(&body).send().await {
        Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
            Ok(v) => {
                h.long_input_ok = true;
                h.dimensions = v["data"][0]["embedding"]
                    .as_array()
                    .map(|a| a.len());
            }
            Err(e) => h.error = Some(format!("embeddings 回應非 JSON：{e}")),
        },
        Ok(r) => {
            let status = r.status();
            let detail = r.text().await.unwrap_or_default();
            h.error = Some(format!(
                "長輸入被拒（{status}）——llama-server 啟動旗標需 `-b 8192 -ub 8192`（預設 512 拒收長 chunk）：{}",
                detail.chars().take(300).collect::<String>()
            ));
        }
        Err(e) => h.error = Some(format!("embeddings 請求失敗：{e}")),
    }
    h
}

/// 解析 `/v1/models`（llama-server 形狀）：(model, input_modalities, n_ctx)。純函式。
pub fn parse_models(v: &serde_json::Value) -> (String, Vec<String>, Option<u64>) {
    let d = &v["data"][0];
    let model = d["id"].as_str().unwrap_or_default().to_string();
    let modalities: Vec<String> = d["architecture"]["input_modalities"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| m.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let n_ctx = d["meta"]["n_ctx"].as_u64();
    (model, modalities, n_ctx)
}

// ── VLM 試探（按端點快取） ────────────────────────────────────────────────

static VLM_CACHE: std::sync::LazyLock<Mutex<HashMap<String, Option<bool>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

fn vlm_cache_key(base: &str, model: &str) -> String {
    format!("{base}::{model}")
}

/// 清除 VLM 能力快取（管理員修正端點設定後重探）。
pub fn clear_vlm_cache() {
    VLM_CACHE.lock().expect("vlm cache").clear();
}

/// chat 端點 VLM 能力試探（1 圖；結果按端點快取）。
pub async fn probe_chat_vlm(base_url: &str, model: &str) -> VlmHealth {
    let key = vlm_cache_key(base_url, model);
    if let Some(cached) = VLM_CACHE.lock().expect("vlm cache").get(&key) {
        return VlmHealth {
            base_url: base_url.into(),
            model: model.into(),
            capable: *cached,
            cached: true,
            error: None,
        };
    }
    let capable = request_chat_vlm(base_url, model).await;
    let out = VlmHealth {
        base_url: base_url.into(),
        model: model.into(),
        capable,
        cached: false,
        error: if capable.is_none() {
            Some("chat 端點不可達或回應異常".into())
        } else {
            None
        },
    };
    if capable.is_some() {
        VLM_CACHE.lock().expect("vlm cache").insert(key, capable);
    }
    out
}

/// 實際的 1 圖試探（None＝端點層級失敗——不快取，下輪重探）。
async fn request_chat_vlm(base_url: &str, model: &str) -> Option<bool> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .ok()?;
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 10,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "What color is this pixel? Answer with one word." },
                { "type": "image_url", "image_url": { "url": TINY_PNG_DATA_URI } }
            ]
        }]
    });
    let resp = client.post(&url).json(&body).send().await.ok()?;
    if !resp.status().is_success() {
        return Some(false); // 端點在但拒絕圖 → 明確「無 VLM」（K5 定位者模式）
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    Some(chat_reply_nonempty(&v))
}

/// 評估 chat 回應（choices[0].message.content 非空）。純函式。
pub fn chat_reply_nonempty(v: &serde_json::Value) -> bool {
    v["choices"][0]["message"]["content"]
        .as_str()
        .is_some_and(|s| !s.trim().is_empty())
}

// ── MinerU 探測 ──────────────────────────────────────────────────────────

/// MinerU 取用階梯探測＋版本（本機階梯才 spawn `--version`）。
pub fn probe_mineru(cfg: &ConvertConfig) -> MineruHealth {
    probe_mineru_with(cfg, true)
}

pub fn probe_mineru_with(cfg: &ConvertConfig, spawn_version: bool) -> MineruHealth {
    let outcome = ladder::resolve(cfg);
    if !outcome.resolved() {
        return MineruHealth {
            resolved: false,
            tier: None,
            program: None,
            version: None,
            hint: Some(
                "未找到本地 MinerU：可 `uv tool install mineru`（PATH 階梯）、或設定 \
                 [knowledge] mineru_command 指 venv launcher exe、或設定 mineru_remote_url \
                 改用自架遠端解析。"
                    .into(),
            ),
        };
    }
    let program = outcome.program.clone().unwrap_or_default();
    let version = if spawn_version && outcome.tier == Some(ladder::EgressTier::Local) {
        crate::prereq::probe_version(&program, &["--version"])
    } else {
        None
    };
    MineruHealth {
        resolved: true,
        tier: outcome.tier.map(|t| t.as_str().to_string()),
        program: Some(program),
        version,
        hint: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// models 解析：llama-server 實測形狀（input_modalities 在 architecture 下）。
    #[test]
    fn parse_models_llama_server_shape() {
        let v: serde_json::Value = serde_json::json!({
            "models": [],
            "data": [{
                "id": "embeddinggemma-2",
                "architecture": { "input_modalities": ["text", "image", "audio", "video"] },
                "meta": { "n_ctx": 32768 }
            }]
        });
        let (model, mods, n_ctx) = parse_models(&v);
        assert_eq!(model, "embeddinggemma-2");
        assert_eq!(mods, vec!["text", "image", "audio", "video"]);
        assert_eq!(n_ctx, Some(32768));
    }

    /// 缺欄位寬容：OpenAI 形狀（無 architecture/meta）→ 空 modalities、vision=false。
    #[test]
    fn parse_models_tolerates_missing_fields() {
        let v: serde_json::Value = serde_json::json!({ "data": [{ "id": "text-embedding-3" }] });
        let (model, mods, n_ctx) = parse_models(&v);
        assert_eq!(model, "text-embedding-3");
        assert!(mods.is_empty());
        assert_eq!(n_ctx, None);
    }

    /// chat 回應評估：非空 content = 可讀圖作答。
    #[test]
    fn chat_reply_evaluation() {
        let ok: serde_json::Value = serde_json::json!({
            "choices": [{ "message": { "content": "red" } }]
        });
        assert!(chat_reply_nonempty(&ok));
        let empty: serde_json::Value = serde_json::json!({
            "choices": [{ "message": { "content": "" } }]
        });
        assert!(!chat_reply_nonempty(&empty));
        let broken: serde_json::Value = serde_json::json!({ "error": { "message": "no vision" } });
        assert!(!chat_reply_nonempty(&broken));
    }

    /// VLM 快取：按端點鍵快取、可清。
    #[test]
    fn vlm_cache_roundtrip() {
        {
            let mut c = VLM_CACHE.lock().unwrap();
            c.insert("http://a::m1".into(), Some(true));
        }
        let key = vlm_cache_key("http://a", "m1");
        assert!(VLM_CACHE.lock().unwrap().contains_key(&key));
        clear_vlm_cache();
        assert!(!VLM_CACHE.lock().unwrap().contains_key(&key));
    }

    /// MinerU 探測（不 spawn）：未設定 → resolved=false＋安裝建議。
    #[test]
    fn mineru_probe_unresolved_gives_hint() {
        let h = probe_mineru_with(&ConvertConfig::default(), false);
        assert!(!h.resolved);
        assert!(h.hint.unwrap().contains("uv tool install mineru"));
    }

    /// MinerU 探測：設定覆寫命中（以測試行程自身當 exe，不 spawn version）。
    #[test]
    fn mineru_probe_resolved_via_config() {
        let mut cfg = ConvertConfig::default();
        cfg.mineru_command = Some(std::env::current_exe().unwrap().to_string_lossy().into_owned());
        let h = probe_mineru_with(&cfg, false);
        assert!(h.resolved);
        assert_eq!(h.tier.as_deref(), Some("local"));
        assert!(h.hint.is_none());
    }

    /// 精簡狀態的提示判定：MinerU 缺、或（嵌入可達但無 vision）→ 提示；
    /// 嵌入離線＝未知，**不**誤報 vision 缺（unknown ≠ missing）。
    #[test]
    fn quick_status_degraded_semantics() {
        let s = QuickStatus { mineru_resolved: false, embedding_reachable: true, embedding_vision: true };
        assert!(s.degraded(), "MinerU 缺應提示");
        let s = QuickStatus { mineru_resolved: true, embedding_reachable: true, embedding_vision: false };
        assert!(s.degraded(), "無 vision 應提示");
        let s = QuickStatus { mineru_resolved: true, embedding_reachable: true, embedding_vision: true };
        assert!(!s.degraded());
        let s = QuickStatus { mineru_resolved: true, embedding_reachable: false, embedding_vision: false };
        assert!(!s.degraded(), "嵌入離線是未知不是缺失——不提示");
    }

    /// K5/K6：VLM 探測 HTTP 回圈——接受圖的端點 → Some(true) 並按端點快取；
    /// 拒圖的端點 → Some(false)（定位者模式）。快取鍵含隨機埠（唯一）——
    /// 不 clear（全域快取的 clear 會與平行測試競態）。
    #[tokio::test]
    async fn vlm_probe_round_trip() {
        use axum::{extract::State, routing::post, Json};
        use std::sync::Arc as StdArc;
        // 接受圖的 stub：回 content。
        let app = axum::Router::new().route(
            "/chat/completions",
            post(|Json(_): Json<serde_json::Value>| async {
                Json(serde_json::json!({
                    "choices": [{"message": {"content": "red"}}]
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let ok_base = format!("http://{addr}");
        let h = probe_chat_vlm(&ok_base, "vision-model").await;
        assert_eq!(
            h.capable,
            Some(true),
            "error={:?} base={}",
            h.error,
            ok_base
        );
        assert!(!h.cached);
        let h2 = probe_chat_vlm(&ok_base, "vision-model").await;
        assert_eq!(h2.capable, Some(true));
        assert!(h2.cached, "第二次應命中快取");

        // 拒圖的 stub：400。
        let app2 = axum::Router::new().route(
            "/chat/completions",
            post(|Json(_): Json<serde_json::Value>| async {
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "image not supported"})),
                )
            }),
        );
        let listener2 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener2, app2).await.unwrap() });
        let bad_base = format!("http://{addr2}");
        let h3 = probe_chat_vlm(&bad_base, "text-only-model").await;
        assert_eq!(h3.capable, Some(false), "明確拒圖＝無 VLM（定位者模式）");
        assert!(!h3.cached);
        clear_vlm_cache();
    }

    /// **K6 實機**（#[ignore]：需本機 llama-server；手動跑）：
    /// `cargo test -p ocore real_doctor -- --ignored --nocapture`
    /// 驗收：reachable、vision（mmproj 已掛）、long_input_ok（`-b/-ub 8192` 紀律）、768 維。
    #[ignore = "真實環境相依：需本機 llama-server（127.0.0.1:8080）"]
    #[tokio::test]
    async fn real_doctor_local_llama_server() {
        let h = probe_embedding(DEFAULT_EMBEDDING_BASE).await;
        eprintln!(
            "[doctor] reachable={} model={:?} modalities={:?} vision={} n_ctx={:?} \
             long_input_ok={} dims={:?} err={:?}",
            h.reachable, h.model, h.input_modalities, h.vision, h.context_tokens,
            h.long_input_ok, h.dimensions, h.error
        );
        assert!(h.reachable, "本機 llama-server 應可達：{:?}", h.error);
        assert!(h.long_input_ok, "長輸入應可嵌入（-b/-ub 8192 紀律）：{:?}", h.error);
        assert_eq!(h.dimensions, Some(768));
        // vision 視 mmproj 是否掛載——掛了就必須如實回報 true。
        if h.vision {
            assert!(h.input_modalities.iter().any(|m| m == "image"));
        }
    }
}
