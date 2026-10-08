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

interface BrainDefaults {
  default_embedding_model: string | null
  default_embedding_dimensions: number | null
}

const brains = ref<BrainRow[]>([])
const prereq = ref<Record<string, unknown> | null>(null)
const offline = ref(false)
const error = ref('')

// ── 新腦預設 embedding（AppConfig 層；僅影響新建腦）──
const defModel = ref('')
const defDim = ref<number | null>(null)
const defBusy = ref(false)
const defOk = ref(false)
const defError = ref('')

async function loadDefaults(): Promise<void> {
  try {
    const d = await api.get<BrainDefaults>('/api/brains/defaults')
    defModel.value = d.default_embedding_model ?? ''
    defDim.value = d.default_embedding_dimensions ?? null
  } catch {
    /* 非致命：卡片留空即可 */
  }
}

async function saveDefaults(): Promise<void> {
  defOk.value = false
  defError.value = ''
  const model = defModel.value.trim()
  const dimRaw = defDim.value
  const dim = typeof dimRaw === 'number' && Number.isFinite(dimRaw) ? dimRaw : null
  if ((model && !model.includes(':')) || (dim !== null && dim <= 0)) {
    defError.value = 'brains.defEmbedInvalid'
    return
  }
  defBusy.value = true
  try {
    const r = await api.put<BrainDefaults>('/api/brains/defaults', {
      default_embedding_model: model || null,
      default_embedding_dimensions: dim,
    })
    defModel.value = r.default_embedding_model ?? ''
    defDim.value = r.default_embedding_dimensions ?? null
    defOk.value = true
  } catch (e) {
    defError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    defBusy.value = false
  }
}

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
  loadDefaults()
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

    <h2 class="sec">{{ t('brains.defaultsHeading') }}</h2>
    <div class="card">
      <p class="muted">{{ t('brains.defaultsDesc') }}</p>
      <div class="form-row">
        <label class="field">
          <span class="muted">{{ t('brains.defaultsModel') }}</span>
          <input v-model="defModel" :placeholder="t('brains.defaultsModelPh')" />
        </label>
        <label class="field field-dim">
          <span class="muted">{{ t('brains.defaultsDim') }}</span>
          <input v-model.number="defDim" type="number" placeholder="768" />
        </label>
        <button class="primary" :disabled="defBusy" @click="saveDefaults">
          {{ t('brains.defaultsSave') }}
        </button>
      </div>
      <p v-if="defError" class="muted err">{{ t(defError) }}</p>
      <p v-else-if="defOk" class="muted">{{ t('brains.defaultsSaved') }}</p>
    </div>

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
  margin: 0; padding: 0.6rem; background: var(--bg);
  font-size: 0.78rem; overflow-x: auto;
}
.form-row { display: flex; flex-wrap: wrap; gap: 0.6rem; align-items: flex-end; }
.field { display: flex; flex-direction: column; gap: 0.25rem; flex: 1; min-width: 16rem; }
.field-dim { flex: 0; min-width: 7rem; max-width: 10rem; }
.field input {
  padding: 0.4rem 0.6rem; border: 1px solid var(--border, #ccc);
  border-radius: 0.375rem; background: transparent; font-family: monospace; font-size: 0.8rem;
}
.err { color: var(--danger, #d33); }
</style>
