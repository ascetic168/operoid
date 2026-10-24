<script setup lang="ts">
import { ApiError, api, OfflineError, openEventStream, type StreamEvent } from '@front/api-client'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

const employees = ref<number | null>(null)
const templates = ref<number | null>(null)
const proposals = ref<number | null>(null)
const flagged = ref<number | null>(null)
const installed = ref<boolean | null>(null)
const streamUp = ref(false)
const latest = ref<StreamEvent | null>(null)
const offline = ref(false)
const loadError = ref('')
const scope = "帳號管理／知識授權／腦與前置檢查（obridge 與工廠寫 UI 過渡期續用統一 GUI）"

// ── 更新檢查（企業版 v1：僅檢查＋提示，升級程序見 DEPLOYMENT.md）──
// oserver 端快取 24h；operoid.toml [update] check_enabled=false 時 check_enabled=false。
interface UpdateCheckResp {
  check_enabled: boolean
  current_version?: string
  latest_version?: string | null
  update_available?: boolean
  release_url?: string | null
  checked_at_secs?: number
  cached?: boolean
}
const update = ref<UpdateCheckResp | null>(null)
const updateChecking = ref(false)
const updateError = ref<string | null>(null)
const currentVersion = ref('')
// 已讀版本存 localStorage：同版不再提示，出現更新版號時橫幅重現。
const UPDATE_DISMISS_KEY = 'operoid.admin.updateDismissedVersion'
const updateNotice = computed(() => {
  const u = update.value
  if (!u?.check_enabled || !u.update_available || !u.latest_version) return null
  if (u.latest_version === localStorage.getItem(UPDATE_DISMISS_KEY)) return null
  return u
})

async function checkUpdate(): Promise<void> {
  updateChecking.value = true
  updateError.value = null
  try {
    update.value = await api.get<UpdateCheckResp>('/api/admin/update/check')
    if (update.value?.current_version) currentVersion.value = update.value.current_version
  } catch (e) {
    updateError.value = e instanceof ApiError ? e.code : 'server.offline'
  } finally {
    updateChecking.value = false
  }
}

function dismissUpdate(): void {
  if (update.value?.latest_version) localStorage.setItem(UPDATE_DISMISS_KEY, update.value.latest_version)
  // computed 依賴 localStorage（非響應式）——改寫 ref 觸發重算。
  update.value = update.value ? { ...update.value } : null
}

let close: (() => void) | null = null

onMounted(async () => {
  try {
    const emps = await api.get<Array<unknown>>('/api/employees')
    employees.value = emps.length
    const tmpls = await api.get<Array<unknown>>('/api/templates')
    templates.value = tmpls.length
  } catch (e) {
    offline.value = e instanceof OfflineError
    loadError.value = e instanceof ApiError ? e.code : offline.value ? 'server.offline' : 'server.internal'
  }
  try {
    const inbox = await api.get<{ proposals: unknown[]; flagged_employees: unknown[] }>('/api/inbox')
    proposals.value = inbox.proposals.length
    flagged.value = inbox.flagged_employees.length
  } catch {
    /* 非 user/manager 語意差——留空 */
  }
  try {
    const svc = await api.get<{ installed: boolean }>('/api/service/status')
    installed.value = svc.installed
  } catch {
    installed.value = null
  }
  // 目前版本預填（/healthz 免認證，僅回 build 資訊）；詳細檢查由按鈕觸發。
  try {
    const h = await api.get<{ version: string }>('/healthz')
    currentVersion.value = h.version
  } catch {
    currentVersion.value = ''
  }
  // SSE：短票 → 事件流（斷線自動以新票重連）。
  close = openEventStream(
    (ev) => {
      streamUp.value = true
      latest.value = ev
    },
    () => {
      streamUp.value = false
    },
  )
})

onBeforeUnmount(() => {
  close?.()
})
</script>

