//! 動作類別登記表（Action Registry）——畫線權的器具（Handbook Ch.20 §5，2026-09-29 修正版）。
//!
//! 一份人類署名的 JSON 檔（`<data_dir>/action-registry.json`，與 operoid.db 同層），
//! 逐項載明動作類別的三問答案、層級、圍欄、屆期與放寬證據；決定哪些承諾類別可免
//! 個案核可直接 Active（Ch.11 §5 自動啟用路徑）。
//!
//! 治理規則（計畫 `docs/Operoid-計畫-動作類別登記表.md`，來源《界線的形狀》）：
//! - **從嚴預設**：缺檔／解析失敗 → 呼叫端視同無登記表，全部走人類核可（fails closed）。
//! - **封閉白名單**：查表即時判斷效力（[`category_auto_active`]）——無狀態、不會漂移，
//!   過期即失效；手改出的無效類別條目也永不啟用。
//! - **不對稱修正**：存檔驗證強制（[`validate_registry`]）——放寬（Fenced/Owned）須
//!   具名課責、未來屆期、放寬證據（V1）；收緊（Human）永遠廉價（V2）；有主自動層
//!   另須緊急停止機制（V5）。每次存檔 version+1（V4）；`registry_changed` 事件由
//!   呼叫端記錄（registry.rs 不碰 Store）。
//! - **最小權限**：`fences.tools` 與員工 allowlist 取**交集**（V3，執行期語意）——
//!   圍欄只能收窄、不能擴權。

use crate::domain::models::Timestamp;
use crate::domain::store::now_rfc3339;
use crate::i18n::AppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 登記表檔名（`data_dir` 下）。
pub const REGISTRY_FILE: &str = "action-registry.json";

/// 登記表路徑（比照 `runtime::agent_db_path_in` 模式）。
pub fn registry_path_in(data_dir: &Path) -> PathBuf {
    data_dir.join(REGISTRY_FILE)
}

// ───────────────── 資料模型 ─────────────────

/// 動作類別登記表本体。全域欄位（保險絲／預算／分歧門檻）與類別清單。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionRegistry {
    /// 每次存檔 +1（V4）；配合 `registry_changed` 事件供稽核回溯。
    #[serde(default)]
    pub version: u32,
    /// 關鍵詞保險絲：提案 title/condition 命中任一 → 強制人類核可（凌駕類別自評）。
    #[serde(default = "default_fuse_keywords")]
    pub fuse_keywords: Vec<String>,
    /// 每週 Proposed 預算（R6c）；`None`＝不啟用。超量＝白名單過窄的警報。
    #[serde(default)]
    pub weekly_proposal_budget: Option<u32>,
    /// 抽審分歧率門檻（R6a）；超過 → `divergence_alarm`。
    #[serde(default = "default_divergence_threshold")]
    pub divergence_threshold: f32,
    #[serde(default)]
    pub categories: Vec<ActionCategory>,
}

