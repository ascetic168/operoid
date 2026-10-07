<script setup lang="ts">
import { computed } from 'vue'
import { marked } from 'marked'
import DOMPurify from 'dompurify'

const props = defineProps<{ text: string }>()

// gbrain 引用格式的行內 tokenizer——規則同原 KnowledgeAskView.linkSegments：
// `[[dir/slug]]`／`[[dir/slug|name]]` 與單括 `[dir/slug]`（須含 `/` 且後不接 `(`，
// 避開 markdown 連結）。渲染為 <span class="wikilink">，仍在 DOMPurify 消毒邊界內。
const WIKI_RE = /^\[\[([^\]]+)\]\]|^\[([^\]\[\s|/]+\/[^\]\[\s|/]+)\](?!\()/

function escapeHtml(s: unknown): string {
  return String(s)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
}

marked.use({
  gfm: true,
  breaks: true,
  extensions: [
    {
      name: 'wikilink',
      level: 'inline',
      start(src: string): number | void {
        const i = src.indexOf('[')
        return i < 0 ? undefined : i
      },
      tokenizer(src: string) {
        const m = WIKI_RE.exec(src)
        if (!m) return undefined
        let target: string
        let display: string
        if (m[1] !== undefined) {
          const inner = m[1]
          const pipe = inner.indexOf('|')
          target = (pipe >= 0 ? inner.slice(0, pipe) : inner).trim()
          const dispRaw = pipe >= 0 ? inner.slice(pipe + 1) : ''
          display = dispRaw.trim() || target.split('/').pop()?.trim() || target
        } else {
          target = m[2].trim()
          display = target.split('/').pop()?.trim() || target
        }
        if (!target) return undefined
        return { type: 'wikilink', raw: m[0], tokens: [], target, display }
      },
      renderer(token) {
        return (
          `<span class="wikilink" title="${escapeHtml((token as Record<string, unknown>).target)}">` +
          `${escapeHtml((token as Record<string, unknown>).display)}</span>`
        )
      },
    },
  ],
})

/** Markdown → HTML（消毒）。聊天語境下單一換行視為斷行（breaks）。 */
const html = computed(() => DOMPurify.sanitize(marked.parse(props.text) as string))
</script>

<template>
  <!-- eslint-disable-next-line vue/no-v-html —— 內容經 DOMPurify 消毒 -->
  <div class="chat-md" v-html="html"></div>
</template>

<style>
/* Markdown 渲染樣式：v-html 內容不吃 scoped 屬性 → 全域樣式但以 .chat-md 命名空間隔離。
   色彩一律由 currentColor 派生——底色反轉的氣泡（primary/accent）與一般卡面皆自適應；
   斷行交給 markdown（breaks:true），容器不使用 white-space: pre-wrap。 */
.chat-md { white-space: normal; word-break: break-word; }
.chat-md > :first-child { margin-top: 0; }
.chat-md > :last-child { margin-bottom: 0; }
.chat-md p { margin: 0.35em 0; }
.chat-md ul, .chat-md ol { margin: 0.35em 0; padding-left: 1.4em; }
.chat-md ul { list-style: disc; }
.chat-md ol { list-style: decimal; }
.chat-md h1, .chat-md h2, .chat-md h3, .chat-md h4 { margin: 0.6em 0 0.3em; font-weight: 600; line-height: 1.3; }
.chat-md h1 { font-size: 1.15em; }
.chat-md h2 { font-size: 1.1em; }
.chat-md h3 { font-size: 1.05em; }
.chat-md code { background: color-mix(in oklab, currentColor 12%, transparent); border-radius: 0.25rem; padding: 0.1em 0.35em; font-size: 0.85em; }
.chat-md pre { background: color-mix(in oklab, currentColor 8%, transparent); border: 1px solid color-mix(in oklab, currentColor 20%, transparent); border-radius: 0.45rem; padding: 0.6em 0.8em; overflow-x: auto; margin: 0.5em 0; }
.chat-md pre code { background: transparent; padding: 0; }
.chat-md table { border-collapse: collapse; margin: 0.5em 0; font-size: 0.9em; display: block; overflow-x: auto; }
.chat-md th, .chat-md td { border: 1px solid color-mix(in oklab, currentColor 25%, transparent); padding: 0.3em 0.6em; text-align: left; }
.chat-md th { background: color-mix(in oklab, currentColor 8%, transparent); font-weight: 600; }
.chat-md blockquote { border-left: 3px solid color-mix(in oklab, currentColor 30%, transparent); margin: 0.4em 0; padding-left: 0.8em; opacity: 0.85; }
.chat-md a { color: inherit; text-decoration: underline; }
.chat-md hr { border: 0; border-top: 1px solid color-mix(in oklab, currentColor 20%, transparent); margin: 0.6em 0; }
.chat-md .wikilink { color: var(--accent, currentColor); font-weight: 600; }
</style>
