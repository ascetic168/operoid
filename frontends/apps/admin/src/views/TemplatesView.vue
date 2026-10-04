<script setup lang="ts">
import { ApiError, api, OfflineError } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { onMounted, ref } from 'vue'

const { t } = useI18n()

interface TemplateRow {
  id: string
  name: string
  brain: { brain_id: string }
  role: string | null
  tools: string[] | null
  created_at: string
}
interface BrainRow {
  id: string
  name: string
  active?: boolean
}
interface BrainsPayload {
  active_id: string | null
  brains: BrainRow[]
}

const templates = ref<TemplateRow[]>([])
const brains = ref<BrainRow[]>([])
const activeBrainId = ref<string | null>(null)
const offline = ref(false)
const error = ref('')
const actionError = ref('')
const okMsg = ref('')
const busyId = ref('')

// 建立
const tName = ref('')
const tBrain = ref('')
const tRole = ref('')
const tTools = ref('')
// 改名（inline）
const renameId = ref('')
const renameName = ref('')

async function load(): Promise<void> {
  try {
    templates.value = await api.get<TemplateRow[]>('/api/templates')
    error.value = ''
    offline.value = false
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
  try {
    const o = await api.get<BrainsPayload>('/api/brains')
    brains.value = o.brains
    activeBrainId.value = o.active_id
  } catch {
    brains.value = []
  }
}

onMounted(load)

async function create(): Promise<void> {
  if (!tName.value.trim()) return
  actionError.value = ''
  okMsg.value = ''
  try {
    await api.post('/api/templates', {
      name: tName.value.trim(),
      brain_id: tBrain.value || null,
      role: tRole.value.trim() || null,
      tools: tTools.value.trim()
        ? tTools.value.split(/[,，\s]+/).filter(Boolean)
        : null,
    })
    okMsg.value = t('templates.created')
    tName.value = ''
    tBrain.value = ''
    tRole.value = ''
    tTools.value = ''
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  }
}

function startRename(row: TemplateRow): void {
  renameId.value = row.id
  renameName.value = row.name
  actionError.value = ''
  okMsg.value = ''
}

async function saveRename(id: string): Promise<void> {
  if (!renameName.value.trim()) return
  actionError.value = ''
  okMsg.value = ''
  busyId.value = id
  try {
    await api.patch('/api/templates/' + encodeURIComponent(id), { name: renameName.value.trim() })
    okMsg.value = t('templates.renamed')
    renameId.value = ''
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    busyId.value = ''
  }
}

async function remove(row: TemplateRow): Promise<void> {
  if (!window.confirm(t('templates.deleteConfirm', { name: row.name }))) return
  actionError.value = ''
  okMsg.value = ''
  busyId.value = row.id
  try {
    await api.del('/api/templates/' + encodeURIComponent(row.id))
    okMsg.value = t('templates.deleted')
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    busyId.value = ''
  }
}
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error" />
    <h2 class="sec">{{ t('templates.heading') }}</h2>

    <div class="grid">
      <div class="card">
        <h2>{{ t('templates.createHeading') }}</h2>
        <label for="tname">{{ t('templates.name') }}</label>
        <input id="tname" v-model="tName" data-test="tpl-name" />
        <label for="tbrain">{{ t('templates.brain') }}</label>
        <select id="tbrain" v-model="tBrain">
          <option value="">{{ t('templates.activeBrain') }}</option>
          <option v-for="b in brains" :key="b.id" :value="b.id">
            {{ b.name }}（{{ b.id }}）{{ b.id === activeBrainId ? ' ✓' : '' }}
          </option>
        </select>
        <label for="trole">{{ t('templates.role') }}</label>
        <input id="trole" v-model="tRole" />
        <label for="ttools">{{ t('templates.tools') }}</label>
        <input id="ttools" v-model="tTools" placeholder="write-note, send-message" />
        <p style="margin-top: 0.9rem">
          <button class="primary" :disabled="!tName.trim()" data-test="tpl-create" @click="create">
            {{ t('templates.create') }}
          </button>
        </p>
      </div>

      <div class="card">
        <h2>{{ t('templates.list') }}</h2>
        <table v-if="templates.length" class="tbl">
          <thead>
            <tr>
              <th>{{ t('templates.name') }}</th>
              <th>id</th>
              <th>brain</th>
              <th>{{ t('templates.role') }}</th>
              <th>tools</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in templates" :key="row.id">
              <td data-test="tpl-row">
                <template v-if="renameId === row.id">
                  <input v-model="renameName" class="rename" @keyup.enter="saveRename(row.id)" />
                  <span class="row">
                    <button class="mini ok" :disabled="!renameName.trim() || busyId === row.id" @click="saveRename(row.id)">
                      {{ t('templates.save') }}
                    </button>
                    <button class="mini" @click="renameId = ''">{{ t('templates.cancel') }}</button>
                  </span>
                </template>
                <template v-else>{{ row.name }}</template>
              </td>
              <td><code>{{ row.id }}</code></td>
              <td><code>{{ row.brain?.brain_id }}</code></td>
              <td>{{ row.role || '—' }}</td>
              <td class="muted">{{ row.tools?.join(', ') || '—' }}</td>
              <td class="row">
                <button class="mini" :disabled="busyId === row.id || renameId === row.id" @click="startRename(row)">
                  {{ t('templates.rename') }}
                </button>
                <button class="mini bad" :disabled="busyId === row.id" data-test="tpl-delete" @click="remove(row)">
                  {{ t('templates.delete') }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
        <p v-else class="muted">{{ t('templates.none') }}</p>
      </div>
    </div>

    <p v-if="actionError" class="errline">{{ actionError }}</p>
    <p v-if="okMsg" class="okline">{{ okMsg }}</p>
  </div>
</template>

<style scoped>
.sec { margin: 1.4rem 0 0.8rem; font-size: 1.05rem; }
.tbl { width: 100%; border-collapse: collapse; font-size: 0.88rem; }
.tbl th, .tbl td { text-align: left; padding: 0.45rem 0.6rem; border-bottom: 1px solid var(--border); vertical-align: top; }
.tbl th { color: var(--muted); font-weight: 600; font-size: 0.8rem; }
.rename { width: auto; min-width: 10rem; }
.row { display: flex; gap: 0.4rem; flex-wrap: wrap; }
td.row { justify-content: flex-end; }
.mini {
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.4rem;
  padding: 0.3rem 0.8rem; cursor: pointer; font-size: 0.85rem;
}
.mini:disabled { opacity: 0.5; cursor: wait; }
.mini.bad { color: var(--danger); }
.mini.ok { color: var(--ok); }
.errline { color: var(--danger); font-size: 0.9rem; }
.okline { color: var(--ok); font-size: 0.9rem; }
</style>