<template>
  <div>
    <p class="muted">{{ t('home.welcome') }}<span v-if="offline">——{{ t('login.offline') }}</span><span v-else-if="loadError">（{{ loadError }}）</span></p>
    <!-- 有新版橫幅（可關閉；同版不再提示） -->
    <div v-if="updateNotice" class="notice">
      <span>
        {{ t('home.updateAvailable', { version: updateNotice.latest_version }) }}
        <span v-if="updateNotice.cached" class="muted">（{{ t('home.updateCached') }}）</span>
      </span>
      <a v-if="updateNotice.release_url" :href="updateNotice.release_url" target="_blank" rel="noopener">
        {{ t('home.updateOpen') }}
      </a>
      <button class="linkbtn" @click="dismissUpdate">{{ t('home.updateDismiss') }}</button>
    </div>
    <div class="grid">
      <div v-if="employees !== null" class="card">
        <h2>{{ t('home.employees') }}</h2>
        <span class="stat">{{ employees }}</span>
      </div>
      <div v-if="templates !== null" class="card">
        <h2>{{ t('home.templates') }}</h2>
        <span class="stat">{{ templates }}</span>
      </div>
      <div v-if="proposals !== null" class="card">
        <h2>{{ t('home.proposals') }}</h2>
        <span class="stat">{{ proposals }}</span>
      </div>
      <div v-if="flagged !== null" class="card">
        <h2>{{ t('home.flagged') }}</h2>
        <span class="stat">{{ flagged }}</span>
      </div>
      <div v-if="installed !== null" class="card">
        <h2>{{ t('home.service') }}</h2>
        <span class="stat">{{ installed ? '✓' : '—' }}</span>
        <p class="muted">{{ installed ? t('home.installed') : t('home.notInstalled') }}</p>
      </div>
      <div class="card">
        <h2>{{ t('home.stream') }}</h2>
        <span class="stat">{{ streamUp ? '✓' : '…' }}</span>
        <p class="muted">{{ streamUp ? t('home.streamOn') : t('home.streamDown') }}</p>
        <p v-if="latest" class="muted">
          {{ t('home.latest') }}：<code>{{ latest.kind }}</code> {{ latest.detail }}
        </p>
      </div>
      <div class="card">
        <h2>{{ t('home.scope') }}</h2>
        <p class="muted">帳號管理／知識授權／腦與前置檢查（obridge 與工廠寫 UI 過渡期續用統一 GUI）</p>
      </div>
      <div class="card">
        <h2>{{ t('home.updateTitle') }}</h2>
        <span class="stat">{{ currentVersion || '—' }}</span>
        <p class="muted">{{ t('home.updateDesc') }}</p>
        <p v-if="update?.latest_version" class="muted">
          {{ t('home.updateLatest', { version: update.latest_version }) }}
          <a v-if="update.release_url" :href="update.release_url" target="_blank" rel="noopener">{{ t('home.updateOpen') }}</a>
        </p>
        <p v-if="updateError" class="muted">{{ t('home.updateFailed', { code: updateError }) }}</p>
        <button class="checkbtn" :disabled="updateChecking" @click="checkUpdate">
          {{ updateChecking ? t('home.updateChecking') : t('home.updateCheck') }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.notice {
  display: flex;
  align-items: center;
  gap: 1rem;
  flex-wrap: wrap;
  margin: 0 0 1rem;
  padding: 0.6rem 1rem;
  border: 1px solid var(--accent);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--accent) 8%, var(--surface));
  font-size: 0.9rem;
}
.notice a { color: var(--accent); font-weight: 600; text-decoration: none; }
.notice a:hover { text-decoration: underline; }
.linkbtn {
  border: none; background: none; color: var(--muted);
  cursor: pointer; font-size: 0.85rem;
}
.linkbtn:hover { color: var(--text); }
.checkbtn {
  margin-top: 0.5rem; padding: 0.35rem 0.8rem; width: auto;
  border: 1px solid var(--border); border-radius: 0.5rem;
  background: var(--surface); color: var(--text);
  font-size: 0.85rem; cursor: pointer;
}
.checkbtn:hover { border-color: var(--accent); }
.checkbtn:disabled { opacity: 0.55; cursor: wait; }
</style>
