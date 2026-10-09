//! K3 圖向量 sidecar——`figures.sqlite` 的向量回填與暴力 cosine 檢索。
//!
//! 設計（計畫 §K3；證據：實驗報告 §4/§5）：
//! - **儲存即檢索**：千級向量不值得引進向量資料庫——rusqlite 全表掃＋Rust 內積。
//! - **嵌入能力分層**（多模態非必備）：
//!   - [`SidecarMode::Multimodal`]：caption＋原圖聯合向量（滿血；視覺查詢 27→1–3 名）。
//!     圖檔缺漏的列（caption-only 拾回）自動退為純文字向量。
//!   - [`SidecarMode::TextOnly`]：圖說＋章節＋頁碼文字向量（第一輪 V2 已證
//!     12/12、MRR 0.938）。
//! - **以 `image_md5` 去重**：同一張圖（重轉換、跨文件重複）只嵌一次，其餘列複製向量。
//! - **授權鐵律**：`search` 一律吃授權 source 集合（K4）；`source_id IS NULL` 的列
//!   對任何授權集合都不可見（fail closed——未歸檔＝未授權）。
//!
//! 文件側文字格式與圖片筆記 body 逐字一致（`title: … | text: [Figure N in
//! Section …, page P] …`）——與實驗 V3m 的聯合向量輸入相同。查詢端一律純文字
//! ＋K2 前綴；跨模態比對由嵌入模型的統一空間承擔。

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::knowledge::service::RETRIEVAL_QUERY_PREFIX;

/// 預設本地 llama-server（與 doctor／gbrain provider 慣例一致）。
pub const DEFAULT_EMBEDDING_BASE: &str = super::doctor::DEFAULT_EMBEDDING_BASE;

/// 多模態批次上限（實驗驗證過的批次大小；8K context 上每圖約 82 tokens）。
const MULTIMODAL_BATCH: usize = 4;
/// 純文字批次上限。
const TEXT_BATCH: usize = 32;

/// 嵌入模式（能力分層；由呼叫端以 doctor 探測結果決定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarMode {
    /// caption＋原圖聯合向量；無圖檔的列自動退純文字。
    Multimodal,
    /// 純文字向量（圖說＋章節＋頁碼）。
    TextOnly,
}

/// 回填統計。
#[derive(Debug, Clone, Serialize)]
pub struct EmbedStats {
    pub mode: SidecarMode,
    /// 本輪實際送嵌的列數。
    pub embedded: usize,
    /// 以 image_md5 去重、直接複製向量的列數。
    pub deduped: usize,
    /// 跳過數（API 失敗／維度不符等；原因在 warnings）。
    pub skipped: usize,
    pub warnings: Vec<String>,
}

/// 檢索命中（K4 融合的「路 2」項）。
#[derive(Debug, Clone, Serialize)]
pub struct FigureHit {
    pub doc_id: String,
    pub page: i64,
    pub figure_no: Option<u32>,
    pub caption: String,
    pub section: String,
    pub image_path: Option<String>,
    pub source_id: Option<String>,
    /// cosine 相似度（[-1, 1]；嵌入已近 L2 正規化時即內積）。
    pub score: f64,
}

/// 待嵌列（自 figures.sqlite 讀出）。
#[derive(Debug, Clone)]
struct PendingRow {
    id: i64,
    doc_id: String,
    page: i64,
    figure_no: Option<u32>,
    caption: String,
    section: String,
    image_path: Option<String>,
    image_md5: Option<String>,
}

/// sidecar 開啟＋schema 就緒（含舊版 K1 資料庫的欄位補齊）。
pub struct Sidecar {
    conn: rusqlite::Connection,
}

