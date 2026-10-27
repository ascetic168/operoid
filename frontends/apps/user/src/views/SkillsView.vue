<script setup lang="ts">
import { ApiError, api, hasRole, OfflineError, session } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { computed, onMounted, ref } from 'vue'

// ── API 資料形（對應 oserver GET /api/skills；SkillEntry 為 ocore::skills 的序列化）──
interface SkillEntry {
  zone: 'draft' | 'personal' | 'library' | 'catalog'
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
  catalog: SkillEntry[]
}

const { t } = useI18n()
const library = ref<SkillEntry[]>([])
const personalAreas = ref<PersonalArea[]>([])
const drafts = ref<DraftArea[]>([])
const catalog = ref<SkillEntry[]>([])
const mode = ref<'personal' | 'enterprise'>('enterprise')
const offline = ref(false)
const error = ref('')
const busyKey = ref('')
const notice = ref('')

async function load(): Promise<void> {
  try {
    const data = await api.get<SkillsPayload>('/api/skills')
    library.value = data.library
    personalAreas.value = data.personal_areas
    drafts.value = data.drafts
    catalog.value = data.catalog
    mode.value = data.mode
    error.value = ''
    offline.value = false
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

/** 依 dir_name 前綴分組（`<plugin>/<skill>`；無 `/` 者歸根層）。 */
function groupBy(skills: SkillEntry[]): [string, SkillEntry[]][] {
  const groups = new Map<string, SkillEntry[]>()
  for (const s of skills) {
    const plugin = s.dir_name.includes('/') ? s.dir_name.split('/')[0] : '·'
    if (!groups.has(plugin)) groups.set(plugin, [])
    groups.get(plugin)!.push(s)
  }
  return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b))
}
const catalogGroups = computed(() => groupBy(catalog.value))
const libraryGroups = computed(() => groupBy(library.value))

// ── 摺疊（區層＋外掛分組層）──
// 區預設：全部展開；外掛分組預設**收合**（書架只看外掛清單＋數量，點開才攤技能）。
const expanded = ref<Record<string, boolean>>({
  catalog: false,
  personal: true,
  drafts: true,
  library: true,
})
function toggleSection(key: string): void {
  expanded.value[key] = !expanded.value[key]
}
const expandedGroups = ref<Set<string>>(new Set())
function toggleGroup(key: string): void {
  const next = new Set(expandedGroups.value)
  if (next.has(key)) next.delete(key)
  else next.add(key)
  expandedGroups.value = next
}

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
 * 內容檢視：path 採 read_file 同款掛載語意（library/…／personal/…／skills/…／catalog/…）。
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

// ── 圖書館挑選／個人區移除（人類是唯一有權移動檔案的角色；伺服器代為執行）──

/** 圖書館 → 我的個人區（同名已存在由 server 409 拒絕）。 */
async function adopt(name: string): Promise<void> {
  busyKey.value = `adopt:${name}`
  notice.value = ''
  error.value = ''
  try {
    await api.post('/api/skills/adopt', { name })
    notice.value = t('skills.adopted', { name })
    await load()
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  } finally {
    busyKey.value = ''
  }
}

// 兩段確認：第一次點擊進入待確認態，再點一次才真的移除。
const confirmKey = ref('')

async function remove(name: string): Promise<void> {
  if (confirmKey.value !== name) {
    confirmKey.value = name
    return
  }
  confirmKey.value = ''
  busyKey.value = `remove:${name}`
  notice.value = ''
  error.value = ''
  try {
    await api.post('/api/skills/remove', { name })
    notice.value = t('skills.removed', { name })
    await load()
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  } finally {
    busyKey.value = ''
  }
}

