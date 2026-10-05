<script setup lang="ts">
import { ApiError, api, OfflineError } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { onMounted, ref } from 'vue'

const { t } = useI18n()

// ── API 資料形（對應 oserver obridge_admin.rs 的模型；密碼欄伺服器端遮蔽為 null）──
interface StatusPayload {
  autostart: boolean
  running: boolean
  pid: number | null
  exe_found: boolean
  exe_path: string | null
  /** null＝無法判定（exe 跑不起來或無 git 建置）；false＝obridge 是舊版二進位。 */
  exe_build_match: boolean | null
  config_path: string
  config_exists: boolean
}
interface RouteRow { address: string; employee: string | null; brain: string | null }
interface SenderRow { employee: string; address: string; name: string | null }
interface ImapForm {
  host: string
  port: number
  username: string
  /** '' ＝保留現值（伺服器語意：null＝保留）。 */
  password: string
  has_password: boolean
  folder: string
  tls_insecure: boolean
}
interface SmtpForm {
  host: string
  port: number
  username: string
  password: string
  has_password: boolean
  subject: string
  tls_insecure: boolean
}
interface ChannelForm {
  source: string
  poll_secs: number
  imap: ImapForm
  smtp: SmtpForm
  routes: RouteRow[]
  senders: SenderRow[]
}
interface PreservedRow { source: string; channel_type: string }
interface ConfigPayload {
  autostart: boolean
  ingress_mode: 'managed' | 'manual'
  ingress_url: string | null
  ingress_secret: string | null
  listen_port: number
  listen_secret: string | null
  channels: ChannelForm[]
  preserved_channels: PreservedRow[]
}
interface EmployeeRow { id: string; name: string; archived: boolean }
interface BrainRow { id: string; name: string }
interface BrainsPayload { active_id: string | null; brains: BrainRow[] }

const status = ref<StatusPayload | null>(null)
const employees = ref<EmployeeRow[]>([])
const brains = ref<BrainRow[]>([])

// 表單狀態（密碼/密鑰欄一律字串——'' 送出時轉 null＝保留現值）
const autostart = ref(false)
const ingressMode = ref<'managed' | 'manual'>('managed')
const ingressUrl = ref('')
const ingressSecret = ref('')
const ingressHasSecret = ref(false)
const listenPort = ref(17401)
const listenSecret = ref('')
const listenHasSecret = ref(false)
const channels = ref<ChannelForm[]>([])
const preserved = ref<PreservedRow[]>([])

const offline = ref(false)
const error = ref('')
const actionError = ref('')
const okMsg = ref('')
const busy = ref(false)

function blankChannel(): ChannelForm {
  return {
    source: '',
    poll_secs: 60,
    imap: { host: '', port: 993, username: '', password: '', has_password: false, folder: 'INBOX', tls_insecure: false },
    smtp: { host: '', port: 465, username: '', password: '', has_password: false, subject: 'Operoid', tls_insecure: false },
    routes: [],
    senders: [],
  }
}

async function load(): Promise<void> {
  try {
    status.value = await api.get<StatusPayload>('/api/obridge/status')
    offline.value = false
    error.value = ''
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
    return
  }
  try {
    const c = await api.get<ConfigPayload>('/api/obridge/config')
    autostart.value = c.autostart
    ingressMode.value = c.ingress_mode === 'manual' ? 'manual' : 'managed'
    ingressUrl.value = c.ingress_url ?? ''
    ingressSecret.value = ''
    ingressHasSecret.value = c.ingress_secret != null
    listenPort.value = c.listen_port
    listenSecret.value = ''
    listenHasSecret.value = c.listen_secret != null
    channels.value = c.channels.map((ch) => ({
      ...ch,
      imap: { ...ch.imap, password: '' },
      smtp: { ...ch.smtp, password: '' },
    }))
    preserved.value = c.preserved_channels ?? []
    error.value = ''
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    offline.value = e instanceof OfflineError
    error.value = e instanceof ApiError ? e.code : 'server.offline'
  }
  try {
    const emps = await api.get<EmployeeRow[]>('/api/employees')
    employees.value = emps.filter((x) => !x.archived)
  } catch {
    employees.value = []
  }
  try {
    const o = await api.get<BrainsPayload>('/api/brains')
    brains.value = o.brains
  } catch {
    brains.value = []
  }
}