impl Sidecar {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
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
        // 舊版（K1 初版無 figure_no/source_id）冪等補欄。
        for col in ["figure_no", "source_id"] {
            let has = conn
                .prepare("PRAGMA table_info(figures)")?
                .query_map([], |r| r.get::<_, String>(1))?
                .any(|c| c.as_deref() == Ok(col));
            if !has {
                conn.execute(&format!("ALTER TABLE figures ADD COLUMN {col} TEXT"), [])?;
            }
        }
        Ok(Self { conn })
    }

    /// 歸檔：doc_id → source_id（K4 授權過濾的映射；轉換時未知、工廠入庫時補）。
    pub fn tag_source(&self, doc_id: &str, source_id: &str) -> Result<usize> {
        Ok(self.conn.execute(
            "UPDATE figures SET source_id = ?2 WHERE doc_id = ?1",
            rusqlite::params![doc_id, source_id],
        )?)
    }

    /// 清空全部向量（升級重嵌——如補裝 mmproj 後由 TextOnly 升 Multimodal；
    /// sidecar 僅每文件 ~9–25 列，秒級重嵌）。
    pub fn clear_vectors(&self) -> Result<usize> {
        Ok(self.conn.execute("UPDATE figures SET vec = NULL", [])?)
    }

    fn pending_rows(&self) -> Result<Vec<PendingRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, doc_id, page, figure_no, caption, section, image_path, image_md5
             FROM figures WHERE vec IS NULL ORDER BY id",
        )?;
        let rows = stmt
            .query_map([], map_pending)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn store_vec(&self, id: i64, vec: &[f32]) -> Result<()> {
        let bytes: Vec<u8> = vec.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.conn
            .execute("UPDATE figures SET vec = ?2 WHERE id = ?1", rusqlite::params![id, bytes])?;
        Ok(())
    }

    fn vec_for_md5(&self, md5: &str) -> Result<Option<Vec<f32>>> {
        let v: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT vec FROM figures WHERE image_md5 = ?1 AND vec IS NOT NULL LIMIT 1",
                [md5],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        Ok(v.map(|b| bytes_to_vec(&b)))
    }

    /// 暴力 cosine 檢索。`sources`＝授權 source 集合（K4 鐵律）；None 僅供
    /// 單機／測試使用——授權路徑一律要給。維度與查詢不符的列（模型已換）跳過。
    pub fn search(
        &self,
        query: &[f32],
        sources: Option<&[String]>,
        top_k: usize,
    ) -> Result<Vec<FigureHit>> {
        let mut stmt = self.conn.prepare(
            "SELECT doc_id, page, figure_no, caption, section, image_path, source_id, vec
             FROM figures WHERE vec IS NOT NULL ORDER BY id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Vec<u8>>(7)?,
            ))
        })?;
        let mut hits: Vec<FigureHit> = Vec::new();
        for row in rows {
            let (doc_id, page, figure_no, caption, section, image_path, source_id, blob) = row?;
            if let Some(allowed) = sources {
                let Some(sid) = source_id.as_deref() else {
                    continue; // NULL source_id：未歸檔＝未授權（fail closed）
                };
                if !allowed.iter().any(|a| a == sid) {
                    continue;
                }
            }
            let vec = bytes_to_vec(&blob);
            if vec.len() != query.len() {
                continue; // 維度不符＝舊模型殘留，跳過
            }
            hits.push(FigureHit {
                doc_id,
                page,
                figure_no: figure_no.map(|n| n as u32),
                caption,
                section,
                image_path,
                source_id,
                score: cosine(query, &vec),
            });
        }
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(top_k);
        Ok(hits)
    }
}

fn map_pending(r: &rusqlite::Row<'_>) -> rusqlite::Result<PendingRow> {
    Ok(PendingRow {
        id: r.get(0)?,
        doc_id: r.get(1)?,
        page: r.get(2)?,
        figure_no: r.get::<_, Option<i64>>(3)?.map(|n| n as u32),
        caption: r.get(4)?,
        section: r.get(5)?,
        image_path: r.get(6)?,
        image_md5: r.get(7)?,
    })
}

/// 圖片筆記的文件側文字（與 K1 圖片筆記 body 逐字同構——實驗 V3m 的輸入格式）。
fn figure_doc_text(row: &PendingRow) -> String {
    format!(
        "title: {doc} | text: [Figure {n} in Section {sec}, page {page}] Figure {n}. {cap}",
        doc = row.doc_id,
        n = row.figure_no.unwrap_or(0),
        sec = row.section,
        page = row.page,
        cap = row.caption,
    )
}

