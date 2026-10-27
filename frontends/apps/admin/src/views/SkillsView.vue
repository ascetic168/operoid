<script setup lang="ts">
import { ApiError, api, OfflineError } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { onMounted, ref } from 'vue'

const { t } = useI18n()

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
interface SkillsPayload {
  mode: 'personal' | 'enterprise'
  library: SkillEntry[]
  personal_areas: PersonalArea[]
}

const library = ref<SkillEntry[]>([])
const personalAreas = ref<PersonalArea[]>([])
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
    mode.value = data.mode
    error.value = ''
    offline.value = false
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

/** 組織級晉升：主體個人區 → 公司共用庫（admin 專屬；伺服器端複製並留收據）。 */
async function promote(principal: string | null, dirName: string): Promise<void> {
  if (!principal) {
    // 個人模式：以 operator 名義晉升（區本身無主體語意）。
    principal = 'operator'
  }
  busyKey.value = dirName
  notice.value = ''
  try {
    await api.post('/api/skills/promote', { principal, skill: dirName })
    notice.value = t('skills.promoted', { name: dirName })
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

    <h2 class="sec">{{ t('skills.library') }}</h2>
    <p class="muted">{{ t('skills.libraryHint') }}</p>
    <div v-if="library.length" class="grid">
      <div v-for="s in library" :key="s.dir_name" class="card">
        <h2>{{ s.name }}</h2>
        <p class="muted">{{ s.description }}</p>
        <p class="muted"><code>{{ s.dir_name }}</code></p>
      </div>
    </div>
    <p v-else class="muted">{{ t('skills.empty') }}</p>

    <h2 class="sec">{{ t('skills.personal') }}</h2>
    <p class="muted">
      {{ mode === 'personal' ? t('skills.personalHintSingle') : t('skills.personalHint') }}
    </p>
    <div v-if="personalAreas.length">
      <section v-for="area in personalAreas" :key="area.principal ?? '__personal__'" class="area">
        <h3 v-if="mode === 'enterprise'">👤 {{ area.principal }}</h3>
        <div v-if="area.skills.length" class="grid">
          <div v-for="s in area.skills" :key="s.dir_name" class="card">
            <h2>{{ s.name }}</h2>
            <p class="muted">{{ s.description }}</p>
            <p class="muted"><code>{{ s.dir_name }}</code></p>
            <div class="row">
              <button
                class="mini"
                :disabled="busyKey === s.dir_name"
                :data-test="'promote-' + s.dir_name"
                @click="promote(area.principal, s.dir_name)"
              >
                {{ t('skills.promote') }}
              </button>
            </div>
          </div>
        </div>
      </section>
    </div>
    <p v-else class="muted">{{ t('skills.empty') }}</p>
  </div>
</template>

<style scoped>
.sec { margin: 1.2rem 0 0.8rem; font-size: 1.05rem; }
.area { margin-bottom: 1rem; }
.area h3 { margin: 0.4rem 0; font-size: 0.95rem; }
.muted { color: var(--muted); font-size: 0.88rem; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 0.8rem; }
.card { border: 1px solid var(--border); background: var(--surface); border-radius: 0.6rem; padding: 0.8rem 1rem; }
.card h2 { font-size: 1rem; margin: 0 0 0.3rem; }
.row { display: flex; gap: 0.5rem; margin-top: 0.6rem; }
.mini {
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.4rem;
  padding: 0.35rem 0.9rem; cursor: pointer; font-size: 0.88rem;
}
.mini:disabled { opacity: 0.5; cursor: wait; }
.mini:hover { border-color: var(--accent); color: var(--accent); }
.notice { color: var(--ok); font-size: 0.9rem; }
</style>
