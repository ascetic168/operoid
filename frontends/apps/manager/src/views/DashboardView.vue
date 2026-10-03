<script setup lang="ts">
import { ApiError, api, OfflineError } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { computed, onMounted, ref } from 'vue'

const { t } = useI18n()

interface EmployeeRow {
  id: string
  name: string
  state: string
  owner_principal: string | null
}

const employees = ref<EmployeeRow[]>([])
const divergence = ref<Record<string, unknown> | null>(null)
const offline = ref(false)
const error = ref('')
const loadError = ref('')

const byState = computed(() => {
  const m = new Map<string, string[]>()
  for (const e of employees.value) {
    const list = m.get(e.state) ?? []
    list.push(e.name)
    m.set(e.state, list)
  }
  return Array.from(m.entries())
    .map(([state, names]) => ({ state, count: names.length, names }))
    .sort((a, b) => b.count - a.count)
})

onMounted(async () => {
  try {
    employees.value = await api.get<EmployeeRow[]>('/api/employees')
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    loadError.value =
      e instanceof ApiError ? e.code : offline.value ? 'server.offline' : 'server.internal'
    return
  }
  try {
    const reg = await api.get<Record<string, unknown>>('/api/registry')
    divergence.value = (reg.divergence as Record<string, unknown>) ?? null
  } catch {
    divergence.value = null
  }
})
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : loadError" />
    <div class="grid">
      <div class="card wide">
        <h2>{{ t('dash.employeesByState') }}（{{ employees.length }}）</h2>
        <table class="tbl">
          <tbody>
            <tr v-for="g in byState" :key="g.state">
              <td class="st"><code>{{ g.state }}</code></td>
              <td class="n">{{ g.count }}</td>
              <td class="names muted">{{ g.names.join('、') }}</td>
            </tr>
            <tr v-if="!byState.length">
              <td class="muted">—</td>
            </tr>
          </tbody>
        </table>
      </div>
      <div class="card wide">
        <h2>{{ t('dash.registry') }}</h2>
        <pre v-if="divergence" class="pre">{{ JSON.stringify(divergence, null, 2) }}</pre>
        <p v-else class="muted">—</p>
        <p class="muted">{{ t('dash.inboxHint') }}</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wide { grid-column: 1 / -1; }
.tbl { width: 100%; border-collapse: collapse; font-size: 0.9rem; }
.tbl td { padding: 0.3rem 0.6rem; border-bottom: 1px solid var(--border); }
.st { width: 7rem; }
.n { width: 3rem; font-weight: 700; color: var(--accent); }
.names { word-break: break-word; }
.pre {
  margin: 0; padding: 0.6rem; background: var(--bg); border-radius: 0.5rem;
  font-size: 0.78rem; overflow-x: auto;
}
</style>