fn default_fuse_keywords() -> Vec<String> {
    ["對外", "客戶", "報價", "金額", "交期", "合約", "刪除"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_divergence_threshold() -> f32 {
    0.2
}

impl Default for ActionRegistry {
    fn default() -> Self {
        serde_json::from_str("{}").expect("ActionRegistry: 全欄位有 default，空物件必可解析")
    }
}

/// 三問（可逆性／爆炸半徑／課責歸屬）——授權單位是動作類別，粒度決定課責粒度。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ThreeQuestions {
    #[serde(default)]
    pub reversible: bool,
    #[serde(default)]
    pub blast_radius: BlastRadius,
    /// 具名人類課責者（V1：Fenced/Owned 必填——課責歸屬具否決權地位）。
    #[serde(default)]
    pub accountable: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BlastRadius {
    Internal,
    Customer,
    Spend,
    Safety,
    Rights,
}

impl Default for BlastRadius {
    fn default() -> Self {
        Self::Internal
    }
}

/// 三層委任（Ch.20 §5.1）：人裁決／圍欄自動／有主自動。從嚴預設＝未登記即 Human。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DelegationTier {
    Human,
    Fenced,
    Owned,
}

/// 圍欄自動層的執行期限制。`tools` 與員工 allowlist 取交集（V3）——只能收窄。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fences {
    /// `None`＝沿用員工 allowlist；有值時實際授予＝交集。
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    /// 圍欄內禁止自主循環外發（對話回合的回覆不受此限）。
    #[serde(default = "default_true")]
    pub no_outbound: bool,
    /// 外發類圍欄：僅模板化回覆（不得新增承諾／金額／日期）。
    #[serde(default)]
    pub template_only: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Fences {
    fn default() -> Self {
        serde_json::from_value(serde_json::json!({}))
            .expect("Fences: 全欄位有 default，空物件必可解析")
    }
}

/// 一個動作類別的完整登記（Ch.20 §5.2：口頭授權在稽核意義上不存在）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionCategory {
    pub id: String,
    /// 分類準則本身——人類文件、可逐字誦讀（條件一：人類出題）。
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub questions: ThreeQuestions,
    pub tier: DelegationTier,
    #[serde(default)]
    pub fences: Fences,
    /// 緊急停止機制的描述（V5：Owned 必填；MVP 僅 error-retry 類別具備）。
    #[serde(default)]
    pub emergency_stop: Option<String>,
    /// 屆期（V1：Fenced/Owned 必填、須在未來）——過期未重簽即降回人裁決層（5.3）。
    #[serde(default)]
    pub expiry: Option<Timestamp>,
    /// 盲抽比例（R6a）；1.0＝全數上送。
    #[serde(default = "default_sampling_rate")]
    pub sampling_rate: f32,
    /// 放寬證據（V1：Fenced/Owned 必填；如 `JOURNEY.md E-xx` 或 `auto: retry_exhausted of {id}`）。
    #[serde(default)]
    pub evidence: Option<String>,
    #[serde(default)]
    pub updated_at: Option<Timestamp>,
}

fn default_sampling_rate() -> f32 {
    1.0
}

// ───────────────── 查表（執行期，無狀態） ─────────────────

impl ActionRegistry {
    pub fn category(&self, id: &str) -> Option<&ActionCategory> {
        self.categories.iter().find(|c| c.id == id)
    }
}

/// 類別此刻是否具自動啟用效力——查表時即時判斷（無狀態、不會漂移）：
/// Fenced/Owned ＋ 具名課責 ＋ 證據 ＋ 屆期在未來，缺一即 false（fails closed）。
/// 屆期掃描（R3）只負責通知重簽；效力不依賴任何已寫回的狀態。
pub fn category_auto_active(cat: &ActionCategory, now: &str) -> bool {
    if !matches!(cat.tier, DelegationTier::Fenced | DelegationTier::Owned) {
        return false;
    }
    if cat.questions.accountable.trim().is_empty() {
        return false;
    }
    if cat.evidence.as_deref().map_or(true, |e| e.trim().is_empty()) {
        return false;
    }
    let (Some(expiry), Ok(now_t)) = (&cat.expiry, chrono::DateTime::parse_from_rfc3339(now)) else {
        return false;
    };
    chrono::DateTime::parse_from_rfc3339(expiry)
        .map_or(false, |exp| exp > now_t)
}

/// 關鍵詞保險絲：提案內容命中任一關鍵詞 → 強制人類通道（回傳命中的詞）。
pub fn fuse_hit<'a>(reg: &'a ActionRegistry, text: &str) -> Option<&'a str> {
    reg.fuse_keywords
        .iter()
        .find(|kw| !kw.is_empty() && text.contains(kw.as_str()))
        .map(|kw| kw.as_str())
}

// ───────────────── 分類（執行期適用線；R2） ─────────────────

/// 提案分類決策（Ch.20 §5.4 執行期適用——員工答題、不出題）。
#[derive(Debug, Clone, PartialEq)]
pub enum GateDecision {
    /// 自動啟用：命中登記且有效的自動層類別（Ch.11 §5 自動啟用路徑）。
    Auto { category_id: String },
    /// 送人類核可：附機讀原因（存入 `Commitment.gate_reason`）——
    /// `unclassified`／`fuse:{關鍵詞}`／`expired:{類別}`／`human_tier:{類別}`。
    Human { reason: String },
}

