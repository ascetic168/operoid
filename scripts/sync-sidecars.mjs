#!/usr/bin/env node
/**
 * sync-sidecars.mjs — 把 cargo 剛建好的 oserver/obridge 同步到 tauri sidecar 來源。
 *
 * 根因（2026-10-17 事故）：tauri.conf 的 externalBin 指向 src-tauri/binaries/
 * `<name>-<triple>.exe`，每次 cargo check/build -p operoid 時 tauri-build 會把該檔
 * 複製蓋寫到 target/debug/<name>.exe——若 binaries/ 裡是舊 artifact，GUI 帶起的
 * oserver 就是舊碼（路由 404 → 前端「本地服務通訊失敗」）。
 * 本腳本在 cargo build -p oserver 之後、tauri dev 啟動之前執行，確保 sidecar
 * 來源永遠是最新建置。ensure_server 另有 byte-compare 拒絕帶起舊碼的防護。
 *
 * 用法：node scripts/sync-sidecars.mjs（dev:all 已接線；可單獨跑）
 */
import { copyFileSync, existsSync, statSync } from 'node:fs';
import { join } from 'node:path';

const triple = process.env.TARGET_TRIPLE ?? 'x86_64-pc-windows-msvc';
let synced = 0;

for (const name of ['oserver', 'obridge']) {
  const src = join('target', 'debug', `${name}.exe`);
  const dst = join('src-tauri', 'binaries', `${name}-${triple}.exe`);
  if (!existsSync(src)) {
    console.log(`[sidecar] ${name}: target/debug/${name}.exe 不存在，略過`);
    continue;
  }
  const before = existsSync(dst) ? statSync(dst).size : -1;
  copyFileSync(src, dst);
  const after = statSync(dst).size;
  console.log(`[sidecar] ${name}: ${before} → ${after} bytes${before === after ? '（未變）' : '（已更新）'}`);
  synced++;
}
if (synced === 0) console.log('[sidecar] 無可同步的執行檔');
process.exit(0);
