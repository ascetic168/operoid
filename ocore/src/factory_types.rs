//! 工廠類型表（schema pack 對齊）——所有「工廠 id → 目錄/型別/管線」對應的唯一來源。
//!
//! gbrain v0.41.22 起預設 schema pack 為 `gbrain-base-v2`（15 型：person…note），
//! 舊版 `gbrain-base`（Operoid 原本支援的 6 工廠、複數目錄）仍在使用者端存在。
//! pack 名稱取自 gbrain config file plane 的 `schema_pack`（`gbrain_config` 模組），
//! 未設定 → v2（gbrain 0.41.22+ 的預設）；未知 pack → 退回 v2 表。
//!
//! 各管線：
//! - [`Pipeline::People`]：CSV 離線解析（一檔多人）＋ txt/md LLM 結構化（`run_people`）
//! - [`Pipeline::Textual`]：txt/md/pdf → LLM 結構化（`run_textual`）
//! - [`Pipeline::Capture`]：`gbrain capture --type note`（不走 notes repo 檔案）

use serde::Serialize;

use crate::app_config::AppConfig;
use crate::gbrain_config;
use crate::i18n::{AppError, L10n};

/// 轉換管線（決定 run_core 分派與前端 accept 副檔名）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Pipeline {
    /// CSV+txt/md（people 式：CSV 離線、文字走 LLM）
    People,
    /// txt/md/pdf → LLM 結構化
    Textual,
    /// gbrain capture（不寫 notes repo）
    Capture,
}

impl Pipeline {
    /// 前端檔案選擇器的副檔名（也用於分類器的支援副檔名集合）。
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Pipeline::People => &["csv", "txt", "md"],
            Pipeline::Textual => &["txt", "md", "pdf"],
            Pipeline::Capture => &["txt", "md"],
        }
    }
}

/// 單一工廠類型的完整規格。
#[derive(Debug)]
pub struct FactoryTypeSpec {
    /// 工廠 id（= 分類器/前端使用的 factory 字串）。
    pub id: &'static str,
    /// 寫入的 notes repo 子目錄。
    pub dir: &'static str,
    /// frontmatter `type` 預設值（LLM 未給時）。
    pub page_type: &'static str,
    /// frontmatter `tags` 預設值。
    pub default_tags: &'static [&'static str],
    /// 轉換管線。
    pub pipeline: Pipeline,
    /// 正規化同義詞（分類器 LLM 回應對應；含單複數與中文，全小寫比對）。
    pub synonyms: &'static [&'static str],
    /// heuristic 命中時的 canonical kind（見 classifier::heuristic）；無特徵規則者空。
    pub heuristic_kind: &'static str,
    /// 中文短述（LLM prompt／UI fallback 用）。
    pub zh: &'static str,
}

impl FactoryTypeSpec {
    /// 是否為 capture 管線（不寫 notes repo、無可瀏覽目錄）。
    pub fn is_capture(&self) -> bool {
        self.pipeline == Pipeline::Capture
    }
}

/// 一個 schema pack 的類型清單。
pub struct Pack {
    pub name: &'static str,
    pub types: &'static [FactoryTypeSpec],
}

