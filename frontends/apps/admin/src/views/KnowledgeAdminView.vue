<script setup lang="ts">
import { ApiError, api, OfflineError } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { computed, onMounted, ref } from 'vue'

const { t } = useI18n()

interface PrincipalRow {
  id: string
  display_name: string
  principal_type: string
  attrs: { roles: string[]; departments: string[]; projects: string[]; clearance: string | null }
  login_name: string | null
}
interface ScopeRow {
  id: string
  visibility: string
  classification: string
  source_ids: string[]
  department: string | null
  project: string | null
}
interface TokenRow {
  id: string
  label: string | null
  expires_at: string | null
  last_used_at: string | null
}
interface PolicyRule {
  [key: string]: unknown
}

const principals = ref<PrincipalRow[]>([])
const scopes = ref<ScopeRow[]>([])
const policyRaw = ref('')
const policyVersion = ref<number | null>(null)
const offline = ref(false)
const error = ref('')
const actionError = ref('')
const okMsg = ref('')
const busyId = ref('')

// principal 建立
const pId = ref('')
const pType = ref('human')
const pEmployeeId = ref('')
// token 簽發
const tokTarget = ref('')
const tokTtl = ref<number | null>(null)
const tokLabel = ref('')
const issued = ref<{ principal: string; token: string } | null>(null)
// scope upsert
const scId = ref('')
const scVisibility = ref('company')
const scClassification = ref('internal')
const scSources = ref('')

function loadPolicy(p: unknown, version: number | null): void {
  policyVersion.value = version
  try {
    policyRaw.value = JSON.stringify(p, null, 2)
  } catch {
    policyRaw.value = ''
  }
}

