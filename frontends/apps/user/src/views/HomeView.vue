<script setup lang="ts">
import { ApiError, api, OfflineError, openEventStream, type StreamEvent } from '@front/api-client'
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

const employees = ref<number | null>(null)
const templates = ref<number | null>(null)
const proposals = ref<number | null>(null)
const flagged = ref<number | null>(null)
const installed = ref<boolean | null>(null)
const streamUp = ref(false)
const latest = ref<StreamEvent | null>(null)
const offline = ref(false)
const loadError = ref('')
const scope = "我的員工＋交辦＋知識查詢（R5）"

let close: (() => void) | null = null

onMounted(async () => {
  try {
    const emps = await api.get<Array<unknown>>('/api/employees')
    employees.value = emps.length
    const tmpls = await api.get<Array<unknown>>('/api/templates')
    templates.value = tmpls.length
  } catch (e) {
    offline.value = e instanceof OfflineError
    loadError.value = e instanceof ApiError ? e.code : offline.value ? 'server.offline' : 'server.internal'
  }
  try {
    const inbox = await api.get<{ proposals: unknown[]; flagged_employees: unknown[] }>('/api/inbox')
    proposals.value = inbox.proposals.length
    flagged.value = inbox.flagged_employees.length
  } catch {
    /* 非 user/manager 語意差——留空 */
  }
  try {
    const svc = await api.get<{ installed: boolean }>('/api/service/status')
    installed.value = svc.installed
  } catch {
    installed.value = null
  }
  // SSE：短票 → 事件流（斷線自動以新票重連）。
  close = openEventStream(
    (ev) => {
      streamUp.value = true
      latest.value = ev
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
    <p class="muted">{{ t('home.welcome') }}<span v-if="offline">——{{ t('login.offline') }}</span><span v-else-if="loadError">（{{ loadError }}）</span></p>
    <div class="grid">
      <div v-if="employees !== null" class="card">
        <h2>{{ t('home.employees') }}</h2>
        <span class="stat">{{ employees }}</span>
      </div>
      <div v-if="templates !== null" class="card">
        <h2>{{ t('home.templates') }}</h2>
        <span class="stat">{{ templates }}</span>
      </div>
      <div v-if="proposals !== null" class="card">
        <h2>{{ t('home.proposals') }}</h2>
        <span class="stat">{{ proposals }}</span>
      </div>
      <div v-if="flagged !== null" class="card">
        <h2>{{ t('home.flagged') }}</h2>
        <span class="stat">{{ flagged }}</span>
      </div>
      <div v-if="installed !== null" class="card">
        <h2>{{ t('home.service') }}</h2>
        <span class="stat">{{ installed ? '✓' : '—' }}</span>
        <p class="muted">{{ installed ? t('home.installed') : t('home.notInstalled') }}</p>
      </div>
      <div class="card">
        <h2>{{ t('home.stream') }}</h2>
        <span class="stat">{{ streamUp ? '✓' : '…' }}</span>
        <p class="muted">{{ streamUp ? t('home.streamOn') : t('home.streamDown') }}</p>
        <p v-if="latest" class="muted">
          {{ t('home.latest') }}：<code>{{ latest.kind }}</code> {{ latest.detail }}
        </p>
      </div>
      <div class="card">
        <h2>{{ t('home.scope') }}</h2>
        <p class="muted">我的員工＋交辦＋知識查詢（R5）</p>
      </div>
    </div>
  </div>
</template>
