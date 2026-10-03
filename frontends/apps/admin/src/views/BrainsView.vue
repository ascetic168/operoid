<script setup lang="ts">
import { ApiError, api, OfflineError } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { onMounted, ref } from 'vue'

const { t } = useI18n()

interface BrainRow {
  id: string
  name: string
  gbrain_home: string
  active?: boolean
}

const brains = ref<BrainRow[]>([])
const prereq = ref<Record<string, unknown> | null>(null)
const offline = ref(false)
const error = ref('')

async function load(): Promise<void> {
  try {
    brains.value = await api.get<BrainRow[]>('/api/brains')
    error.value = ''
    offline.value = false
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
  try {
    prereq.value = await api.get<Record<string, unknown>>('/api/prereq')
  } catch {
    prereq.value = null
  }
}

onMounted(load)
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error" />
    <h2 class="sec">{{ t('brains.heading') }}</h2>
    <div v-if="brains.length" class="grid">
      <div v-for="b in brains" :key="b.id" class="card">
        <h2>{{ b.name }}</h2>
        <p class="muted"><code>{{ b.id }}</code></p>
        <p class="muted">{{ b.gbrain_home }}</p>
      </div>
    </div>
    <p v-else class="muted">{{ t('brains.none') }}</p>

    <h2 class="sec">{{ t('brains.prereq') }}</h2>
    <div v-if="prereq" class="card">
      <pre class="pre">{{ JSON.stringify(prereq, null, 2) }}</pre>
    </div>
    <p v-else class="muted">—</p>
  </div>
</template>

<style scoped>
.sec { margin: 1.4rem 0 0.8rem; font-size: 1.05rem; }
.pre {
  margin: 0; padding: 0.6rem; background: var(--bg); border-radius: 0.5rem;
  font-size: 0.78rem; overflow-x: auto;
}
</style>
