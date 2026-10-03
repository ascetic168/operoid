<script setup lang="ts">
import { ApiError, api, OfflineError, openEventStream, session, type StreamEvent } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

const { t } = useI18n()

interface EmployeeRow {
  id: string
  name: string
  state: string
  owner_principal: string | null
  template_id: string | null
}
interface TemplateRow {
  id: string
  name: string
  role: string | null
}

const mine = ref<EmployeeRow[]>([])
const others = ref<EmployeeRow[]>([])
const templates = ref<TemplateRow[]>([])
const offline = ref(false)
const loadError = ref('')
const actionError = ref('')
const streamUp = ref(false)
const latest = ref<StreamEvent | null>(null)

const me = computed(() => session()?.principal.id ?? '')

// 部署表單
const templateId = ref('')
const instanceName = ref('')
const deploying = ref(false)

let close: (() => void) | null = null

async function load(): Promise<void> {
  try {
    const emps = await api.get<EmployeeRow[]>('/api/employees')
    const meId = me.value
    mine.value = emps.filter((e) => e.owner_principal === meId)
    others.value = emps.filter((e) => e.owner_principal !== meId)
    templates.value = await api.get<TemplateRow[]>('/api/templates')
    if (!templateId.value && templates.value.length) {
      templateId.value = templates.value[0].id // 預設選第一個——select 未互動時 v-model 為空
    }
    offline.value = false
    loadError.value = ''
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return // 已由 api-client 清 session 導回登入
    offline.value = e instanceof OfflineError
    loadError.value =
      e instanceof ApiError ? e.code : offline.value ? 'server.offline' : 'server.internal'
  }
}

async function deploy(): Promise<void> {
  if (!templateId.value || !instanceName.value.trim()) return
  deploying.value = true
  actionError.value = ''
  try {
    await api.post('/api/employees/deploy', {
      template_id: templateId.value,
      instance_name: instanceName.value.trim(),
    })
    instanceName.value = ''
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    deploying.value = false
  }
}

async function stop(id: string): Promise<void> {
  actionError.value = ''
  try {
    await api.post('/api/employees/' + encodeURIComponent(id) + '/stop')
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  }
}

onMounted(async () => {
  await load()
  // SSE：任何（自身相關）事件到達 → 重新載入清單。
  close = openEventStream(
    (ev: StreamEvent) => {
      streamUp.value = true
      latest.value = ev
      void load()
    },
    () => {
      streamUp.value = false
    },
  )
})

onBeforeUnmount(() => {
  close?.()
})
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : loadError || actionError" />

    <div class="grid">
      <div class="card">
        <h2>{{ t('home.deploy.heading') }}</h2>
        <template v-if="templates.length">
          <label for="tpl">{{ t('home.deploy.template') }}</label>
          <select id="tpl" v-model="templateId" data-test="template">
            <option v-for="tpl in templates" :key="tpl.id" :value="tpl.id">
              {{ tpl.name }}<span v-if="tpl.role">（{{ tpl.role }}）</span>
            </option>
          </select>
          <label for="iname">{{ t('home.deploy.name') }}</label>
          <input id="iname" v-model="instanceName" data-test="instance" />
          <p style="margin-top: 0.9rem">
            <button class="primary" :disabled="deploying" data-test="deploy" @click="deploy">
              {{ t('home.deploy.submit') }}
            </button>
          </p>
        </template>
        <p v-else class="muted">{{ t('home.deploy.empty') }}</p>
      </div>
    </div>

    <h2 class="sec">{{ t('home.mine') }}</h2>
    <div v-if="mine.length" class="grid">
      <div v-for="e in mine" :key="e.id" class="card emp" data-test="my-employee">
        <h2>{{ e.name }}</h2>
        <p class="muted">{{ t('home.emp.state') }}：<code>{{ e.state }}</code></p>
        <div class="row">
          <router-link class="btn" :to="'/chat/' + encodeURIComponent(e.id)" data-test="chat">
            {{ t('home.emp.chat') }}
          </router-link>
          <button
            v-if="e.state === 'working'"
            class="btn warn"
            data-test="stop"
            @click="stop(e.id)"
          >
            {{ t('home.emp.stop') }}
          </button>
        </div>
      </div>
    </div>
    <p v-else class="muted">{{ t('home.none') }}</p>

    <h2 v-if="others.length" class="sec">{{ t('home.others') }}</h2>
    <div v-if="others.length" class="grid">
      <div v-for="e in others" :key="e.id" class="card emp ro">
        <h2>{{ e.name }}</h2>
        <p class="muted">{{ t('home.emp.state') }}：<code>{{ e.state }}</code></p>
      </div>
    </div>

    <p v-if="streamUp" class="muted">● {{ t('home.streamOn') }}</p>
  </div>
</template>

<style scoped>
.sec { margin: 1.6rem 0 0.8rem; font-size: 1.05rem; }
.emp .row { display: flex; gap: 0.5rem; margin-top: 0.6rem; }
.btn {
  display: inline-block; padding: 0.4rem 0.9rem; border-radius: 0.5rem;
  border: 1px solid var(--border); background: var(--surface); color: var(--accent);
  text-decoration: none; font-size: 0.88rem; cursor: pointer;
}
.btn:hover { border-color: var(--accent); }
.btn.warn { color: var(--danger); }
.btn.warn:hover { border-color: var(--danger); }
.ro { opacity: 0.75; }
</style>