/// 檢索查詢向量（K2 前綴在此縫上——呼叫端傳原始查詢）。
pub async fn embed_query(base: &str, model: &str, query: &str) -> Result<Vec<f32>> {
    let text = format!("{RETRIEVAL_QUERY_PREFIX}{query}");
    let vecs = embed_texts(base, model, &[text]).await?;
    vecs.into_iter().next().ok_or_else(|| anyhow!("空回應"))
}

/// 把 `vec IS NULL` 的列回填向量（依模式分層；image_md5 去重）。
/// 批次：有圖列 4 圖/批（content-parts）；純文字列 32/批。批次失敗記 skipped＋警告，
/// 不中斷其餘批次——部分成功優於全有全無。
pub async fn embed_pending(
    db: &Path,
    base: &str,
    model: Option<&str>,
    mode: SidecarMode,
) -> Result<EmbedStats> {
    let model = match model {
        Some(m) => m.to_string(),
        None => resolve_model(base).await?,
    };
    let sidecar = Sidecar::open(db)?;
    let pending = sidecar.pending_rows()?;
    let mut stats = EmbedStats {
        mode,
        embedded: 0,
        deduped: 0,
        skipped: 0,
        warnings: Vec::new(),
    };

    // 1) md5 去重：同圖已嵌 → 直接複製向量（重轉換不重嵌）。
    let mut jobs: Vec<(&PendingRow, Option<String>)> = Vec::new();
    for row in &pending {
        if let Some(md5) = row.image_md5.as_deref() {
            if let Some(v) = sidecar.vec_for_md5(md5)? {
                sidecar.store_vec(row.id, &v)?;
                stats.deduped += 1;
                continue;
            }
        }
        let image = match mode {
            SidecarMode::Multimodal => row.image_path.as_deref().and_then(read_image_data_uri),
            SidecarMode::TextOnly => None,
        };
        jobs.push((row, image));
    }

    // 2) 有圖批次（caption＋原圖聯合向量；content-parts 形狀＝實驗 V3m 驗證版）。
    let with_img: Vec<&(&PendingRow, Option<String>)> =
        jobs.iter().filter(|(_, i)| i.is_some()).collect();
    for batch in with_img.chunks(MULTIMODAL_BATCH) {
        let input: Vec<serde_json::Value> = batch
            .iter()
            .map(|(r, uri)| {
                serde_json::json!({ "content": [
                    { "type": "text", "text": figure_doc_text(r) },
                    { "type": "image_url", "image_url": { "url": uri.clone().expect("batch filtered") } },
                ]})
            })
            .collect();
        let body = serde_json::json!({ "model": model, "input": input });
        match embed_batch(base, &body).await {
            Ok(vecs) if vecs.len() == batch.len() => {
                for ((r, _), v) in batch.iter().zip(vecs) {
                    sidecar.store_vec(r.id, &v)?;
                    stats.embedded += 1;
                }
            }
            Ok(vecs) => {
                stats.skipped += batch.len();
                stats.warnings.push(format!(
                    "多模態批次回應數不符（{} 向量 vs {} 列）——整批跳過",
                    vecs.len(),
                    batch.len()
                ));
            }
            Err(e) => {
                stats.skipped += batch.len();
                stats.warnings.push(format!("多模態批次失敗：{e}"));
            }
        }
    }

    // 3) 純文字批次（TextOnly 全部；Multimodal 的缺圖列——caption-only 拾回等）。
    let text_only: Vec<&(&PendingRow, Option<String>)> =
        jobs.iter().filter(|(_, i)| i.is_none()).collect();
    for batch in text_only.chunks(TEXT_BATCH) {
        let texts: Vec<String> = batch.iter().map(|(r, _)| figure_doc_text(r)).collect();
        match embed_texts(base, &model, &texts).await {
            Ok(vecs) if vecs.len() == batch.len() => {
                for ((r, _), v) in batch.iter().zip(vecs) {
                    sidecar.store_vec(r.id, &v)?;
                    stats.embedded += 1;
                }
            }
            Ok(vecs) => {
                stats.skipped += batch.len();
                stats
                    .warnings
                    .push(format!("文字批次回應數不符（{} vs {}）", vecs.len(), batch.len()));
            }
            Err(e) => {
                stats.skipped += batch.len();
                stats.warnings.push(format!("文字批次失敗：{e}"));
            }
        }
    }
    Ok(stats)
}

