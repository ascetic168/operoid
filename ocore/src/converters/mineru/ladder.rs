//! MinerU 取用解析階梯（K1）——安裝形態因人而異，逐級 fallback。
//!
//! 階梯（計畫 §K1）：
//! 1. **設定覆寫**：`mineru_command`＝完整可執行路徑或命令模板。機理（2026-10-09 實測）：
//!    venv `Scripts/*.exe` 是 **launcher**——內部寫死該 venv `python.exe` 絕對路徑，
//!    **原地以絕對路徑呼叫免 activate**；但不可複製攜帶、venv 搬移即失效
//!    （`Fatal error in launcher`）→ 探測失敗自動降下一級。`conda run`／`uv run`／WSL
//!    皆以模板機制涵蓋。
//! 2. **PATH 查找**：`mineru-kit`（涵蓋 `uv tool install`／`pipx install`／pip --user）。
//! 3. **遠端解析——依「信任層級」治理，public 預設擋**：
//!    - `private`：自架端點（`mineru_remote_url`，區網 api-server）——**設定本身即同意**
//!      （端點 URL 必須明確配置才存在），文件只在自有基礎設施內流動。
//!    - `public`：第三方公有雲（mineru.net，需 `mineru_api_key`）——
//!      `allow_public_egress`（**預設 false**）鎖住；關閉時跳過該級並警告。
//! 4. **降級**：pdf_extract naive 文字抽取（無圖、品質降），標記轉換品質警告。
//!
//! egress 稽核：每次解析結果（層級＋端點）回報給呼叫端寫進 conversion-report.json。

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use super::ConvertConfig;

/// egress 信任層級（K1 分層治理；K5/K8 的 VLM 呼叫同受此分層約束）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EgressTier {
    /// 本機（venv/PATH 安裝）——永遠允許。
    Local,
    /// 使用者／企業自設端點——設定本身即同意。
    Private,
    /// 第三方公有雲——`allow_public_egress`（預設 false）鎖住。
    Public,
}

impl EgressTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            EgressTier::Local => "local",
            EgressTier::Private => "private",
            EgressTier::Public => "public",
        }
    }
}

/// 階梯單級的嘗試紀錄（可稽核）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LadderAttempt {
    pub step: &'static str,
    pub detail: String,
    pub resolved: bool,
}

/// 階梯解析結果。`program.is_none()`＝全級落空 → 呼叫端降級 pdf_extract。
#[derive(Debug, Clone, Default)]
pub struct LadderOutcome {
    pub attempts: Vec<LadderAttempt>,
    /// 解析到的程式（本機＝可執行路徑；遠端＝"mineru-kit" 名義）。
    pub program: Option<String>,
    /// 命令模板的前置參數（如 `conda run -n m` 的 `["conda","run","-n","m"]`）。
    pub template_args: Vec<String>,
    pub tier: Option<EgressTier>,
    /// 遠端端點（稽核用；本機為 None）。
    pub endpoint: Option<String>,
    pub warnings: Vec<String>,
}

impl LadderOutcome {
    pub fn resolved(&self) -> bool {
        self.program.is_some()
    }
    fn ok(&mut self, step: &'static str, detail: String) {
        self.attempts.push(LadderAttempt { step, detail: detail.clone(), resolved: true });
    }
    fn miss(&mut self, step: &'static str, detail: String) {
        self.attempts.push(LadderAttempt { step, detail, resolved: false });
    }
}

