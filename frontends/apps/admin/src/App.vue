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
      :title="t('app.title.admin')"
      :labels="{ logout: t('header.logout'), loggedAs: t('header.loggedAs') }"
      :display-name="s?.principal.display_name ?? ''"
      :roles="s?.principal.roles ?? []"
      @logout="doLogout"
    />
    <nav class="nav">
      <router-link to="/">{{ t('nav.home') }}</router-link>
      <router-link to="/accounts">{{ t('nav.accounts') }}</router-link>
      <router-link to="/knowledge">{{ t('nav.knowledge') }}</router-link>
      <router-link to="/brains">{{ t('nav.brains') }}</router-link>
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
</style>
