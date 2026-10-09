// 員工回覆中「檢索圖片／來源文件」確定性段落的行樣式與抽取器——
// 行格式由後端確定性附加（ocore runtime.write_final_reply；企業 ops 附加於
// oserver gbrain）：`[[doc/figN-pP]]` wikilink 之後一行 `圖檔：<伺服器絕對路徑>`、
// 「**來源文件**」段一行 `原論文 PDF：<絕對路徑>`。規則與桌面
// src/components/MarkdownText.vue 相同；web 端 `<img>`/`<a>` 帶不了 Bearer，
// 渲染所需 URL 由呼叫端供應（imageUrls props／open-pdf 事件）。

/** `圖檔：<伺服器絕對路徑>`（檢索命中圖片；延伸名限縮——防誤咬一般文字行）。 */
export const FIGFILE_RE = /^圖檔：(.+?\.(?:jpe?g|png|gif|webp))\s*$/gm

/** `原論文 PDF：<伺服器絕對路徑>`（來源文件）。 */
export const PDFFILE_RE = /^原論文 PDF：(.+?\.pdf)\s*$/gm

/** 收集回覆文字中全部檢索圖片路徑（去重、保持出現序）。 */
export function figureImagePaths(text: string): string[] {
  return collectPaths(text, FIGFILE_RE)
}

/** 收集回覆文字中全部來源 PDF 路徑（去重、保持出現序）。 */
export function sourcePdfPaths(text: string): string[] {
  return collectPaths(text, PDFFILE_RE)
}

function collectPaths(text: string, re: RegExp): string[] {
  const out: string[] = []
  for (const m of text.matchAll(re)) {
    const p = m[1]?.trim()
    if (p && !out.includes(p)) out.push(p)
  }
  return out
}
