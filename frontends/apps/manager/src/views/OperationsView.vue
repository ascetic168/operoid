<script setup lang="ts">
import { ApiError, api, OfflineError } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { nextTick, ref } from 'vue'

const { t } = useI18n()

type OpName =
  | 'stats' | 'sync' | 'extract' | 'ask' | 'query' | 'think'
  | 'doctor' | 'orphans' | 'storage' | 'graph-query' | 'unify-types'

interface OpDef {
  id: OpName
  title: string
  descKey: string
  needsArg?: 'query' | 'slug'
}

// 與桌面 GUI OperationsView 同一組操作（gbrain CLI／知識檢索）；
// ask/query/think 走 KnowledgeService——按登入者身份做知識範圍過濾（C12a）。
const ops: OpDef[] = [
  { id: 'stats', title: 'stats', descKey: 'operations.ops.statsDesc' },
  { id: 'sync', title: 'sync', descKey: 'operations.ops.syncDesc' },
  { id: 'extract', title: 'extract', descKey: 'operations.ops.extractDesc' },
  { id: 'ask', title: 'ask', descKey: 'operations.ops.askDesc', needsArg: 'query' },
  { id: 'query', title: 'query', descKey: 'operations.ops.queryDesc', needsArg: 'query' },
  { id: 'think', title: 'think', descKey: 'operations.ops.thinkDesc', needsArg: 'query' },
]
const diagnostics: OpDef[] = [
  { id: 'doctor', title: 'doctor', descKey: 'operations.diag.doctorDesc' },
  { id: 'orphans', title: 'orphans', descKey: 'operations.diag.orphansDesc' },
  { id: 'storage', title: 'storage', descKey: 'operations.diag.storageDesc' },
  { id: 'graph-query', title: 'graph-query', descKey: 'operations.diag.graphQueryDesc', needsArg: 'slug' },
  { id: 'unify-types', title: 'unify-types', descKey: 'operations.diag.unifyTypesDesc' },
]

interface LogEntry {
  stream: string
  text: string
}
interface Snap {
  operation_id: string
  lines: LogEntry[]
  done: boolean
  result: { success?: boolean } | null
  dropped: boolean
}

const log = ref<LogEntry[]>([])
const running = ref<string | null>(null)
const query = ref('') // ask / query / think 共用
const slug = ref('') // graph-query
const offline = ref(false)
const error = ref('')
const consoleEl = ref<HTMLElement | null>(null)

async function push(stream: string, text: string): Promise<void> {
  log.value.push({ stream, text })
  await nextTick()
  if (consoleEl.value) consoleEl.value.scrollTop = consoleEl.value.scrollHeight
}

async function run(op: OpDef): Promise<void> {
  if (running.value) return
  const arg =
    op.needsArg === 'query' ? query.value.trim() : op.needsArg === 'slug' ? slug.value.trim() : null
  if (op.needsArg && !arg) return
  running.value = op.id
  offline.value = false
  error.value = ''
  await push('step', t('operations.stepOp', { op: op.title }))
  try {
    const { operation_id } = await api.post<{ operation_id: string }>('/api/operations', {
      op: op.id,
      arg,
    })
    let since = 0
    const started = Date.now()
    for (;;) {
      const snap = await api.get<Snap>(
        '/api/operations/' + encodeURIComponent(operation_id) + '?since=' + since,
      )
      for (const l of snap.lines) await push(l.stream, l.text)
      since += snap.lines.length
      if (snap.done) {
        const ok = snap.result?.success !== false
        await push('step', ok ? t('operations.done') : t('operations.failed'))
        break
      }
      if (snap.dropped) {
        await push('stderr', t('operations.dropped'))
        break
      }
      if (Date.now() - started > 300_000) {
        await push('stderr', t('operations.timeout'))
        break
      }
      await new Promise((r) => setTimeout(r, 500))
    }
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
    await push('stderr', error.value)
  } finally {
    running.value = null
  }
}

function clearLog(): void {
  log.value = []
}

