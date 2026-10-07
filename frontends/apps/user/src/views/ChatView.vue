<script setup lang="ts">
import { ApiError, api } from '@front/api-client'
import { ErrorBox, MarkdownText } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'

const { t } = useI18n()
const route = useRoute()
const empId = decodeURIComponent(route.params.id as string)

interface WatchPayload {
  employee: { id: string; name: string; state: string; owner_principal: string | null }
  commitments: Array<{
    id: string
    title: string
    status: string
    completion_condition: string
  }>
  proposals: Array<{ id: string; title: string; status: string }>
  artifacts: Array<{
    id: string
    title: string
    artifact_type: string
    content: string
    status: string
    created_at: string
  }>
  events: Array<{ id: string; kind: string; detail: string; created_at: string }>
  messages: Array<{
    id: string
    direction: 'in' | 'out'
    text: string
    created_at?: string
    proposed_commitment_id?: string | null
    artifact_id?: string | null
  }>
  llm_model?: string | null
}

const w = ref<WatchPayload | null>(null)
const error = ref('')
const forbidden = ref(false)
const offline = ref(false)
const text = ref('')
const sending = ref(false)
const showCommit = ref(false)
const commitTitle = ref('')
const commitCond = ref('')
const actionError = ref('')
const logEl = ref<HTMLElement | null>(null)

let timer: ReturnType<typeof setInterval> | null = null

const empName = computed(() => w.value?.employee.name ?? empId)
const empState = computed(() => w.value?.employee.state ?? '')
const activeCommitments = computed(
  () => (w.value?.commitments ?? []).filter((c) => c.status === 'active'),
)
const proposals = computed(() => w.value?.proposals ?? [])

// ── 對話串：訊息（最新在前 → 反轉成時序）＋插入的「工具過程列」 ──
type ChatMessage = NonNullable<WatchPayload['messages'][number]>
type ThreadItem = { kind: 'msg'; msg: ChatMessage } | { kind: 'trace' }

/** tool_call 事件 detail（契約 v1：v/step/tool/args/status/ms/note；ocore record_tool_call_event）。 */
interface ToolCallStep {
  v: number
  step: number
  tool: string
  args: string
  status: string
  ms: number
  note: string
}

function parseToolCall(detail: string): ToolCallStep | null {
  try {
    const d = JSON.parse(detail) as ToolCallStep
    if (d && typeof d.tool === 'string') return d
  } catch {
    /* 非 JSON 或舊格式事件：略過 */
  }
  return null
}

const working = computed(() => w.value?.employee.state === 'working')
/** 送出訊息後、回覆抵達前：員工 working 且最新一則是 In（watch 的 messages 最新在前）。 */
const awaitingReply = computed(() => {
  if (!working.value) return false
  const msgs = w.value?.messages ?? []
  return msgs.length === 0 || (msgs[0]?.direction ?? 'in') === 'in'
})

/** 本回合的過程：最後一則 In 訊息之後的 tool_call 事件（時序排列）。 */
const toolCalls = computed<ToolCallStep[]>(() => {
  const events = w.value?.events ?? []
  let sinceTs = 0
  for (const m of w.value?.messages ?? []) {
    if (m.direction !== 'in') continue
    const ts = new Date(m.created_at ?? '').getTime()
    if (!Number.isNaN(ts) && ts > sinceTs) sinceTs = ts
  }
  const out: ToolCallStep[] = []
  for (const e of events) {
    if (e.kind !== 'tool_call') continue
    if (sinceTs && new Date(e.created_at).getTime() < sinceTs) continue
    const d = parseToolCall(e.detail)
    if (d) out.push(d)
  }
  return out.reverse() // events 最新在前 → 時序
})

const traceOpen = ref(false)
const showTrace = computed(() => toolCalls.value.length > 0 || awaitingReply.value)

const thread = computed<ThreadItem[]>(() => {
  const msgs = (w.value?.messages ?? []).slice().reverse()
  const items: ThreadItem[] = msgs.map((m) => ({ kind: 'msg', msg: m }))
  if (showTrace.value) {
    let lastInIdx = -1
    for (let i = msgs.length - 1; i >= 0; i--) {
      if (msgs[i].direction === 'in') {
        lastInIdx = i
        break
      }
    }
    items.splice(lastInIdx + 1, 0, { kind: 'trace' })
  }
  return items
})