/// 跑階梯（純檔案存在性／設定判斷，**不 spawn**——與 prereq 的快速路徑同紀律）。
pub fn resolve(cfg: &ConvertConfig) -> LadderOutcome {
    let mut o = LadderOutcome::default();

    // 1. 設定覆寫：完整路徑或命令模板。
    if let Some(cmd) = cfg.mineru_command.as_deref().filter(|s| !s.trim().is_empty()) {
        let parts = split_template(cmd);
        if parts.is_empty() {
            o.miss("1.config", "設定為空".into());
        } else {
            let program = resolve_executable(&parts[0]);
            match program {
                Some(p) => {
                    o.ok("1.config", format!("設定覆寫命中：{}", p.display()));
                    o.program = Some(p.to_string_lossy().into_owned());
                    o.template_args = parts[1..].to_vec();
                    o.tier = Some(EgressTier::Local);
                    return o;
                }
                None => {
                    // 模板首詞允許不是路徑（conda/uv/ssh 等——在 PATH 上）
                    if looks_like_bare_command(&parts[0]) {
                        o.ok("1.config", format!("設定覆寫（命令模板）：{}", parts[0]));
                        o.program = Some(parts[0].clone());
                        o.template_args = parts[1..].to_vec();
                        o.tier = Some(EgressTier::Local);
                        return o;
                    }
                    o.miss(
                        "1.config",
                        format!("設定的程式不存在（venv 搬移或刪除？）：{}", parts[0]),
                    );
                }
            }
        }
    }

    // 2. PATH 查找 mineru-kit。
    let path_env = std::env::var("PATH").unwrap_or_default();
    match find_on_path("mineru-kit", &path_env) {
        Some(p) => {
            o.ok("2.path", format!("PATH 命中：{}", p.display()));
            o.program = Some(p.to_string_lossy().into_owned());
            o.tier = Some(EgressTier::Local);
            return o;
        }
        None => o.miss("2.path", "PATH 上無 mineru-kit".into()),
    }

    // 3. 遠端——private（自架端點）先行：設定即同意。
    if let Some(url) = cfg.mineru_remote_url.as_deref().filter(|s| !s.trim().is_empty()) {
        o.ok("3.remote-private", format!("自架遠端解析端點（設定即同意）：{url}"));
        o.program = Some("mineru-kit".into());
        o.tier = Some(EgressTier::Private);
        o.endpoint = Some(url.to_string());
        return o;
    }
    o.miss("3.remote-private", "未設定 mineru_remote_url".into());

    // 3b. 遠端——public（mineru.net）：預設擋，須 allow_public_egress opt-in。
    if cfg.mineru_api_key.as_deref().is_some_and(|k| !k.trim().is_empty()) {
        if cfg.allow_public_egress {
            o.ok("3.remote-public", "mineru.net（public；已 opt-in allow_public_egress）".into());
            o.program = Some("mineru-kit".into());
            o.tier = Some(EgressTier::Public);
            o.endpoint = Some("https://mineru.net".into());
            return o;
        }
        o.warnings.push(
            "偵測到 mineru.net API key，但 allow_public_egress=false（預設）——public 第三方雲 \
             egress 已擋；如需使用請明確開啟設定（文件會上傳第三方雲）。"
                .into(),
        );
    }
    o.miss("3.remote-public", "未設定 mineru_api_key 或 public egress 未開".into());

    // 4. 全級落空 → 呼叫端降級（此處僅註記；warning 由呼叫端加品質警告）。
    o
}

/// 粗判是否「裸命令」（非路徑——交由 PATH 解析；如 conda/uv/python）。
fn looks_like_bare_command(s: &str) -> bool {
    !s.contains('/') && !s.contains('\\') && !s.trim_end_matches(".exe").contains('.')
        || s.eq_ignore_ascii_case("mineru-kit")
        || s.starts_with("conda ")
        || s.starts_with("uv ")
}