impl Pack {
    /// 以 id 查規格；capture 型（note/inbox）為 fallback 對象。
    pub fn spec(&self, id: &str) -> Result<&'static FactoryTypeSpec, AppError> {
        self.types
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| AppError::new("factory.unknown").p("factory", id))
    }

    /// 分類器同義詞正規化：命中任一 synonym → 該 id；未命中 → capture 型（note/inbox）。
    pub fn normalize(&self, s: &str) -> &'static str {
        let low = s.trim().to_ascii_lowercase();
        for t in self.types {
            if t.id == low || t.synonyms.iter().any(|x| *x == low) {
                return t.id;
            }
        }
        self.catchall_id()
    }

    /// capture 型 id（清單最後一個；v2=note、legacy=inbox）。
    pub fn catchall_id(&self) -> &'static str {
        self.types[self.types.len() - 1].id
    }

    /// heuristic canonical kind（people/company/meeting/projects）→ id。
    pub fn id_of_kind(&self, kind: &str) -> Option<&'static str> {
        self.types
            .iter()
            .find(|t| t.heuristic_kind == kind)
            .map(|t| t.id)
    }

    /// LLM 分類 prompt 的類型枚舉行（`- id：描述`）。
    pub fn prompt_lines(&self) -> String {
        self.types
            .iter()
            .map(|t| format!("- {}：{}", t.id, t.zh))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// gbrain-base-v2（15 型，v0.41.22+ 預設）。dir=type 名（單數）。
pub static V2_PACK: Pack = Pack {
    name: "gbrain-base-v2",
    types: &[
        FactoryTypeSpec {
            id: "person",
            dir: "person",
            page_type: "person",
            default_tags: &["person", "contact"],
            pipeline: Pipeline::People,
            synonyms: &["people", "person", "contact", "聯絡人", "联络人", "人物"],
            heuristic_kind: "people",
            zh: "聯絡人/通訊錄/名片/個人資料",
        },
        FactoryTypeSpec {
            id: "company",
            dir: "company",
            page_type: "company",
            default_tags: &["company", "contact"],
            pipeline: Pipeline::Textual,
            synonyms: &["companies", "company", "公司"],
            heuristic_kind: "company",
            zh: "公司或組織的介紹/背景",
        },
        FactoryTypeSpec {
            id: "media",
            dir: "media",
            page_type: "media",
            default_tags: &["media"],
            pipeline: Pipeline::Textual,
            synonyms: &["media", "媒體", "媒体"],
            heuristic_kind: "",
            zh: "媒體報導/報導文章/新聞剪報",
        },
        FactoryTypeSpec {
            id: "tweet",
            dir: "tweet",
            page_type: "tweet",
            default_tags: &["tweet"],
            pipeline: Pipeline::Textual,
            synonyms: &["tweet", "tweets", "推文"],
            heuristic_kind: "",
            zh: "單則推文/X 貼文",
        },
        FactoryTypeSpec {
            id: "social-digest",
            dir: "social-digest",
            page_type: "social-digest",
            default_tags: &["social-digest"],
            pipeline: Pipeline::Textual,
            synonyms: &["social-digest", "social_digest", "socialdigest", "社群摘要"],
            heuristic_kind: "",
            zh: "社群動態摘要（多則貼文彙整）",
        },
        FactoryTypeSpec {
            id: "analysis",
            dir: "analysis",
            page_type: "analysis",
            default_tags: &["analysis"],
            pipeline: Pipeline::Textual,
            synonyms: &["analysis", "分析"],
            heuristic_kind: "",
            zh: "分析報告/研究筆記",
        },
        FactoryTypeSpec {
            id: "atom",
            dir: "atom",
            page_type: "atom",
            default_tags: &["atom"],
            pipeline: Pipeline::Textual,
            synonyms: &["atom", "atoms", "知識原子"],
            heuristic_kind: "",
            zh: "知識原子（最小可引用單位）",
        },
        FactoryTypeSpec {
            id: "concept",
            dir: "concept",
            page_type: "concept",
            default_tags: &["concept"],
            pipeline: Pipeline::Textual,
            synonyms: &["concepts", "concept", "概念", "主題", "主题"],
            heuristic_kind: "",
            zh: "主題/概念/知識wiki（技術原理、名詞解釋、主題整理）",
        },
        FactoryTypeSpec {
            id: "source",
            dir: "source",
            page_type: "source",
            default_tags: &["source"],
            pipeline: Pipeline::Textual,
            synonyms: &["source", "sources", "來源", "来源"],
            heuristic_kind: "",
            zh: "資料來源/出處說明",
        },
        FactoryTypeSpec {
            id: "deal",
            dir: "deal",
            page_type: "deal",
            default_tags: &["deal"],
            pipeline: Pipeline::Textual,
            synonyms: &["deal", "deals", "交易", "商機"],
            heuristic_kind: "",
            zh: "交易/商機/合作案",
        },
        FactoryTypeSpec {
            id: "email",
            dir: "email",
            page_type: "email",
            default_tags: &["email"],
            pipeline: Pipeline::Textual,
            synonyms: &["email", "emails", "郵件", "邮件"],
            heuristic_kind: "",
            zh: "電子郵件內容",
        },
        FactoryTypeSpec {
            id: "slack",
            dir: "slack",
            page_type: "slack",
            default_tags: &["slack"],
            pipeline: Pipeline::Textual,
            synonyms: &["slack"],
            heuristic_kind: "",
            zh: "Slack 對話/頻道訊息",
        },
        FactoryTypeSpec {
            id: "writing",
            dir: "writing",
            page_type: "writing",
            default_tags: &["writing"],
            pipeline: Pipeline::Textual,
            synonyms: &["writing", "寫作", "写作"],
            heuristic_kind: "",
            zh: "寫作草稿/文章",
        },
        FactoryTypeSpec {
            id: "project",
            dir: "project",
            page_type: "project",
            default_tags: &["project"],
            pipeline: Pipeline::Textual,
            synonyms: &["projects", "project", "專案", "项目"],
            heuristic_kind: "projects",
            zh: "專案/計畫（有里程碑、交付項目、時程、工作包）",
        },
        FactoryTypeSpec {
            id: "note",
            dir: "note",
            page_type: "note",
            default_tags: &["note"],
            pipeline: Pipeline::Capture,
            synonyms: &["note", "notes", "inbox", "筆記", "笔记"],
            heuristic_kind: "",
            zh: "其他一般筆記",
        },
    ],
};

/// gbrain-base（legacy，Operoid 原本支援的 6 工廠；meeting 寫複數 meetings/）。
pub static LEGACY_PACK: Pack = Pack {
    name: "gbrain-base",
    types: &[
        FactoryTypeSpec {
            id: "people",
            dir: "people",
            page_type: "person",
            default_tags: &["people", "contact"],
            pipeline: Pipeline::People,
            synonyms: &["people", "person", "contact", "聯絡人", "联络人"],
            heuristic_kind: "people",
            zh: "聯絡人/通訊錄/名片/個人資料",
        },
        FactoryTypeSpec {
            id: "companies",
            dir: "companies",
            page_type: "company",
            default_tags: &["companies", "contact"],
            pipeline: Pipeline::Textual,
            synonyms: &["companies", "company", "公司"],
            heuristic_kind: "company",
            zh: "公司或組織的介紹/背景",
        },
        FactoryTypeSpec {
            id: "meeting",
            dir: "meetings",
            page_type: "meeting",
            default_tags: &["meeting"],
            pipeline: Pipeline::Textual,
            synonyms: &["meeting", "meetings", "會議", "会议"],
            heuristic_kind: "meeting",
            zh: "會議記錄/逐字稿/議程/開會筆記",
        },
        FactoryTypeSpec {
            id: "concepts",
            dir: "concepts",
            page_type: "concept",
            default_tags: &["concept"],
            pipeline: Pipeline::Textual,
            synonyms: &["concepts", "concept", "概念", "主題", "主题"],
            heuristic_kind: "",
            zh: "主題/概念/知識wiki",
        },
        FactoryTypeSpec {
            id: "projects",
            dir: "projects",
            page_type: "project",
            default_tags: &["project"],
            pipeline: Pipeline::Textual,
            synonyms: &["projects", "project", "專案", "项目"],
            heuristic_kind: "projects",
            zh: "專案/計畫",
        },
        FactoryTypeSpec {
            id: "inbox",
            dir: "inbox",
            page_type: "note",
            default_tags: &["note"],
            pipeline: Pipeline::Capture,
            synonyms: &["note", "notes", "inbox", "筆記", "笔记"],
            heuristic_kind: "",
            zh: "其他一般筆記",
        },
    ],
};

/// 依 pack 名稱取 Pack：None 或未知 → v2（gbrain 0.41.22+ 預設）。
pub fn pack_for(name: Option<&str>) -> &'static Pack {
    match name {
        Some(n) if n.contains("v2") => &V2_PACK,
        Some("gbrain-base") => &LEGACY_PACK,
        _ => &V2_PACK,
    }
}