/// 分類決策樹（純函式；R6b 植入演練直接呼叫）。順序即優先級：
/// 保險絲凌駕類別自評（確定性的保守解析，不依賴對分類的信任）；其餘按封閉白名單——
/// 查無、未聲稱、Human 層、逾期，一律送人類通道（條件二保守解析＋條件四新穎性上送）。
pub fn classify_proposal(
    reg: Option<&ActionRegistry>,
    title: &str,
    condition: &str,
    claimed_category: Option<&str>,
    now: &str,
) -> GateDecision {
    let Some(reg) = reg else {
        return GateDecision::Human { reason: "unclassified".into() };
    };
    let text = format!("{title}\n{condition}");
    if let Some(kw) = fuse_hit(reg, &text) {
        return GateDecision::Human { reason: format!("fuse:{kw}") };
    }
    let Some(claimed) = claimed_category.map(str::trim).filter(|s| !s.is_empty()) else {
        return GateDecision::Human { reason: "unclassified".into() };
    };
    let Some(cat) = reg.category(claimed) else {
        return GateDecision::Human { reason: "unclassified".into() };
    };
    if cat.tier == DelegationTier::Human {
        return GateDecision::Human { reason: format!("human_tier:{claimed}") };
    }
    if !category_auto_active(cat, now) {
        return GateDecision::Human { reason: format!("expired:{claimed}") };
    }
    GateDecision::Auto { category_id: claimed.to_string() }
}

/// 抽審取樣（R6a）：以承諾 id 的 FNV-1a 雜湊**確定性**判定——同一 id 永遠同一結果，
/// 事後可稽核「為何這筆被抽中」。`rate >= 1.0` 恆抽（全數，預設）、`<= 0.0` 恆免。
pub fn sampled_for_review(commitment_id: &str, sampling_rate: f32) -> bool {
    if sampling_rate >= 1.0 {
        return true;
    }
    if sampling_rate <= 0.0 {
        return false;
    }
    let mut hash: u32 = 0x811c_9dc5;
    for b in commitment_id.as_bytes() {
        hash ^= u32::from(*b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash % 10_000) as f32 / 10_000.0 < sampling_rate
}

// ───────────────── 載入／存檔 ─────────────────

/// 載入登記表。缺檔 → `Ok(None)`（從嚴預設）；解析失敗 → Err（呼叫端 fails closed
/// 視同 None，並記 `registry_invalid` 事件）。**載入不驗證 V1–V5**——效力由
/// [`category_auto_active`] 查表時把關，過期或手改壞的條目永不啟用。
pub fn load_registry(data_dir: &Path) -> anyhow::Result<Option<ActionRegistry>> {
    let path = registry_path_in(data_dir);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)?;
    let reg: ActionRegistry = serde_json::from_str(&text)?;
    Ok(Some(reg))
}

/// 存檔：先驗證（V1–V5，不對稱修正的強制點）→ version+1（V4）→ 更新 updated_at
/// → 原子寫（tmp+rename）。`registry_changed` 事件由呼叫端記錄（含版本摘要）。
pub fn save_registry(data_dir: &Path, mut reg: ActionRegistry) -> Result<ActionRegistry, AppError> {
    validate_registry(&reg)?;
    reg.version = reg.version.wrapping_add(1);
    let now = now_rfc3339();
    for cat in &mut reg.categories {
        cat.updated_at = Some(now.clone());
    }
    let path = registry_path_in(data_dir);
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(&reg)
        .map_err(|e| write_err(&e.to_string()))?;
    std::fs::write(&tmp, json)
        .and_then(|_| std::fs::rename(&tmp, &path))
        .map_err(|e| write_err(&e.to_string()))?;
    Ok(reg)
}

fn write_err(detail: &str) -> AppError {
    AppError::new("agent_os.registryWriteFailed").p("detail", detail)
}

fn invalid(id: &str, rule: &str, detail: &str) -> AppError {
    AppError::new("agent_os.registryInvalid")
        .p("id", id)
        .p("rule", rule)
        .p("detail", detail)
}

