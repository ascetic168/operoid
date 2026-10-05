// 共用的 build script 內容——`oserver/build.rs` 與 `obridge/build.rs` 以
// `include!("../build_common.rs")` 引用（單一真相，兩邊行為一致）。
//
// 把 **git 短 hash（含 -dirty）＋建置時間** 編譯進執行檔（`OPEROID_BUILD_HASH`／
// `OPEROID_BUILD_TIME`）。動機：oserver 以 sibling 解析帶起 obridge、桌面殼以
// sibling 解析帶起 oserver——「跑到舊版二進位」在開發期是隱性事故（本次企業版
// obridge 功能開發中即發生：target/debug 的 obridge 被換回月前舊 artifact，
// `--check` 旗標被無視）。build id 讓新舊混用**當場可見**：
// - 兩個執行檔啟動 log 都帶 build id；
// - oserver 帶起 obridge 前以 `obridge --version` 核對，不一致即警告；
// - `/api/obridge/status` 回 `exe_build_match` 供 admin 介面顯示。
//
// rerun 語意：只 watch `.git/HEAD`＋解析後的 ref 檔——commit 變更才重編（工作區
// 改動以 `-dirty` 標記，不觸發重編）。無 git（tarball 建置）→ `unknown`，不擋建置。

fn emit_build_info() {
    // HEAD 與（若指向 ref）該 ref 檔都 watch——commit／branch 切換才重跑 build script。
    println!("cargo:rerun-if-changed=../.git/HEAD");
    let head = match std::fs::read_to_string("../.git/HEAD") {
        Ok(s) => s.trim().to_string(),
        Err(_) => String::new(),
    };
    if let Some(rel) = head.strip_prefix("ref: ") {
        println!("cargo:rerun-if-changed=../.git/{rel}");
    }
    println!("cargo:rerun-if-changed=../.git/packed-refs");

    let hash = match run_git(&["rev-parse", "--short", "HEAD"]) {
        Some(h) => {
            let dirty = match run_git(&["status", "--porcelain"]) {
                Some(out) if !out.trim().is_empty() => "-dirty",
                _ => "",
            };
            format!("{h}{dirty}")
        }
        None => "unknown".to_string(),
    };
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    println!("cargo:rustc-env=OPEROID_BUILD_HASH={hash}");
    println!("cargo:rustc-env=OPEROID_BUILD_TIME={time}");
}

fn run_git(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
