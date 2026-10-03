<script setup lang="ts">
import { ApiError, api, OfflineError, openEventStream, type StreamEvent } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { onBeforeUnmount, onMounted, ref } from 'vue'

const { t } = useI18n()

interface EventRow {
  id: string
  employee_id: string
  employee_name: string
  kind: string
  detail: string
  created_at: string
}

const events = ref<EventRow[]>([])
const offline = ref(false)
const error = ref('')
let close: (() => void) | null = null

async function load(): Promise<void> {
  try {
    events.value = await api.get<EventRow[]>('/api/events?limit=80')
    error.value = ''
    offline.value = false
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

onMounted(async () => {
  await load()
  // SSE：新事件到達 → 重整清單（伺服器端已過濾為自身相關）。
  close = openEventStream((ev: StreamEvent) => {
    events.value.unshift({
      id: ev.id,
      employee_id: ev.employee_id,
      employee_name: ev.employee_id,
      kind: ev.kind,
      detail: ev.detail,
      created_at: ev.created_at,
    })
    if (events.value.length > 120) events.value.pop()
  })
})

onBeforeUnmount(() => {
  close?.()
})
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error" />
    <div v-if="events.length" class="card">
      <table class="tbl">
        <tbody>
          <tr v-for="e in events" :key="e.id">
            <td class="time">{{ e.created_at.slice(11, 19) }}</td>
            <td class="who">{{ e.employee_name }}</td>
            <td class="kind"><code>{{ e.kind }}</code></td>
            <td class="detail">{{ e.detail }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <p v-else class="muted">{{ t('events.empty') }}</p>
  </div>
</template>

<style scoped>
.tbl { width: 100%; border-collapse: collapse; font-size: 0.88rem; }
.tbl td { padding: 0.35rem 0.6rem; border-bottom: 1px solid var(--border); vertical-align: top; }
.time { color: var(--muted); white-space: nowrap; }
.who { white-space: nowrap; font-weight: 600; }
.kind { white-space: nowrap; }
.detail { word-break: break-word; }
</style>