/// 存檔驗證（V1–V5）——不對稱修正的強制點：放寬有門檻、收緊永遠廉價。
pub fn validate_registry(reg: &ActionRegistry) -> Result<(), AppError> {
    let mut seen = std::collections::HashSet::new();
    for cat in &reg.categories {
        if cat.id.trim().is_empty() {
            return Err(invalid("(無 id)", "id", "類別 id 不可為空"));
        }
        if !seen.insert(cat.id.as_str()) {
            return Err(invalid(&cat.id, "id", "類別 id 重複"));
        }
        if !(0.0..=1.0).contains(&cat.sampling_rate) {
            return Err(invalid(&cat.id, "range", "sampling_rate 須介於 0.0–1.0"));
        }
        if matches!(cat.tier, DelegationTier::Human) {
            continue; // V2：收緊（Human）免填任何東西——永遠廉價
        }
        // V1：Fenced/Owned 三件套（具名課責／未來屆期／放寬證據），缺一拒絕存檔。
        if cat.questions.accountable.trim().is_empty() {
            return Err(invalid(&cat.id, "V1", "自動層類別須具名課責（questions.accountable）"));
        }
        if cat.evidence.as_deref().map_or(true, |e| e.trim().is_empty()) {
            return Err(invalid(&cat.id, "V1", "自動層類別須放寬證據（evidence）"));
        }
        let Some(expiry) = &cat.expiry else {
            return Err(invalid(&cat.id, "V1", "自動層類別須屆期（expiry）"));
        };
        let exp = chrono::DateTime::parse_from_rfc3339(expiry)
            .map_err(|_| invalid(&cat.id, "V1", "expiry 須為 RFC3339"))?;
        let now = chrono::DateTime::parse_from_rfc3339(&now_rfc3339())
            .expect("now_rfc3339 必為 RFC3339");
        if exp <= now {
            return Err(invalid(&cat.id, "V1", "expiry 須在未來（過期請先重簽改期）"));
        }
        // V5：有主自動層須載明緊急停止機制（MVP 僅 error-retry 類別具備）。
        if cat.tier == DelegationTier::Owned
            && cat.emergency_stop.as_deref().map_or(true, |s| s.trim().is_empty())
        {
            return Err(invalid(&cat.id, "V5", "有主自動層須載明緊急停止機制（emergency_stop）"));
        }
    }
    Ok(())
}

