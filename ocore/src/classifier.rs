//! 檔案自動分類 — 把任意（支援的）檔案判斷歸入 people/companies/meeting/inbox。
//!
//! 三層判斷：① 副檔名/CSV 表頭（免費、離線、免 key）→ ② 特徵規則（免費）→
//! ③ LLM（需 key）。分類結果只是一個 factory 字串，前端拿到後直接呼叫既有
//! `factory_run`，本模組不重做工廠邏輯。
//!
//! 安全設計：只有「確定性訊號」（副檔名/表頭/特徵）才會回 High/Medium（前端自動跑）；
//! LLM 層只回 High 或 Low —— 確保 LLM 猜測絕不會靜默自動寫入（這正是本功能要消除的
//! 「分錯工廠→靜靜寫錯頁面」風險）。
//!
//! factory 為空字串代表「不支援的副檔名」（前端顯示已略過）；其餘低信心一律帶一個
//! 預設 factory（通常 inbox）供確認框預選。

use std::path::Path;

use serde::{Deserialize, Serialize};
use crate::app_config;
use crate::factory_types::{self, Pack};
use crate::gbrain_config::LlmEndpoint;
use crate::converters::{pdf_text, text_to_md};
use crate::llm;

/// 分類用的文字抽樣上限（chars）。只用來判斷歸屬，正式解析仍由各工廠原流程。
const SAMPLE_CHARS: usize = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClassifySource {
    Extension,
    Heuristic,
    Llm,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileClassification {
    pub path: String,
    /// "people"|"companies"|"meeting"|"inbox"；空字串 = 不支援的副檔名。
    pub factory: String,
    pub confidence: Confidence,
    /// 給 UI 顯示的一句話理由（v1 直接用中文）。
    pub reason: String,
    pub source: ClassifySource,
}

/// 同步判斷的兩種結果：已決定（副檔名/特徵），或需要交給 LLM（附抽樣內容）。
enum Verdict {
    Done(FileClassification),
    Llm(String),
}

struct Hit {
    factory: &'static str,
    reason: &'static str,
}

// ── 對外：分類單檔 ──────────────────────────────────────────────────────
/// pack 由 cfg 解析（作用中腦的 `schema_pack`；未知/未設 → v2 表）。
pub async fn classify_one(
    path: &Path,
    cfg: &app_config::AppConfig,
    endpoint: Option<&LlmEndpoint>,
) -> FileClassification {
    let (pack, _) = factory_types::active_pack(cfg);
    let has_ep = endpoint.is_some();
    match verdict(pack, path, has_ep) {
        Verdict::Done(c) => c,
        Verdict::Llm(content) => match endpoint {
            Some(ep) => classify_llm(pack, &path.to_string_lossy(), &content, cfg, ep).await,
            None => no_llm_fallback(pack, &path.to_string_lossy()),
        },
    }
}

/// 規則判不準、又沒有可用 LLM 端點時的退化結果：低信心、capture 型（note/inbox）、交確認框。
fn no_llm_fallback(pack: &Pack, p: &str) -> FileClassification {
    FileClassification {
        path: p.into(),
        factory: pack.catchall_id().into(),
        confidence: Confidence::Low,
        reason: "規則無法判斷，且未設 API key（無法用 LLM 判讀）".into(),
        source: ClassifySource::Extension,
    }
}

// ── Tier 1+2：純同步判斷 ────────────────────────────────────────────────
fn verdict(pack: &Pack, path: &Path, has_ep: bool) -> Verdict {
    let p = path.to_string_lossy().to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "csv" => Verdict::Done(classify_csv(pack, path, &p)),
        "txt" | "md" | "markdown" => text_verdict(pack, &p, read_text(path).ok(), has_ep),
        "pdf" => text_verdict(pack, &p, pdf_text::extract(path).ok(), has_ep),
        other => Verdict::Done(FileClassification {
            path: p,
            factory: String::new(),
            confidence: Confidence::Low,
            reason: format!("不支援的副檔名：{other}"),
            source: ClassifySource::Extension,
        }),
    }
}