async function load(): Promise<void> {
  try {
    const o = await api.get<{
      principals: PrincipalRow[]
      scopes: ScopeRow[]
      policy: { version?: number; rules?: PolicyRule[] } | null
    }>('/api/knowledge/overview')
    principals.value = o.principals
    scopes.value = o.scopes
    loadPolicy(o.policy?.rules ?? [], o.policy?.version ?? null)
    error.value = ''
    offline.value = false
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

async function createPrincipal(): Promise<void> {
  if (!pId.value.trim()) return
  actionError.value = ''
  okMsg.value = ''
  try {
    await api.post('/api/knowledge/principals', {
      id: pId.value.trim(),
      principal_type: pType.value,
      employee_id: pType.value === 'ai_employee' ? pEmployeeId.value.trim() || null : null,
    })
    okMsg.value = t('ka.principalCreated')
    pId.value = ''
    pEmployeeId.value = ''
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  }
}

async function savePolicy(): Promise<void> {
  actionError.value = ''
  okMsg.value = ''
  try {
    const rules = JSON.parse(policyRaw.value) as PolicyRule[]
    await api.post('/api/knowledge/policy', rules)
    okMsg.value = t('ka.policySaved')
    await load()
  } catch (e) {
    actionError.value =
      e instanceof SyntaxError ? 'ka.policyInvalidJson' : e instanceof ApiError ? e.code : 'server.internal'
  }
}

async function upsertScope(): Promise<void> {
  if (!scId.value.trim() || !scSources.value.trim()) return
  actionError.value = ''
  okMsg.value = ''
  try {
    await api.post('/api/knowledge/scopes', {
      id: scId.value.trim(),
      visibility: scVisibility.value,
      classification: scClassification.value,
      source_ids: scSources.value.split(/[,，\s]+/).filter(Boolean),
    })
    okMsg.value = t('ka.scopeSaved')
    scId.value = ''
    scSources.value = ''
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  }
}

async function issueToken(id: string): Promise<void> {
  busyId.value = id
  actionError.value = ''
  try {
    const body: Record<string, unknown> = {}
    if (tokTtl.value) body.ttl_secs = Math.round(tokTtl.value * 3600)
    if (tokLabel.value.trim()) body.label = tokLabel.value.trim()
    const r = await api.post<{ token: string; token_id: string }>(
      '/api/knowledge/principals/' + encodeURIComponent(id) + '/token',
      body,
    )
    issued.value = { principal: id, token: r.token } // ⚠️ 明文僅此一次
    tokTarget.value = ''
    tokLabel.value = ''
    tokTtl.value = null
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    busyId.value = ''
  }
}

async function revokeAllTokens(id: string): Promise<void> {
  busyId.value = id
  actionError.value = ''
  try {
    await api.del('/api/knowledge/principals/' + encodeURIComponent(id) + '/token')
    issued.value = null
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    busyId.value = ''
  }
}

const humanPrincipals = computed(() => principals.value.filter((p) => p.principal_type === 'human'))

onMounted(load)
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error || actionError" />
    <p v-if="okMsg" class="okline">{{ okMsg }}</p>

    <!-- 帳號主體（ai_employee 服務型 principal）＋ token -->
    <h2 class="sec">{{ t('ka.principals') }}</h2>
    <div class="grid">
      <div class="card">
        <h2>{{ t('ka.createPrincipal') }}</h2>
        <label for="pid">ID</label>
        <input id="pid" v-model="pId" placeholder="principal-... / ai:..." />
        <label for="ptype">{{ t('ka.type') }}</label>
        <select id="ptype" v-model="pType">
          <option value="human">human</option>
          <option value="ai_employee">ai_employee</option>
        </select>
        <label v-if="pType === 'ai_employee'" for="peid">employee_id</label>
        <input v-if="pType === 'ai_employee'" id="peid" v-model="pEmployeeId" />
        <p style="margin-top: 0.9rem">
          <button class="primary" :disabled="!pId.trim()" @click="createPrincipal">
            {{ t('ka.create') }}
          </button>
        </p>
      </div>
      <div class="card tok">
        <h2>{{ t('ka.issueToken') }}</h2>
        <label for="tt">{{ t('ka.targetPrincipal') }}</label>
        <select id="tt" v-model="tokTarget">
          <option v-for="p in humanPrincipals" :key="p.id" :value="p.id">{{ p.id }}</option>
        </select>
        <label for="ttl">{{ t('ka.ttlHours') }}（{{ t('ka.emptyNoExpiry') }}）</label>
        <input id="ttl" v-model.number="tokTtl" type="number" min="1" />
        <label for="tlb">{{ t('ka.label') }}</label>
        <input id="tlb" v-model="tokLabel" />
        <p style="margin-top: 0.9rem">
          <button class="primary" :disabled="!tokTarget" @click="issueToken(tokTarget)">
            {{ t('ka.issue') }}
          </button>
        </p>
        <div v-if="issued" class="issued" data-test="issued-token">
          ⚠️ {{ t('ka.tokenOnce') }}
          <p><code>{{ issued.principal }}</code> → <code>{{ issued.token }}</code></p>
        </div>
      </div>
    </div>

    <div v-for="p in humanPrincipals" :key="p.id" class="card acc">
      <div class="acchead">
        <b>{{ p.display_name }}</b>
        <code class="id">{{ p.id }}</code>
        <span class="muted roles">[{{ p.attrs.roles.join(', ') }}]</span>
        <span class="muted">{{ t('ka.clearance') }}: {{ p.attrs.clearance ?? 'internal' }}</span>
      </div>
      <div class="row">
        <button class="mini" @click="tokTarget = p.id">{{ t('ka.issueToken') }}</button>
        <button class="mini bad" :disabled="busyId === p.id" @click="revokeAllTokens(p.id)">
          {{ t('ka.revokeAll') }}
        </button>
      </div>
    </div>

    <!-- scopes -->
    <h2 class="sec">{{ t('ka.scopes') }}</h2>
    <div class="grid">
      <div class="card">
        <h2>{{ t('ka.upsertScope') }}</h2>
        <label for="sid">scope id</label>
        <input id="sid" v-model="scId" placeholder="co-engineering" />
        <label for="svis">visibility</label>
        <select id="svis" v-model="scVisibility">
          <option value="company">company</option>
          <option value="department">department</option>
          <option value="project">project</option>
          <option value="restricted">restricted</option>
        </select>
        <label for="scls">classification</label>
        <select id="scls" v-model="scClassification">
          <option value="public">public</option>
          <option value="internal">internal</option>
          <option value="confidential">confidential</option>
          <option value="secret">secret</option>
        </select>
        <label for="ssrc">source_ids（逗號分隔）</label>
        <input id="ssrc" v-model="scSources" />
        <p style="margin-top: 0.9rem">
          <button class="primary" :disabled="!scId.trim() || !scSources.trim()" @click="upsertScope">
            {{ t('ka.upsert') }}
          </button>
        </p>
      </div>
      <div class="card">
        <h2>{{ t('ka.scopeList') }}</h2>
        <table v-if="scopes.length" class="tbl">
          <tbody>
            <tr v-for="s in scopes" :key="s.id">
              <td><code>{{ s.id }}</code></td>
              <td>{{ s.visibility }}</td>
              <td>{{ s.classification }}</td>
              <td class="muted">{{ s.source_ids.join(', ') }}</td>
            </tr>
          </tbody>
        </table>
        <p v-else class="muted">—</p>
      </div>
    </div>

    <!-- policy -->
    <h2 class="sec">{{ t('ka.policy') }}<span v-if="policyVersion !== null" class="muted"> v{{ policyVersion }}</span></h2>
    <div class="card">
      <textarea v-model="policyRaw" class="policy" rows="12" spellcheck="false"></textarea>
      <p style="margin-top: 0.7rem">
        <button class="primary savebtn" @click="savePolicy">{{ t('ka.policySave') }}</button>
      </p>
    </div>
  </div>
</template>

<style scoped>
.sec { margin: 1.4rem 0 0.8rem; font-size: 1.05rem; }
.acc { margin-bottom: 0.8rem; }
.acchead { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
.acchead .id { font-size: 0.8rem; color: var(--muted); }
.roles { font-size: 0.82rem; }
.row { display: flex; gap: 0.5rem; margin-top: 0.6rem; }
.mini {
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.4rem;
  padding: 0.3rem 0.8rem; cursor: pointer; font-size: 0.85rem;
}
.mini:disabled { opacity: 0.5; cursor: wait; }
.mini.bad { color: var(--danger); }
.issued {
  margin-top: 0.8rem; padding: 0.6rem; background: var(--bg); border-radius: 0.5rem;
  font-size: 0.85rem;
}
.tbl { width: 100%; border-collapse: collapse; font-size: 0.85rem; }
.tbl td { padding: 0.3rem 0.5rem; border-bottom: 1px solid var(--border); }
.policy {
  width: 100%; font-family: ui-monospace, monospace; font-size: 0.8rem;
  padding: 0.6rem; border: 1px solid var(--border); border-radius: 0.5rem;
  background: var(--bg); color: var(--text);
}
.savebtn { width: auto; }
</style>