onMounted(load)

function genSecret(target: 'ingress' | 'listen'): void {
  const b = new Uint8Array(32)
  crypto.getRandomValues(b)
  const hex = Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('')
  if (target === 'ingress') ingressSecret.value = hex
  else listenSecret.value = hex
}

function addChannel(): void {
  channels.value.push(blankChannel())
}
function removeChannel(ci: number): void {
  if (!window.confirm(t('obridge.removeChannelConfirm'))) return
  channels.value.splice(ci, 1)
}
function addRoute(ch: ChannelForm): void {
  ch.routes.push({ address: '', employee: null, brain: null })
}
function addSender(ch: ChannelForm): void {
  ch.senders.push({ employee: '', address: '', name: null })
}

// 路由目標下拉（e:<employee id>／b:<brain id>）↔ employee/brain 擇一欄位
function selOf(r: RouteRow): string {
  if (r.employee) return 'e:' + r.employee
  if (r.brain) return 'b:' + r.brain
  return ''
}
function setSel(r: RouteRow, v: string): void {
  if (v.startsWith('e:')) {
    r.employee = v.slice(2)
    r.brain = null
  } else if (v.startsWith('b:')) {
    r.brain = v.slice(2)
    r.employee = null
  } else {
    r.employee = null
    r.brain = null
  }
}

async function save(): Promise<void> {
  actionError.value = ''
  okMsg.value = ''
  busy.value = true
  try {
    const payload = {
      autostart: autostart.value,
      ingress_mode: ingressMode.value,
      ingress_url: ingressUrl.value.trim() || null,
      ingress_secret: ingressSecret.value.trim() || null,
      listen_port: listenPort.value,
      listen_secret: listenSecret.value.trim() || null,
      channels: channels.value.map((ch) => ({
        source: ch.source.trim(),
        poll_secs: ch.poll_secs,
        imap: { ...ch.imap, password: ch.imap.password || null },
        smtp: { ...ch.smtp, password: ch.smtp.password || null },
      })),
    }
    const r = await api.put<{ action: string; pid: number | null }>('/api/obridge/config', payload)
    okMsg.value = t('obridge.act.' + r.action) + (r.pid ? ` (pid=${r.pid})` : '')
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    busy.value = false
  }
}