/// txt/md/pdf 共用：先特徵規則，沒命中就交 LLM。
/// `has_ep`：特徵命中但執行需 LLM 的文字工廠（company/meeting/person-text），
/// 無可用 endpoint 時降為 Low（交確認），避免自動跑卻在 factory_run 因無 key 失敗。
fn text_verdict(pack: &Pack, p: &str, content: Option<String>, has_ep: bool) -> Verdict {
    let content = match content {
        Some(c) if !c.trim().is_empty() => sample(&c),
        _ => {
            return Verdict::Done(FileClassification {
                path: p.into(),
                factory: pack.catchall_id().into(),
                confidence: Confidence::Low,
                reason: "檔案為空或無法讀取文字".into(),
                source: ClassifySource::Extension,
            })
        }
    };
    if let Some(hit) = heuristic(pack, &content) {
        let (confidence, reason): (Confidence, String) = if has_ep {
            (Confidence::High, hit.reason.into())
        } else {
            (Confidence::Low, format!("{}（執行需 LLM，未設 API key）", hit.reason))
        };
        return Verdict::Done(FileClassification {
            path: p.into(),
            factory: hit.factory.into(),
            confidence,
            reason,
            source: ClassifySource::Heuristic,
        });
    }
    Verdict::Llm(content)
}

/// CSV：嗅探首行表頭判斷是否為聯絡人格式。csv 在本系統一律預設 person/people，
/// 但表頭不像聯絡人時降為 Low（交確認），避免非聯絡人 CSV 靜默生空白 person 頁。
fn classify_csv(pack: &Pack, path: &Path, p: &str) -> FileClassification {
    let header = read_text(path)
        .ok()
        .and_then(|t| t.lines().next().map(|l| l.to_ascii_lowercase()))
        .unwrap_or_default();
    let has_name = header.contains("name");
    let contact = [
        "phone",
        "e-mail",
        "email",
        "organization",
        "given name",
        "family name",
        "address",
    ]
    .iter()
    .any(|c| header.contains(c));
    let (conf, reason) = if has_name && contact {
        (Confidence::High, "CSV 表頭為聯絡人格式（Google Contacts）")
    } else if has_name || contact {
        (Confidence::Medium, "CSV 表頭部分像聯絡人")
    } else {
        (Confidence::Low, "CSV 表頭不像聯絡人，預設 people（請確認）")
    };
    FileClassification {
        path: p.into(),
        factory: pack.id_of_kind("people").unwrap_or_else(|| pack.catchall_id()).into(),
        confidence: conf,
        reason: reason.into(),
        source: ClassifySource::Extension,
    }
}

