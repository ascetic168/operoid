//! 測試用假後端（M1-WP-C3）——實作 GBrain `pageReadFilter` 的授權語意：
//! **只回 `source_ids` 內的文件**，授權集合之外的內容連候選集都不進（I1）。
//!
//! 真實 GBrain 的對應強制點：`src/core/search/read-policy-sql.ts:11-32`
//! （`source_id = ANY($n)`，M0-V1 實測逐呼叫純量）。

use std::collections::BTreeMap;

use serde_json::json;

use super::backend::{BackendQuery, KnowledgeBackend};
use crate::domain::tools::{ToolFuture, ToolOutput};

#[derive(Debug, Clone)]
pub struct FakeDoc {
    pub title: String,
    pub body: String,
}

/// `source_id → 該 source 的文件`。`BTreeMap` 保證輸出順序確定性（可稽核）。
#[derive(Default)]
pub struct FakeBackend {
    docs: BTreeMap<String, Vec<FakeDoc>>,
}

impl FakeBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_doc(&mut self, source_id: &str, doc: FakeDoc) -> &mut Self {
        self.docs.entry(source_id.to_string()).or_default().push(doc);
        self
    }

    /// 直接檢視某 source 的文件（測試斷言用）。
    #[cfg(test)]
    pub fn docs_of(&self, source_id: &str) -> &[FakeDoc] {
        self.docs.get(source_id).map(Vec::as_slice).unwrap_or(&[])
    }
}

impl KnowledgeBackend for FakeBackend {
    fn retrieve<'a>(
        &'a self,
        _access: &'a super::types::AccessContext,
        req: BackendQuery,
        _ctx: &'a crate::domain::tools::ToolCtx,
    ) -> ToolFuture<'a> {
        Box::pin(async move {
            // I1：逐 source 過濾——不在授權集合內的 source 完全不可見。
            let mut lines: Vec<String> = Vec::new();
            let mut used: Vec<String> = Vec::new();
            for sid in &req.source_ids {
                if let Some(docs) = self.docs.get(sid) {
                    used.push(sid.clone());
                    for d in docs {
                        let hay = format!("{}\n{}", d.title, d.body).to_lowercase();
                        if hay.contains(req.query.to_lowercase().as_str())
                            || req.query
                                .split_whitespace()
                                .any(|w| !w.is_empty() && hay.contains(&w.to_lowercase()))
                        {
                            lines.push(format!("[{}] {} -- {}", sid, d.title, d.body));
                        }
                    }
                }
            }
            lines.sort();
            let text = if lines.is_empty() {
                "No results.".to_string()
            } else {
                lines.join("\n")
            };
            Ok(ToolOutput {
                text,
                meta: json!({
                    "backend": "fake",
                    "authorized_sources": req.source_ids,
                    "sources_with_hits": used,
                    "count": lines.len(),
                }),
                images: Vec::new(),
            })
        })
    }
}
