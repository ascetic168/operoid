//! MinerU 取用解析階梯（K1）——安裝形態因人而異，逐級 fallback。
//!
//! 階梯（計畫 §K1）：
//! 1. **設定覆寫**：`mineru_command`＝完整可執行路徑或命令模板。機理（2026-10-09 實測）：
//!    venv `Scripts/*.exe` 是 **launcher**——內部寫死該 venv `python.exe` 絕對路徑，
//!    **原地以絕對路徑呼叫免 activate**；但不可複製攜帶、venv 搬移即失效
//!    （`Fatal error in launcher`）→ 探測失敗自動降下一級。`conda run`／`uv run`／WSL
//!    皆以模板機制涵蓋。
//! 1b. **設定目錄**：`mineru_dir`＝安裝根目錄（環境變數展開支援 `%VAR%`／`${VAR}`），
//!     探測 `<dir>/mineru-kit`、`<dir>/Scripts/mineru-kit`（Windows venv）、
//!     `<dir>/bin/mineru-kit`（Unix venv/pipx/uv）。寫成 `%USERPROFILE%\…`／`${HOME}/…`
//!     即同一份設定字串跨機器、跨使用者成立。
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
    let path_env = std::env::var("PATH").unwrap_or_default();
    resolve_in_env(cfg, &path_env)
}

