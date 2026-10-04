<script setup lang="ts">
import { ApiError, api, OfflineError } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { ref } from 'vue'

const { t } = useI18n()

const query = ref('')
const running = ref<'' | 'ask' | 'think'>('')
const offline = ref(false)
const error = ref('')
const output = ref('') // 檢索結果（含 gbrain 引用格式）
const denied = ref(false)

/** 一段輸出切成「純文字／wikilink」段落（不用 v-html，防 XSS）。
 *  支援 gbrain 兩種標籤：`[[dir/slug]]`／`[[dir/slug|name]]` 與單括 `[dir/slug]`
 *  （ask/think 輸出的引用格式；須含 `/` 且後不接 `(`，避開 markdown 連結）。 */
interface LinkSeg {
  kind: 'text' | 'link'
  text: string
  target?: string
}
function linkSegments(text: string): LinkSeg[] {
  const segs: LinkSeg[] = []
  const re = /\[\[([^\]]+)\]\]|\[([^\]\[\s|/]+\/[^\]\[\s|/]+)\](?!\()/g
  let last = 0
  for (const m of text.matchAll(re)) {
    const idx = m.index ?? 0
    if (idx > last) segs.push({ kind: 'text', text: text.slice(last, idx) })
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
    if (target) segs.push({ kind: 'link', text: display, target })
    else segs.push({ kind: 'text', text: m[0] })
    last = idx + m[0].length
  }
  if (last < text.length) segs.push({ kind: 'text', text: text.slice(last) })
  return segs
}

/** 使用者級知識檢索：POST /api/knowledge/ask（Req::User）——伺服器端按登入者
 *  身份的知識範圍過濾（C12a：policy fail-closed＋receipt），同步回應。 */
async function run(op: 'ask' | 'think'): Promise<void> {
  const q = query.value.trim()
  if (!q || running.value) return
  running.value = op
  offline.value = false
  error.value = ''
  output.value = ''
  denied.value = false
  try {
    const r = await api.post<{ text: string; denied: boolean }>('/api/knowledge/ask', {
      op,
      arg: q,
    })
    output.value = r.text
    denied.value = r.denied
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  } finally {
    running.value = ''
  }
}
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error" />
    <h2 class="sec">{{ t('ask.title') }}</h2>
    <p class="muted">{{ t('ask.desc') }}</p>

    <div class="askrow">
      <input
        v-model="query"
        class="qinput"
        :placeholder="t('ask.placeholder')"
        data-test="ask-query"
        @keydown.enter="run('think')"
      />
      <button class="primary askbtn" :disabled="running !== '' || !query.trim()" data-test="ask-ask" @click="run('ask')">
        {{ running === 'ask' ? t('ask.running') : t('ask.ask') }}
      </button>
      <button class="primary askbtn" :disabled="running !== '' || !query.trim()" data-test="ask-think" @click="run('think')">
        {{ running === 'think' ? t('ask.running') : t('ask.think') }}
      </button>
    </div>

    <div v-if="denied" class="denied">{{ t('ask.denied') }}</div>
    <div v-else-if="output" class="result" data-test="ask-output">
      <template v-for="(seg, j) in linkSegments(output)" :key="j">
        <span v-if="seg.kind === 'link'" class="wikilink" :title="seg.target">{{ seg.text }}</span>
        <template v-else>{{ seg.text }}</template>
      </template>
    </div>
  </div>
</template>

<style scoped>
.sec { margin: 1.4rem 0 0.4rem; font-size: 1.05rem; }
.askrow { display: flex; gap: 0.5rem; margin: 0.8rem 0; flex-wrap: wrap; }
.qinput { flex: 1 1 20rem; }
.askbtn { flex: 0 0 auto; width: auto; padding: 0.55rem 1.1rem; }
.denied {
  margin-top: 0.8rem; padding: 0.7rem; border: 1px solid var(--border);
  border-radius: 0.5rem; background: var(--surface); color: var(--muted); font-size: 0.9rem;
}
.result {
  margin-top: 0.8rem; padding: 0.9rem; border: 1px solid var(--border);
  border-radius: 0.5rem; background: var(--surface);
  white-space: pre-wrap; word-break: break-word; font-size: 0.9rem; line-height: 1.55;
}
.wikilink { color: var(--accent); font-weight: 600; }
</style>
