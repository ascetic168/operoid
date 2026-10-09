<script setup lang="ts">
import { AppHeader } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api, session } from '@front/api-client'
import { sessionActive, sessionPresent } from './sessionActive'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const s = sessionPresent
const isLogin = computed(() => route.name === 'login')

async function doLogout(): Promise<void> {
  await api.logout()
  router.push({ name: 'login' })
}

// 401（token 過期/撤銷）→ api-client 清 session → 路由守衛導回登入（此處僅補一刀）。
watch(
  () => sessionActive(),
  (v) => {
    if (!v && route.name !== 'login') router.push({ name: 'login' })
  },
  { immediate: true },
)
</script>

<template>
  <router-view v-if="isLogin" />
  <template v-else>
    <AppHeader
      :title="t('app.title.manager')"
      :labels="{ logout: t('header.logout'), loggedAs: t('header.loggedAs') }"
      :display-name="s?.principal.display_name ?? ''"
      :roles="s?.principal.roles ?? []"
      @logout="doLogout"
    />
    <nav class="nav">
      <router-link to="/">{{ t('nav.dashboard') }}</router-link>
      <router-link to="/inbox">{{ t('nav.inbox') }}</router-link>
      <router-link to="/grants">{{ t('nav.grants') }}</router-link>
      <router-link to="/operations">{{ t('nav.operations') }}</router-link>
      <router-link to="/events">{{ t('nav.events') }}</router-link>
    </nav>
    <main class="main">
      <router-view />
    </main>
  </template>
</template>

<style scoped>
.nav { display: flex; gap: 1.2rem; padding: 0.6rem 1.25rem; background: var(--surface); border-bottom: 1px solid var(--border); }
.nav a { color: var(--muted); text-decoration: none; font-size: 0.92rem; }
.nav a.router-link-active { color: var(--accent); font-weight: 600; }
.main { padding: 1.5rem; max-width: 72rem; margin: 0 auto; }
.hintwarn { margin-top: 0.8rem; padding: 0.6rem 0.8rem; border: 1px solid color-mix(in srgb, var(--danger) 35%, var(--border)); border-radius: 0.5rem; background: color-mix(in srgb, var(--danger) 7%, var(--surface)); color: var(--danger); font-size: 0.85rem; }
</style>
