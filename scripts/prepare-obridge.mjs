#!/usr/bin/env node
/**
 * 打包前準備 obridge 執行檔（tauri build 的 beforeBundleCommand 呼叫）。
 *
 * 建置 obridge（release）→ 複製到 src-tauri/binaries/ 並附 target-triple 檔名
 * （Tauri externalBin 的慣例：`binaries/obridge` 實際檔名為
 * `obridge-<triple>.exe`）。安裝後 Tauri 會把它放在 Operoid 執行檔同目錄——
 * 正好被 obridge_cfg::resolve_executable 的自動偵測找到（autostart 零設定）。
 */
import { execSync } from "node:child_process";
import { mkdirSync, copyFileSync } from "node:fs";

const triple = execSync("rustc -vV").toString().match(/host: (\S+)/)?.[1];
if (!triple) {
  console.error("[obridge] 無法取得 rustc host triple");
  process.exit(1);
}
execSync("cargo build -p obridge --release", { stdio: "inherit" });
mkdirSync("src-tauri/binaries", { recursive: true });
const ext = process.platform === "win32" ? ".exe" : "";
const from = `target/release/obridge${ext}`;
const to = `src-tauri/binaries/obridge-${triple}${ext}`;
copyFileSync(from, to);
console.log(`[obridge] 已準備 externalBin：${to}`);