onMounted(load)
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error" />
    <p v-if="notice" class="notice">{{ notice }}</p>
    <button class="sec head" data-test="section-drafts" @click="toggleSection('drafts')">
      <span class="chev">{{ expanded.drafts ? '▾' : '▸' }}</span>
      <h2>{{ t('skills.drafts') }}</h2>
      <span class="count">{{ drafts.reduce((n, d) => n + d.skills.length, 0) }}</span>
    </button>
    <template v-if="expanded.drafts">
      <p class="muted">{{ t('skills.draftsHint') }}</p>
      <template v-if="drafts.length">
        <section v-for="d in drafts" :key="d.employee_id" class="area">
          <h3>🛠 {{ d.employee_name }}</h3>
          <div v-for="s in d.skills" :key="s.mount_path" class="item">
            <div class="row" :data-test="'skill-' + s.mount_path">
              <span class="name">{{ s.name }}</span>
              <span class="desc" :title="s.description">{{ s.description }}</span>
              <code class="mount">{{ s.mount_path }}</code>
              <button class="mini" @click="toggle(s.mount_path, d.employee_id, { employee: d.employee_id })">
                {{ openKey === keyOf(s.mount_path, d.employee_id) ? t('skills.hideContent') : t('skills.viewContent') }}
              </button>
            </div>
            <pre v-if="openKey === keyOf(s.mount_path, d.employee_id)" class="content">{{ contentBusy && !contentText ? '…' : contentText }}</pre>
          </div>
        </section>
      </template>
      <p v-else class="muted">{{ t('skills.empty') }}</p>
    </template>

    <button class="sec head" data-test="section-personal" @click="toggleSection('personal')">
      <span class="chev">{{ expanded.personal ? '▾' : '▸' }}</span>
      <h2>{{ t('skills.personal') }}</h2>
      <span class="count">{{ personalAreas.reduce((n, a) => n + a.skills.length, 0) }}</span>
    </button>
    <template v-if="expanded.personal">
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
              <button
                class="mini danger"
                :disabled="busyKey === 'remove:' + s.dir_name"
                :data-test="'remove-' + s.dir_name"
                @click="remove(s.dir_name)"
              >
                {{ confirmKey === s.dir_name ? t('skills.confirmRemove') : t('skills.remove') }}
              </button>
            </div>
            <pre v-if="openKey === keyOf(s.mount_path, area.principal ?? 'personal')" class="content">{{ contentBusy && !contentText ? '…' : contentText }}</pre>
          </div>
        </section>
      </template>
      <p v-else class="muted">{{ t('skills.empty') }}</p>
    </template>

    <button class="sec head" data-test="section-library" @click="toggleSection('library')">
      <span class="chev">{{ expanded.library ? '▾' : '▸' }}</span>
      <h2>{{ t('skills.library') }}</h2>
      <span class="count">{{ library.length }}</span>
    </button>
    <template v-if="expanded.library">
      <p class="muted">{{ t('skills.libraryHint') }}</p>
      <template v-if="libraryGroups.length">
        <section v-for="[plugin, skills] in libraryGroups" :key="plugin" class="area">
          <button class="grouphead" @click="toggleGroup('library:' + plugin)">
            <span class="chev">{{ expandedGroups.has('library:' + plugin) ? '▾' : '▸' }}</span>
            <h3>📦 {{ plugin }}</h3>
            <span class="count">{{ t('skills.skillCount', { n: skills.length }) }}</span>
          </button>
          <template v-if="expandedGroups.has('library:' + plugin)">
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
          </template>
        </section>
      </template>
      <p v-else class="muted">{{ t('skills.empty') }}</p>
    </template>

    <button class="sec head" :data-test="'section-catalog'" @click="toggleSection('catalog')">
      <span class="chev">{{ expanded.catalog ? '▾' : '▸' }}</span>
      <h2>{{ t('skills.catalog') }}</h2>
      <span class="count">{{ catalog.length }}</span>
    </button>
    <template v-if="expanded.catalog">
      <p class="muted">{{ t('skills.catalogHint') }}</p>
      <template v-if="catalogGroups.length">
        <section v-for="[plugin, skills] in catalogGroups" :key="plugin" class="area">
          <button class="grouphead" :data-test="'group-' + plugin" @click="toggleGroup('catalog:' + plugin)">
            <span class="chev">{{ expandedGroups.has('catalog:' + plugin) ? '▾' : '▸' }}</span>
            <h3>📦 {{ plugin }}</h3>
            <span class="count">{{ t('skills.skillCount', { n: skills.length }) }}</span>
          </button>
          <template v-if="expandedGroups.has('catalog:' + plugin)">
            <div v-for="s in skills" :key="s.mount_path" class="item">
              <div class="row" :data-test="'skill-' + s.mount_path">
                <span class="name">{{ s.name }}</span>
                <span class="desc" :title="s.description">{{ s.description }}</span>
                <code class="mount">{{ s.mount_path }}</code>
                <button class="mini" @click="toggle(s.mount_path, 'catalog')">
                  {{ openKey === keyOf(s.mount_path, 'catalog') ? t('skills.hideContent') : t('skills.viewContent') }}
                </button>
                <button
                  class="mini accent"
                  :disabled="busyKey === 'adopt:' + s.dir_name"
                  :data-test="'adopt-' + s.dir_name"
                  @click="adopt(s.dir_name)"
                >
                  {{ t('skills.adopt') }}
                </button>
              </div>
              <pre v-if="openKey === keyOf(s.mount_path, 'catalog')" class="content">{{ contentBusy && !contentText ? '…' : contentText }}</pre>
            </div>
          </template>
        </section>
      </template>
      <p v-else class="muted">{{ t('skills.empty') }}</p>
    </template>


  </div>
</template>

<style scoped>
.sec { margin: 1.2rem 0 0.8rem; font-size: 1.05rem; }
.head {
  display: flex; align-items: center; gap: 0.5rem; width: 100%;
  background: none; border: none; padding: 0; cursor: pointer; text-align: left;
  color: inherit;
}
.head h2 { font-size: 1.05rem; margin: 0; }
.chev { color: var(--muted); width: 1rem; display: inline-block; }
.count { color: var(--muted); font-weight: 400; font-size: 0.85rem; margin-left: 0.4rem; }
.grouphead {
  display: flex; align-items: center; gap: 0.4rem; width: 100%;
  background: none; border: none; padding: 0.15rem 0.25rem; margin: 0.4rem 0;
  cursor: pointer; text-align: left; color: inherit; border-radius: 0.3rem;
}
.grouphead:hover { background: var(--surface); }
.grouphead h3 { margin: 0; font-size: 0.95rem; }
.area { margin-bottom: 1rem; }
.area h3 { margin: 0.4rem 0; font-size: 0.95rem; }
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
.mini.accent { border-color: var(--accent); color: var(--accent); }
.mini.danger:hover { border-color: var(--danger, #c0392b); color: var(--danger, #c0392b); }
.mini:disabled { opacity: 0.5; cursor: wait; }
.content {
  margin: 0; padding: 0.8rem 1rem; border-top: 1px solid var(--border);
  max-height: 26rem; overflow: auto; font-size: 0.82rem; line-height: 1.45;
  white-space: pre-wrap; word-break: break-word; background: var(--bg, transparent);
}
.notice { color: var(--ok); font-size: 0.9rem; }
</style>