// ───────────────── 測試 ─────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "operoid-registry-{tag}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fenced(id: &str) -> ActionCategory {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "description": "內部定期報告／彙整",
            "questions": { "reversible": true, "blast_radius": "internal", "accountable": "charlie" },
            "tier": "fenced",
            "expiry": "2099-12-31T00:00:00Z",
            "evidence": "JOURNEY.md E-test"
        }))
        .unwrap()
    }

    fn registry_with(cats: Vec<ActionCategory>) -> ActionRegistry {
        let mut reg = ActionRegistry::default();
        reg.categories = cats;
        reg
    }

    // ── 路徑 ──

    #[test]
    fn registry_path_is_in_data_dir() {
        let p = registry_path_in(Path::new("/data"));
        assert_eq!(p, Path::new("/data/action-registry.json"));
    }

    // ── 載入（從嚴預設） ──

    #[test]
    fn missing_file_loads_as_none() {
        let dir = test_dir("missing");
        assert_eq!(load_registry(&dir).unwrap(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_json_is_err_fails_closed() {
        let dir = test_dir("badjson");
        std::fs::write(registry_path_in(&dir), "{ not json").unwrap();
        assert!(load_registry(&dir).is_err(), "壞檔須回 Err，呼叫端 fails closed");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_fields_and_missing_globals_are_tolerated() {
        let dir = test_dir("forward");
        // 未來新增欄位／缺全域欄位（serde default）皆可載——登記表要能活過版本演進。
        std::fs::write(
            registry_path_in(&dir),
            r#"{"version": 1, "categories": [], "future_field": true}"#,
        )
        .unwrap();
        let reg = load_registry(&dir).unwrap().unwrap();
        assert_eq!(reg.version, 1);
        assert_eq!(reg.fuse_keywords, default_fuse_keywords());
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── 存檔（原子寫＋版本） ──

    #[test]
    fn save_roundtrip_increments_version_and_stamps_updated_at() {
        let dir = test_dir("roundtrip");
        let reg = registry_with(vec![fenced("internal-report")]);
        let saved = save_registry(&dir, reg.clone()).unwrap();
        assert_eq!(saved.version, 1, "首次存檔 version 0→1");
        assert!(saved.categories[0].updated_at.is_some());
        let loaded = load_registry(&dir).unwrap().unwrap();
        assert_eq!(loaded, saved, "原子寫後載回須一致");
        let saved2 = save_registry(&dir, loaded).unwrap();
        assert_eq!(saved2.version, 2, "再次存檔 version 遞增（V4）");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_rejects_validation_failure_without_touching_file() {
        let dir = test_dir("reject");
        let good = save_registry(&dir, registry_with(vec![fenced("ok")])).unwrap();
        let mut bad = good.clone();
        bad.categories.push(fenced("ok")); // id 重複
        assert!(save_registry(&dir, bad).is_err());
        // 驗證失敗不得動到既有檔案（版本停在 1）。
        assert_eq!(load_registry(&dir).unwrap().unwrap().version, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── V1：放寬三件套（不對稱修正的強制點） ──

    fn v1_case(mutate: impl FnOnce(&mut ActionCategory)) -> AppError {
        let mut cat = fenced("x");
        mutate(&mut cat);
        validate_registry(&registry_with(vec![cat])).unwrap_err()
    }

    #[test]
    fn v1_missing_accountable_rejected() {
        assert_eq!(v1_case(|c| c.questions.accountable = String::new()).params["rule"], "V1");
    }

    #[test]
    fn v1_missing_evidence_rejected() {
        assert_eq!(v1_case(|c| c.evidence = None).params["rule"], "V1");
    }

    #[test]
    fn v1_missing_expiry_rejected() {
        assert_eq!(v1_case(|c| c.expiry = None).params["rule"], "V1");
    }

    #[test]
    fn v1_past_or_malformed_expiry_rejected() {
        assert_eq!(
            v1_case(|c| c.expiry = Some("2000-01-01T00:00:00Z".into())).params["rule"],
            "V1",
            "過期須拒——過期授權等同無主，重簽才可放寬"
        );
        assert_eq!(
            v1_case(|c| c.expiry = Some("next year".into())).params["rule"],
            "V1",
            "非 RFC3339 須拒"
        );
    }

    #[test]
    fn v1_sampling_rate_out_of_range_rejected() {
        assert_eq!(v1_case(|c| c.sampling_rate = 1.5).params["rule"], "range");
    }

    // ── V2：收緊永遠廉價 ──

    #[test]
    fn v2_human_tier_needs_nothing() {
        let cat: ActionCategory = serde_json::from_value(serde_json::json!({
            "id": "spend-or-commit", "tier": "human"
        }))
        .unwrap();
        validate_registry(&registry_with(vec![cat])).expect("Human 層免填三件套（V2）");
    }

    // ── V5：有主自動層須緊急停止 ──

    #[test]
    fn v5_owned_requires_emergency_stop() {
        let mut cat = fenced("error-retry");
        cat.tier = DelegationTier::Owned;
        assert_eq!(
            validate_registry(&registry_with(vec![cat.clone()])).unwrap_err().params["rule"],
            "V5",
            "Owned 無緊急停止須拒"
        );
        cat.emergency_stop = Some("retry_count 達 3 次上限＋人工介入".into());
        validate_registry(&registry_with(vec![cat])).expect("補上緊急停止後通過");
    }

    // ── id 唯一 ──

    #[test]
    fn duplicate_id_rejected() {
        let err = validate_registry(&registry_with(vec![fenced("x"), fenced("x")])).unwrap_err();
        assert_eq!(err.params["rule"], "id");
    }

    // ── 查表效力（無狀態、fails closed） ──

    #[test]
    fn auto_active_requires_future_expiry_and_full_v1_set() {
        let mut cat = fenced("internal-report");
        assert!(category_auto_active(&cat, "2026-09-29T00:00:00Z"));
        // 過期 → 失效（屆期未重簽，自動降回人裁決層的效力面）。
        assert!(!category_auto_active(&cat, "2099-12-31T12:00:00Z"));
        // 缺證據（如被手改掉）→ 失效。
        cat.evidence = None;
        assert!(!category_auto_active(&cat, "2026-09-29T00:00:00Z"));
        // Human 層 → 永不自動啟用。
        cat.tier = DelegationTier::Human;
        cat.evidence = Some("x".into());
        assert!(!category_auto_active(&cat, "2026-09-29T00:00:00Z"));
    }

    #[test]
    fn fuse_hit_returns_first_matching_keyword() {
        let reg = ActionRegistry::default(); // 預設保險絲：對外／客戶／報價／金額／交期／合約／刪除
        assert_eq!(fuse_hit(&reg, "每日站會彙整"), None, "內部彙整不觸發保險絲");
        assert_eq!(fuse_hit(&reg, "回覆客戶詢問"), Some("客戶"));
        assert_eq!(fuse_hit(&reg, "刪除暫存"), Some("刪除"));
    }

    // ── 分類決策樹（R2；封閉白名單——查無即人類通道） ──

    fn classify(reg: Option<&ActionRegistry>, title: &str, claimed: Option<&str>) -> GateDecision {
        classify_proposal(reg, title, "完成條件", claimed, "2026-09-29T00:00:00Z")
    }

    #[test]
    fn classify_without_registry_gates_everything() {
        assert_eq!(
            classify(None, "每日產線摘要", Some("internal-report")),
            GateDecision::Human { reason: "unclassified".into() },
            "無登記表＝從嚴預設：全部走人類核可"
        );
    }

    #[test]
    fn classify_unclaimed_or_unknown_gates_to_human() {
        let reg = registry_with(vec![fenced("internal-report")]);
        assert_eq!(
            classify(Some(&reg), "每日產線摘要", None),
            GateDecision::Human { reason: "unclassified".into() },
            "未聲稱類別 → 人類通道"
        );
        assert_eq!(
            classify(Some(&reg), "每日產線摘要", Some("  ")),
            GateDecision::Human { reason: "unclassified".into() },
            "空白類別視同未聲稱"
        );
        assert_eq!(
            classify(Some(&reg), "每日產線摘要", Some("no-such-cat")),
            GateDecision::Human { reason: "unclassified".into() },
            "白名單是封閉集合——查無即未授權（新穎性上送）"
        );
    }

    #[test]
    fn classify_human_tier_and_expired_gates() {
        let mut lapsed = fenced("old-cat");
        lapsed.expiry = Some("2020-01-01T00:00:00Z".into());
        let human: ActionCategory =
            serde_json::from_value(serde_json::json!({ "id": "spend", "tier": "human" })).unwrap();
        let reg = registry_with(vec![fenced("internal-report"), lapsed, human]);
        assert_eq!(
            classify(Some(&reg), "每日產線摘要", Some("spend")),
            GateDecision::Human { reason: "human_tier:spend".into() }
        );
        assert_eq!(
            classify(Some(&reg), "每日產線摘要", Some("old-cat")),
            GateDecision::Human { reason: "expired:old-cat".into() },
            "逾期類別 → 人類通道（屆期未重簽自動失效）"
        );
        assert_eq!(
            classify(Some(&reg), "每日產線摘要", Some("internal-report")),
            GateDecision::Auto { category_id: "internal-report".into() }
        );
    }

    #[test]
    fn classify_fuse_overrides_valid_claim() {
        let reg = registry_with(vec![fenced("internal-report")]);
        assert_eq!(
            classify(Some(&reg), "回覆客戶詢問的彙整", Some("internal-report")),
            GateDecision::Human { reason: "fuse:客戶".into() },
            "保險絲凌駕類別自評——確定性的保守解析"
        );
    }

    #[test]
    fn sampled_for_review_is_deterministic_with_boundaries() {
        assert!(sampled_for_review("x", 1.0), "rate=1.0 恆抽（全數）");
        assert!(sampled_for_review("x", 1.5), "超界上界同樣恆抽");
        assert!(!sampled_for_review("x", 0.0), "rate=0 恆免");
        assert!(!sampled_for_review("x", -0.1));
        assert_eq!(
            sampled_for_review("commit-42", 0.5),
            sampled_for_review("commit-42", 0.5),
            "同一 id 永遠同一判定（可稽核）"
        );
        let mixed = (0..50)
            .map(|i| sampled_for_review(&format!("c{i}"), 0.5))
            .collect::<Vec<_>>();
        assert!(mixed.iter().any(|b| *b) && mixed.iter().any(|b| !*b), "0.5 應兩種結果皆出現");
    }
}
