//! gbrain MCP client（stdio）——`gbrain serve` 的輕量封裝。
//!
//! D1 的 provider 演進：Employee 知識工具（think/query）優先走 MCP（結構化、免 parse
//! CLI stdout），任何失敗由呼叫端 fallback 回 CLI 子行程（見 `runtime::GbrainThinkTool`）。
//! Session 採 lazy 單例：首次呼叫才 spawn `<exe> serve` 並完成 initialize 握手；
//! 呼叫失敗即丟棄 session（下次重連），不做常駐重試。管理操作（sync/sources/config）
//! 仍走 CLI，不經此模組。

use rmcp::model::{CallToolRequestParams, ContentBlock};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::ServiceExt;

/// 一個 gbrain MCP client（對應一個腦：exe + GBRAIN_HOME）。
pub struct GbrainMcpClient {
    exe: String,
    home: Option<String>,
    session: tokio::sync::Mutex<Option<RunningService<RoleClient, ()>>>,
}

impl std::fmt::Debug for GbrainMcpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GbrainMcpClient")
            .field("exe", &self.exe)
            .field("home", &self.home)
            .finish_non_exhaustive()
    }
}

impl GbrainMcpClient {
    pub fn new(exe: String, home: Option<String>) -> Self {
        Self {
            exe,
            home,
            session: tokio::sync::Mutex::new(None),
        }
    }

    /// 取得（或建立）MCP session：spawn `gbrain serve`（stdio）並完成 initialize 握手。
    async fn ensure_session(
        &self,
    ) -> anyhow::Result<
        tokio::sync::MutexGuard<'_, Option<RunningService<RoleClient, ()>>>,
    > {
        use tokio::sync::MutexGuard;
        let mut guard: MutexGuard<'_, _> = self.session.lock().await;
        if guard.is_none() {
            let mut cmd = tokio::process::Command::new(&self.exe);
            cmd.arg("serve");
            crate::proc::no_console_async(&mut cmd);
            // rmcp 自管 stdio（piped）；僅 stderr 保持繼承以便診斷。
            for (k, v) in crate::proc::env_for_brain(self.home.as_deref()) {
                cmd.env(k, v);
            }
            let transport = TokioChildProcess::new(cmd)?;
            let client: RunningService<RoleClient, ()> = ().serve(transport).await?;
            *guard = Some(client);
        }
        Ok(guard)
    }

    /// 呼叫一個 MCP tool，回傳所有 text content 合併的文字。失敗時丟棄 session
    /// （下次呼叫重連）——呼叫端以 CLI fallback 兜底。
    pub async fn call(&self, tool: &str, args: serde_json::Value) -> anyhow::Result<String> {
        let result = {
            let mut guard = self.ensure_session().await?;
            let Some(client) = guard.as_mut() else {
                anyhow::bail!("MCP session unavailable");
            };
            let mut params = CallToolRequestParams::new(tool.to_owned());
            params.arguments = args.as_object().cloned();
            client.call_tool(params).await
        };
        match result {
            Ok(res) => {
                if res.is_error.unwrap_or(false) {
                    anyhow::bail!("MCP tool {tool} 回報錯誤");
                }
                let mut text = String::new();
                for c in &res.content {
                    if let ContentBlock::Text(t) = c {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(&t.text);
                    }
                }
                Ok(text)
            }
            Err(e) => {
                // 丟棄 session：process 可能已死；close best-effort。
                let mut guard = self.session.lock().await;
                if let Some(mut client) = guard.take() {
                    let _ = client.close().await;
                }
                Err(anyhow::anyhow!("MCP call {tool} 失敗：{e}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_format_contains_exe() {
        let c = GbrainMcpClient::new("gbrain".into(), None);
        let s = format!("{c:?}");
        assert!(s.contains("gbrain"));
    }
}