// ── Artifact 展開：watch 只帶近 10 筆——更舊的展開時 fallback 載 API（F3）──
const openArtifacts = ref<Set<string>>(new Set())
function artifactOf(id: string) {
  return (
    loadedArtifacts.value[id] ??
    (w.value?.artifacts ?? []).find((a) => a.id === id) ??
    null
  )
}
const loadedArtifacts = ref<Record<string, { title: string; content: string }>>({})
const artifactErrors = ref<Record<string, boolean>>({})
async function toggleArtifact(id: string): Promise<void> {
  if (!openArtifacts.value.has(id) && !artifactOf(id) && !artifactErrors.value[id]) {
    try {
      loadedArtifacts.value[id] = await api.get('/api/artifacts/' + encodeURIComponent(id))
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) return
      artifactErrors.value[id] = true
    }
  }
  const next = new Set(openArtifacts.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  openArtifacts.value = next
}

// ── 過程列 args 展開 ──
const openArgs = ref<Set<number>>(new Set())
function toggleArgs(i: number) {
  const next = new Set(openArgs.value)
  if (next.has(i)) next.delete(i)
  else next.add(i)
  openArgs.value = next
}

// ── 捲動：黏底偵測（比照桌面版）──
const stickToBottom = ref(true)

function onScroll() {
  const el = logEl.value
  if (!el) return
  stickToBottom.value = el.scrollHeight - el.scrollTop - el.clientHeight < 48
}

function scrollToBottom(force = false) {
  const el = logEl.value
  if (!el) return
  if (force) stickToBottom.value = true
  if (stickToBottom.value) el.scrollTop = el.scrollHeight
}