/// 特徵規則：明確關鍵字命中且僅單一類別命中才回該類別（High）；
/// 多類別同時命中視為模糊 → None（交 LLM），避免誤判自動跑。
/// 命中結果為 canonical kind（people/company/meeting/projects），經 pack 映射為實際 id。
fn heuristic(pack: &Pack, content: &str) -> Option<Hit> {
    let low = content.to_ascii_lowercase();
    let people = content.contains("BEGIN:VCARD")
        || low.contains("vcard")
        || ((content.contains("電話") || content.contains("手機") || low.contains("phone"))
            && (content.contains("信箱")
                || low.contains("email")
                || content.contains('@'))
            && (content.contains("姓名") || content.contains("聯絡") || low.contains("name")));
    let companies = ["公司簡介", "公司介紹", "統一編號", "資本額", "營業額", "成立於", "創辦人"]
        .iter()
        .any(|k| content.contains(k))
        || ["company profile", "founded in", "headquarters", "revenue"]
            .iter()
            .any(|k| low.contains(k));
    let meeting = ["會議記錄", "會議逐字稿", "會議議程", "出席者", "出席人員", "議程"]
        .iter()
        .any(|k| content.contains(k))
        || ["meeting minutes", "attendees", "agenda"]
            .iter()
            .any(|k| low.contains(k));
    let projects = ["專案", "專項", "里程碑", "交付項目", "工作包", "甘特圖"]
        .iter()
        .any(|k| content.contains(k))
        || ["deliverable", "milestone", "sprint", "wbs", "gantt"]
            .iter()
            .any(|k| low.contains(k));
    let hits: Vec<&str> = [
        people.then_some("people"),
        companies.then_some("company"),
        meeting.then_some("meeting"),
        projects.then_some("projects"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if hits.len() != 1 {
        return None;
    }
    let id = pack.id_of_kind(hits[0])?;
    let reason = match hits[0] {
        "people" => "偵測到聯絡人特徵",
        "company" => "偵測到公司/組織特徵",
        "meeting" => "偵測到會議特徵",
        _ => "偵測到專案特徵",
    };
    Some(Hit { factory: id, reason })
}

// ── Tier 3：LLM ─────────────────────────────────────────────────────────
async fn classify_llm(
    pack: &Pack,
    p: &str,
    content: &str,
    cfg: &app_config::AppConfig,
    ep: &LlmEndpoint,
) -> FileClassification {
    let ids: Vec<&str> = pack.types.iter().map(|t| t.id).collect();
    let system = format!(
        "你是檔案分類器。判斷這份文件最適合歸入哪個知識庫分類：\n\
        {types}\n\
        只回傳一個 JSON 物件：{{\"factory\":\"{id_enum}\",\"confidence\":\"high|low\",\"reason\":\"一句話理由\"}}。\n\
        規則：非常有把握才回 confidence=high，否則給 low。不要任何說明文字。",
        types = pack.prompt_lines(),
        id_enum = ids.join("|"),
    );
    let user = format!("文件內容：\n{content}\n\n請只回傳 JSON 物件。");
    match llm::complete(ep, &cfg.llm_sampling(), &system, &user).await {
        Ok(resp) => match serde_json::from_str::<LlmVerdict>(&text_to_md::strip_fence(&resp)) {
            Ok(v) => FileClassification {
                path: p.into(),
                factory: pack.normalize(&v.factory).to_string(),
                confidence: if v.confidence.eq_ignore_ascii_case("high") {
                    Confidence::High
                } else {
                    Confidence::Low
                },
                reason: if v.reason.trim().is_empty() {
                    "LLM 判斷（未附理由）".into()
                } else {
                    v.reason
                },
                source: ClassifySource::Llm,
            },
            Err(_) => FileClassification {
                // 回應非預期 JSON → 不自動跑，預設 capture 型交確認。
                path: p.into(),
                factory: pack.catchall_id().into(),
                confidence: Confidence::Low,
                reason: format!("LLM 回應無法解析，預設 {}：{}", pack.catchall_id(), cap(&resp, 80)),
                source: ClassifySource::Llm,
            },
        },
        Err(e) => FileClassification {
            path: p.into(),
            factory: pack.catchall_id().into(),
            confidence: Confidence::Low,
            reason: format!("LLM 分類失敗，預設 {}：{e}", pack.catchall_id()),
            source: ClassifySource::Llm,
        },
    }
}

#[derive(Deserialize)]
struct LlmVerdict {
    #[serde(default)]
    factory: String,
    #[serde(default)]
    confidence: String,
    #[serde(default)]
    reason: String,
}

// ── 小工具 ──────────────────────────────────────────────────────────────
fn read_text(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)?;
    let bytes = bytes
        .strip_prefix(b"\xef\xbb\xbf")
        .map(|b| b.to_vec())
        .unwrap_or(bytes); // 去 UTF-8 BOM
    Ok(String::from_utf8(bytes)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

fn sample(s: &str) -> String {
    if s.chars().count() <= SAMPLE_CHARS {
        return s.to_string();
    }
    let cut: String = s.chars().take(SAMPLE_CHARS).collect();
    format!("{cut}\n[…]")
}

fn cap(s: &str, n: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= n {
        t.to_string()
    } else {
        let cut: String = t.chars().take(n).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factory_types::{LEGACY_PACK, V2_PACK};
    use std::io::Write;

    fn tmp(name: &str, ext: &str, body: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("gbrain_cls_{name}_{}.{ext}", std::process::id()));
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        p
    }

    #[test]
    fn csv_contacts_header_is_people_high() {
        let p = tmp("contacts", "csv", "Name,Given Name,Phone 1 - Value,E-mail 1 - Value\nAlice,Alice,0911,a@b.c\n");
        let c = classify_csv(&V2_PACK, &p, &p.to_string_lossy());
        assert_eq!(c.factory, "person");
        assert_eq!(c.confidence, Confidence::High);
        let c2 = classify_csv(&LEGACY_PACK, &p, &p.to_string_lossy());
        assert_eq!(c2.factory, "people");
    }

    #[test]
    fn csv_non_contact_header_is_not_high() {
        let p = tmp("sales", "csv", "month,revenue,region\n1,100,TW\n");
        let c = classify_csv(&V2_PACK, &p, &p.to_string_lossy());
        assert_eq!(c.factory, "person");
        assert_ne!(c.confidence, Confidence::High); // Medium/Low → 需確認
    }

    #[test]
    fn meeting_text_detected_by_heuristic() {
        let p = tmp("mtg", "txt", "產品周會 會議記錄\n出席者：甲、乙\n議程：檢視進度");
        match verdict(&LEGACY_PACK, &p, true) {
            Verdict::Done(c) => {
                assert_eq!(c.factory, "meeting");
                assert_eq!(c.confidence, Confidence::High);
                assert_eq!(c.source, ClassifySource::Heuristic);
            }
            _ => panic!("會議特徵應被規則命中"),
        }
        // v2 無 meeting 型：會議特徵在 v2 pack 交 LLM 判讀（可能落 analysis/note）。
        assert!(matches!(verdict(&V2_PACK, &p, true), Verdict::Llm(_)));
    }

    #[test]
    fn company_text_detected_by_heuristic() {
        let p = tmp("co", "txt", "晶瀚半導體 公司簡介\n統一編號 12345\n資本額 10億");
        match verdict(&V2_PACK, &p, true) {
            Verdict::Done(c) => assert_eq!(c.factory, "company"),
            _ => panic!("公司特徵應被規則命中"),
        }
        match verdict(&LEGACY_PACK, &p, true) {
            Verdict::Done(c) => assert_eq!(c.factory, "companies"),
            _ => panic!("legacy pack 公司特徵應命中 companies"),
        }
    }

    #[test]
    fn heuristic_without_endpoint_downgrades_to_low() {
        // 特徵命中，但無 endpoint → 降為 Low（交確認），避免自動跑卻因無 key 失敗。
        let p = tmp("mtg2", "txt", "產品周會 會議記錄\n出席者：甲、乙");
        match verdict(&LEGACY_PACK, &p, false) {
            Verdict::Done(c) => {
                assert_eq!(c.factory, "meeting");
                assert_eq!(c.confidence, Confidence::Low);
            }
            _ => panic!("無 endpoint 時特徵命中應降為 Low，而非交 LLM"),
        }
    }

    #[test]
    fn ambiguous_text_escalates_to_llm() {
        let p = tmp("prose", "txt", "今天天氣不錯，我們去散步，順便聊了一下未來的計畫。");
        assert!(matches!(verdict(&V2_PACK, &p, true), Verdict::Llm(_)));
    }

    #[test]
    fn no_endpoint_fallback_is_low_catchall() {
        let c = no_llm_fallback(&V2_PACK, "x.txt");
        assert_eq!(c.factory, "note");
        assert_eq!(c.confidence, Confidence::Low);
        let c2 = no_llm_fallback(&LEGACY_PACK, "x.txt");
        assert_eq!(c2.factory, "inbox");
    }

    #[test]
    fn unsupported_extension_is_skipped() {
        let p = tmp("doc", "docx", "fake docx body");
        match verdict(&V2_PACK, &p, true) {
            Verdict::Done(c) => {
                assert_eq!(c.factory, "");
                assert_eq!(c.confidence, Confidence::Low);
            }
            _ => panic!("不支援的副檔名應為 Done(factory 空)"),
        }
    }

    #[test]
    fn project_text_detected_by_heuristic() {
        let p = tmp("proj", "txt", "E-07 機台改善 專案\n里程碑：4/30 試作、5/30 量產\n交付項目：新 recipe");
        match verdict(&V2_PACK, &p, true) {
            Verdict::Done(c) => assert_eq!(c.factory, "project"),
            _ => panic!("專案特徵應被規則命中"),
        }
    }

    #[test]
    fn llm_prompt_enumerates_pack_types() {
        let system = format!(
            "你是檔案分類器。判斷這份文件最適合歸入哪個知識庫分類：\n{}\n{}",
            V2_PACK.prompt_lines(),
            V2_PACK.types.iter().map(|t| t.id).collect::<Vec<_>>().join("|")
        );
        assert!(system.contains("- person："));
        assert!(system.contains("- social-digest："));
        assert!(system.contains("- note："));
        assert!(system.contains("person|company"));
    }
}
