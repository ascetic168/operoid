<script setup lang="ts">
import { ApiError, api, OfflineError } from '@front/api-client'
import { ErrorBox, MarkdownText } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { ref } from 'vue'

const { t } = useI18n()

const query = ref('')
const running = ref<'' | 'ask' | 'think'>('')
const offline = ref(false)
const error = ref('')
const output = ref('') // 檢索結果（gbrain think 輸出為 Markdown＋[[wikilink]] 引用格式）
const denied = ref(false)

/** 使用者級知識檢索：POST /api/knowledge/ask（Req::User）——伺服器端按登入者
 *  身份的知識範圍過濾（C12a：policy fail-closed＋receipt），同步回應。 */
async function run(op: 'ask' | 'think'): Promise<void> {
  const q = query.value.trim()
  if (!q || running.value) return
  running.value = op
  offline.value = false
  error.value = ''
  output.value = ''
  denied.value = false
  try {
    const r = await api.post<{ text: string; denied: boolean }>('/api/knowledge/ask', {
      op,
      arg: q,
    })
    output.value = r.text
    denied.value = r.denied
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  } finally {
    running.value = ''
  }
}
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error" />
    <h2 class="sec">{{ t('ask.title') }}</h2>
    <p class="muted">{{ t('ask.desc') }}</p>

    <div class="askrow">
      <input
        v-model="query"
        class="qinput"
        :placeholder="t('ask.placeholder')"
        data-test="ask-query"
        @keydown.enter="run('think')"
      />
      <button class="primary askbtn" :disabled="running !== '' || !query.trim()" data-test="ask-ask" @click="run('ask')">
        {{ running === 'ask' ? t('ask.running') : t('ask.ask') }}
      </button>
      <button class="primary askbtn" :disabled="running !== '' || !query.trim()" data-test="ask-think" @click="run('think')">
        {{ running === 'think' ? t('ask.running') : t('ask.think') }}
      </button>
    </div>

    <div v-if="denied" class="denied">{{ t('ask.denied') }}</div>
    <!-- think 輸出為 Markdown；wikilink 引用由 MarkdownText 內建的行內 tokenizer 上色 -->
    <div v-else-if="output" class="result" data-test="ask-output">
      <MarkdownText :text="output" />
    </div>
  </div>
</template>

<style scoped>
.sec { margin: 1.4rem 0 0.4rem; font-size: 1.05rem; }
.askrow { display: flex; gap: 0.5rem; margin: 0.8rem 0; flex-wrap: wrap; }
.qinput { flex: 1 1 20rem; }
.askbtn { flex: 0 0 auto; width: auto; padding: 0.55rem 1.1rem; }
.denied {
  margin-top: 0.8rem; padding: 0.7rem; border: 1px solid var(--border);
  border-radius: 0.5rem; background: var(--surface); color: var(--muted); font-size: 0.9rem;
}
.result {
  margin-top: 0.8rem; padding: 0.9rem; border: 1px solid var(--border);
  border-radius: 0.5rem; background: var(--surface);
  font-size: 0.9rem; line-height: 1.55;
}
</style>
