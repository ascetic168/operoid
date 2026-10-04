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
  must_change_password: boolean
}

const accounts = ref<PrincipalRow[]>([])
const offline = ref(false)
const error = ref('')
const actionError = ref('')
const okMsg = ref('')
const busyId = ref('')

// 建號表單
const loginName = ref('')
const displayName = ref('')
const newRoles = ref<string[]>(['user'])
const created = ref<{ id: string; password: string } | null>(null)
const creating = ref(false)

// 角色編輯／重設密碼的展開列
const editId = ref('')
const editRoles = ref<string[]>([])
const resetTarget = ref('')
const resetPw = ref('')

const humans = computed(() => accounts.value.filter((a) => a.principal_type === 'human'))

async function load(): Promise<void> {
  try {
    const o = await api.get<{ principals: PrincipalRow[] }>('/api/knowledge/overview')
    accounts.value = o.principals
    error.value = ''
    offline.value = false
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

async function create(): Promise<void> {
  if (!loginName.value.trim()) return
  creating.value = true
  actionError.value = ''
  okMsg.value = ''
  created.value = null
  try {
    const r = await api.post<{ id: string; temporary_password: string }>('/api/accounts', {
      login_name: loginName.value.trim(),
      display_name: displayName.value.trim() || loginName.value.trim(),
      roles: newRoles.value,
    })
    created.value = { id: r.id, password: r.temporary_password }
    loginName.value = ''
    displayName.value = ''
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    creating.value = false
  }
}

async function setDisabled(id: string, disabled: boolean): Promise<void> {
  busyId.value = id
  actionError.value = ''
  try {
    await api.post('/api/accounts/' + encodeURIComponent(id) + (disabled ? '/disable' : '/enable'))
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    busyId.value = ''
  }
}

async function resetPassword(id: string): Promise<void> {
  if (!resetPw.value || resetPw.value.length < 8) return
  busyId.value = id
  actionError.value = ''
  try {
    await api.post('/api/accounts/' + encodeURIComponent(id) + '/password', {
      new_password: resetPw.value,
    })
    okMsg.value = t('accounts.resetDone').replace('{id}', id)
    resetTarget.value = ''
    resetPw.value = ''
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    busyId.value = ''
  }
}

async function saveRoles(id: string): Promise<void> {
  busyId.value = id
  actionError.value = ''
  try {
    await api.post('/api/accounts/' + encodeURIComponent(id) + '/roles', { roles: editRoles.value })
    editId.value = ''
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    busyId.value = ''
  }
}

onMounted(load)
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error || actionError" />
    <p v-if="okMsg" class="okline">{{ okMsg }}</p>

    <div class="grid">
      <div class="card">
        <h2>{{ t('accounts.createHeading') }}</h2>
        <label for="ln">{{ t('accounts.loginName') }}</label>
        <input id="ln" v-model="loginName" data-test="login-name" />
        <label for="dn">{{ t('accounts.displayName') }}</label>
        <input id="dn" v-model="displayName" data-test="display-name" />
        <fieldset class="roles">
          <legend>{{ t('accounts.roles') }}</legend>
          <label class="chk"><input v-model="newRoles" type="checkbox" value="admin" /> admin</label>
          <label class="chk"><input v-model="newRoles" type="checkbox" value="manager" /> manager</label>
          <label class="chk"><input v-model="newRoles" type="checkbox" value="user" /> user</label>
        </fieldset>
        <p style="margin-top: 0.9rem">
          <button class="primary" :disabled="creating || !loginName.trim()" data-test="create" @click="create">
            {{ t('accounts.create') }}
          </button>
        </p>
        <div v-if="created" class="created" data-test="created-password">
          ⚠️ {{ t('accounts.createdNote') }}
          <p><code>{{ created.id }}</code> → <code>{{ created.password }}</code></p>
        </div>
      </div>
    </div>

    <h2 class="sec">{{ t('accounts.list') }}</h2>
    <div v-for="a in humans" :key="a.id" class="card acc" data-test="account">
      <div class="acchead">
        <b>{{ a.display_name }}</b>
        <code class="id">{{ a.id }}</code>
        <span v-if="a.disabled" class="badge dis">{{ t('accounts.disabled') }}</span>
        <span v-if="a.must_change_password" class="badge mcp">{{ t('accounts.mustChange') }}</span>
        <span class="muted roles">[{{ a.attrs.roles.join(', ') }}]</span>
      </div>
      <div class="row">
        <button class="mini" @click="editId = editId === a.id ? '' : a.id; editRoles = a.attrs.roles">
          {{ t('accounts.editRoles') }}
        </button>
        <button class="mini" @click="resetTarget = resetTarget === a.id ? '' : a.id">
          {{ t('accounts.resetPw') }}
        </button>
        <button v-if="!a.disabled" class="mini bad" :disabled="busyId === a.id" @click="setDisabled(a.id, true)">
          {{ t('accounts.disable') }}
        </button>
        <button v-else class="mini ok" :disabled="busyId === a.id" @click="setDisabled(a.id, false)">
          {{ t('accounts.enable') }}
        </button>
      </div>
      <div v-if="editId === a.id" class="sub">
        <label class="chk"><input v-model="editRoles" type="checkbox" value="admin" /> admin</label>
        <label class="chk"><input v-model="editRoles" type="checkbox" value="manager" /> manager</label>
        <label class="chk"><input v-model="editRoles" type="checkbox" value="user" /> user</label>
        <button class="mini ok" :disabled="busyId === a.id" @click="saveRoles(a.id)">
          {{ t('accounts.save') }}
        </button>
      </div>
      <div v-if="resetTarget === a.id" class="sub">
        <input v-model="resetPw" type="password" :placeholder="t('accounts.newTempPw')" />
        <button class="mini" :disabled="resetPw.length < 8 || busyId === a.id" @click="resetPassword(a.id)">
          {{ t('accounts.resetSubmit') }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sec { margin: 1.4rem 0 0.8rem; font-size: 1.05rem; }
.roles { border: 1px solid var(--border); border-radius: 0.5rem; margin-top: 0.7rem; }
.chk { display: inline-flex; align-items: center; gap: 0.3rem; margin: 0.3rem 0.8rem 0.3rem 0; }
.chk input { width: auto; }
.created {
  margin-top: 0.8rem; padding: 0.6rem; background: var(--bg);
  border-radius: 0.5rem; font-size: 0.85rem;
}
.acc { margin-bottom: 0.8rem; }
.acchead { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
.acchead .id { font-size: 0.8rem; color: var(--muted); }
.badge { font-size: 0.75rem; padding: 0.1rem 0.5rem; border-radius: 999px; }
.badge.dis { background: var(--danger); color: #fff; }
.badge.mcp { background: #e8a33d; color: #fff; }
.roles { font-size: 0.82rem; }
.row { display: flex; gap: 0.5rem; margin-top: 0.6rem; flex-wrap: wrap; }
.mini {
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.4rem;
  padding: 0.3rem 0.8rem; cursor: pointer; font-size: 0.85rem;
}
.mini:disabled { opacity: 0.5; cursor: wait; }
.mini.bad { color: var(--danger); }
.mini.ok { color: var(--ok); }
.sub {
  margin-top: 0.6rem; padding: 0.6rem; background: var(--bg); border-radius: 0.5rem;
  display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap;
}
.okline { color: var(--ok); font-size: 0.9rem; }
</style>
