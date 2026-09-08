//! W3：員工的第一個「行動」工具——把產出寫成 markdown 筆記（D-H2）。
//!
//! 沙箱：根目錄＝[`AppConfig::employee_output_path`]（預設 `{home}/employee-output`，
//! 在 notes repo **之外**——不觸發 sync、不入圖譜）。人工 review 滿意後手動移入
//! notes repo，走既有 sync 晉升進知識圖譜——即 Handbook Ch.07 §6「衍生知識：
//! 產出→驗證→晉升」的最小落地。收回＝刪檔（零風險；gbrain「刪檔＋sync＝軟刪除」
//! 僅在晉升之後才需要）。
//!
//! 防護：檔名僅接受單一 `.md` 檔名（拒絕絕對路徑／路徑分隔／`.` 開頭）、拒覆寫、
//! canonicalize 包含檢查（逃離根目錄一律拒絕）。每檔 frontmatter 帶
//! `origin: operoid-employee`＋produced_by——可批次識別員工寫入。
//!
//! 失敗語意仿 [`crate::outbound::SendTool`]：回 `Ok`（人類可讀＋`meta.outcome=error`），
//! 進上下文不自爆——「能不能寫」的後續判斷歸員工。

use serde_json::json;

use crate::domain::tools::{Tool, ToolCtx, ToolFuture, ToolInput, ToolOutput, ToolSpec};

/// write-note 工具 id（Employee/Template `tools` allowlist 的比對鍵）。
pub const TOOL_WRITE_NOTE: &str = "write-note";

pub struct WriteNoteTool {
    spec: ToolSpec,
    root: std::path::PathBuf,
    employee_id: String,
}

impl WriteNoteTool {
    /// `root` 由呼叫端解析（`cfg.employee_output_path`）；工具本身不讀 config——
    /// 每回合建構（仿 SendTool），攜當時的員工身分。
    pub fn new(root: impl Into<std::path::PathBuf>, employee_id: impl Into<String>) -> Self {
        Self {
            spec: ToolSpec {
                id: TOOL_WRITE_NOTE.into(),
                description: "把一份完整產出寫成 markdown 筆記檔（沙箱：員工專屬產出目錄；\
                    不可覆寫既有檔）。params: {\"filename\":\"report.md\",\"title\":\"標題\",\
                    \"content\":\"全文（markdown）\"}"
                    .into(),
            },
            root: root.into(),
            employee_id: employee_id.into(),
        }
    }

    /// 檔名防護＋路徑解析：僅接受根目錄下的單一 `.md` 檔名。
    fn resolve_path(root: &std::path::Path, filename: &str) -> Result<std::path::PathBuf, String> {
        // 拒絕任何路徑分隔——v1 僅接受根目錄下的單一檔名（子目錄是未來擴充）。
        if filename.contains('/') || filename.contains(std::path::MAIN_SEPARATOR) {
            return Err(format!("filename 僅接受單一檔名（不可含路徑分隔）：{filename}"));
        }
        let p = std::path::Path::new(filename);
        if p.is_absolute() {
            return Err(format!("filename 不可為絕對路徑：{filename}"));
        }
        for comp in p.components() {
            if !matches!(comp, std::path::Component::Normal(_)) {
                return Err(format!("filename 僅接受單一檔名（不可含路徑／..）：{filename}"));
            }
        }
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("無效檔名：{filename}"))?
            .to_string();
        if !name.to_lowercase().ends_with(".md") {
            return Err(format!("僅接受 .md 檔（收到：{name}）"));
        }
        if name.starts_with('.') {
            return Err("檔名不可以 . 開頭".into());
        }
        Ok(root.join(name))
    }

    /// 組檔案內容：frontmatter（origin／produced_by／created_at／title）＋本文。
    fn file_body(employee_id: &str, title: &str, content: &str) -> String {
        format!(
            "---\norigin: operoid-employee\nproduced_by: {employee_id}\ncreated_at: {}\ntitle: {}\n---\n\n{content}\n",
            crate::domain::now_rfc3339(),
            title.replace('\n', " "),
        )
    }
}

