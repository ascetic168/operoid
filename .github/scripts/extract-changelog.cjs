// 從 CHANGELOG.md 抽出 $TAG_NAME 版本段落，作為 GitHub Release body（輸出到 stdout）。
// 由 .github/workflows/release.yml 的「extract changelog」步驟呼叫。
//
// 抓取規則：找 CHANGELOG.md 中 `## [vX.Y.Z]` 開頭的段落（到下一個 `## ` 為止）。
// 找到 → 該段落 + 下方安裝包說明；找不到 → fallback 佔位。
const fs = require("fs");

const tag = process.env.TAG_NAME;
const FALLBACK =
  "（本版本變更說明待補；各平台安裝包見下方 assets。）";
const ASSETS =
  "\n\n---\n\n各平台安裝包（Windows `.msi`／macOS `.dmg`／Linux `.deb` `.rpm` `.AppImage`）見下方 assets。";

let body = "";
try {
  const md = fs.readFileSync("CHANGELOG.md", "utf8");
  const lines = md.split(/\r?\n/);
  let capture = false;
  const out = [];
  for (const ln of lines) {
    if (/^##\s/.test(ln)) {
      if (capture) break; // 進入下一個版本段落，停止
      if (ln.includes(tag)) capture = true; // 命中目標版本
    } else if (capture) {
      out.push(ln);
    }
  }
  body = out.join("\n").trim();
} catch (e) {
  // CHANGELOG.md 不存在或讀取失敗 → 留空走 fallback
}

if (!body) body = FALLBACK;
else body += ASSETS;

process.stdout.write(body);