async function restart(): Promise<void> {
  actionError.value = ''
  okMsg.value = ''
  busy.value = true
  try {
    const r = await api.post<{ ok: boolean; pid: number }>('/api/obridge/restart')
    okMsg.value = t('obridge.restartDone') + ` (pid=${r.pid})`
    await load()
  } catch (e) {
    actionError.value = e instanceof ApiError ? e.code : 'server.internal'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div>
    <ErrorBox :message="offline ? t('login.offline') : error" />
    <h2 class="sec">{{ t('obridge.heading') }}</h2>
    <p class="muted desc">{{ t('obridge.desc') }}</p>

    <!-- 狀態 -->
    <div class="card" data-test="ob-status">
      <h2>{{ t('obridge.statusHeading') }}</h2>
      <div class="formgrid">
        <label class="chk">
          <input v-model="autostart" type="checkbox" data-test="ob-autostart" />
          {{ t('obridge.autostart') }}
        </label>
        <p class="muted hint">{{ t('obridge.autostartHint') }}</p>
      </div>
      <p>
        <span :class="status?.running ? 'okline' : 'muted'">
          {{
            status?.running
              ? t('obridge.running', { pid: String(status.pid ?? '') })
              : t('obridge.notRunning')
          }}
        </span>
        <span v-if="status" class="muted">　{{ t('obridge.configPath') }}：<code>{{ status.config_path }}</code></span>
        <span v-if="status && !status.exe_found" class="errline">　{{ t('obridge.exeMissing') }}</span>
        <span v-if="status && status.exe_build_match === false" class="errline" data-test="ob-exe-mismatch">
            　{{ t('obridge.exeBuildMismatch') }}
        </span>
      </p>
      <p class="row">
        <button class="primary" :disabled="busy" data-test="ob-save" @click="save">
          {{ t('obridge.save') }}
        </button>
        <button class="mini" :disabled="busy || !status?.autostart" :title="status?.autostart ? '' : t('obridge.notEnabledHint')" @click="restart">
          {{ t('obridge.restartBtn') }}
        </button>
        <button class="mini" :disabled="busy" @click="load">{{ t('obridge.reload') }}</button>
      </p>
    </div>

    <!-- 進氣（ingress） -->
    <div class="card">
      <h2>{{ t('obridge.connHeading') }}</h2>
      <div class="formgrid">
        <label class="chk">
          <input v-model="ingressMode" type="radio" value="managed" />
          {{ t('obridge.modeManaged') }}
        </label>
        <label class="chk">
          <input v-model="ingressMode" type="radio" value="manual" />
          {{ t('obridge.modeManual') }}
        </label>
      </div>
      <p class="muted hint">
        {{ ingressMode === 'managed' ? t('obridge.modeManagedHint') : t('obridge.modeManualHint') }}
      </p>
      <template v-if="ingressMode === 'manual'">
        <label for="ob-url">{{ t('obridge.ingressUrl') }}</label>
        <input id="ob-url" v-model="ingressUrl" placeholder="http://&lt;伺服器IP&gt;:7340/event" />
        <label for="ob-sec">{{ t('obridge.ingressSecret') }}</label>
        <input
          id="ob-sec"
          v-model="ingressSecret"
          type="password"
          autocomplete="new-password"
          :placeholder="ingressHasSecret ? t('obridge.keepPlaceholder') : t('obridge.ingressSecretRequired')"
        />
        <p class="row">
          <button class="mini" type="button" @click="genSecret('ingress')">{{ t('obridge.genBtn') }}</button>
        </p>
      </template>
    </div>

    <!-- 出氣（listen） -->
    <div class="card">
      <h2>{{ t('obridge.listenHeading') }}</h2>
      <p class="muted hint">{{ t('obridge.listenHint') }}</p>
      <div class="formgrid">
        <div>
          <label for="ob-port">{{ t('obridge.listenPort') }}</label>
          <input id="ob-port" v-model.number="listenPort" type="number" min="1" max="65535" />
        </div>
        <div>
          <label for="ob-lsec">{{ t('obridge.listenSecret') }}</label>
          <input
            id="ob-lsec"
            v-model="listenSecret"
            type="password"
            autocomplete="new-password"
            :placeholder="listenHasSecret ? t('obridge.keepPlaceholder') : t('obridge.listenSecretAuto')"
          />
          <p class="row">
            <button class="mini" type="button" @click="genSecret('listen')">{{ t('obridge.genBtn') }}</button>
          </p>
        </div>
      </div>
    </div>

    <!-- 頻道 -->
    <div class="card">
      <h2>{{ t('obridge.channelsHeading') }}</h2>
      <p v-if="preserved.length" class="muted hint">
        {{ t('obridge.preserved', { n: String(preserved.length) }) }}
        <code v-for="p in preserved" :key="p.source" class="pill">{{ p.source }}（{{ p.channel_type }}）</code>
      </p>
      <p v-if="!channels.length" class="muted">{{ t('obridge.noChannels') }}</p>

      <div v-for="(ch, ci) in channels" :key="ci" class="channel" data-test="ob-channel">
        <p class="row spread">
          <strong>{{ t('obridge.channelN', { n: String(ci + 1) }) }}</strong>
          <button class="mini bad" :disabled="busy" data-test="ob-channel-del" @click="removeChannel(ci)">
            {{ t('obridge.removeChannel') }}
          </button>
        </p>
        <div class="formgrid">
          <div>
            <label :for="`ob-src-${ci}`">{{ t('obridge.channelSource') }}</label>
            <input :id="`ob-src-${ci}`" v-model="ch.source" placeholder="email" />
          </div>
          <div>
            <label :for="`ob-poll-${ci}`">{{ t('obridge.pollSecs') }}</label>
            <input :id="`ob-poll-${ci}`" v-model.number="ch.poll_secs" type="number" min="1" />
          </div>
        </div>

        <h3>{{ t('obridge.imapHeading') }}</h3>
        <div class="formgrid">
          <div>
            <label :for="`ob-ih-${ci}`">host</label>
            <input :id="`ob-ih-${ci}`" v-model="ch.imap.host" placeholder="imap.corp.com" />
          </div>
          <div>
            <label :for="`ob-ip-${ci}`">port</label>
            <input :id="`ob-ip-${ci}`" v-model.number="ch.imap.port" type="number" min="1" max="65535" />
          </div>
          <div>
            <label :for="`ob-iu-${ci}`">{{ t('obridge.username') }}</label>
            <input :id="`ob-iu-${ci}`" v-model="ch.imap.username" placeholder="bot@corp.com" />
          </div>
          <div>
            <label :for="`ob-iw-${ci}`">{{ t('obridge.password') }}</label>
            <input
              :id="`ob-iw-${ci}`"
              v-model="ch.imap.password"
              type="password"
              autocomplete="new-password"
              :placeholder="ch.imap.has_password ? t('obridge.keepPlaceholder') : t('obridge.passwordRequired')"
            />
          </div>
          <div>
            <label :for="`ob-if-${ci}`">{{ t('obridge.folder') }}</label>
            <input :id="`ob-if-${ci}`" v-model="ch.imap.folder" />
          </div>
        </div>

        <h3>{{ t('obridge.smtpHeading') }}</h3>
        <div class="formgrid">
          <div>
            <label :for="`ob-sh-${ci}`">host</label>
            <input :id="`ob-sh-${ci}`" v-model="ch.smtp.host" placeholder="smtp.corp.com" />
          </div>
          <div>
            <label :for="`ob-sp-${ci}`">port</label>
            <input :id="`ob-sp-${ci}`" v-model.number="ch.smtp.port" type="number" min="1" max="65535" />
          </div>
          <div>
            <label :for="`ob-su-${ci}`">{{ t('obridge.username') }}</label>
            <input :id="`ob-su-${ci}`" v-model="ch.smtp.username" placeholder="bot@corp.com" />
          </div>
          <div>
            <label :for="`ob-sw-${ci}`">{{ t('obridge.password') }}</label>
            <input
              :id="`ob-sw-${ci}`"
              v-model="ch.smtp.password"
              type="password"
              autocomplete="new-password"
              :placeholder="ch.smtp.has_password ? t('obridge.keepPlaceholder') : t('obridge.passwordRequired')"
            />
          </div>
          <div>
            <label :for="`ob-sj-${ci}`">{{ t('obridge.subject') }}</label>
            <input :id="`ob-sj-${ci}`" v-model="ch.smtp.subject" />
          </div>
        </div>
        <label class="chk">
          <input v-model="ch.imap.tls_insecure" type="checkbox" />
          {{ t('obridge.imapTlsInsecure') }}
        </label>
        <label class="chk">
          <input v-model="ch.smtp.tls_insecure" type="checkbox" />
          {{ t('obridge.smtpTlsInsecure') }}
        </label>

        <!-- 路由 -->
        <h3>{{ t('obridge.routesHeading') }}</h3>
        <table v-if="ch.routes.length" class="tbl">
          <thead>
            <tr>
              <th>{{ t('obridge.routeAddress') }}</th>
              <th>{{ t('obridge.routeTarget') }}</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(r, ri) in ch.routes" :key="ri">
              <td><input v-model="r.address" placeholder="steve@corp.com" /></td>
              <td>
                <select :value="selOf(r)" @change="setSel(r, ($event.target as HTMLSelectElement).value)">
                  <option value="">{{ t('obridge.targetSelect') }}</option>
                  <optgroup :label="t('obridge.optEmployees')">
                    <option v-for="emp in employees" :key="'e' + emp.id" :value="'e:' + emp.id">
                      {{ emp.name }}（{{ emp.id }}）
                    </option>
                  </optgroup>
                  <optgroup :label="t('obridge.optBrains')">
                    <option v-for="b in brains" :key="'b' + b.id" :value="'b:' + b.id">
                      {{ b.name }}（{{ b.id }}）
                    </option>
                  </optgroup>
                </select>
              </td>
              <td>
                <button class="mini bad" :disabled="busy" @click="ch.routes.splice(ri, 1)">
                  {{ t('obridge.removeRoute') }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
        <p class="row"><button class="mini" :disabled="busy" @click="addRoute(ch)">{{ t('obridge.addRoute') }}</button></p>

        <!-- 寄件身分 -->
        <h3>{{ t('obridge.sendersHeading') }}</h3>
        <table v-if="ch.senders.length" class="tbl">
          <thead>
            <tr>
              <th>{{ t('obridge.senderEmployee') }}</th>
              <th>{{ t('obridge.senderAddress') }}</th>
              <th>{{ t('obridge.senderName') }}</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(s, si) in ch.senders" :key="si">
              <td>
                <select v-model="s.employee">
                  <option value="">{{ t('obridge.targetSelect') }}</option>
                  <option v-for="emp in employees" :key="emp.id" :value="emp.id">
                    {{ emp.name }}（{{ emp.id }}）
                  </option>
                </select>
              </td>
              <td><input v-model="s.address" placeholder="steve@corp.com" /></td>
              <td><input v-model="s.name" :placeholder="t('obridge.senderNamePh')" /></td>
              <td>
                <button class="mini bad" :disabled="busy" @click="ch.senders.splice(si, 1)">
                  {{ t('obridge.removeSender') }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
        <p class="row"><button class="mini" :disabled="busy" @click="addSender(ch)">{{ t('obridge.addSender') }}</button></p>
      </div>

      <p class="row">
        <button class="mini" :disabled="busy" data-test="ob-channel-add" @click="addChannel">
          {{ t('obridge.addChannel') }}
        </button>
      </p>
    </div>

    <p v-if="actionError" class="errline">{{ actionError }}</p>
    <p v-if="okMsg" class="okline">{{ okMsg }}</p>
  </div>
</template>

<style scoped>
.sec { margin: 1.4rem 0 0.4rem; font-size: 1.05rem; }
.desc { margin-top: 0; }
.card { margin-bottom: 1rem; }
.card h2 { margin-top: 0; }
.card h3 { margin: 1rem 0 0.4rem; font-size: 0.92rem; color: var(--muted); }
.formgrid { display: grid; grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr)); gap: 0.5rem 1rem; align-items: start; }
.formgrid label, label { display: block; }
.formgrid .hint, .hint { grid-column: 1 / -1; font-size: 0.85rem; margin: 0.1rem 0 0.4rem; }
.chk { display: flex; align-items: center; gap: 0.4rem; font-size: 0.92rem; }
.chk input { width: auto; }
.channel { border: 1px solid var(--border); border-radius: var(--radius); padding: 0.8rem 1rem; margin: 0.8rem 0; }
.row { display: flex; gap: 0.4rem; flex-wrap: wrap; align-items: center; }
.spread { justify-content: space-between; margin-top: 0; }
.tbl { width: 100%; border-collapse: collapse; font-size: 0.88rem; }
.tbl th, .tbl td { text-align: left; padding: 0.35rem 0.5rem; border-bottom: 1px solid var(--border); }
.tbl th { color: var(--muted); font-weight: 600; font-size: 0.8rem; }
.mini {
  border: 1px solid var(--border); background: var(--surface); border-radius: 0.4rem;
  padding: 0.3rem 0.8rem; cursor: pointer; font-size: 0.85rem;
}
.mini:disabled { opacity: 0.5; cursor: wait; }
.mini.bad { color: var(--danger); }
.errline { color: var(--danger); font-size: 0.9rem; }
.okline { color: var(--ok); font-size: 0.9rem; }
.pill { display: inline-block; margin: 0 0.3rem; padding: 0.1rem 0.5rem; border: 1px solid var(--border); border-radius: 999px; font-size: 0.8rem; }
</style>
