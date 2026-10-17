include!("../build_common.rs");

fn main() {
    // A3 舊碼防護前置：把 repo build hash 編進桌面殼，ensure_server 帶起
    // oserver 前以此核對（與 build_common 註記的「跑舊碼」事故同源）。
    emit_build_info();
    tauri_build::build()
}