impl Tool for WriteNoteTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn invoke<'a>(&'a self, input: ToolInput, _ctx: &'a ToolCtx) -> ToolFuture<'a> {
        let params = input.params.clone().unwrap_or_default();
        let filename = params
            .get("filename")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let root = self.root.clone();
        let employee_id = self.employee_id.clone();
        Box::pin(async move {
            let fail = |msg: String| {
                Ok(ToolOutput {
                    text: format!("寫入失敗：{msg}"),
                    meta: json!({"outcome": "error", "error": msg}),
                })
            };
            if filename.is_empty() {
                return fail("缺少 filename".into());
            }
            if content.trim().is_empty() {
                return fail("缺少 content".into());
            }
            let path = match Self::resolve_path(&root, &filename) {
                Ok(p) => p,
                Err(e) => return fail(e),
            };
            if let Err(e) = std::fs::create_dir_all(&root) {
                return fail(format!("無法建立產出目錄（{}）：{e}", root.display()));
            }
            // 包含檢查：以 canonical 根目錄驗父目錄（目標檔尚不存在，canonicalize 不了自身）。
            let canon_root = match std::fs::canonicalize(&root) {
                Ok(r) => r,
                Err(e) => return fail(format!("產出目錄解析失敗：{e}")),
            };
            let parent = path.parent().unwrap_or(&canon_root);
            let canon_parent = std::fs::canonicalize(parent).unwrap_or_else(|_| canon_root.clone());
            if !canon_parent.starts_with(&canon_root) {
                return fail("路徑逃離產出目錄（拒絕）".into());
            }
            if path.exists() {
                return fail(format!("檔案已存在（v1 不覆寫，請換檔名）：{}", path.display()));
            }
            let title_line = if title.is_empty() {
                filename.trim_end_matches(".md").to_string()
            } else {
                title
            };
            let body = Self::file_body(&employee_id, &title_line, &content);
            if let Err(e) = std::fs::write(&path, body) {
                return fail(format!("寫檔失敗：{e}"));
            }
            Ok(ToolOutput {
                text: format!("已寫入筆記：{}", path.display()),
                meta: json!({"outcome": "written", "path": path.to_string_lossy()}),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(dir: &std::path::Path) -> WriteNoteTool {
        WriteNoteTool::new(dir, "emp-1")
    }
    fn input(filename: &str, title: &str, content: &str) -> ToolInput {
        let mut m = serde_json::Map::new();
        m.insert("filename".into(), json!(filename));
        m.insert("title".into(), json!(title));
        m.insert("content".into(), json!(content));
        ToolInput { query: String::new(), anchor: None, params: Some(m) }
    }
    fn dummy_ctx() -> ToolCtx {
        ToolCtx {
            gbrain_exe: "gbrain".into(),
            gbrain_home: None,
            chat_model: None,
            mcp: None,
            employee_output_root: std::path::PathBuf::from("/tmp"),
            allowed_tools: Default::default(),
        }
    }
    fn dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("operoid-wn-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 快樂路徑：檔案落地＋frontmatter（origin／produced_by）＋meta.path。
    #[tokio::test]
    async fn writes_note_with_frontmatter() {
        let d = dir();
        let out = tool(&d)
            .invoke(input("report.md", "週報", "# 本週重點\n- A"), &dummy_ctx())
            .await
            .unwrap();
        assert_eq!(out.meta["outcome"], "written");
        let p = std::path::Path::new(out.meta["path"].as_str().unwrap());
        let body = std::fs::read_to_string(p).unwrap();
        assert!(body.contains("origin: operoid-employee"), "{body}");
        assert!(body.contains("produced_by: emp-1"), "{body}");
        assert!(body.contains("title: 週報"), "{body}");
        assert!(body.contains("# 本週重點"), "{body}");
        std::fs::remove_dir_all(&d).ok();
    }

    /// 路徑防護：`..` 逃逸／絕對路徑／子目錄／非 .md／`.` 開頭——全拒，且不落地任何檔。
    #[tokio::test]
    async fn rejects_path_escapes_and_bad_names() {
        let d = dir();
        let t = tool(&d);
        for bad in ["../evil.md", "sub/dir.md", "notes.txt", ".hidden.md"] {
            let out = t.invoke(input(bad, "t", "c"), &dummy_ctx()).await.unwrap();
            assert_eq!(out.meta["outcome"], "error", "{bad} 應被拒：{out:?}");
        }
        // 絕對路徑（Windows／Unix 各試一種形式）
        for bad in ["/etc/evil.md", "C:\\tmp\\evil.md"] {
            let out = t.invoke(input(bad, "t", "c"), &dummy_ctx()).await.unwrap();
            assert_eq!(out.meta["outcome"], "error", "{bad} 應被拒：{out:?}");
        }
        assert!(std::fs::read_dir(&d).unwrap().count() == 0, "不應有任何檔案落地");
        std::fs::remove_dir_all(&d).ok();
    }

    /// 拒覆寫：同名檔已存在 → error，原內容不變。
    #[tokio::test]
    async fn refuses_overwrite() {
        let d = dir();
        let t = tool(&d);
        t.invoke(input("a.md", "t1", "第一版"), &dummy_ctx()).await.unwrap();
        let out = t.invoke(input("a.md", "t2", "第二版"), &dummy_ctx()).await.unwrap();
        assert_eq!(out.meta["outcome"], "error", "覆寫應被拒");
        let body = std::fs::read_to_string(d.join("a.md")).unwrap();
        assert!(body.contains("第一版"), "原內容應保留：{body}");
        std::fs::remove_dir_all(&d).ok();
    }

    /// 缺 filename／content → error（不落地）。
    #[tokio::test]
    async fn missing_params_fail_cleanly() {
        let d = dir();
        let t = tool(&d);
        let out = t.invoke(input("", "t", "c"), &dummy_ctx()).await.unwrap();
        assert_eq!(out.meta["outcome"], "error");
        let out = t.invoke(input("b.md", "t", "  "), &dummy_ctx()).await.unwrap();
        assert_eq!(out.meta["outcome"], "error");
        assert!(std::fs::read_dir(&d).unwrap().count() == 0);
        std::fs::remove_dir_all(&d).ok();
    }
}
