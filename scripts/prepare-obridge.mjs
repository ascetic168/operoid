#!/usr/bin/env node
/**
 * 打包前準備 obridge 執行檔（tauri build 的 beforeBuildCommand 呼叫——必須在
 * cargo build **之前**：tauri 的 build.rs 一開始就檢查 externalBin 檔案存在）。
 *
 * Triple 判別：優先 Tauri 提供的 TAURI_ENV_TARGET_TRIPLE（CI 的 cross-target
 * matrix 如 aarch64-apple-darwin 會與 host 不同——需 cargo build --target），
 * 缺省（本機無 --target 建置）退回 rustc host。
 *
 * 產出 src-tauri/binaries/obridge-<triple>(.exe)（Tauri externalBin 慣例）；
 * 安裝後與 Operoid 執行檔同目錄——obridge_cfg::resolve_executable 自動偵測命中。
 */
import { execSync } from "node:child_process";
import { mkdirSync, copyFileSync } from "node:fs";

const host = execSync("rustc -vV").toString().match(/host: (\S+)/)?.[1];
if (!host) {
  console.error("[obridge] 無法取得 rustc host triple");
  process.exit(1);
}
const target = process.env.TAURI_ENV_TARGET_TRIPLE || host;
const isWindows = target.includes("windows");
const ext = isWindows ? ".exe" : "";

if (target === host) {
  execSync("cargo build -p obridge -p oserver --release", { stdio: "inherit" });
} else {
  console.log(`[obridge] cross build（host=${host} → target=${target}）`);
  execSync(`cargo build -p obridge -p oserver --release --target ${target}`, { stdio: "inherit" });
}
mkdirSync("src-tauri/binaries", { recursive: true });
for (const name of ["obridge", "oserver"]) {
  const from = target === host
    ? `target/release/${name}${ext}`
    : `target/${target}/release/${name}${ext}`;
  const to = `src-tauri/binaries/${name}-${target}${ext}`;
  copyFileSync(from, to);
  console.log(`[${name}] 已準備 externalBin：${to}`);
}