/// 作用中腦的 pack：讀 gbrain config file plane 的 `schema_pack`（best-effort，
/// 讀不到 → v2 預設）。回傳 (pack, 實際偵測到的 pack 名)。
pub fn active_pack(cfg: &AppConfig) -> (&'static Pack, Option<String>) {
    let name = gbrain_config::load_for(cfg.active_env_home())
        .ok()
        .and_then(|l| l.config.schema_pack.clone());
    (pack_for(name.as_deref()), name)
}

/// 前端 `factory_types` 回傳的單一類型資訊。
#[derive(Debug, Serialize)]
pub struct FactoryTypeInfo {
    pub id: String,
    pub dir: String,
    pub pipeline: Pipeline,
    pub extensions: Vec<&'static str>,
}

/// 作用中 pack 的類型清單（給前端動態渲染）。
pub fn type_infos(pack: &Pack) -> Vec<FactoryTypeInfo> {
    pack.types
        .iter()
        .map(|t| FactoryTypeInfo {
            id: t.id.into(),
            dir: t.dir.into(),
            pipeline: t.pipeline,
            extensions: t.pipeline.extensions().to_vec(),
        })
        .collect()
}

/// LLM 抽取後修正 page_type：空白或非作用中 pack 的型別 → 該工廠預設型。
/// （v2 把 subtype/origin 推到 frontmatter，page_type 一律是 pack 標準型。）
pub fn normalize_page_type(spec: &FactoryTypeSpec, raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        spec.page_type.to_string()
    } else {
        t.to_string()
    }
}

