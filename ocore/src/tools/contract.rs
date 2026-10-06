//! ToolContract——對話工具的單一真相源（M2）。
//!
//! 每個工具宣告：JSON Schema 參數、timeout、輸出上限。native ToolDef（`llm::ToolDef`）
//! 由此生成——system prompt 的能力說明也由同一份清單生成，員工看不到的工具就選不了
//! （封閉白名單，Ch.20 §5 的廉價實現）。
//!
//! allowlist 慣例：工具 id 用連字號（如 `write-note`，Employee/Template `tools` 欄位的
//! 比對鍵）；native function 名用底線（如 `write_note`，provider 相容性最佳）。

use crate::llm::ToolDef;

// ── allowlist id（Employee/Template `tools` 比對鍵）──
pub const TOOL_READ_FILE: &str = "read-file";
pub const TOOL_WRITE_FILE: &str = "write-file";
pub const TOOL_EDIT_FILE: &str = "edit-file";
pub const TOOL_RUN_COMMAND: &str = "run-command";

// ── native function 名（OpenAI function calling）──
pub const FN_READ_FILE: &str = "read_file";
pub const FN_WRITE_FILE: &str = "write_file";
pub const FN_EDIT_FILE: &str = "edit_file";
pub const FN_RUN_COMMAND: &str = "run_command";

/// 單一工具的契約（schema＋預算）。M2 先列原生五工具＋桌面四工具；連接器未來由此註冊。
pub struct ToolContract {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: serde_json::Value,
}

fn obj_schema(props: serde_json::Value, required: &[&str]) -> serde_json::Value {
    let mut v = serde_json::json!({ "type": "object", "properties": props });
    if !required.is_empty() {
        v["required"] = serde_json::json!(required);
    }
    v
}

/// 對話回合可用的工具契約（allowlist 閘門在此套用：write_note／桌面工具未授權即不存在）。
pub fn turn_tool_contracts(ctx: &crate::domain::tools::ToolCtx) -> Vec<ToolContract> {
    let mut defs = vec![
        ToolContract {
            name: "gbrain_search",
            description: "快速檢索知識圖譜的相關頁面（無合成、省時；查資料／找原文時優先用）。",
            parameters: obj_schema(
                serde_json::json!({"query": {"type": "string", "description": "檢索語詞"}}),
                &["query"],
            ),
        },
        ToolContract {
            name: "gbrain_think",
            description: "對知識圖譜做多跳引用合成（需要綜合結論／依據才回答時用；較慢）。",
            parameters: obj_schema(
                serde_json::json!({"query": {"type": "string", "description": "要綜合的問題"}}),
                &["query"],
            ),
        },
        ToolContract {
            name: "send_message",
            description: "把訊息寄給外部對象（經 bridge；內部對話也會留紀錄）。",
            parameters: obj_schema(
                serde_json::json!({
                    "to": {"type": "string", "description": "送達目標（可省＝回覆喚醒你的這則訊息）"},
                    "text": {"type": "string", "description": "要外發的訊息全文"}
                }),
                &["text"],
            ),
        },
        ToolContract {
            name: "propose_commitment",
            description: "提案一個長期承諾；屬已授權類別者可免核可自動啟用，其餘待人類核可。",
            parameters: obj_schema(
                serde_json::json!({
                    "title": {"type": "string", "description": "承諾標題"},
                    "condition": {"type": "string", "description": "完成條件"},
                    "category": {"type": "string", "description": "已授權類別 id（可選）"}
                }),
                &["title", "condition"],
            ),
        },
        ToolContract {
            name: "finish",
            description: "結束本回合。text 為給人類的最終回覆（省略＝不回覆）。直接以文字回應也可以——兩者擇一。",
            parameters: obj_schema(
                serde_json::json!({"text": {"type": "string", "description": "給人類的最終回覆（可省）"}}),
                &[],
            ),
        },
    ];
    if ctx.allowed_tools.contains(crate::write_note::TOOL_WRITE_NOTE) {
        defs.push(ToolContract {
            name: "write_note",
            description: "把一份完整產出寫成筆記檔（落於你的專屬產出目錄，供人類審閱）。",
            parameters: obj_schema(
                serde_json::json!({
                    "filename": {"type": "string", "description": "檔名（.md）"},
                    "title": {"type": "string", "description": "標題"},
                    "content": {"type": "string", "description": "完整 markdown 全文"}
                }),
                &["filename", "content"],
            ),
        });
    }
    // ── 桌面工具（M2）：allowlist 閘門——未授權即不存在 ──
    if ctx.allowed_tools.contains(TOOL_READ_FILE) {
        defs.push(ToolContract {
            name: FN_READ_FILE,
            description: "讀取你工作區裡的一個檔案（帶行號；可 offset/limit 分段）。",
            parameters: obj_schema(
                serde_json::json!({
                    "path": {"type": "string", "description": "工作區內的相對路徑"},
                    "offset": {"type": "integer", "description": "起始行（1-based；可省）"},
                    "limit": {"type": "integer", "description": "最多讀幾行（可省，預設 2000）"}
                }),
                &["path"],
            ),
        });
    }
    if ctx.allowed_tools.contains(TOOL_WRITE_FILE) {
        defs.push(ToolContract {
            name: FN_WRITE_FILE,
            description: "把完整內容寫入工作區的一個檔案（建目錄；覆寫既有檔）。",
            parameters: obj_schema(
                serde_json::json!({
                    "path": {"type": "string", "description": "工作區內的相對路徑"},
                    "content": {"type": "string", "description": "完整檔案內容"}
                }),
                &["path", "content"],
            ),
        });
    }
    if ctx.allowed_tools.contains(TOOL_EDIT_FILE) {
        defs.push(ToolContract {
            name: FN_EDIT_FILE,
            description: "以精確字串替換編輯工作區裡的檔案（必須先用 read_file 讀過）。",
            parameters: obj_schema(
                serde_json::json!({
                    "path": {"type": "string", "description": "工作區內的相對路徑"},
                    "old_string": {"type": "string", "description": "要替換的原文（須精確且唯一）"},
                    "new_string": {"type": "string", "description": "替換後的新文"},
                    "replace_all": {"type": "boolean", "description": "替換所有相符處（預設 false）"}
                }),
                &["path", "old_string", "new_string"],
            ),
        });
    }
    if ctx.allowed_tools.contains(TOOL_RUN_COMMAND) {
        defs.push(ToolContract {
            name: FN_RUN_COMMAND,
            description: "在你的工作區執行一個 shell 指令（cwd＝工作區；逾時 120 秒；輸出上限 30KB）。",
            parameters: obj_schema(
                serde_json::json!({
                    "command": {"type": "string", "description": "要執行的指令"},
                    "timeout_ms": {"type": "integer", "description": "逾時毫秒（可省，上限 600000）"}
                }),
                &["command"],
            ),
        });
    }
    defs
}

/// native 工具定義（`llm::ToolDef`）——system prompt 的能力說明由同一份清單生成。
pub fn turn_tool_defs(ctx: &crate::domain::tools::ToolCtx) -> Vec<ToolDef> {
    turn_tool_contracts(ctx)
        .into_iter()
        .map(|c| ToolDef {
            name: c.name.to_string(),
            description: c.description.to_string(),
            parameters: c.parameters,
        })
        .collect()
}
