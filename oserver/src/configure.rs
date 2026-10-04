//! 部署設定精靈（configure）——首次執行時以終端選項對話完成 DEPLOYMENT.md §2–§6 事宜。
//!
//! 進入點：
//! - **自動觸發**：`run()` 偵測首次執行（settings 目錄無任何設定檔）且 stdin 是 TTY；
//! - **手動子命令**：`oserver configure`（明示呼叫不限 TTY，可管線餵答案）。
//!
//! 寫入：個人模式 → `app-settings.json`（`config::save_config`）；企業模式 → 另寫
//! `operoid.toml`（工作區唯一的 toml writer；存在即企業模式）。管理員帳號直接建於
//! agent DB（Argon2id、`must_change_password=true`——首登強改由 RBAC 閘與前端接手）。
//!
//! 互動紀律：每題有偵測而來的預設值（Enter 採用）、`?` 顯示說明、輸入當場驗證重問、
//! 寫入前有摘要確認閘——取消則不動任何檔案。

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;

use ocore::app_config::AppConfig;
use ocore::domain::store::Store as _;
use ocore::knowledge::identity::{self, AccountSpec};
use ocore::prereq::{check_all, PrereqCache};

use crate::config::{self, DataDirs};
use crate::operoid_toml::{
    GbrainSection, LlmSection, OperoidToml, ServerSection, TlsSection,
};

// ── 首次執行偵測 ─────────────────────────────────────────────────────────────

/// 首次執行＝settings 目錄尚無任何設定檔（operoid.toml 與 app-settings.json 皆缺）。
/// GUI／桌面流程會先寫 app-settings.json，故不受精靈影響。
pub fn needs_setup(settings_dir: &Path) -> bool {
    !settings_dir.join("operoid.toml").exists()
        && !settings_dir.join("app-settings.json").exists()
}

// ── 純函式（單元測試涵蓋） ───────────────────────────────────────────────────

/// gbrain 執行檔候選解析：既有設定值 → `~/.bun/bin` → PATH（where/which，只找位置
/// 不執行——gbrain 是 bun runtime，冷啟可達 20s）。找不到回空字串。
fn resolve_gbrain_candidate(cfg_value: &str, home: Option<&Path>) -> String {
    if !cfg_value.trim().is_empty() && Path::new(cfg_value).is_file() {
        return cfg_value.to_string();
    }
    if let Some(h) = home {
        let exe = if cfg!(windows) { "gbrain.exe" } else { "gbrain" };
        let p = h.join(".bun").join("bin").join(exe);
        if p.is_file() {
            return p.to_string_lossy().into_owned();
        }
    }
    let (cmd, arg) = if cfg!(windows) { ("where", "gbrain") } else { ("which", "gbrain") };
    if let Ok(out) = std::process::Command::new(cmd).arg(arg).output() {
        if out.status.success() {
            if let Some(first) = String::from_utf8_lossy(&out.stdout).lines().next() {
                let p = first.trim();
                if !p.is_empty() && Path::new(p).is_file() {
                    return p.to_string();
                }
            }
        }
    }
    String::new()
}

/// 腦工作目錄三態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrainPathState {
    /// 空字串——略過腦設定。
    Empty,
    /// 目錄存在且含 .gbrain——直接登錄。
    Ready,
    /// 目錄存在但無 .gbrain——需 gbrain init。
    NeedsInit,
    /// 目錄不存在——需建立＋init。
    Missing,
}