/// pack 名稱不為 v2 系時給前端的升級提示（None=已是 v2 或未偵測到 pack）。
pub fn v2_hint(pack_name: Option<&str>) -> Option<L10n> {
    match pack_name {
        Some(n) if !n.contains("v2") => {
            Some(L10n::new("factories.v2Hint").p("pack", n))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_pack_has_15_types() {
        assert_eq!(V2_PACK.types.len(), 15);
        assert_eq!(V2_PACK.catchall_id(), "note");
        assert_eq!(LEGACY_PACK.types.len(), 6);
        assert_eq!(LEGACY_PACK.catchall_id(), "inbox");
    }

    #[test]
    fn pack_for_fallbacks() {
        assert_eq!(pack_for(None).name, "gbrain-base-v2");
        assert_eq!(pack_for(Some("gbrain-base-v2")).name, "gbrain-base-v2");
        assert_eq!(pack_for(Some("gbrain-base")).name, "gbrain-base");
        assert_eq!(pack_for(Some("custom-pack")).name, "gbrain-base-v2");
    }

    #[test]
    fn normalize_synonyms() {
        assert_eq!(V2_PACK.normalize("People"), "person");
        assert_eq!(V2_PACK.normalize("COMPANY"), "company");
        assert_eq!(LEGACY_PACK.normalize("會議"), "meeting");
        assert_eq!(V2_PACK.normalize("會議"), "note"); // v2 無 meeting 型 → catch-all
        assert_eq!(V2_PACK.normalize("Concept"), "concept");
        assert_eq!(V2_PACK.normalize("專案"), "project");
        assert_eq!(V2_PACK.normalize("inbox"), "note"); // 舊 id 對應到 v2 catch-all
        assert_eq!(V2_PACK.normalize("agenda"), "note"); // 未知 → catch-all
        assert_eq!(LEGACY_PACK.normalize("person"), "people");
        assert_eq!(LEGACY_PACK.normalize("筆記"), "inbox");
    }

    #[test]
    fn kind_lookup_covers_heuristic_kinds() {
        for kind in ["people", "company", "projects"] {
            assert!(V2_PACK.id_of_kind(kind).is_some(), "v2 缺 {kind}");
            assert!(LEGACY_PACK.id_of_kind(kind).is_some(), "legacy 缺 {kind}");
        }
        assert!(LEGACY_PACK.id_of_kind("meeting").is_some());
        assert_eq!(V2_PACK.id_of_kind("meeting"), None); // v2 無 meeting 型 → 交 LLM
        assert_eq!(V2_PACK.id_of_kind("people"), Some("person"));
        assert_eq!(LEGACY_PACK.id_of_kind("meeting"), Some("meeting"));
        assert_eq!(V2_PACK.id_of_kind("nope"), None);
    }

    #[test]
    fn spec_lookup_and_capture() {
        assert_eq!(V2_PACK.spec("person").unwrap().dir, "person");
        assert!(V2_PACK.spec("note").unwrap().is_capture());
        assert!(V2_PACK.spec("people").is_err(), "v2 不該有舊 id");
        assert!(LEGACY_PACK.spec("meeting").unwrap().dir == "meetings");
    }

    #[test]
    fn v2_hint_only_for_non_v2() {
        assert!(v2_hint(Some("gbrain-base")).is_some());
        assert!(v2_hint(Some("gbrain-base-v2")).is_none());
        assert!(v2_hint(None).is_none());
    }
}
