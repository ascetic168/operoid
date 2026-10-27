<script setup lang="ts">
import { ApiError, api, hasRole, OfflineError, session } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { computed, onMounted, ref } from 'vue'

// ── API 資料形（對應 oserver GET /api/skills；SkillEntry 為 ocore::skills 的序列化）──
interface SkillEntry {
  zone: 'draft' | 'personal' | 'library'
  dir_name: string
  name: string
  description: string
  mount_path: string
}
interface PersonalArea {
  /** null＝個人模式（單一共用個人區，如 ~/.agents/skills）。 */
  principal: string | null
  skills: SkillEntry[]
}
interface DraftArea {
  employee_id: string
  employee_name: string
  skills: SkillEntry[]
}
interface SkillsPayload {
  mode: 'personal' | 'enterprise'
  library: SkillEntry[]
  personal_areas: PersonalArea[]
  drafts: DraftArea[]
}

const { t } = useI18n()
const library = ref<SkillEntry[]>([])
const personalAreas = ref<PersonalArea[]>([])
const drafts = ref<DraftArea[]>([])
const mode = ref<'personal' | 'enterprise'>('enterprise')
const offline = ref(false)
const error = ref('')

async function load(): Promise<void> {
  try {
    const data = await api.get<SkillsPayload>('/api/skills')
    library.value = data.library
    personalAreas.value = data.personal_areas
    drafts.value = data.drafts
    mode.value = data.mode
    error.value = ''
    offline.value = false
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

// 共用庫以外掛分組（dir_name 形如 <plugin>/<skill>；無 `/` 者歸根層技能）。
const libraryGroups = computed(() => {
  const groups = new Map<string, SkillEntry[]>()
  for (const s of library.value) {
    const plugin = s.dir_name.includes('/') ? s.dir_name.split('/')[0] : '·'
    if (!groups.has(plugin)) groups.set(plugin, [])
    groups.get(plugin)!.push(s)
  }
  return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b))
})

// 內容檢視：同時只展開一列（key＝scope 前綴＋mount）。
const openKey = ref('')
const contentText = ref('')
const contentBusy = ref(false)

function keyOf(mount: string, scope: string): string {
  return `${scope}:${mount}`
}

function isManager(): boolean {
  return hasRole(session()?.principal.roles ?? [], 'manager')
}

/**
 * 內容檢視：path 採 read_file 同款掛載語意（library/…／personal/…／skills/…）。
 * personal/ 他人區帶 principal——僅 Manager+ 生效（server 裁定，一般使用者帶了會 403）。
 */
async function toggle(
  mount: string,
  scope: string,
  opts: { employee?: string; principal?: string | null } = {},
): Promise<void> {
  const key = keyOf(mount, scope)
  if (openKey.value === key) {
    openKey.value = ''
    contentText.value = ''
    return
  }
  openKey.value = key
  contentText.value = ''
  contentBusy.value = true
  try {
    const qs = new URLSearchParams({ path: mount })
    if (opts.employee) qs.set('employee', opts.employee)
    if (opts.principal && isManager()) qs.set('principal', opts.principal)
    const data = await api.get<{ path: string; text: string }>(`/api/skills/content?${qs.toString()}`)
    contentText.value = data.text
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    contentText.value = t('skills.contentError')
  } finally {
    contentBusy.value = false
  }
}