/// [`resolve`] 的純函式核心：PATH 由呼叫端注入——測試密閉（機器以 `uv tool install`
/// 裝了 mineru-kit 也不影響「落空／遠端分層」情境的斷言）。
pub fn resolve_in_env(cfg: &ConvertConfig, path_env: &str) -> LadderOutcome {
    let mut o = LadderOutcome::default();

    // 1. 設定覆寫：完整路徑或命令模板（支援環境變數展開——設定字串可攜）。
    if let Some(cmd) = cfg.mineru_command.as_deref().filter(|s| !s.trim().is_empty()) {
        let parts = split_template(cmd);
        if parts.is_empty() {
            o.miss("1.config", "設定為空".into());
        } else {
            let program = resolve_executable(&expand_env(&parts[0]));
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

    // 1b. 設定目錄：`mineru_dir`（安裝根目錄；跨平台候選位——根／Scripts（Windows
    //     venv）／bin（Unix venv、pipx、uv tool））。
    if let Some(dir) = cfg
        .mineru_dir
        .as_deref()
        .map(expand_env)
        .filter(|s| !s.trim().is_empty())
    {
        match probe_dir(Path::new(&dir)) {
            Some(p) => {
                o.ok("1.config-dir", format!("設定目錄命中：{}", p.display()));
                o.program = Some(p.to_string_lossy().into_owned());
                o.tier = Some(EgressTier::Local);
                return o;
            }
            None => o.miss("1.config-dir", format!("目錄內無 mineru-kit：{dir}")),
        }
    }

    // 2. PATH 查找 mineru-kit（涵蓋 uv tool install／pipx／pip --user）。
    match find_on_path("mineru-kit", path_env) {
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
    if is_executable_file(path) {
        return Some(path.to_path_buf());
    }
    let with_exe = PathBuf::from(format!("{p}.exe"));
    if is_executable_file(&with_exe) {
        return Some(with_exe);
    }
    None
}

/// 設定目錄的跨平台候選位：根／`Scripts`（Windows venv）／`bin`（Unix venv、
/// pipx、uv tool）。
fn probe_dir(dir: &Path) -> Option<PathBuf> {
    for sub in ["", "Scripts", "bin"] {
        let base = if sub.is_empty() {
            dir.to_path_buf()
        } else {
            dir.join(sub)
        };
        if let Some(p) = resolve_executable(&base.join("mineru-kit").to_string_lossy()) {
            return Some(p);
        }
    }
    None
}

/// 可執行檔判定：Windows 看存在性（PATHEXT 已由呼叫端語意涵蓋）；Unix 另要求
/// 執行位（venv/pipx/uv 的 launcher 皆帶 +x——擋掉同名非執行檔誤命中）。
fn is_executable_file(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// 展開設定值中的環境變數：`%VAR%`（Windows 慣用）與 `${VAR}`（Unix 慣用）。
/// **未設定的變數保留原樣**（錯誤可見於 miss 訊息，不靜默清空）。
pub fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let pct = rest.find('%');
        let brace = rest.find("${");
        let (pos, kind) = match (pct, brace) {
            (None, None) => {
                out.push_str(rest);
                return out;
            }
            (Some(a), Some(b)) => {
                if a <= b {
                    (a, 0)
                } else {
                    (b, 1)
                }
            }
            (Some(a), None) => (a, 0),
            (_, Some(b)) => (b, 1),
        };
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let (name, consumed) = if kind == 0 {
            // %VAR%（VAR 非空）
            match tail[1..].find('%') {
                Some(e) if e > 0 => (Some(&tail[1..1 + e]), e + 2),
                _ => (None, 0),
            }
        } else {
            // ${VAR}
            match tail[2..].find('}') {
                Some(e) => (Some(&tail[2..2 + e]), e + 3),
                _ => (None, 0),
            }
        };
        match name.and_then(|n| std::env::var(n).ok()) {
            Some(v) => {
                out.push_str(&v);
                rest = &tail[consumed..];
            }
            None => {
                // 變數未設定或模式不完整：原樣保留此一前綴字元（ASCII），繼續掃描。
                out.push_str(&tail[..1]);
                rest = &tail[1..];
            }
        }
    }
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
            if is_executable_file(&cand) {
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
    /// 無遠端設定 → 全落空（degraded）。PATH 由測試注入空值——密閉（機器 PATH 上
    /// 有 `uv tool install` 的 mineru-kit 也不影響本情境）。
    #[test]
    fn missing_venv_falls_through_to_degrade() {
        let mut c = cfg();
        c.mineru_command = Some("Z:/nonexistent/venv/mineru-kit.exe".into());
        let o = resolve_in_env(&c, "");
        assert!(!o.resolved());
        assert!(o.attempts.iter().any(|a| a.step == "1.config" && !a.resolved));
        assert!(o.attempts.iter().any(|a| a.step == "2.path" && !a.resolved));
    }

    /// PATH 階梯命中＝`uv tool install` 安裝形（執行檔在 PATH 上，無 venv）。
    #[test]
    fn path_lookup_resolves_uv_tool_install() {
        let dir = std::env::temp_dir().join(format!("mineru-path-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let name = if cfg!(windows) { "mineru-kit.exe" } else { "mineru-kit" };
        let p = dir.join(name);
        std::fs::write(&p, b"").unwrap();
        make_executable(&p);
        let o = resolve_in_env(&cfg(), &dir.to_string_lossy());
        assert!(o.resolved(), "attempts: {:?}", o.attempts);
        assert_eq!(o.tier, Some(EgressTier::Local));
        assert!(o.attempts.iter().any(|a| a.step == "2.path" && a.resolved));
        std::fs::remove_file(&p).ok();
    }

    /// egress 分層：private 自架端點＝設定即同意；public 預設擋、opt-in 才放行。
    /// PATH 注入空值——本地安裝（venv／uv tool）不得短路遠端分層的斷言。
    #[test]
    fn egress_trust_tiers() {
        let mut c = cfg();
        c.mineru_remote_url = Some("http://192.168.1.10:8000".into());
        let o = resolve_in_env(&c, "");
        assert_eq!(o.tier, Some(EgressTier::Private));
        assert_eq!(o.endpoint.as_deref(), Some("http://192.168.1.10:8000"));

        let mut c = cfg();
        c.mineru_api_key = Some("k-test".into());
        let o = resolve_in_env(&c, "");
        assert!(!o.resolved(), "public 預設必須擋");
        assert!(o.warnings.iter().any(|w| w.contains("allow_public_egress")));

        c.allow_public_egress = true;
        let o = resolve_in_env(&c, "");
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

    /// PATH 查找為純函式：餵自訂值不碰環境（跨平台檔名＋Unix 執行位）。
    #[test]
    fn find_on_path_is_pure() {
        let dir = std::env::temp_dir().join(format!("mineru-ladder-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let name = if cfg!(windows) { "mineru-kit.exe" } else { "mineru-kit" };
        let p = dir.join(name);
        std::fs::write(&p, b"").unwrap();
        make_executable(&p);
        let found = find_on_path("mineru-kit", &dir.to_string_lossy());
        assert_eq!(found.unwrap(), p);
        std::fs::remove_file(&p).ok();
    }

    /// 階梯 1b：設定目錄（根／Scripts／bin 候選位）→ Local。
    #[test]
    fn config_dir_resolves_local() {
        let dir = std::env::temp_dir().join(format!("mineru-dir-{}", std::process::id()));
        let (sub, name) = if cfg!(windows) {
            (Some("Scripts"), "mineru-kit.exe")
        } else {
            (Some("bin"), "mineru-kit")
        };
        let sub_dir = dir.join(sub.unwrap());
        std::fs::create_dir_all(&sub_dir).unwrap();
        let p = sub_dir.join(name);
        std::fs::write(&p, b"").unwrap();
        make_executable(&p);
        let mut c = cfg();
        c.mineru_dir = Some(dir.to_string_lossy().into_owned());
        let o = resolve(&c);
        assert!(o.resolved(), "attempts: {:?}", o.attempts);
        assert_eq!(o.tier, Some(EgressTier::Local));
        assert_eq!(o.program.as_deref(), Some(p.to_string_lossy().as_ref()));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 環境變數展開：`%VAR%`（Windows）與 `${VAR}`（Unix）同效；未設定變數保留原樣。
    #[test]
    fn expand_env_supports_windows_and_unix_syntax() {
        // 家目錄變數兩平台皆設（USERPROFILE＝Windows、HOME＝Unix）。
        let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .expect("家目錄環境變數應存在");
        let win = expand_env(&format!("%{}%/pdf", if cfg!(windows) { "USERPROFILE" } else { "HOME" }));
        let unix = expand_env(&format!("${{{}}}/pdf", if cfg!(windows) { "USERPROFILE" } else { "HOME" }));
        assert_eq!(win, format!("{home}/pdf"));
        assert_eq!(unix, format!("{home}/pdf"));
        // 未設定的變數：保留原樣（不靜默清空）。
        assert_eq!(expand_env("%OPEROID_NOPE_9%/x"), "%OPEROID_NOPE_9%/x");
        assert_eq!(expand_env("${OPEROID_NOPE_9}/x"), "${OPEROID_NOPE_9}/x");
    }

    fn make_executable(p: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(not(unix))]
        let _ = p;
    }

    /// 本機階梯探測（#[ignore]——印出各級嘗試與 PATH 實測；純診斷不斷言）。
    /// 跑法：`cargo test -p ocore real_ladder_probe -- --ignored --nocapture`
    #[ignore = "真實環境探測：印本機 PATH／展開／階梯解析結果"]
    #[test]
    fn real_ladder_probe() {
        let o = resolve(&cfg());
        eprintln!("[ladder] ── 階梯解析（本機環境，無設定覆寫）──");
        for a in &o.attempts {
            eprintln!("[ladder] {:<16} {} {}", a.step, if a.resolved { "HIT " } else { "miss" }, a.detail);
        }
        eprintln!(
            "[ladder] resolved={} program={:?} tier={:?}",
            o.resolved(),
            o.program,
            o.tier.map(|t| t.as_str())
        );
        let path_env = std::env::var("PATH").unwrap_or_default();
        eprintln!(
            "[ladder] find_on_path(mineru-kit, 實際 PATH) → {:?}",
            find_on_path("mineru-kit", &path_env).map(|p| p.display().to_string())
        );
        let home_ref = if cfg!(windows) { "%USERPROFILE%" } else { "${HOME}" };
        eprintln!("[ladder] expand_env({home_ref}) → {}", expand_env(home_ref));
    }
}
