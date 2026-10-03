<script setup lang="ts">
import { ApiError, OfflineError, api, hasRole, session } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { serverUrl, setServerUrl } from '../session'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const role = "admin" as 'admin' | 'manager' | 'user'

const server = ref(serverUrl())
const account = ref('')
const password = ref('')
const newPassword = ref('')
const busy = ref(false)
const errorCode = ref('')
const mustChange = ref(false)

function describe(e: unknown): string {
  if (e instanceof OfflineError) return t('login.offline')
  if (e instanceof ApiError) {
    if (e.code === 'auth.accountLocked') return t('login.locked')
    if (e.code === 'auth.accountDisabled') return t('login.forbidden')
    if (e.code === 'auth.weakPassword') return t('login.newPassword')
    return t('login.invalid')
  }
  return t('login.offline')
}

function done(): void {
  const s = session()
  const redirect = route.query.redirect
  if (s && !hasRole(s.principal.roles, role)) {
    router.push({ name: 'forbidden' })
    return
  }
  router.push(typeof redirect === 'string' ? redirect : '/')
}

async function submit(): Promise<void> {
  busy.value = true
  errorCode.value = ''
  setServerUrl(server.value.trim())
  try {
    const r = await api.login(account.value.trim(), password.value)
    if (r.must_change_password) {
      mustChange.value = true
      return
    }
    done()
  } catch (e) {
    errorCode.value = describe(e)
  } finally {
    busy.value = false
  }
}

async function submitChange(): Promise<void> {
  busy.value = true
  errorCode.value = ''
  try {
    await api.changePassword(password.value, newPassword.value)
    done()
  } catch (e) {
    errorCode.value = describe(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <main class="wrap">
    <form class="card box" @submit.prevent="mustChange ? submitChange() : submit()">
      <h1>{{ t('login.heading') }}</h1>
      <template v-if="!mustChange">
        <label for="server">{{ t('login.server') }}</label>
        <input id="server" v-model="server" :placeholder="'https://'" autocomplete="url" />
        <label for="account">{{ t('login.account') }}</label>
        <input id="account" v-model="account" autocomplete="username" required />
        <label for="password">{{ t('login.password') }}</label>
        <input id="password" v-model="password" type="password" autocomplete="current-password" required />
        <p style="margin-top: 1rem">
          <button class="primary" :disabled="busy" type="submit">
            {{ busy ? t('login.submitting') : t('login.submit') }}
          </button>
        </p>
      </template>
      <template v-else>
        <p class="muted">{{ t('login.mustChange') }}</p>
        <label for="new">{{ t('login.newPassword') }}</label>
        <input id="new" v-model="newPassword" type="password" autocomplete="new-password" required minlength="12" />
        <p style="margin-top: 1rem">
          <button class="primary" :disabled="busy" type="submit">
            {{ busy ? t('login.submitting') : t('login.changeSubmit') }}
          </button>
        </p>
      </template>
      <ErrorBox :message="errorCode" />
    </form>
  </main>
</template>

<style scoped>
.wrap { min-height: 100vh; display: flex; align-items: center; justify-content: center; }
.box { width: 22rem; }
h1 { margin: 0 0 0.5rem; font-size: 1.2rem; }
</style>