onMounted(load)
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error" />

    <h2 class="sec">{{ t('skills.drafts') }}</h2>
    <p class="muted">{{ t('skills.draftsHint') }}</p>
    <template v-if="drafts.length">
      <section v-for="d in drafts" :key="d.employee_id" class="area">
        <h3>🛠 {{ d.employee_name }}</h3>
        <div v-for="s in d.skills" :key="s.mount_path" class="item">
          <div class="row" :data-test="'skill-' + s.mount_path">
            <span class="name">{{ s.name }}</span>
            <span class="desc" :title="s.description">{{ s.description }}</span>
            <code class="mount">{{ s.mount_path }}</code>
            <button
              class="mini"
              @click="toggle(s.mount_path, d.employee_id, { employee: d.employee_id })"
            >
              {{ openKey === keyOf(s.mount_path, d.employee_id) ? t('skills.hideContent') : t('skills.viewContent') }}
            </button>
          </div>
          <pre v-if="openKey === keyOf(s.mount_path, d.employee_id)" class="content">{{ contentBusy && !contentText ? '…' : contentText }}</pre>
        </div>
      </section>
    </template>
    <p v-else class="muted">{{ t('skills.empty') }}</p>

    <h2 class="sec">{{ t('skills.personal') }}</h2>
    <p class="muted">
      {{ mode === 'personal' ? t('skills.personalHintSingle') : t('skills.personalHint') }}
    </p>
    <template v-if="personalAreas.length">
      <section v-for="area in personalAreas" :key="area.principal ?? '__personal__'" class="area">
        <h3 v-if="mode === 'enterprise'">👤 {{ area.principal }}</h3>
        <div v-for="s in area.skills" :key="s.mount_path" class="item">
          <div class="row" :data-test="'skill-' + s.mount_path">
            <span class="name">{{ s.name }}</span>
            <span class="desc" :title="s.description">{{ s.description }}</span>
            <code class="mount">{{ s.mount_path }}</code>
            <button
              class="mini"
              @click="toggle(s.mount_path, area.principal ?? 'personal', { principal: area.principal })"
            >
              {{ openKey === keyOf(s.mount_path, area.principal ?? 'personal') ? t('skills.hideContent') : t('skills.viewContent') }}
            </button>
          </div>
          <pre v-if="openKey === keyOf(s.mount_path, area.principal ?? 'personal')" class="content">{{ contentBusy && !contentText ? '…' : contentText }}</pre>
        </div>
      </section>
    </template>
    <p v-else class="muted">{{ t('skills.empty') }}</p>

    <h2 class="sec">{{ t('skills.library') }}</h2>
    <p class="muted">{{ t('skills.libraryHint') }}</p>
    <template v-if="libraryGroups.length">
      <section v-for="[plugin, skills] in libraryGroups" :key="plugin" class="area">
        <h3>
          📦 {{ plugin }}
          <span class="count">{{ t('skills.skillCount', { n: skills.length }) }}</span>
        </h3>
        <div v-for="s in skills" :key="s.mount_path" class="item">
          <div class="row" :data-test="'skill-' + s.mount_path">
            <span class="name">{{ s.name }}</span>
            <span class="desc" :title="s.description">{{ s.description }}</span>
            <code class="mount">{{ s.mount_path }}</code>
            <button class="mini" @click="toggle(s.mount_path, 'library')">
              {{ openKey === keyOf(s.mount_path, 'library') ? t('skills.hideContent') : t('skills.viewContent') }}
            </button>
          </div>
          <pre v-if="openKey === keyOf(s.mount_path, 'library')" class="content">{{ contentBusy && !contentText ? '…' : contentText }}</pre>
        </div>
      </section>
    </template>
    <p v-else class="muted">{{ t('skills.empty') }}</p>
  </div>
</template>

<style scoped>
.sec { margin: 1.2rem 0 0.8rem; font-size: 1.05rem; }
.area { margin-bottom: 1rem; }
.area h3 { margin: 0.4rem 0; font-size: 0.95rem; }
.count { color: var(--muted); font-weight: 400; font-size: 0.85rem; margin-left: 0.4rem; }
.muted { color: var(--muted); font-size: 0.88rem; }
.item { border: 1px solid var(--border); background: var(--surface); border-radius: 0.6rem; margin-bottom: 0.5rem; }
.row { display: flex; align-items: center; gap: 0.8rem; padding: 0.55rem 0.9rem; }
.name { font-weight: 600; font-size: 0.95rem; white-space: nowrap; }
.desc { flex: 1; color: var(--muted); font-size: 0.85rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mount { color: var(--muted); font-size: 0.78rem; white-space: nowrap; }
.mini {
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.4rem;
  padding: 0.3rem 0.8rem; cursor: pointer; font-size: 0.85rem; white-space: nowrap;
}
.mini:hover { border-color: var(--accent); color: var(--accent); }
.content {
  margin: 0; padding: 0.8rem 1rem; border-top: 1px solid var(--border);
  max-height: 26rem; overflow: auto; font-size: 0.82rem; line-height: 1.45;
  white-space: pre-wrap; word-break: break-word; background: var(--bg, transparent);
}
</style>
