//! 服務註冊（P5，三平台）——`install`／`uninstall`／`is_installed`／服務模式。
//!
//! - **Windows**：SCM（`windows-service` crate——服務行程須實作 dispatcher 協議）。
//!   服務以 LocalSystem 跑，**binPath 帶 `--settings-dir/--db-dir` 指向使用者目錄**，
//!   讀同一份 app-settings.json。**本機實機驗證**。
//! - **Linux**：systemd **system** unit（`/etc/systemd/system`——開機即啟，免登入；
//!   `User=` 以安裝使用者執行，檔案屬主與 GUI 一致）。
//! - **macOS**：launchd **LaunchDaemon**（`/Library/LaunchDaemons`——開機即啟，免登入；
//!   `UserName` 以安裝使用者執行，同上）。
//!   兩者皆以 `sudo` 自我重啟提權（root 僅用於寫系統位置）。**未實機驗證**（CI 編譯覆蓋）。
//!
//! 三平台共用介面：[`install`]／[`uninstall`]／[`is_installed`]。

use std::path::Path;

pub const SERVICE_NAME: &str = "Operoid";

/// 服務 ExecStart 的參數（--service ＋指向使用者資料目錄；服務行程無使用者環境）。
fn service_args(settings_dir: &Path, db_dir: &Path) -> Vec<String> {
    vec![
        "--service".into(),
        "--settings-dir".into(),
        settings_dir.to_string_lossy().into_owned(),
        "--db-dir".into(),
        db_dir.to_string_lossy().into_owned(),
    ]
}

// ───────────────────────── Windows（SCM）─────────────────────────

#[cfg(windows)]
mod imp {
    use super::*;
    use std::time::Duration;
    use windows_service::service::{
        ServiceAccess, ServiceControlAccept, ServiceErrorControl, ServiceInfo, ServiceStartType,
        ServiceState, ServiceType,
    };
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
    use windows_service::{define_windows_service, service_control_handler};

    fn mgr_access(access: ServiceManagerAccess) -> Result<ServiceManager, String> {
        ServiceManager::local_computer(None::<&str>, access).map_err(|e| e.to_string())
    }

    /// 是否已以管理員身分執行（SCM 操作需要）。
    fn is_elevated() -> bool {
        // 簡單可靠的判斷：嘗試寫入需要提權的位置失敗即未提權——改用 OpenProcess token 更準；
        // 此處用常見作法：寫 %WINDIR%\Temp 測試。
        std::env::var("WINDIR")
            .map(|w| {
                let probe = std::path::PathBuf::from(w).join(".operoid-elev-probe");
                std::fs::write(&probe, b"1").is_ok() && std::fs::remove_file(&probe).is_ok()
            })
            .unwrap_or(false)
    }