async function poll(): Promise<void> {
  try {
    w.value = await api.get<WatchPayload>(
      '/api/employees/' + encodeURIComponent(empId) + '/watch',
    )
    error.value = ''
    offline.value = false
    forbidden.value = false
    await nextTick()
    scrollToBottom()
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    if (e instanceof ApiError && e.status === 403) {
      forbidden.value = true
      if (timer) clearInterval(timer)
      return
    }
    offline.value = !(e instanceof ApiError)
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

async function send(): Promise<void> {
  const v = text.value.trim()
  if (!v || sending.value) return
  sending.value = true
  actionError.value = ''
  traceOpen.value = true // 送出後自動展開過程列，讓「正在做什麼」可見
  try {
    await api.post('/api/employees/' + encodeURIComponent(empId) + '/messages', { text: v })
    text.value = ''
    await poll()
    scrollToBottom(true)
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.offline'
  } finally {
    sending.value = false
  }
}

async function createCommitment(): Promise<void> {
  if (!commitTitle.value.trim() || !commitCond.value.trim()) return
  actionError.value = ''
  try {
    await api.post('/api/commitments', {
      employee_id: empId,
      title: commitTitle.value.trim(),
      completion_condition: commitCond.value.trim(),
    })
    commitTitle.value = ''
    commitCond.value = ''
    showCommit.value = false
    await poll()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

async function approve(id: string): Promise<void> {
  actionError.value = ''
  try {
    await api.post('/api/commitments/' + encodeURIComponent(id) + '/approve')
    await poll()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

async function reject(id: string): Promise<void> {
  actionError.value = ''
  try {
    await api.post('/api/commitments/' + encodeURIComponent(id) + '/reject')
    await poll()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

onMounted(() => {
  void poll()
  // 輪詢為主（訊息＋tool_call 過程）；SSE 事件由事件流頁呈現。
  timer = setInterval(poll, 2000)
})

onBeforeUnmount(() => {
  if (timer) clearInterval(timer)
})
</script>

<template>
  <div>
    <div class="head">
      <router-link class="back" to="/">{{ t('chat.back') }}</router-link>
      <h1>{{ empName }}</h1>
      <code class="state">{{ empState }}</code>
      <span v-if="w?.llm_model" class="muted model">{{ w.llm_model }}</span>
    </div>
    <ErrorBox :message="offline ? t('login.offline') : error || actionError" />

    <p v-if="forbidden" class="muted">{{ t('chat.forbidden') }}</p>

    <div v-else class="layout">
      <div class="chat card">
        <div ref="logEl" class="log" data-test="log" @scroll.passive="onScroll">
          <p v-if="!thread.length" class="muted">{{ t('chat.empty') }}</p>
          <template v-for="item in thread" :key="item.kind === 'msg' ? item.msg.id : 'trace'">
            <!-- 工具過程列：插在最後一則 In 訊息之後（本回合「正在做什麼」） -->
            <div v-if="item.kind === 'trace'" class="trace">
              <button class="tracehead" type="button" @click="traceOpen = !traceOpen">
                <span v-if="awaitingReply" class="spinner" aria-hidden="true"></span>
                <span v-if="awaitingReply" class="tracework">{{ t('chat.processing') }}</span>
                <span v-if="toolCalls.length" class="tracecount">{{ t('chat.traceCount', toolCalls.length) }}</span>
                <span class="muted tri">{{ traceOpen ? '▾' : '▸' }}</span>
              </button>
              <div v-if="traceOpen" class="tracebody">
                <div v-for="(c, ci) in toolCalls" :key="ci" class="step">
                  <div class="steprow">
                    <span class="dot" :class="c.status === 'ok' ? 'ok' : 'warn'" :title="c.status"></span>
                    <code class="tool">{{ c.tool }}</code>
                    <span class="muted">· {{ c.ms }}ms</span>
                    <span class="muted stepno">#{{ c.step }}</span>
                  </div>
                  <p
                    v-if="c.note"
                    class="note"
                    :class="{ clipped: !openArgs.has(ci) }"
                    :title="c.args"
                    @click="toggleArgs(ci)"
                  >
                    {{ c.note }}
                  </p>
                  <pre v-if="openArgs.has(ci)" class="args">{{ c.args }}</pre>
                </div>
              </div>
            </div>

            <!-- 訊息氣泡 -->
            <div v-else class="msg" :class="item.msg.direction === 'in' ? 'mine' : 'theirs'">
              <div class="bubble">
                <!-- 員工回覆：Markdown（共用元件內部 marked→DOMPurify 消毒） -->
                <MarkdownText
                  v-if="item.msg.direction === 'out'"
                  :text="item.msg.text"
                />
                <p v-else class="txt">{{ item.msg.text }}</p>
                <p v-if="item.msg.proposed_commitment_id" class="muted">📎 {{ t('chat.proposed') }}</p>
              </div>
              <!-- Artifact 展開卡：watch 近 10 筆內直接顯示，更舊的 fallback 載 API -->
              <div v-if="item.msg.artifact_id" class="artwrap">
                <button class="arttoggle" type="button" @click="toggleArtifact(item.msg.artifact_id)">
                  📦 {{ artifactOf(item.msg.artifact_id)?.title ?? item.msg.artifact_id }}
                  {{ openArtifacts.has(item.msg.artifact_id) ? '▾' : '▸' }}
                </button>
                <pre
                  v-if="openArtifacts.has(item.msg.artifact_id) && artifactOf(item.msg.artifact_id)"
                  class="artcontent"
                >{{ artifactOf(item.msg.artifact_id)?.content ?? '' }}</pre>
                <p
                  v-else-if="openArtifacts.has(item.msg.artifact_id) && artifactErrors[item.msg.artifact_id]"
                  class="muted arterror"
                >{{ t('chat.artifactLoadFailed') }}</p>
              </div>
            </div>
          </template>
        </div>
        <form class="send" @submit.prevent="send">
          <input
            v-model="text"
            :placeholder="t('chat.input')"
            data-test="chat-input"
            :disabled="forbidden"
          />
          <button class="primary sendbtn" :disabled="sending || !text.trim()" type="submit">
            {{ t('chat.send') }}
          </button>
        </form>
      </div>

      <aside class="side">
        <div class="card">
          <div class="sidehead">
            <h2>{{ t('chat.commitments') }}</h2>
            <button class="mini" data-test="new-commit" @click="showCommit = !showCommit">
              {{ showCommit ? '×' : '+' }}
            </button>
          </div>
          <form v-if="showCommit" class="cform" @submit.prevent="createCommitment">
            <input
              v-model="commitTitle"
              :placeholder="t('chat.commitTitle')"
              data-test="commit-title"
            />
            <input
              v-model="commitCond"
              :placeholder="t('chat.commitCond')"
              data-test="commit-cond"
            />
            <button class="primary" type="submit">{{ t('chat.commitSubmit') }}</button>
          </form>
          <ul class="list">
            <li v-for="c in activeCommitments" :key="c.id">
              <b>{{ c.title }}</b>
              <span class="muted">{{ c.completion_condition }}</span>
            </li>
            <li v-if="!activeCommitments.length" class="muted">—</li>
          </ul>
        </div>
        <div v-if="proposals.length" class="card">
          <h2>{{ t('chat.proposals') }}</h2>
          <div v-for="p in proposals" :key="p.id" class="prop">
            <b>{{ p.title }}</b>
            <div class="row">
              <button class="mini ok" @click="approve(p.id)">{{ t('inbox.approve') }}</button>
              <button class="mini bad" @click="reject(p.id)">{{ t('inbox.reject') }}</button>
            </div>
          </div>
        </div>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.head { display: flex; align-items: baseline; gap: 0.8rem; }
.head h1 { margin: 0; font-size: 1.3rem; }
.back { color: var(--accent); text-decoration: none; font-size: 0.88rem; }
.state { font-size: 0.8rem; }
.model { font-size: 0.78rem; }
.layout { display: grid; grid-template-columns: 1fr 18rem; gap: 1rem; margin-top: 1rem; }
.log {
  height: 26rem; overflow-y: auto; display: flex; flex-direction: column; gap: 0.5rem;
  padding: 0.5rem;
}
.msg { display: flex; flex-direction: column; flex: none; }
.msg.mine { align-items: flex-end; }
.bubble {
  max-width: 78%; padding: 0.5rem 0.8rem; border-radius: 0.9rem;
  background: var(--bg); border: 1px solid var(--border);
}
.mine .bubble { background: var(--accent); color: #fff; border-color: var(--accent); }
.mine .bubble .muted { color: rgba(255, 255, 255, 0.75); }
.txt { margin: 0; white-space: pre-wrap; word-break: break-word; }
.bubble .muted { margin: 0.2rem 0 0; font-size: 0.75rem; }

/* 工具過程列（flex: none——column flex 下避免被壓縮裁切） */
.trace {
  width: 100%; flex: none; border: 1px solid var(--border); border-radius: 0.6rem;
  background: var(--surface); overflow: hidden;
}
.tracehead {
  display: flex; align-items: center; gap: 0.5rem; width: 100%;
  padding: 0.35rem 0.7rem; border: 0; background: transparent; cursor: pointer;
  font-size: 0.78rem; color: var(--muted); text-align: left;
}
.tracehead:hover { color: var(--text); }
.tracework { font-weight: 600; color: var(--ok); }
.tracecount { color: inherit; }
.tri { margin-left: auto; }
.spinner {
  width: 0.7rem; height: 0.7rem; border-radius: 50%; flex: none;
  border: 2px solid var(--border); border-top-color: var(--ok);
  animation: spin 0.9s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }
.tracebody { border-top: 1px solid var(--border); padding: 0.3rem 0.7rem; }
.step { padding: 0.25rem 0; font-size: 0.78rem; }
.steprow { display: flex; align-items: center; gap: 0.45rem; }
.dot { width: 0.4rem; height: 0.4rem; border-radius: 50%; flex: none; }
.dot.ok { background: var(--ok); }
.dot.warn { background: var(--danger); }
.tool { font-weight: 600; }
.stepno { margin-left: auto; font-size: 0.7rem; }
.note { margin: 0.15rem 0 0; padding-left: 0.95rem; color: var(--muted); cursor: pointer; word-break: break-word; }
.note.clipped { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.args {
  margin: 0.25rem 0 0; padding: 0.3rem 0.5rem; padding-left: 0.95rem;
  font-size: 0.68rem; color: var(--muted); background: var(--bg);
  border: 1px solid var(--border); border-radius: 0.4rem;
  overflow-x: auto; white-space: pre-wrap; word-break: break-all;
}

/* Artifact 展開卡 */
.artwrap { margin-top: 0.2rem; padding: 0 0.25rem; align-self: flex-start; max-width: 78%; }
.arttoggle { border: 0; background: transparent; padding: 0; cursor: pointer; font-size: 0.75rem; color: var(--muted); }
.arttoggle:hover { color: var(--text); }
.artcontent {
  margin: 0.25rem 0 0; padding: 0.4rem 0.6rem; font-size: 0.75rem;
  background: var(--bg); border: 1px solid var(--border); border-radius: 0.5rem;
  max-height: 15rem; overflow-y: auto; white-space: pre-wrap; word-break: break-word;
}
.arterror { margin: 0.25rem 0 0; font-size: 0.75rem; color: var(--danger); }

.send { display: flex; gap: 0.5rem; margin-top: 0.7rem; }
.sendbtn { width: auto; padding: 0.55rem 1.2rem; }
.side { display: flex; flex-direction: column; gap: 1rem; }
.sidehead { display: flex; align-items: center; justify-content: space-between; }
.sidehead h2 { margin: 0; }
.mini {
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.4rem;
  padding: 0.2rem 0.6rem; cursor: pointer; font-size: 0.82rem;
}
.mini.ok { color: var(--ok); }
.mini.bad { color: var(--danger); }
.cform { display: flex; flex-direction: column; gap: 0.4rem; margin-top: 0.5rem; }
.list { list-style: none; margin: 0.5rem 0 0; padding: 0; display: flex; flex-direction: column; gap: 0.5rem; }
.list li { font-size: 0.88rem; display: flex; flex-direction: column; }
.prop { margin-bottom: 0.7rem; }
.row { display: flex; gap: 0.4rem; margin-top: 0.3rem; }
@media (max-width: 52rem) { .layout { grid-template-columns: 1fr; } }
</style>

