<script setup lang="ts">
import { ApiError, api } from '@front/api-client'
import { ErrorBox } from '@front/ui'
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
const messages = computed(() => w.value?.messages ?? [])
const activeCommitments = computed(
  () => (w.value?.commitments ?? []).filter((c) => c.status === 'active'),
)
const proposals = computed(() => w.value?.proposals ?? [])

async function poll(): Promise<void> {
  try {
    w.value = await api.get<WatchPayload>(
      '/api/employees/' + encodeURIComponent(empId) + '/watch',
    )
    error.value = ''
    offline.value = false
    forbidden.value = false
    await nextTick()
    if (logEl.value) logEl.value.scrollTop = logEl.value.scrollHeight
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
  try {
    await api.post('/api/employees/' + encodeURIComponent(empId) + '/messages', { text: v })
    text.value = ''
    await poll()
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
  // 輪詢為主（訊息）；SSE 事件由事件流頁呈現。
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
        <div ref="logEl" class="log" data-test="log">
          <p v-if="!messages.length" class="muted">{{ t('chat.empty') }}</p>
          <div
            v-for="m in messages"
            :key="m.id"
            class="msg"
            :class="m.direction === 'in' ? 'mine' : 'theirs'"
          >
            <div class="bubble">
              <p class="txt">{{ m.text }}</p>
              <p v-if="m.proposed_commitment_id" class="muted">📎 {{ t('chat.proposed') }}</p>
              <p v-if="m.artifact_id" class="muted">📦 {{ m.artifact_id }}</p>
            </div>
          </div>
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
.msg { display: flex; }
.msg.mine { justify-content: flex-end; }
.bubble {
  max-width: 78%; padding: 0.5rem 0.8rem; border-radius: 0.9rem;
  background: var(--bg); border: 1px solid var(--border);
}
.mine .bubble { background: var(--accent); color: #fff; border-color: var(--accent); }
.mine .bubble .muted { color: rgba(255, 255, 255, 0.75); }
.txt { margin: 0; white-space: pre-wrap; word-break: break-word; }
.bubble .muted { margin: 0.2rem 0 0; font-size: 0.75rem; }
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
