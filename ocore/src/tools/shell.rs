//! 桌面工具：工作區 shell（M2）——員工的「辦公室電腦」：資料處理、腳本、報表轉換。
//!
//! **誠實邊界**：沙箱＝工作區慣例（cwd 鎖工作區）＋allowlist 人類授權（`run-command`），
//! **非 OS 級強制**——shell 技術上可觸及工作區外。OS 級隔離（job object／AppContainer）
//! 是後續工程；本期以「逐員工人類授權」把關（Handbook 授權面：工具未授權即不存在）。
//!
//! 預算：timeout 預設 120s、上限 600s；輸出（stdout+stderr 合計）上限 30KB＋截斷旗標；
//! 逾時 kill 子行程。員工典型用途：處理 FDC 匯出的 CSV、跑公司提供的腳本、產報告。

use std::time::Duration;

use serde_json::Value;

use super::workspace;

/// 輸出上限（bytes；stdout+stderr 合計，各自讀到 cap+1 以偵測截斷）。
pub const MAX_OUTPUT_BYTES: usize = 30_000;
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;

/// 在工作區執行 shell 指令。回傳員工可讀的結果文字（exit code＋輸出）。
pub async fn run_command(root: &std::path::Path, args: &Value) -> Result<String, String> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or("缺 command 參數")?;
    if command.trim().is_empty() {
        return Err("command 不可為空".into());
    }
    let timeout_ms = args
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1_000, MAX_TIMEOUT_MS);
    workspace::ensure_workspace(root).map_err(|e| format!("工作區不可用：{e}"))?;

    let (program, pre) = if cfg!(windows) { ("cmd", "/C") } else { ("sh", "-c") };
    let mut cmd = tokio::process::Command::new(program);
    cmd.arg(pre)
        .arg(&command)
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    crate::proc::no_console_async(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("無法啟動 shell（{program}）：{e}"))?;
    let so = child.stdout.take().expect("stdout piped");
    let se = child.stderr.take().expect("stderr piped");
    let cap = MAX_OUTPUT_BYTES;
    let t_out = tokio::spawn(read_capped(so, cap + 1));
    let t_err = tokio::spawn(read_capped(se, cap + 1));

    let mut timed_out = false;
    let status = match tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait()).await {
        Ok(st) => Some(st.map_err(|e| format!("等待結束失敗：{e}"))?),
        Err(_) => {
            timed_out = true;
            kill_tree(&mut child).await;
            child.wait().await.ok()
        }
    };
    // 讀取端限時收尾（樹狀 kill 後管線應快速 EOF；殘留孫行程不再無限等）。
    let grace = Duration::from_secs(2);
    let out = tokio::time::timeout(grace, t_out).await.ok().and_then(|r| r.ok()).unwrap_or_default();
    let err = tokio::time::timeout(grace, t_err).await.ok().and_then(|r| r.ok()).unwrap_or_default();

    let code = status.and_then(|s| s.code()).unwrap_or(-1);
    let mut body = String::new();
    if !out.is_empty() {
        body.push_str(&format!("\n--- stdout ---\n{}", crate::proc::decode_buf(&out)));
    }
    if !err.is_empty() {
        body.push_str(&format!("\n--- stderr ---\n{}", crate::proc::decode_buf(&err)));
    }
    if out.len() > cap || err.len() > cap {
        body.push_str("\n[輸出已截斷：超過 30KB 上限——請縮小指令輸出（如 head、find 加條件）]");
    }
    let head = if timed_out {
        format!("exit=timeout（超過 {timeout_ms}ms 已強制停止；部分輸出如下）")
    } else {
        format!("exit={code}")
    };
    Ok(format!("{head}{body}"))
}

/// 讀取串流至多 `cap` bytes（防止失控輸出灌爆記憶體；之後續讀但丟棄至 EOF）。
async fn read_capped<S>(mut stream: S, cap: usize) -> Vec<u8>
where
    S: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::with_capacity(cap.min(64 * 1024));
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf.len() < cap {
                    let take = n.min(cap - buf.len());
                    buf.extend_from_slice(&chunk[..take]);
                }
            }
        }
    }
    buf
}

/// 逾時的樹狀 kill：Windows 用 taskkill /T /F（cmd /C 的孫行程共享 stdout 管線——
/// 只殺 cmd 會讓管線被孫行程佔住）；Unix 用 kill。
async fn kill_tree(child: &mut tokio::process::Child) {
    #[cfg(windows)]
    {
        if let Some(pid) = child.id() {
            let mut tk = tokio::process::Command::new("taskkill");
            tk.args(["/PID", &pid.to_string(), "/T", "/F"]);
            crate::proc::no_console_async(&mut tk);
            tk.stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            let _ = tk.output().await;
        }
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "operoid-shell-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// cwd 鎖工作區：列出的檔案即工作區內容（跨平台：走 shell 內建 + 寫檔再讀）。
    #[tokio::test]
    async fn runs_in_workspace_and_reports_exit_code() {
        let root = tmp();
        std::fs::write(root.join("marker.txt"), "hello").unwrap();
        let script = if cfg!(windows) {
            "type marker.txt"
        } else {
            "cat marker.txt"
        };
        let out = run_command(&root, &json!({"command": script})).await.unwrap();
        assert!(out.starts_with("exit=0"), "{out}");
        assert!(out.contains("hello"), "{out}");
        std::fs::remove_dir_all(&root).ok();
    }

    /// 非零 exit code 如實回報（員工可據此自癒）。
    #[tokio::test]
    async fn reports_nonzero_exit() {
        let root = tmp();
        let script = if cfg!(windows) { "exit 3" } else { "exit 3" };
        let out = run_command(&root, &json!({"command": script})).await.unwrap();
        assert!(out.starts_with("exit=3"), "{out}");
        std::fs::remove_dir_all(&root).ok();
    }

    /// 逾時強制停止＋部分輸出保留。
    #[tokio::test]
    async fn times_out_and_kills() {
        let root = tmp();
        // Windows 用 ping（System32 內建；GNU coreutils 的 timeout.exe 在 Git Bash
        // PATH 中會蓋台）——等待 30 秒但不佔 CPU。
        let script = if cfg!(windows) {
            "echo partial & ping -n 30 127.0.0.1 >nul"
        } else {
            "echo partial; sleep 30"
        };
        let out = run_command(&root, &json!({"command": script, "timeout_ms": 1000}))
            .await
            .unwrap();
        assert!(out.contains("exit=timeout"), "{out}");
        assert!(out.contains("partial"), "{out}");
        std::fs::remove_dir_all(&root).ok();
    }

    /// 缺參數／空指令 → 錯誤文字。
    #[tokio::test]
    async fn rejects_empty_command() {
        let root = tmp();
        assert!(run_command(&root, &json!({"command": "  "})).await.is_err());
        assert!(run_command(&root, &json!({})).await.is_err());
        std::fs::remove_dir_all(&root).ok();
    }
}