fn classify_brain_path(home: &str) -> BrainPathState {
    if home.trim().is_empty() {
        return BrainPathState::Empty;
    }
    let p = Path::new(home);
    if !p.is_dir() {
        return BrainPathState::Missing;
    }
    if p.join(".gbrain").is_dir() {
        BrainPathState::Ready
    } else {
        BrainPathState::NeedsInit
    }
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// 組企業模式 operoid.toml（純函式；序列化 round-trip 由測試把關）。
fn build_toml(
    host: &str,
    port: u16,
    token: &str,
    frontends_dir: Option<&str>,
    tls_cert: Option<&str>,
    tls_key: Option<&str>,
    llm_env: &BTreeMap<String, String>,
    gbrain_exe: &str,
) -> OperoidToml {
    OperoidToml {
        server: ServerSection {
            host: Some(host.to_string()),
            port: Some(port),
            token: Some(token.to_string()),
            frontends_dir: frontends_dir.map(str::to_string),
        },
        tls: TlsSection { cert: tls_cert.map(str::to_string), key: tls_key.map(str::to_string) },
        llm: LlmSection { env: llm_env.clone() },
        gbrain: GbrainSection {
            exe_path: if gbrain_exe.is_empty() { None } else { Some(gbrain_exe.to_string()) },
        },
        ingress: Default::default(),
    }
}

fn gen_token() -> String {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).expect("OS 熵源不可用");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn mask_token(t: &str) -> String {
    let chars: Vec<char> = t.chars().collect();
    if chars.len() > 12 {
        format!("{}…{}", chars[..6].iter().collect::<String>(), chars[chars.len() - 4..].iter().collect::<String>())
    } else {
        "****".into()
    }
}

/// 預設管理員初始密碼（≥8 碼；首登強改）。
pub const DEFAULT_ADMIN_PASSWORD: &str = "admin1234";

// ── 互動輔助 ─────────────────────────────────────────────────────────────────

fn flush_out() {
    let _ = io::stdout().flush();
}

fn read_line() -> anyhow::Result<String> {
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

/// 單行提問：Enter=預設、`?`=說明後重問。
fn ask(prompt: &str, default: &str, help: Option<&str>) -> anyhow::Result<String> {
    loop {
        if default.is_empty() {
            print!("{prompt}：");
        } else {
            print!("{prompt} [{default}]：");
        }
        flush_out();
        let line = read_line()?;
        if line == "?" {
            println!("{}", help.unwrap_or("（無補充說明）"));
            continue;
        }
        return Ok(if line.is_empty() { default.to_string() } else { line });
    }
}

/// y/n 提問；Enter=預設。
fn ask_yn(prompt: &str, default_yes: bool) -> anyhow::Result<bool> {
    let hint = if default_yes { "Y/n" } else { "y/N" };
    loop {
        print!("{prompt} [{hint}]：");
        flush_out();
        match read_line()?.to_ascii_lowercase().as_str() {
            "" => return Ok(default_yes),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("  請輸入 y 或 n。"),
        }
    }
}

/// 帶驗證的提問：驗證失敗印訊息重問。
fn ask_until(prompt: &str, default: &str, help: Option<&str>, validate: impl Fn(&str) -> Result<(), String>) -> anyhow::Result<String> {
    loop {
        let v = ask(prompt, default, help)?;
        match validate(&v) {
            Ok(()) => return Ok(v),
            Err(msg) => println!("  ✗ {msg}"),
        }
    }
}

// ── 1/7 前置檢查 ─────────────────────────────────────────────────────────────

/// 探測 git／bun／gbrain；缺項顯示用途＋安裝指引，可 r 重新檢查。
/// 回傳解析出的 gbrain 執行檔路徑（可能為空）。
fn prereq_step(existing_gbrain: &str) -> anyhow::Result<String> {
    let home = dirs::home_dir();
    loop {
        let gbrain = resolve_gbrain_candidate(existing_gbrain, home.as_deref());
        let deps = check_all(&gbrain, home.as_deref(), &PrereqCache::default());
        println!();
        for d in &deps {
            let mark = if d.available { "✓" } else { "✗" };
            let detail = d.detail.clone().unwrap_or_else(|| {
                if d.name == "bun" || d.name == "gbrain" {
                    "（路徑檢查通過）".into()
                } else {
                    String::new()
                }
            });
            println!("  {mark} {:<8} {}", d.name, detail);
        }
        let missing: Vec<_> = deps.iter().filter(|d| !d.available).collect();
        if missing.is_empty() {
            let _ = ask("前置程式齊備——按 Enter 繼續", "", None)?;
            return Ok(gbrain);
        }
        println!("\n  缺少前置程式——安裝指引：");
        for d in &missing {
            let usage = match d.name.as_str() {
                "git" => "版本控制；工廠寫入後 sync 前需要 git commit",
                "bun" => "gbrain 的執行環境（bun runtime）",
                "gbrain" => "知識腦 CLI——員工的檢索／記憶後端",
                _ => "Operoid 需要的前置程式",
            };
            println!("    · {}：{usage}", d.name);
            println!("      安裝：{}", d.url);
            if d.name == "git" {
                println!("      （Windows 可用：winget install --id Git.Git）");
            }
        }
        println!("    安裝完成後輸入 r 重新檢查。");
        let ans = ask("仍要繼續設定嗎？（r=重新檢查，q=離開）", "Y", None)?;
        match ans.to_ascii_lowercase().as_str() {
            "r" => continue,
            "q" => anyhow::bail!("已取消——未變更任何設定"),
            _ => {
                println!("  ⚠ 缺項環境下繼續：gbrain 相關功能可能無法使用。");
                return Ok(gbrain);
            }
        }
    }
}

// ── 答案蒐集與套用 ───────────────────────────────────────────────────────────

struct Answers {
    enterprise: bool,
    host: String,
    port: u16,
    frontends_dir: Option<String>,
    tls_cert: Option<String>,
    tls_key: Option<String>,
    gbrain_exe: String,
    brain_home: Option<String>,
    brain_create: bool,
    notes_repo: Option<String>,
    llm_env: BTreeMap<String, String>,
    token: String,
    admin_login: String,
    admin_password: String,
    install_service: bool,
}

/// 執行精靈。回傳 false＝使用者在摘要閘取消（未變更任何檔案）。
pub fn run_wizard(dirs: &DataDirs) -> anyhow::Result<bool> {
    let existing_cfg: AppConfig = config::load_config(&dirs.settings_dir);
    let existing_toml = crate::operoid_toml::load(&dirs.settings_dir).unwrap_or(None);

    println!("════════ Operoid oserver 部署設定精靈 ════════");
    println!("  設定寫入位置：{}", dirs.settings_dir.display());
    println!("  [預設] 直接按 Enter 採用；輸入 ? 顯示說明。");

    // 1/7 前置檢查
    let gbrain_exe = prereq_step(&existing_cfg.gbrain_exe_path)?;

    // 2/7 部署模式
    let enterprise = {
        println!("\n── 2/7 部署模式 ──");
        let v = ask_until(
            "部署模式 1)個人（本機 127.0.0.1） 2)企業（內網服務，寫入 operoid.toml）",
            "1",
            Some("個人＝僅本機 loopback，設定寫 app-settings.json；企業＝對內網提供服務，host/port/TLS/token 寫 operoid.toml（存在即企業模式）。"),
            |v| match v {
                "1" | "2" => Ok(()),
                _ => Err("請輸入 1 或 2".into()),
            },
        )?;
        v == "2"
    };

    // 3/7 監聽與前端
    let (host, port, frontends_dir, tls_cert, tls_key) = if enterprise {
        println!("\n── 3/7 監聽與前端 ──");
        let prev = existing_toml.as_ref().map(|t| &t.server);
        let prev_tls = existing_toml.as_ref().map(|t| &t.tls);
        let host_default =
            prev.and_then(|s| s.host.clone()).unwrap_or_else(|| "0.0.0.0".into());
        let host = ask_until(
            "bind host",
            &host_default,
            Some("內網 IP 或 0.0.0.0。非 loopback（127.0.0.1/localhost/::1）必須啟用 TLS（DR-E5 fail-closed）。"),
            |v| if v.trim().is_empty() { Err("host 不可空".into()) } else { Ok(()) },
        )?;
        let port_default =
            prev.and_then(|s| s.port).map(|p: u16| p.to_string()).unwrap_or_else(|| "7340".into());
        let port = ask_until(
            "port",
            &port_default,
            None,
            |v| v.parse::<u16>().map(|_| ()).map_err(|_| "port 必須是 1–65535 的數字".to_string()),
        )?
        .parse::<u16>()?;
        let fe_default =
            prev.and_then(|s| s.frontends_dir.clone()).unwrap_or_else(|| "./frontends".into());
        let frontends_dir = ask(
            "frontends 目錄（含 admin/manager/user 三子目錄）",
            &fe_default,
            Some("三角色前端靜態檔目錄。目錄缺漏僅警告不阻擋——頁面會 404。"),
        )?;
        let fe = Path::new(&frontends_dir);
        for app in ["admin", "manager", "user"] {
            if !fe.join(app).is_dir() {
                println!("  ⚠ 找不到 {frontends_dir}\\{app}——Web 頁面會 404（可稍後補建置複製）。");
            }
        }
        // TLS：非 loopback 強制（與 ensure_transport_safe 同規則）；loopback 可略。
        let (mut cert, mut key) = (None, None);
        loop {
            let cert_default = prev_tls.and_then(|s| s.cert.clone()).unwrap_or_default();
            let c = ask(
                "TLS 憑證 cert 路徑（PEM；Enter=不啟用 TLS）",
                &cert_default,
                Some("內網 CA 簽發（首選）或自簽。自簽產生方式見 DEPLOYMENT.md §4（openssl 指令）。"),
            )?;
            if c.is_empty() {
                if !is_loopback(&host) {
                    println!("  ✗ host「{host}」非 loopback——Bearer token 不得走明文網路，必須設定 TLS。");
                    continue;
                }
                break;
            }
            if !Path::new(&c).is_file() {
                println!("  ✗ 檔案不存在：{c}");
                continue;
            }
            let key_default = prev_tls.and_then(|s| s.key.clone()).unwrap_or_default();
            let k = ask_until(
                "TLS 金鑰 key 路徑（PEM）",
                &key_default,
                None,
                |v| {
                    if v.trim().is_empty() {
                        Err("key 不可空（cert/key 必須成對）".into())
                    } else if !Path::new(v).is_file() {
                        Err(format!("檔案不存在：{v}"))
                    } else {
                        Ok(())
                    }
                },
            )?;
            cert = Some(c);
            key = Some(k);
            break;
        }
        (host, port, Some(frontends_dir), cert, key)
    } else {
        println!("\n── 3/7 監聽與前端 ──");
        println!("  個人模式：127.0.0.1:7340（可用 --host/--port 覆寫）；前端由啟動參數 --frontends-dir 指定。");
        ("127.0.0.1".into(), 7340u16, None, None, None)
    };

    // 4/7 知識腦工作目錄
    let (brain_home, brain_create, notes_repo, llm_env) = {
        println!("\n── 4/7 知識腦工作目錄 ──");
        let env_home = std::env::var("GBRAIN_HOME").unwrap_or_default();
        let existing_home = existing_cfg
            .active_brain_id
            .as_deref()
            .and_then(|id| existing_cfg.brains.iter().find(|b| b.id == id))
            .and_then(|b| b.gbrain_home.clone())
            .unwrap_or_default();
        let brain_default = if !existing_home.is_empty() {
            existing_home
        } else {
            env_home.clone()
        };
        if gbrain_exe.is_empty() {
            println!("  ⚠ 未偵測到 gbrain 執行檔——本節僅能登錄既有腦目錄，無法執行 gbrain init。");
        }
        let home_raw = ask(
            "腦工作目錄（gbrain_home；Enter=略過腦設定）",
            &brain_default,
            Some("腦＝GBrain 知識庫工作目錄（內含 .gbrain/）。員工的檢索與記憶都在這裡。"),
        )?;
        let (brain_home, brain_create) = match classify_brain_path(&home_raw) {
            BrainPathState::Empty => (None, false),
            BrainPathState::Ready => (Some(home_raw.clone()), false),
            BrainPathState::NeedsInit => {
                if ask_yn("目錄存在但尚未初始化（無 .gbrain）——執行 gbrain init 建立新腦？", true)?
                    && !gbrain_exe.is_empty()
                {
                    (Some(home_raw.clone()), true)
                } else {
                    println!("  已略過此腦設定。");
                    (None, false)
                }
            }
            BrainPathState::Missing => {
                if ask_yn("目錄不存在——建立並初始化新腦？", false)? && !gbrain_exe.is_empty() {
                    (Some(home_raw.clone()), true)
                } else {
                    println!("  已略過此腦設定。");
                    (None, false)
                }
            }
        };
        let notes_repo_raw = ask(
            "筆記 repo 路徑（notes_repo_path；Enter=沿用現值）",
            &existing_cfg.notes_repo_path,
            Some("員工寫筆記的 git repo；可留空之後於設定頁補。"),
        )?;
        let notes_repo = if notes_repo_raw.is_empty() { None } else { Some(notes_repo_raw) };
        let existing_keys: Vec<String> = existing_cfg.llm_env.keys().cloned().collect();
        let llm_default = if existing_keys.is_empty() {
            String::new()
        } else {
            format!("（現有：{}）", existing_keys.join(", "))
        };
        let llm_hint = if llm_default.is_empty() { "略過" } else { "沿用" };
        let llm_raw = ask(
            &format!("LLM 環境變數 KEY=VALUE（逗號分隔；Enter={llm_hint}）"),
            &llm_default,
            Some("服務帳戶看不到使用者環境變數——LLM keys 需寫入設定。例：ZHIPUAI_API_KEY=xxx,OPENAI_API_KEY=yyy"),
        )?;
        let mut llm_env = existing_cfg.llm_env.clone();
        if !llm_raw.starts_with('（') {
            for pair in llm_raw.split(|c| c == ',' || c == '，') {
                let pair = pair.trim();
                if pair.is_empty() {
                    continue;
                }
                match pair.split_once('=') {
                    Some((k, v)) if !k.trim().is_empty() && !v.trim().is_empty() => {
                        llm_env.insert(k.trim().to_string(), v.trim().to_string());
                    }
                    _ => println!("  ⚠ 忽略格式錯誤的項目：{pair}（應為 KEY=VALUE）"),
                }
            }
        }
        (brain_home, brain_create, notes_repo, llm_env)
    };

    // 5/7 master token
    let token = {
        println!("\n── 5/7 master token ──");
        let from_toml = existing_toml.as_ref().and_then(|t| t.server.token.clone());
        let existing = existing_cfg.server_token.clone().or(from_toml).filter(|t| !t.is_empty());
        let help = "master token＝admin 全權 Bearer token（開通帳號、管理 API）。建議現場生成 64 碼隨機值並妥善保存。";
        loop {
            let prompt = if existing.is_some() { "master token（Enter=保留現值，-=重新生成，或貼上新值）" } else { "master token（Enter=自動生成，或貼上既有值）" };
            let v = ask(prompt, "", Some(help))?;
            if v == "?" {
                println!("{help}");
                continue;
            }
            if v.is_empty() {
                if let Some(t) = &existing {
                    println!("  保留現有 token（{}）", mask_token(t));
                    break t.clone();
                }
                let t = gen_token();
                println!("  已生成 master token（僅此一次完整顯示，請妥善保存）：\n  {t}");
                break t;
            }
            if v == "-" {
                let t = gen_token();
                println!("  已生成 master token（僅此一次完整顯示，請妥善保存）：\n  {t}");
                break t;
            }
            if v.len() < 16 {
                println!("  ✗ token 太短——至少 16 字元（建議 64 碼隨機 hex）。");
                continue;
            }
            break v;
        }
    };

    // 6/7 管理員帳號
    let (admin_login, admin_password) = {
        println!("\n── 6/7 管理員帳號 ──");
        let login = ask_until(
            "管理員登入名",
            "admin",
            Some("此帳號用 Web 介面（/admin）登入；初始密碼簡單，首次登入會被強制更改。"),
            |v| {
                if v.trim().is_empty() {
                    Err("登入名不可空".into())
                } else if v.split_whitespace().count() != 1 {
                    Err("登入名不可含空白".into())
                } else {
                    Ok(())
                }
            },
        )?;
        let pw = ask_until(
            "管理員初始密碼",
            DEFAULT_ADMIN_PASSWORD,
            Some("簡單初始密碼僅供首次登入——登入後系統強制設定新密碼（至少 8 碼）。"),
            |v| identity::validate_password(v).map_err(|e| e.to_string()),
        )?;
        (login, pw)
    };

    // 7/7 系統服務
    let install_service = {
        println!("\n── 7/7 系統服務 ──");
        ask_yn("註冊為系統服務（開機自啟）？", false)?
    };

    // 摘要確認閘
    println!("\n════════ 設定摘要 ════════");
    println!("  模式      ：{}", if enterprise { "企業（operoid.toml）" } else { "個人（app-settings.json）" });
    println!("  監聽      ：{host}:{port}{}", if tls_cert.is_some() { "（TLS）" } else { "" });
    if let Some(fe) = &frontends_dir {
        println!("  前端目錄  ：{fe}");
    }
    match &brain_home {
        Some(h) => println!("  知識腦    ：{h}（{}）", if brain_create { "gbrain init 建立新腦" } else { "登錄既有" }),
        None => println!("  知識腦    ：略過（使用預設腦）"),
    }
    if !llm_env.is_empty() {
        println!("  LLM keys  ：{}", llm_env.keys().cloned().collect::<Vec<_>>().join(", "));
    }
    println!("  master token：{}", mask_token(&token));
    println!("  管理員    ：{admin_login} / {admin_password}（首次登入強制更改）");
    println!("  系統服務  ：{}", if install_service { "註冊" } else { "不註冊" });
    let mut files = vec!["app-settings.json".to_string(), "operoid.db（管理員帳號）".to_string()];
    if enterprise {
        files.insert(0, "operoid.toml".to_string());
    }
    println!("  將寫入    ：{}（於 {}）", files.join("＋"), dirs.settings_dir.display());
    if !ask_yn("\n確定寫入並套用？", true)? {
        println!("已取消——未變更任何設定。");
        return Ok(false);
    }

    apply(dirs, &Answers {
        enterprise,
        host,
        port,
        frontends_dir,
        tls_cert,
        tls_key,
        gbrain_exe,
        brain_home,
        brain_create,
        notes_repo,
        llm_env,
        token,
        admin_login,
        admin_password,
        install_service,
    })?;
    Ok(true)
}

/// 寫入設定檔、登錄／建立腦、建立管理員帳號、（可選）註冊服務。
fn apply(dirs: &DataDirs, a: &Answers) -> anyhow::Result<()> {
    std::fs::create_dir_all(&dirs.settings_dir)?;
    let mut cfg: AppConfig = config::load_config(&dirs.settings_dir);

    // token／LLM keys／gbrain／筆記 repo
    cfg.server_token = Some(a.token.clone());
    if a.enterprise {
        // 企業模式：llm env 走 operoid.toml（服務帳戶讀檔）；app-settings 留原值。
    } else {
        cfg.llm_env = a.llm_env.clone();
    }
    if !a.gbrain_exe.is_empty() {
        cfg.gbrain_exe_path = a.gbrain_exe.clone();
    }
    if let Some(nr) = &a.notes_repo {
        cfg.notes_repo_path = nr.clone();
    }

    // 腦：冪等登錄／建立（dedupe：同 gbrain_home 已登錄即沿用）
    if let Some(home) = &a.brain_home {
        if cfg.brains.iter().any(|b| b.gbrain_home.as_deref() == Some(home.as_str())) {
            println!("[configure] 腦已登錄——沿用：{home}");
        } else {
            let name = Path::new(home)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "brain".into());
            let req = ocore::brains::AddBrainReq {
                name,
                gbrain_home: Some(home.clone()),
                create: a.brain_create,
                embedding_model: None,
                embedding_dimensions: None,
                chat_model: None,
            };
            if a.brain_create {
                println!("[configure] 正在初始化新腦（gbrain init，首次冷啟較慢）……");
            }
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let (cfg2, entry) = rt
                .block_on(ocore::brains::add_brain_core(&cfg, &req))
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            cfg = cfg2;
            cfg.active_brain_id = Some(entry.id);
            println!("[configure] 腦已就緒：{}（id={}）", home, cfg.active_brain_id.as_deref().unwrap_or(""));
        }
    }

    // 設定檔（app-settings.json——兩種模式都需要：腦清單／notes repo 在這裡）
    config::save_config(&dirs.settings_dir, &cfg)?;
    println!("[configure] 已寫入 {}", dirs.settings_dir.join("app-settings.json").display());

    // 企業模式：operoid.toml
    if a.enterprise {
        let t = build_toml(
            &a.host,
            a.port,
            &a.token,
            a.frontends_dir.as_deref(),
            a.tls_cert.as_deref(),
            a.tls_key.as_deref(),
            &a.llm_env,
            &a.gbrain_exe,
        );
        let body = toml::to_string_pretty(&t)?;
        let path = dirs.settings_dir.join("operoid.toml");
        std::fs::write(&path, body)?;
        println!("[configure] 已寫入 {}（企業模式）", path.display());
    }

    // 管理員帳號（冪等：login_name 已存在即略過）
    let db_path = ocore::runtime::agent_db_path_in(&dirs.db_dir);
    let store = ocore::domain::SqliteStore::open(&db_path)?;
    let exists = store
        .list_principals()?
        .iter()
        .any(|p| p.login_name.as_deref() == Some(a.admin_login.as_str()));
    if exists {
        println!("[configure] 帳號「{}」已存在——略過建立", a.admin_login);
    } else {
        let spec = AccountSpec {
            id: format!("principal-{}", a.admin_login),
            login_name: a.admin_login.clone(),
            display_name: a.admin_login.clone(),
            roles: vec!["admin".into()],
            temp_password: Some(a.admin_password.clone()),
        };
        identity::create_account(&store, spec)?;
        println!(
            "[configure] 管理員已建立：{} / {}（首次登入強制更改密碼）",
            a.admin_login, a.admin_password
        );
    }

    // 系統服務（失敗不視為整體失敗——可手動補 install）
    if a.install_service {
        match crate::service::install(&dirs.settings_dir, &dirs.db_dir) {
            Ok(()) => println!("[configure] 系統服務已註冊並啟動。"),
            Err(e) => println!("[configure] ⚠ 服務註冊失敗（可稍後手動執行 oserver install）：{e}"),
        }
    }

    // 下一步指引
    let scheme = if a.tls_cert.is_some() { "https" } else { "http" };
    let display_host = if a.host == "0.0.0.0" { "127.0.0.1" } else { &a.host };
    println!("\n[configure] 完成。下一步：");
    println!("  1. 啟動服務：cargo run -p oserver -- --data-dir {}（或以 oserver install 註冊服務則開機自啟）", dirs.settings_dir.display());
    println!(
        "  2. 開啟 {scheme}://{display_host}:{}/admin 並以 {} / {} 登入——系統將強制設定新密碼",
        a.port, a.admin_login, a.admin_password
    );    println!("  3. master token 已寫入設定檔——請另行抄錄保存（管理 API／obridge 用）");
    Ok(())
}

// ── 測試 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ocore::knowledge::identity::MIN_PASSWORD_LEN;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "oserver-cfg-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn needs_setup_detects_fresh_and_existing() {
        let fresh = temp_dir("fresh");
        assert!(needs_setup(&fresh), "空目錄＝首次執行");

        let with_settings = temp_dir("settings");
        std::fs::write(with_settings.join("app-settings.json"), "{}").unwrap();
        assert!(!needs_setup(&with_settings), "有 app-settings.json≠首次");

        let with_toml = temp_dir("toml");
        std::fs::write(with_toml.join("operoid.toml"), "").unwrap();
        assert!(!needs_setup(&with_toml), "有 operoid.toml≠首次");
    }

    #[test]
    fn default_admin_password_passes_policy() {
        assert!(identity::validate_password(DEFAULT_ADMIN_PASSWORD).is_ok());
        assert!(DEFAULT_ADMIN_PASSWORD.chars().count() >= MIN_PASSWORD_LEN);
    }

    #[test]
    fn toml_build_round_trips() {
        let mut env = BTreeMap::new();
        env.insert("ZHIPUAI_API_KEY".to_string(), "k".to_string());
        let t = build_toml("0.0.0.0", 7340, "tok-abc123456789", Some("./frontends"), Some("c.pem"), Some("k.pem"), &env, "gbrain.exe");
        let s = toml::to_string_pretty(&t).unwrap();
        let back: OperoidToml = toml::from_str(&s).unwrap();
        assert_eq!(back.server.host.as_deref(), Some("0.0.0.0"));
        assert_eq!(back.server.port, Some(7340));
        assert_eq!(back.server.token.as_deref(), Some("tok-abc123456789"));
        assert_eq!(back.server.frontends_dir.as_deref(), Some("./frontends"));
        assert_eq!(back.tls.cert.as_deref(), Some("c.pem"));
        assert_eq!(back.llm.env.get("ZHIPUAI_API_KEY").map(String::as_str), Some("k"));
        assert_eq!(back.gbrain.exe_path.as_deref(), Some("gbrain.exe"));

        // 無 TLS／無 gbrain → 欄位省略，load 端視為個人化預設
        let t2 = build_toml("127.0.0.1", 7340, "tok", None, None, None, &BTreeMap::new(), "");
        let s2 = toml::to_string_pretty(&t2).unwrap();
        assert!(!s2.contains("cert"), "未設 TLS 不應序列化 cert");
        assert!(!s2.contains("exe_path"), "未設 gbrain 不應序列化 exe_path");
    }

    #[test]
    fn classify_brain_path_triage() {
        assert_eq!(classify_brain_path(""), BrainPathState::Empty);
        let missing = temp_dir("nonexistent-brain-parent").join("no-such");
        assert_eq!(classify_brain_path(&missing.to_string_lossy()), BrainPathState::Missing);
        let plain = temp_dir("plain-brain");
        assert_eq!(classify_brain_path(&plain.to_string_lossy()), BrainPathState::NeedsInit);
        std::fs::create_dir_all(plain.join(".gbrain")).unwrap();
        assert_eq!(classify_brain_path(&plain.to_string_lossy()), BrainPathState::Ready);
    }

    #[test]
    fn gbrain_candidate_prefers_existing_config_then_home() {
        // 既有設定值存在 → 直接採用
        let dir = temp_dir("gbrain-cfg");
        let exe = dir.join("gbrain.exe");
        std::fs::write(&exe, b"").unwrap();
        assert_eq!(
            resolve_gbrain_candidate(&exe.to_string_lossy(), None),
            exe.to_string_lossy()
        );
        // 既有設定值失效 → 落到 ~/.bun/bin
        let home = temp_dir("gbrain-home");
        let bun_exe = home.join(".bun").join("bin").join("gbrain.exe");
        std::fs::create_dir_all(home.join(".bun").join("bin")).unwrap();
        std::fs::write(&bun_exe, b"").unwrap();
        assert_eq!(
            resolve_gbrain_candidate("Z:\\no\\such\\gbrain.exe", Some(&home)),
            bun_exe.to_string_lossy()
        );
        // 全落空 → PATH 探測屬環境相依（本機可能真裝了 gbrain）——只驗「絕不回傳失效設定值」
        let empty_home = temp_dir("gbrain-empty");
        let s = resolve_gbrain_candidate("Z:\\no\\such.exe", Some(&empty_home));
        assert!(s.is_empty() || Path::new(&s).is_file());
    }

    #[test]
    fn mask_keeps_prefix_and_suffix() {
        assert_eq!(mask_token("abcdef1234567890"), "abcdef…7890");
        assert_eq!(mask_token("short"), "****");
    }
}