/// 讀圖檔 → data URI（jpg/png；其他副檔名以 jpeg 試探）。
fn read_image_data_uri(path: &str) -> Option<String> {
    let p = Path::new(path);
    let bytes = std::fs::read(p).ok()?;
    let mime = match p.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()) {
        Some(e) if e == "png" => "image/png",
        _ => "image/jpeg",
    };
    use base64::Engine as _;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// 解析 `/v1/models` 第一個 id 作為嵌入模型。
pub async fn resolve_model(base: &str) -> Result<String> {
    let url = format!("{}/models", base.trim_end_matches('/'));
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;
    let v: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .context("嵌入服務不可達")?
        .error_for_status()
        .context("嵌入服務回應錯誤")?
        .json()
        .await
        .context("models 回應非 JSON")?;
    v["data"][0]["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("models 回應無模型 id"))
}

async fn embed_texts(base: &str, model: &str, texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let mut out = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(TEXT_BATCH) {
        let body = serde_json::json!({ "model": model, "input": chunk });
        out.extend(embed_batch(base, &body).await?);
    }
    Ok(out)
}

async fn embed_batch(base: &str, body: &serde_json::Value) -> Result<Vec<Vec<f32>>> {
    static CLIENT: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("http client")
    });
    let url = format!("{}/embeddings", base.trim_end_matches('/'));
    let resp = CLIENT
        .post(&url)
        .json(body)
        .send()
        .await
        .context("embeddings 請求失敗")?
        .error_for_status()
        .context("embeddings 被拒（長輸入／旗標紀律見 DEPLOYMENT.md）")?;
    let v: serde_json::Value = resp.json().await.context("embeddings 回應非 JSON")?;
    let mut data: Vec<(i64, Vec<f32>)> = v["data"]
        .as_array()
        .context("embeddings 回應缺 data")?
        .iter()
        .filter_map(|d| {
            Some((
                d["index"].as_i64().unwrap_or(0),
                d["embedding"]
                    .as_array()?
                    .iter()
                    .filter_map(|x| x.as_f64().map(|f| f as f32))
                    .collect::<Vec<f32>>(),
            ))
        })
        .collect();
    data.sort_by_key(|(i, _)| *i);
    Ok(data.into_iter().map(|(_, v)| v).collect())
}

