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
  disabled: boolean
}
interface ScopeRow {
  id: string
  visibility: string
  classification: string
  source_ids: string[]
}
interface GrantRow {
  id: string
  principal_id: string
  scope_id: string
  purpose: string | null
  expires_at: string
  state: string
}

const principals = ref<PrincipalRow[]>([])
const scopes = ref<ScopeRow[]>([])
const grants = ref<GrantRow[]>([])
const offline = ref(false)
const error = ref('')
const actionError = ref('')
const busyId = ref('')
const okMsg = ref('')

// 發放表單
const gPrincipal = ref('')
const gScope = ref('')
const gHours = ref(24)
const gPurpose = ref('')
const granting = ref(false)

const humans = computed(() => principals.value.filter((p) => p.principal_type === 'human'))
const activeGrants = computed(() => grants.value.filter((g) => g.state === 'active'))

async function load(): Promise<void> {
  try {
    const o = await api.get<{
      principals: PrincipalRow[]
      grants: GrantRow[]
      scopes: ScopeRow[]
    }>('/api/knowledge/overview')
    principals.value = o.principals
    scopes.value = o.scopes
    grants.value = o.grants
    if (!gPrincipal.value && humans.value.length) gPrincipal.value = humans.value[0].id
    if (!gScope.value && scopes.value.length) gScope.value = scopes.value[0].id
    error.value = ''
    offline.value = false
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

async function grant(): Promise<void> {
  if (!gPrincipal.value || !gScope.value) return
  granting.value = true
  actionError.value = ''
  okMsg.value = ''
  try {
    await api.post('/api/knowledge/grants', {
      principal_id: gPrincipal.value,
      scope_id: gScope.value,
      ttl_secs: Math.max(1, Math.round(gHours.value * 3600)),
      purpose: gPurpose.value.trim() || null,
    })
    okMsg.value = t('grants.granted')
    gPurpose.value = ''
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.offline'
  } finally {
    granting.value = false
  }
}

async function revoke(id: string): Promise<void> {
  busyId.value = id
  actionError.value = ''
  try {
    await api.post('/api/knowledge/grants/' + encodeURIComponent(id) + '/revoke')
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.offline'
  } finally {
    busyId.value = ''
  }
}

function principalLabel(id: string): string {
  const p = principals.value.find((x) => x.id === id)
  return p ? p.display_name + '（' + id + '）' : id
}

onMounted(load)
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error || actionError" />
    <p v-if="okMsg" class="okline" data-test="granted">{{ okMsg }}</p>

    <div class="grid">
      <div class="card">
        <h2>{{ t('grants.grantHeading') }}</h2>
        <label for="gp">{{ t('grants.principal') }}</label>
        <select id="gp" v-model="gPrincipal">
          <option v-for="p in humans" :key="p.id" :value="p.id">
            {{ p.display_name }}（{{ p.login_name ?? p.id }}）
          </option>
        </select>
        <label for="gs">{{ t('grants.scope') }}</label>
        <select id="gs" v-model="gScope">
          <option v-for="s in scopes" :key="s.id" :value="s.id">
            {{ s.id }}〔{{ s.visibility }}／{{ s.classification }}〕
          </option>
        </select>
        <label for="gh">{{ t('grants.ttl') }}</label>
        <input id="gh" v-model.number="gHours" type="number" min="1" />
        <label for="gpu">{{ t('grants.purpose') }}</label>
        <input id="gpu" v-model="gPurpose" />
        <p style="margin-top: 0.9rem">
          <button class="primary" :disabled="granting" data-test="grant" @click="grant">
            {{ t('grants.submit') }}
          </button>
        </p>
      </div>

      <div class="card">
        <h2>{{ t('grants.list') }}</h2>
        <table v-if="activeGrants.length" class="tbl">
          <tbody>
            <tr v-for="g in activeGrants" :key="g.id">
              <td>{{ principalLabel(g.principal_id) }}</td>
              <td><code>{{ g.scope_id }}</code></td>
              <td class="muted">{{ g.expires_at.slice(0, 16).replace('T', ' ') }}</td>
              <td>
                <button
                  class="mini bad"
                  :disabled="busyId === g.id"
                  :data-test="'revoke-' + g.id"
                  @click="revoke(g.id)"
                >
                  {{ t('grants.revoke') }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
        <p v-else class="muted">{{ t('grants.none') }}</p>
      </div>
    </div>

    <h2 class="sec">{{ t('grants.scopes') }}</h2>
    <div class="grid">
      <div v-for="s in scopes" :key="s.id" class="card">
        <h2><code>{{ s.id }}</code></h2>
        <p class="muted">{{ s.visibility }}／{{ s.classification }}</p>
        <p class="muted">{{ s.source_ids.join('、') }}</p>
      </div>
      <p v-if="!scopes.length" class="muted">{{ t('grants.none') }}</p>
    </div>
  </div>
</template>

<style scoped>
.sec { margin: 1.4rem 0 0.8rem; font-size: 1.05rem; }
.okline { color: var(--ok); font-size: 0.9rem; }
.tbl { width: 100%; border-collapse: collapse; font-size: 0.88rem; }
.tbl td { padding: 0.35rem 0.5rem; border-bottom: 1px solid var(--border); }
.mini {
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.4rem;
  padding: 0.25rem 0.7rem; cursor: pointer; font-size: 0.82rem;
}
.mini.bad { color: var(--danger); }
.mini.bad:hover { border-color: var(--danger); }
.mini:disabled { opacity: 0.5; cursor: wait; }
</style>