    /// SCM 操作需要管理員——非提權時以 runas（UAC）自我重啟並等待完成。
    /// 回 true 表示已由提權行程完成（呼叫端直接退出）；false 表示當前已是提權行程。
    fn elevate(cmd: &str, args: &[String]) -> Result<bool, String> {
        if is_elevated() {
            return Ok(false);
        }
        eprintln!("[service] 需要管理員權限——彈出 UAC 提權視窗……");
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut full = vec![cmd.to_string()];
        full.extend_from_slice(args);
        let status = runas::Command::new(exe)
            .args(&full)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("提權執行失敗（UAC 被拒或執行錯誤）".into());
        }
        Ok(true)
    }

    pub fn install(settings_dir: &Path, db_dir: &Path) -> Result<(), String> {
        let sdir = settings_dir.to_string_lossy().into_owned();
        let ddir = db_dir.to_string_lossy().into_owned();
        let args = vec!["--settings-dir".into(), sdir, "--db-dir".into(), ddir];
        if elevate("install", &args)? {
            return Ok(());
        }
        let settings_dir = Path::new(&args[1]);
        let db_dir = Path::new(&args[3]);
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let args = service_args(settings_dir, db_dir);

        let mgr = mgr_access(ServiceManagerAccess::CREATE_SERVICE | ServiceManagerAccess::CONNECT)
            .map_err(|e| format!("連 ServiceManager 失敗：{e}"))?;
        let info = ServiceInfo {
            name: super::SERVICE_NAME.into(),
            display_name: "Operoid Agent Service".into(),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe,
            launch_arguments: args.into_iter().map(std::ffi::OsString::from).collect(),
            dependencies: vec![],
            account_name: None,     // LocalSystem
            account_password: None,
        };
        // 已存在（重複 install）→ 開既有服務繼續（冪等）。
        let svc = match mgr.create_service(&info, ServiceAccess::QUERY_STATUS | ServiceAccess::START) {
            Ok(svc) => svc,
            Err(_) => {
                eprintln!("[service] 服務已存在——沿用既有註冊");
                mgr.open_service(super::SERVICE_NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::START)
                    .map_err(|e| format!("開啟既有服務失敗：{e}"))?
            }
        };
        let _ = svc.set_description("Operoid 本地服務（agent runtime＋HTTP API）");
        // 立即啟動（失敗不視為安裝失敗——重開機亦會自啟）。
        let _ = svc.start::<std::ffi::OsString>(&[]);
        println!("[service] 已安裝並啟動 Windows 服務「{}」（開機自啟）", super::SERVICE_NAME);
        Ok(())
    }

    pub fn uninstall() -> Result<(), String> {
        if elevate("uninstall", &[])? {
            return Ok(());
        }
        let mgr = mgr_access(ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)
            .map_err(|e| format!("連 ServiceManager 失敗：{e}"))?;
        let svc = mgr
            .open_service(super::SERVICE_NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE)
            .map_err(|e| format!("服務不存在或無法開啟：{e}"))?;
        // 先停（等 Stopped；服務自身的優雅關機會等員工）。
        let status = svc.query_status().map_err(|e| e.to_string())?;
        if status.current_state != ServiceState::Stopped {
            let _ = svc.stop();
            for _ in 0..60 {
                std::thread::sleep(std::time::Duration::from_secs(2));
                let s = svc.query_status().map_err(|e| e.to_string())?;
                if s.current_state == ServiceState::Stopped {
                    break;
                }
            }
        }
        svc.delete().map_err(|e| format!("delete_service 失敗：{e}"))?;
        println!("[service] 已移除 Windows 服務「{}」", super::SERVICE_NAME);
        Ok(())
    }

    pub fn is_installed() -> Result<bool, String> {
        let out = std::process::Command::new("sc")
            .args(["query", super::SERVICE_NAME])
            .output()
            .map_err(|e| e.to_string())?;
        Ok(out.status.success())
    }

    // ── 服務模式（SCM dispatcher）──

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<std::ffi::OsString>) {
        // SCM 起的行程沒有使用者 env——token 由 app-settings.json 的 server_token 提供。
        // 停止路徑：SCM Stop → SERVICE_STOP（與 Ctrl+C 等價）→ run() 的優雅關機。
        std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("建 tokio runtime 失敗");
            let _ = rt.block_on(crate::run(&crate::parse_args()));
            std::process::exit(0);
        });
        if let Err(e) = run_ctrl_handler() {
            eprintln!("[service] SCM 處理失敗：{e}");
            std::process::exit(1);
        }
    }

    fn service_status(state: ServiceState, accept: ServiceControlAccept) -> windows_service::service::ServiceStatus {
        windows_service::service::ServiceStatus {
            service_type: windows_service::service::ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accept,
            exit_code: windows_service::service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        }
    }

    fn run_ctrl_handler() -> Result<(), String> {
        use windows_service::service::ServiceControl;
        
        let (tx, rx) = std::sync::mpsc::channel::<ServiceControl>();
        let handler = service_control_handler::register(super::SERVICE_NAME, move |ctrl| {
            let _ = tx.send(ctrl);
            windows_service::service_control_handler::ServiceControlHandlerResult::NoError
        })
        .map_err(|e| e.to_string())?;
        handler
            .set_service_status(service_status(ServiceState::StartPending, ServiceControlAccept::STOP))
            .map_err(|e| e.to_string())?;
        // 等就緒（healthz ready 之後）——以 port 可連線為準（簡單可靠）。
        let port = crate::parse_args().port;
        for _ in 0..120 {
            if std::net::TcpStream::connect_timeout(
                &format!("127.0.0.1:{port}").parse().expect("addr"),
                std::time::Duration::from_millis(500),
            )
            .is_ok()
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        handler
            .set_service_status(service_status(ServiceState::Running, ServiceControlAccept::STOP))
            .map_err(|e| e.to_string())?;
        eprintln!("[service] SCM: Running");
        while let Ok(ctrl) = rx.recv() {
            match ctrl {
                ServiceControl::Stop => {
                    handler
                        .set_service_status(service_status(ServiceState::StopPending, ServiceControlAccept::empty()))
                        .map_err(|e| e.to_string())?;
                    crate::SERVICE_STOP.store(true, std::sync::atomic::Ordering::SeqCst);
                    // run() 的關機執行緒會在退出時 process::exit；此處報 Stopped 收尾。
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    handler
                        .set_service_status(service_status(ServiceState::Stopped, ServiceControlAccept::empty()))
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// SCM 進入點：目前行程即服務。阻塞至服務停止。
    pub fn run_service() -> Result<(), String> {
        windows_service::service_dispatcher::start(super::SERVICE_NAME, ffi_service_main)
            .map_err(|e| e.to_string())
    }
}

#[cfg(windows)]
pub fn install(settings_dir: &Path, db_dir: &Path) -> Result<(), String> {
    imp::install(settings_dir, db_dir)
}
#[cfg(windows)]
pub fn uninstall() -> Result<(), String> {
    imp::uninstall()
}
#[cfg(windows)]
pub fn is_installed() -> Result<bool, String> {
    imp::is_installed()
}
#[cfg(windows)]
pub fn run_service() -> Result<(), String> {
    imp::run_service()
}

// ───────────────────────── Linux（systemd system unit——開機即啟，免登入）─────────────────────────

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use std::path::PathBuf;

    fn unit_path() -> PathBuf {
        PathBuf::from("/etc/systemd/system/operoid.service")
    }

    /// 安裝目標使用者（服務以此身分跑——檔案屬主與 GUI 一致）。
    /// `SUDO_USER`（sudo 保留）優先；缺漏時以 settings_dir 的屬主（`stat -c %U`）反推。
    fn resolve_user(settings_dir: &Path) -> Result<(String, String), String> {
        if let Ok(u) = std::env::var("SUDO_USER") {
            if !u.trim().is_empty() {
                let home = home_of(&u)?;
                return Ok((u, home));
            }
        }
        let out = std::process::Command::new("stat")
            .args(["-c", "%U"])
            .arg(settings_dir)
            .output()
            .map_err(|e| format!("stat 失敗：{e}"))?;
        let owner = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !out.status.success() || owner.is_empty() || owner == "root" {
            return Err(
                "無法判定服務執行使用者——請以一般使用者身分透過 sudo 執行 install（從你的帳號裝，服務才能以你的身分跑）".into(),
            );
        }
        Ok((owner.clone(), home_of(&owner)?))
    }

    /// 使用者的 home（getent passwd 第 6 欄）。
    fn home_of(user: &str) -> Result<String, String> {
        let out = std::process::Command::new("getent")
            .args(["passwd", user])
            .output()
            .map_err(|e| format!("getent 失敗：{e}"))?;
        let text = String::from_utf8_lossy(&out.stdout);
        let home = text.lines().next().and_then(|l| l.split(':').nth(5)).unwrap_or("").to_string();
        if !out.status.success() || home.is_empty() {
            return Err(format!("無法解析使用者 {user} 的 home 目錄"));
        }
        Ok(home)
    }

    /// unit 檔內容（純字串產生——跨平台可測）。`User=` 讓服務以安裝使用者跑
    /// （root 只在 install 寫 /etc 時需要）——DB/設定檔屬主與 GUI 一致，免權限打架。
    pub fn unit_content(exe: &str, settings_dir: &str, db_dir: &str, user: &str, home: &str) -> String {
        format!(
            "[Unit]\n\
             Description=Operoid Agent Service\n\
             After=network.target\n\n\
             [Service]\n\
             User={user}\n\
             Environment=HOME={home}\n\
             ExecStart={exe} --service --settings-dir {settings_dir} --db-dir {db_dir}\n\
             Restart=on-failure\n\
             RestartSec=5\n\n\
             [Install]\n\
             WantedBy=multi-user.target\n"
        )
    }

    fn systemctl(args: &[&str]) -> Result<(), String> {
        let out = std::process::Command::new("systemctl").args(args).output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "systemctl {} 失敗：{}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    fn is_root() -> bool {
        std::process::Command::new("id")
            .arg("-u")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
            .unwrap_or(false)
    }

    /// root 權限自我重啟（sudo）——回 true 表示已由提權行程完成（呼叫端直接退出）。
    fn elevate(args: &[String]) -> Result<bool, String> {
        if is_root() {
            return Ok(false);
        }
        eprintln!("[service] 需要 root 權限——透過 sudo 重新執行……");
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let status = std::process::Command::new("sudo")
            .arg(&exe)
            .args(args)
            .status()
            .map_err(|e| format!("sudo 執行失敗：{e}"))?;
        if !status.success() {
            return Err("sudo 執行失敗（密碼被拒或執行錯誤）".into());
        }
        Ok(true)
    }

    pub fn install(settings_dir: &Path, db_dir: &Path) -> Result<(), String> {
        let elevate_args = vec![
            "install".to_string(),
            "--settings-dir".into(),
            settings_dir.to_string_lossy().into_owned(),
            "--db-dir".into(),
            db_dir.to_string_lossy().into_owned(),
        ];
        if elevate(&elevate_args)? {
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let unit = unit_path();
        let (user, home) = resolve_user(settings_dir)?;
        let content = unit_content(
            &exe.to_string_lossy(),
            &settings_dir.to_string_lossy(),
            &db_dir.to_string_lossy(),
            &user,
            &home,
        );
        std::fs::write(&unit, content).map_err(|e| format!("寫入 {unit:?} 失敗（需要 root）：{e}"))?;
        systemctl(&["daemon-reload"])?;
        systemctl(&["enable", "--now", "operoid.service"])?;
        println!("[service] 已安裝並啟動 systemd system unit（開機自啟，免登入）");
        Ok(())
    }

    pub fn uninstall() -> Result<(), String> {
        if elevate(&["uninstall".to_string()])? {
            return Ok(());
        }
        let _ = systemctl(&["disable", "--now", "operoid.service"]);
        std::fs::remove_file(unit_path()).map_err(|e| format!("移除 unit 失敗：{e}"))?;
        systemctl(&["daemon-reload"])?;
        println!("[service] 已移除 systemd system unit");
        Ok(())
    }

    pub fn is_installed() -> Result<bool, String> {
        Ok(unit_path().exists())
    }

    pub fn run_service() -> Result<(), String> {
        // systemd 前景執行——與一般模式相同，只是 Restart=on-failure 託管重啟。
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        rt.block_on(crate::run(&crate::parse_args())).map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "linux")]
pub use imp::{install, is_installed, run_service, uninstall, unit_content};

// ───────────────────────── macOS（launchd LaunchDaemon——開機即啟，免登入）─────────────────────────

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use std::path::PathBuf;

    fn plist_path() -> PathBuf {
        PathBuf::from("/Library/LaunchDaemons/com.operoid.studio.server.plist")
    }

    /// 安裝目標使用者（LaunchDaemon 以此身分跑——檔案屬主與 GUI 一致）。
    /// `SUDO_USER`（sudo 保留）優先；缺漏時以 settings_dir 的屬主（`stat -f %Su`）反推。
    fn resolve_user(settings_dir: &Path) -> Result<String, String> {
        if let Ok(u) = std::env::var("SUDO_USER") {
            if !u.trim().is_empty() {
                return Ok(u);
            }
        }
        let out = std::process::Command::new("stat")
            .args(["-f", "%Su"])
            .arg(settings_dir)
            .output()
            .map_err(|e| format!("stat 失敗：{e}"))?;
        let owner = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !out.status.success() || owner.is_empty() || owner == "root" {
            return Err(
                "無法判定服務執行使用者——請以一般使用者身分透過 sudo 執行 install（從你的帳號裝，服務才能以你的身分跑）".into(),
            );
        }
        Ok(owner)
    }

    /// plist 內容（純字串產生——跨平台可測）。
    /// LaunchDaemon 由 root 載入但以 `UserName` 指定使用者執行——開機即啟且免登入，
    /// 檔案屬主又與 GUI 一致（root 只在 install 寫 /Library/LaunchDaemons 時需要）。
    pub fn plist_content(exe: &str, settings_dir: &str, db_dir: &str, user: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
               <key>Label</key><string>com.operoid.studio.server</string>\n\
               <key>UserName</key><string>{user}</string>\n\
               <key>ProgramArguments</key>\n\
               <array>\n\
                 <string>{exe}</string>\n\
                 <string>--service</string>\n\
                 <string>--settings-dir</string>\n\
                 <string>{settings_dir}</string>\n\
                 <string>--db-dir</string>\n\
                 <string>{db_dir}</string>\n\
               </array>\n\
               <key>RunAtLoad</key><true/>\n\
               <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n\
             </dict>\n\
             </plist>\n"
        )
    }

    fn is_root() -> bool {
        std::process::Command::new("id")
            .arg("-u")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
            .unwrap_or(false)
    }

    /// root 權限自我重啟（sudo）——回 true 表示已由提權行程完成（呼叫端直接退出）。
    fn elevate(args: &[String]) -> Result<bool, String> {
        if is_root() {
            return Ok(false);
        }
        eprintln!("[service] 需要 root 權限——透過 sudo 重新執行……");
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let status = std::process::Command::new("sudo")
            .arg(&exe)
            .args(args)
            .status()
            .map_err(|e| format!("sudo 執行失敗：{e}"))?;
        if !status.success() {
            return Err("sudo 執行失敗（密碼被拒或執行錯誤）".into());
        }
        Ok(true)
    }

    pub fn install(settings_dir: &Path, db_dir: &Path) -> Result<(), String> {
        let elevate_args = vec![
            "install".to_string(),
            "--settings-dir".into(),
            settings_dir.to_string_lossy().into_owned(),
            "--db-dir".into(),
            db_dir.to_string_lossy().into_owned(),
        ];
        if elevate(&elevate_args)? {
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let plist = plist_path();
        let user = resolve_user(settings_dir)?;
        let content = plist_content(
            &exe.to_string_lossy(),
            &settings_dir.to_string_lossy(),
            &db_dir.to_string_lossy(),
            &user,
        );
        std::fs::write(&plist, content).map_err(|e| format!("寫入 {plist:?} 失敗（需要 root）：{e}"))?;
        let out = std::process::Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&plist)
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("launchctl load 失敗：{}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        println!("[service] 已安裝並啟動 launchd LaunchDaemon（開機自啟，免登入）");
        Ok(())
    }

    pub fn uninstall() -> Result<(), String> {
        if elevate(&["uninstall".to_string()])? {
            return Ok(());
        }
        let plist = plist_path();
        let _ = std::process::Command::new("launchctl")
            .args(["unload", "-w"])
            .arg(&plist)
            .output();
        std::fs::remove_file(&plist).map_err(|e| format!("移除 plist 失敗：{e}"))?;
        println!("[service] 已移除 launchd LaunchDaemon");
        Ok(())
    }

    pub fn is_installed() -> Result<bool, String> {
        Ok(plist_path().exists())
    }

    pub fn run_service() -> Result<(), String> {
        // launchd 前景執行——與一般模式相同（KeepAlive 託管重啟）。
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        rt.block_on(crate::run(&crate::parse_args())).map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "macos")]
pub use imp::{install, is_installed, plist_content, run_service, uninstall};

#[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
compile_error!("oserver 服務註冊僅支援 Windows/Linux/macOS");