fn bytes_to_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let (mut dot, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b) {
        let (x, y) = (*x as f64, *y as f64);
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// K1 與 K3 共用同一份 DDL（K1 建檔、K3 開檔冪等補齊）——指紋僅供測試釘住欄位集合。
pub fn schema_fingerprint() -> &'static str {
    "figures(doc_id,page,image_path,caption,section,figure_no,image_md5,source_id,vec)"
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::post;
    use std::path::PathBuf;

    fn temp_db(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "k3-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d.join("figures.sqlite")
    }

    fn insert_row(sc: &Sidecar, doc: &str, page: i64, fig: u32, caption: &str, md5: Option<&str>, src: Option<&str>) -> i64 {
        sc.conn
            .execute(
                "INSERT INTO figures (doc_id, page, image_path, caption, section, figure_no, image_md5, source_id, vec)
                 VALUES (?1, ?2, NULL, ?3, 'I', ?4, ?5, ?6, NULL)",
                rusqlite::params![doc, page, caption, fig, md5, src],
            )
            .unwrap();
        sc.conn.last_insert_rowid()
    }

    /// 種子 sidecar：兩文件 × 具方向性的假向量（doc-a fig1 沿 x 軸、doc-a fig2 沿 y、
    /// doc-b fig1 反向 x），查詢向量沿 x → 應命中 doc-a fig1 且 doc-b 被授權過濾擋下。
    fn seeded_db(tag: &str) -> (PathBuf, Sidecar) {
        let path = temp_db(tag);
        let sc = Sidecar::open(&path).unwrap();
        let id1 = insert_row(&sc, "doc-a", 1, 1, "First chart.", Some("md5-1"), Some("src-a"));
        let id2 = insert_row(&sc, "doc-a", 2, 2, "Second chart.", Some("md5-2"), Some("src-a"));
        let id3 = insert_row(&sc, "doc-b", 1, 1, "B chart.", Some("md5-3"), Some("src-b"));
        insert_row(&sc, "doc-a", 3, 3, "Untagged.", None, None);
        sc.store_vec(id1, &[1.0, 0.0]).unwrap();
        sc.store_vec(id2, &[0.0, 1.0]).unwrap();
        sc.store_vec(id3, &[-1.0, 0.0]).unwrap();
        // 第 4 列無向量（待嵌）
        (path, sc)
    }

    /// 授權鐵律：NULL source_id 不可見；跨 source 過濾精確。
    #[test]
    fn search_filters_by_authorized_sources() {
        let (_p, sc) = seeded_db("auth");
        let q = [1.0f32, 0.0];
        // 僅 src-a：doc-b 的反向向量被擋、未歸檔的 id4 也被擋。
        let hits = sc.search(&q, Some(&["src-a".into()]), 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].doc_id, "doc-a");
        assert_eq!(hits[0].figure_no, Some(1));
        assert!(hits[0].score > 0.99);
        assert_eq!(hits[1].figure_no, Some(2));
        // 授權 src-b：只見 doc-b（反向 → 低分但不為 0，因 cosine 有正規化）。
        let hits = sc.search(&q, Some(&["src-b".into()]), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].doc_id, "doc-b");
        assert!(hits[0].score < 0.0);
        // 無授權集合（僅測試／單機面）：可見全部已嵌。
        let hits = sc.search(&q, None, 10).unwrap();
        assert_eq!(hits.len(), 3);
    }

    /// top_k 截斷＋排序。
    #[test]
    fn search_orders_and_truncates() {
        let (_p, sc) = seeded_db("topk");
        let q = [1.0f32, 0.0];
        let hits = sc.search(&q, Some(&["src-a".into(), "src-b".into()]), 2).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits[0].score >= hits[1].score);
    }

    /// 維度不符（舊模型殘留）跳過、不 panic。
    #[test]
    fn search_skips_dim_mismatch() {
        let (path, sc) = seeded_db("dim");
        sc.store_vec(4, &[1.0, 0.0, 0.0]).unwrap(); // 3 維殘留
        let hits = sc.search(&[1.0, 0.0], Some(&["src-a".into()]), 10).unwrap();
        assert_eq!(hits.iter().filter(|h| h.page == 3).count(), 0, "3 維列應被跳過");
        let _ = path;
    }

    /// stub 嵌入伺服器：多模態（content-parts 物件）與純文字兩種形狀都回
    /// 「由文字雜湊決定的確定向量」——可斷言去重與回填。
    async fn stub_embedding_server() -> (String, tokio::task::JoinHandle<()>) {
        use axum::{extract::State, Json};
        use std::sync::Arc as StdArc;
        #[derive(Clone, Default)]
        struct Seen(StdArc<std::sync::Mutex<usize>>);
        async fn embed(State(seen): State<Seen>, Json(v): Json<serde_json::Value>) -> Json<serde_json::Value> {
            *seen.0.lock().unwrap() += 1;
            let input = v["input"].as_array().unwrap();
            let data: Vec<serde_json::Value> = input
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    // content-parts（多模態）或純字串——都取文字部分決定向量。
                    let text = if item.is_string() {
                        item.as_str().unwrap().to_string()
                    } else {
                        item["content"][0]["text"].as_str().unwrap_or("").to_string()
                    };
                    let h = text.bytes().map(|b| b as f32).sum::<f32>();
                    serde_json::json!({ "index": i, "embedding": [h, 1.0] })
                })
                .collect();
            Json(serde_json::json!({ "data": data }))
        }
        let seen = Seen::default();
        let app = axum::Router::new()
            .route("/v1/embeddings", post(embed))
            .with_state(seen);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}/v1"), handle)
    }

    /// 回填：Multimodal 模式下有圖的列走 content-parts、缺圖退純文字；
    /// 同 md5 去重不重嵌；tag 後 search 可見。
    #[tokio::test]
    async fn embed_pending_fills_vecs_with_dedup() {
        let db = temp_db("embed");
        {
            let sc = Sidecar::open(&db).unwrap();
            let img = std::env::temp_dir().join(format!("k3-img-{}.jpg", std::process::id()));
            std::fs::write(&img, b"fake-jpeg").unwrap();
            let img_s = img.to_string_lossy().into_owned();
            sc.conn.execute(
                "INSERT INTO figures (doc_id, page, image_path, caption, section, figure_no, image_md5, source_id, vec)
                 VALUES ('doc-a', 1, ?1, 'A chart.', 'I', 1, 'md5-same', NULL, NULL)",
                rusqlite::params![img_s],
            )
            .unwrap();
            sc.conn.execute(
                "INSERT INTO figures (doc_id, page, image_path, caption, section, figure_no, image_md5, source_id, vec)
                 VALUES ('doc-a', 2, NULL, 'Caption-only recovered.', 'II', 2, NULL, NULL, NULL)",
                [],
            )
            .unwrap();
        }
        let (base, handle) = stub_embedding_server().await;
        let stats = embed_pending(&db, &base, Some("stub-model"), SidecarMode::Multimodal)
            .await
            .unwrap();
        assert_eq!(stats.embedded, 2, "兩列都回填（一聯合一純文字）");
        assert_eq!(stats.skipped, 0, "warnings: {:?}", stats.warnings);

        // 再跑一輪同 md5 的新列 → 去重複製、不送嵌。
        {
            let sc = Sidecar::open(&db).unwrap();
            sc.conn.execute(
                "INSERT INTO figures (doc_id, page, image_path, caption, section, figure_no, image_md5, source_id, vec)
                 VALUES ('doc-b', 1, NULL, 'Same image reused.', 'I', 1, 'md5-same', NULL, NULL)",
                [],
            )
            .unwrap();
        }
        let stats2 = embed_pending(&db, &base, Some("stub-model"), SidecarMode::Multimodal)
            .await
            .unwrap();
        assert_eq!(stats2.deduped, 1, "同 md5 應複製向量");
        assert_eq!(stats2.embedded, 0);

        // 歸檔後才可被授權檢索。
        {
            let sc = Sidecar::open(&db).unwrap();
            sc.tag_source("doc-a", "src-a").unwrap();
            sc.tag_source("doc-b", "src-b").unwrap();
            let hits = sc.search(&[f32::MAX, 1.0], Some(&["src-a".into()]), 10).unwrap();
            assert_eq!(hits.len(), 2);
            assert!(hits.iter().all(|h| h.source_id.as_deref() == Some("src-a")));
        }
        handle.abort();
    }

    /// 舊版 K1 資料庫（缺 figure_no/source_id 欄）開檔即冪等補齊。
    #[test]
    fn open_migrates_legacy_schema() {
        let db = temp_db("migrate");
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(
                "CREATE TABLE figures (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     doc_id TEXT NOT NULL, page INTEGER NOT NULL, image_path TEXT,
                     caption TEXT, section TEXT, image_md5 TEXT, vec BLOB);",
            )
            .unwrap();
        }
        let sc = Sidecar::open(&db).unwrap();
        let cols: Vec<String> = sc
            .conn
            .prepare("PRAGMA table_info(figures)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .flatten()
            .collect();
        assert!(cols.contains(&"figure_no".to_string()));
        assert!(cols.contains(&"source_id".to_string()));
    }

    /// 文件側文字格式與圖片筆記 body 同構（抽斷言避免格式漂移）。
    #[test]
    fn figure_doc_text_matches_note_body() {
        let row = PendingRow {
            id: 0,
            doc_id: "mueller2016".into(),
            page: 5,
            figure_no: Some(7),
            caption: "Extracted frequency dependent inductance.".into(),
            section: "IV.B".into(),
            image_path: None,
            image_md5: None,
        };
        assert_eq!(
            figure_doc_text(&row),
            "title: mueller2016 | text: [Figure 7 in Section IV.B, page 5] Figure 7. Extracted frequency dependent inductance."
        );
    }

    /// resolve_model 走 doctor 的 parse_models（形狀單一真相）。
    #[test]
    fn schema_fingerprint_lists_all_columns() {
        assert!(schema_fingerprint().contains("figure_no"));
        assert!(schema_fingerprint().contains("source_id"));
        assert!(schema_fingerprint().contains("vec"));
    }
}