/** 一行輸出切成「純文字／wikilink」段落（不用 v-html，防 XSS）。
 *  支援 gbrain 兩種標籤：`[[dir/slug]]`／`[[dir/slug|name]]`（筆記內文）與
 *  單括 `[dir/slug]`（ask/think 輸出的引用格式；須含 `/` 且後不接 `(`，
 *  避開 markdown 連結 `[text](url)`）。Web 版引用為唯讀標註（GUI 可點開筆記）。 */
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
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error" />
    <h2 class="sec">{{ t('operations.title') }}</h2>
    <p class="muted">{{ t('operations.desc') }}</p>

    <!-- 主操作 -->
    <div class="opgrid">
      <button
        v-for="op in ops"
        :key="op.id"
        class="opcard"
        :disabled="running !== null"
        data-test="op"
        @click="run(op)"
      >
        <span class="optitle">{{ op.title }}</span>
        <span class="muted">{{ t(op.descKey) }}</span>
      </button>
    </div>

    <!-- ask / query / think 輸入 -->
    <input
      v-model="query"
      class="qinput"
      :placeholder="t('operations.askPlaceholder')"
      data-test="op-query"
      @keydown.enter="run(ops.find((o) => o.id === 'think')!)"
    />

    <!-- 診斷 -->
    <div class="diagrow">
      <button
        v-for="d in diagnostics"
        :key="d.id"
        class="mini"
        :disabled="running !== null"
        @click="run(d)"
      >
        {{ d.title }}
      </button>
      <input v-model="slug" class="sluginput" :placeholder="t('operations.slugPlaceholder')" />
    </div>

    <!-- 主控台 -->
    <div class="console">
      <div class="conhead">
        <span class="muted">{{ t('operations.output') }}</span>
        <button class="mini" @click="clearLog">{{ t('operations.clear') }}</button>
      </div>
      <div ref="consoleEl" class="conbody">
        <div v-if="log.length === 0" class="muted">{{ t('operations.empty') }}</div>
        <div v-for="(entry, i) in log" :key="i" :class="'line-' + entry.stream">
          <template v-for="(seg, j) in linkSegments(entry.text)" :key="j">
            <span v-if="seg.kind === 'link'" class="wikilink" :title="seg.target">{{ seg.text }}</span>
            <template v-else>{{ seg.text }}</template>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sec { margin: 1.4rem 0 0.4rem; font-size: 1.05rem; }
.opgrid {
  display: grid; gap: 0.6rem; margin: 0.8rem 0;
  grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
}
.opcard {
  display: flex; flex-direction: column; align-items: flex-start; gap: 0.25rem;
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.5rem;
  padding: 0.7rem 0.8rem; text-align: left; cursor: pointer; font-size: 0.88rem;
}
.opcard:hover:not(:disabled) { border-color: var(--accent); }
.opcard:disabled { opacity: 0.5; cursor: wait; }
.optitle { font-family: ui-monospace, monospace; font-weight: 600; }
.qinput { margin: 0.4rem 0 0.8rem; }
.diagrow { display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: center; margin-bottom: 0.8rem; }
.sluginput { width: 16rem; padding: 0.35rem 0.6rem; font-size: 0.82rem; }
.mini {
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.4rem;
  padding: 0.3rem 0.8rem; cursor: pointer; font-size: 0.82rem;
}
.mini:disabled { opacity: 0.5; cursor: wait; }
.console {
  border: 1px solid var(--border); border-radius: 0.5rem; background: var(--surface);
  display: flex; flex-direction: column; min-height: 18rem;
}
.conhead {
  display: flex; justify-content: space-between; align-items: center;
  padding: 0.4rem 0.7rem; border-bottom: 1px solid var(--border);
}
.conbody {
  padding: 0.7rem; overflow-y: auto; max-height: 28rem;
  font-family: ui-monospace, monospace; font-size: 0.8rem; line-height: 1.5;
  white-space: pre-wrap; word-break: break-word;
}
.line-stdout { color: var(--text); }
.line-stderr { color: var(--danger); }
.line-step { color: var(--accent); }
.wikilink { color: var(--accent); font-weight: 600; }
</style>
