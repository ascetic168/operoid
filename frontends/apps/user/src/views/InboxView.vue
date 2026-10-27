<script setup lang="ts">
import { ApiError, api, OfflineError } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { onMounted, ref } from 'vue'

const { t } = useI18n()

interface Proposal {
  commitment_id: string
  title: string
  completion_condition: string
  employee_id: string
  employee_name: string
}
interface SkillProposal {
  id: string
  skill_name: string
  skill_title: string
  description: string
  employee_id: string
  employee_name: string
  risk: string
  linter_summary: string
  review_excerpt: string | null
  note: string | null
}
interface Flagged {
  employee_id: string
  employee_name: string
  state: string
}

const proposals = ref<Proposal[]>([])
const skillProposals = ref<SkillProposal[]>([])
const flagged = ref<Flagged[]>([])
const offline = ref(false)
const error = ref('')
const busyId = ref('')
const busySkillId = ref('')

async function load(): Promise<void> {
  try {
    const inbox = await api.get<{
      proposals: Proposal[]
      skill_proposals: SkillProposal[]
      flagged_employees: Flagged[]
    }>('/api/inbox')
    proposals.value = inbox.proposals
    skillProposals.value = inbox.skill_proposals
    flagged.value = inbox.flagged_employees
    error.value = ''
    offline.value = false
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
}

async function act(id: string, kind: 'approve' | 'reject'): Promise<void> {
  busyId.value = id
  try {
    await api.post('/api/commitments/' + encodeURIComponent(id) + '/' + kind)
    await load()
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  } finally {
    busyId.value = ''
  }
}

async function actSkill(id: string, kind: 'approve' | 'reject'): Promise<void> {
  busySkillId.value = id
  try {
    await api.post(
      '/api/skill-proposals/' + encodeURIComponent(id) + '/' + kind,
      kind === 'reject' ? {} : undefined,
    )
    await load()
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  } finally {
    busySkillId.value = ''
  }
}

onMounted(load)
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error" />
    <h2 class="sec">{{ t('inbox.proposals') }}</h2>
    <div v-if="proposals.length" class="grid">
      <div v-for="p in proposals" :key="p.commitment_id" class="card" data-test="proposal">
        <h2>{{ p.title }}</h2>
        <p class="muted">{{ p.completion_condition }}</p>
        <p class="muted">👤 {{ p.employee_name }}</p>
        <div class="row">
          <button
            class="mini ok"
            :disabled="busyId === p.commitment_id"
            data-test="approve"
            @click="act(p.commitment_id, 'approve')"
          >
            {{ t('inbox.approve') }}
          </button>
          <button
            class="mini bad"
            :disabled="busyId === p.commitment_id"
            data-test="reject"
            @click="act(p.commitment_id, 'reject')"
          >
            {{ t('inbox.reject') }}
          </button>
        </div>
      </div>
    </div>
    <p v-else class="muted">{{ t('inbox.empty') }}</p>

    <h2 class="sec">{{ t('inbox.skillProposals') }}</h2>
    <div v-if="skillProposals.length" class="grid">
      <div v-for="s in skillProposals" :key="s.id" class="card" data-test="skill-proposal">
        <h2>{{ s.skill_title || s.skill_name }} <code class="risk" :data-risk="s.risk">{{ s.risk }}</code></h2>
        <p v-if="s.description" class="muted">{{ s.description }}</p>
        <p class="muted">👤 {{ s.employee_name }}</p>
        <p class="muted"><code>{{ s.linter_summary }}</code></p>
        <p v-if="s.note" class="muted">💬 {{ s.note }}</p>
        <details v-if="s.review_excerpt" class="review">
          <summary>{{ t('inbox.skillReview') }}</summary>
          <pre>{{ s.review_excerpt }}</pre>
        </details>
        <div class="row">
          <button
            class="mini ok"
            :disabled="busySkillId === s.id"
            data-test="skill-approve"
            @click="actSkill(s.id, 'approve')"
          >
            {{ t('inbox.approve') }}
          </button>
          <button
            class="mini bad"
            :disabled="busySkillId === s.id"
            data-test="skill-reject"
            @click="actSkill(s.id, 'reject')"
          >
            {{ t('inbox.reject') }}
          </button>
        </div>
      </div>
    </div>
    <p v-else class="muted">—</p>

    <h2 class="sec">{{ t('inbox.flagged') }}</h2>
    <div v-if="flagged.length" class="grid">
      <div v-for="f in flagged" :key="f.employee_id" class="card">
        <h2>{{ f.employee_name }}</h2>
        <p class="muted"><code>{{ f.state }}</code></p>
        <router-link class="chat" :to="'/chat/' + encodeURIComponent(f.employee_id)">
          {{ t('home.emp.chat') }}
        </router-link>
      </div>
    </div>
    <p v-else class="muted">—</p>
  </div>
</template>

<style scoped>
.sec { margin: 1.2rem 0 0.8rem; font-size: 1.05rem; }
.row { display: flex; gap: 0.5rem; margin-top: 0.6rem; }
.mini {
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.4rem;
  padding: 0.35rem 0.9rem; cursor: pointer; font-size: 0.88rem;
}
.mini:disabled { opacity: 0.5; cursor: wait; }
.mini.ok { color: var(--ok); }
.mini.ok:hover { border-color: var(--ok); }
.mini.bad { color: var(--danger); }
.mini.bad:hover { border-color: var(--danger); }
.chat { color: var(--accent); font-size: 0.88rem; }
.risk { font-size: 0.75rem; margin-left: 0.4rem; }
.risk[data-risk='high'] { color: var(--danger); }
.risk[data-risk='medium'] { color: #b8860b; }
.risk[data-risk='low'] { color: var(--ok); }
.review { margin-top: 0.4rem; font-size: 0.85rem; }
.review summary { cursor: pointer; color: var(--accent); }
.review pre { white-space: pre-wrap; margin: 0.4rem 0 0; }
</style>