/// 命令模板切分（簡單引號感知；`conda run -n m mineru-kit` → 4 段）。
pub fn split_template(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in cmd.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// 執行檔解析：存在即回；無副檔名時補 .exe 再試（Windows 慣例）。
pub fn resolve_executable(p: &str) -> Option<PathBuf> {
    let path = Path::new(p);
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let with_exe = PathBuf::from(format!("{p}.exe"));
    if with_exe.is_file() {
        return Some(with_exe);
    }
    None
}

/// 在 `path_var`（PATH 值）中找程式——純函式（測試可餵自訂值，不碰環境）。
pub fn find_on_path(program: &str, path_var: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .map(|s| s.to_ascii_lowercase())
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(path_var) {
        for ext in &exts {
            let cand = dir.join(format!("{program}{ext}"));
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// 執行 `mineru-kit parse … -f zip` 並回傳解壓後的 structured_content.json 路徑。
/// work 目錄會留下原始 zip 與解壓內容（圖檔是 K3 sidecar 的嵌入來源，不可清）。
pub async fn parse_zip(
    res: &LadderOutcome,
    cfg: &ConvertConfig,
    pdf: &Path,
    work: &Path,
) -> Result<PathBuf> {
    let program = res
        .program
        .as_deref()
        .ok_or_else(|| anyhow!("MinerU 階梯未解析"))?;
    std::fs::create_dir_all(work)?;
    let args = build_parse_args(res, cfg, &pdf.to_string_lossy(), &work.to_string_lossy());

    let mut cmd = tokio::process::Command::new(program);
    cmd.args(&args).env("PYTHONUTF8", "1");
    crate::proc::no_console_async(&mut cmd);
    let out = tokio::time::timeout(std::time::Duration::from_secs(900), cmd.output())
        .await
        .map_err(|_| anyhow!("MinerU 解析逾時（>900s）——CPU basic 約 10 秒/頁，可改 --tier flash"))?
        .with_context(|| format!("spawn {program} {:?} 失敗", args.first().unwrap_or(&String::new())))?;
    if !out.status.success() {
        return Err(anyhow!(
            "MinerU 解析失敗（exit {:?}）：{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim().chars().take(600).collect::<String>()
        ));
    }

    // 產出：work 下應有 <stem>.zip（mineru-kit -f zip 不自動解壓）。
    let zip = newest_zip(work)?;
    let dest = work.join("unzipped");
    std::fs::create_dir_all(&dest)?;
    unzip(&zip, &dest).with_context(|| format!("解壓 {} 失敗", zip.display()))?;
    let sc = dest.join("structured_content.json");
    if !sc.exists() {
        return Err(anyhow!("zip 內無 structured_content.json：{}", zip.display()));
    }
    Ok(sc)
}

/// 組 `parse` 參數（純函式；測試斷言遠端旗標與信任層的組合）。
pub fn build_parse_args(res: &LadderOutcome, cfg: &ConvertConfig, pdf: &str, out_dir: &str) -> Vec<String> {
    let mut a: Vec<String> = res.template_args.clone();
    a.push("parse".into());
    a.push(pdf.into());
    a.push("-o".into());
    a.push(out_dir.into());
    a.push("-f".into());
    a.push("zip".into());
    a.push("-p".into());
    a.push("all".into());
    a.push("--tier".into());
    a.push(cfg.mineru_tier.clone());
    match res.tier {
        Some(EgressTier::Private) => {
            a.push("--remote-url".into());
            a.push(res.endpoint.clone().unwrap_or_default());
            if let Some(k) = cfg.mineru_api_key.as_deref().filter(|k| !k.trim().is_empty()) {
                a.push("--api-key".into());
                a.push(k.into());
            }
        }
        Some(EgressTier::Public) => {
            a.push("--remote".into());
            if let Some(k) = cfg.mineru_api_key.as_deref().filter(|k| !k.trim().is_empty()) {
                a.push("--api-key".into());
                a.push(k.into());
            }
        }
        _ => {}
    }
    a
}

fn newest_zip(dir: &Path) -> Result<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in std::fs::read_dir(dir)?.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")) {
            let m = e.metadata()?.modified()?;
            if best.as_ref().is_none_or(|(t, _)| m > *t) {
                best = Some((m, p));
            }
        }
    }
    best.map(|(_, p)| p).ok_or_else(|| anyhow!("MinerU 未產出 zip：{}", dir.display()))
}

/// 解壓（zip crate；deflate/stored 涵蓋 mineru-kit 產出）。
pub fn unzip(archive: &Path, dest: &Path) -> Result<()> {
    let f = std::fs::File::open(archive)?;
    let mut z = zip::ZipArchive::new(f)?;
    for i in 0..z.len() {
        let mut entry = z.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else { continue }; // 跳過路徑攻擊條目
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut w = std::fs::File::create(&out)?;
            std::io::copy(&mut entry, &mut w)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ConvertConfig {
        ConvertConfig::default()
    }

    /// 階梯 1：設定覆寫（存在的 exe）→ Local。
    #[test]
    fn config_override_resolves_local() {
        let exe = std::env::current_exe().unwrap(); // 測試行程自身必存在
        let mut c = cfg();
        c.mineru_command = Some(exe.to_string_lossy().into_owned());
        let o = resolve(&c);
        assert!(o.resolved());
        assert_eq!(o.tier, Some(EgressTier::Local));
        assert_eq!(o.attempts[0].step, "1.config");
    }

    /// 階梯 1：命令模板（conda run …）——首詞非路徑亦接受（PATH 解析）。
    #[test]
    fn config_template_bare_command() {
        let mut c = cfg();
        c.mineru_command = Some("conda run -n mineru mineru-kit".into());
        let o = resolve(&c);
        assert!(o.resolved());
        assert_eq!(o.template_args, vec!["run", "-n", "mineru", "mineru-kit"]);
    }

    /// 階梯 1 落空（venv 搬移）→ 不擋階梯：記 miss 續走。此處 PATH 無 mineru-kit、
    /// 無遠端設定 → 全落空（degraded）。
    #[test]
    fn missing_venv_falls_through_to_degrade() {
        let mut c = cfg();
        c.mineru_command = Some("Z:/nonexistent/venv/mineru-kit.exe".into());
        let o = resolve(&c);
        assert!(!o.resolved());
        assert!(o.attempts.iter().any(|a| a.step == "1.config" && !a.resolved));
    }

    /// egress 分層：private 自架端點＝設定即同意；public 預設擋、opt-in 才放行。
    #[test]
    fn egress_trust_tiers() {
        let mut c = cfg();
        c.mineru_remote_url = Some("http://192.168.1.10:8000".into());
        let o = resolve(&c);
        assert_eq!(o.tier, Some(EgressTier::Private));
        assert_eq!(o.endpoint.as_deref(), Some("http://192.168.1.10:8000"));

        let mut c = cfg();
        c.mineru_api_key = Some("k-test".into());
        let o = resolve(&c);
        assert!(!o.resolved(), "public 預設必須擋");
        assert!(o.warnings.iter().any(|w| w.contains("allow_public_egress")));

        c.allow_public_egress = true;
        let o = resolve(&c);
        assert_eq!(o.tier, Some(EgressTier::Public));
    }

    /// 遠端旗標組合：private → --remote-url；public → --remote（＋api-key）。
    #[test]
    fn parse_args_carry_remote_flags() {
        let mut c = cfg();
        c.mineru_api_key = Some("key-1".into());
        let mut res = LadderOutcome {
            program: Some("mineru-kit".into()),
            tier: Some(EgressTier::Private),
            endpoint: Some("http://intra:8000".into()),
            ..Default::default()
        };
        let a = build_parse_args(&res, &c, "a.pdf", "out");
        let s = a.join(" ");
        assert!(s.contains("--remote-url http://intra:8000"));
        assert!(s.contains("--api-key key-1"));
        assert!(s.contains("-f zip -p all --tier"));
        res.tier = Some(EgressTier::Public);
        res.endpoint = None;
        let a = build_parse_args(&res, &c, "a.pdf", "out");
        assert!(a.contains(&"--remote".to_string()));
        assert!(!a.contains(&"--remote-url".to_string()));
    }

    /// 模板切分：引號內空白保留。
    #[test]
    fn template_split_respects_quotes() {
        assert_eq!(
            split_template(r#""C:\My Tools\kit.exe" parse"#),
            vec![r"C:\My Tools\kit.exe", "parse"]
        );
    }

    /// PATH 查找為純函式：餵自訂值不碰環境。
    #[test]
    fn find_on_path_is_pure() {
        let dir = std::env::temp_dir().join(format!("mineru-ladder-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("mineru-kit.exe");
        std::fs::write(&p, b"").unwrap();
        let found = find_on_path("mineru-kit", &dir.to_string_lossy());
        assert_eq!(found.unwrap(), p);
        std::fs::remove_file(&p).ok();
    }
}
