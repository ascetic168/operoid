//! oserver 建置資訊（build id 編譯進執行檔——見 ../build_common.rs 的動機說明）。
include!("../build_common.rs");

fn main() {
    emit_build_info();
}
