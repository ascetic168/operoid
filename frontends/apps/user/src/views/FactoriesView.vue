<script setup lang="ts">
import { ApiError, OfflineError, api } from '@front/api-client'
import { ErrorBox } from '@front/ui'
import { useI18n } from 'vue-i18n'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

// 工廠（企業版 user 級）——個人版 GUI FactoriesView 的對應：
// 類型清單（schema pack 動態）→ 上傳檔（瀏覽器無本機路徑，先進伺服器暫存）→
// 轉換寫入 → 預覽／編輯／覆蓋；自動分類（低信心走確認框）；「+ 新增」撰寫器
// （C13c 圈子×等級寫入目標——指定時伺服器自動供給 scope/source 並立即入圖）。

const { t } = useI18n()

// ── 後端契約形狀（鏡像 ocore factories／classifier 的 serde 輸出）──
interface L10n {
  code: string
  params?: Record<string, string>
}
interface FactoryTypeInfo {
  id: string
  dir: string
  pipeline: 'people' | 'textual' | 'capture'
  extensions: string[]
}
interface FactoryTypesResult {
  pack_name: string | null
  pack_effective: string
  is_v2: boolean
  types: FactoryTypeInfo[]
  v2_hint: L10n | null
}
interface PreviewPage {
  slug: string
  target_dir: string
  name: string
  markdown: string
}
interface ProcessedFile {
  path: string
  ok: boolean
  message: L10n | null
  pages: PreviewPage[]
}
interface PreviewResult {
  factory: string
  summary: L10n
  sample: PreviewPage[]
  total: number
  written: string[]
  errors: L10n[]
  files: ProcessedFile[]
  /** C13c：指定圈子×等級轉換時，實際落點（伺服器注入）。 */
  scope?: string
  source?: string
}
interface WriteResult {
  written: string[]
  errors: L10n[]
  note: L10n | null
}
interface AuthoredResult {
  slug: string
  target_dir: string
  path: string
  used_fallback: boolean
  enriched_markdown: string
  names_count: number
  enriched: boolean
  scope?: string
  source?: string
}
interface FileClassification {
  path: string
  factory: string
  confidence: 'high' | 'medium' | 'low'
  reason: string
  source: 'extension' | 'heuristic' | 'llm'
}
type SecurityLevel = 'public' | 'internal' | 'confidential' | 'secret'
interface UploadResult {
  paths: string[]
  staging: string
  skipped: { name: string; reason: string; detail?: string }[]
}

// ── L10n（後端 {code,params} → 當前語言；缺翻譯回原碼）──
type LooseT = (key: string, params?: Record<string, unknown>) => string
const gt = t as unknown as LooseT
function tL10n(m: L10n | null | undefined): string {
  if (!m) return ''
  return m.params ? gt(m.code, m.params) : gt(m.code)
}
function fmtError(e: unknown): string {
  if (e instanceof ApiError) return tL10n({ code: e.code, params: e.params })
  if (e instanceof OfflineError) return t('login.offline')
  return String(e)
}
// i18n key 缺漏時（未知／自訂 pack 類型）fallback 顯示 id。
function tOr(key: string, fallback: string): string {
  const v = t(key)
  return v === key ? fallback : v
}
function typeTitle(id: string): string {
  return tOr(`factories.defs.${id}.title`, id)
}
function typeTarget(id: string): string {
  const info = types.value.find((x) => x.id === id)
  return tOr(`factories.defs.${id}.target`, info ? info.dir : id)
}

// ── 類型清單（動態來自作用中 schema pack）──
const types = ref<FactoryTypeInfo[]>([])
const packName = ref<string | null>(null)
const v2Hint = ref<string | null>(null)
const selected = ref<string>('')
const selectedInfo = computed(() => types.value.find((x) => x.id === selected.value) ?? null)

async function loadTypes(): Promise<void> {
  try {
    const r = await api.get<FactoryTypesResult>('/api/factories/types')
    types.value = r.types
    packName.value = r.pack_name ?? r.pack_effective
    v2Hint.value = tL10n(r.v2_hint) || null
    if (!r.types.some((x) => x.id === selected.value)) {
      selected.value = (r.types.find((x) => x.pipeline !== 'capture') ?? r.types[0])?.id ?? ''
    }
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    errorMsg.value = fmtError(e)
  }
}

// 點選擇器的 accept 過濾（依類型管線副檔名）；自動分類收全部支援副檔名。
function acceptOf(id: string | 'auto'): string {
  const list =
    id === 'auto'
      ? [...new Set(types.value.flatMap((x) => x.extensions))]
      : (types.value.find((x) => x.id === id)?.extensions ?? ['txt', 'md'])
  return list.map((e) => '.' + e).join(',')
}
const factoryOptions = computed(() => types.value.map((x) => x.id))

// ── 狀態 ──
const hovered = ref<string | null>(null)
const busy = ref<string | null>(null)
const preview = ref<PreviewResult | null>(null)
const errorMsg = ref<string | null>(null)
const skippedFiles = ref<{ name: string; reason: string }[]>([])
const selectedSample = ref(0)
const selectedFile = ref<number | null>(null) // >1 檔：null=清單、數字=預覽某檔
const editedMd = ref('')
const overwriteRes = ref<WriteResult | null>(null)

const currentPages = computed<PreviewPage[]>(() => {
  const p = preview.value
  if (!p) return []
  if (p.files.length > 1 && selectedFile.value !== null) {
    return p.files[selectedFile.value]?.pages ?? []
  }
  return p.sample
})

// 暫存批次 id——新上傳前／流程收尾時清掉（伺服器端 24h 清道夫兜底）。
let stagingId: string | null = null

// ── 轉換寫入目標（C13c 圈子×等級；company+internal＝企業預設來源，不送 target）──
const convTargetKind = ref<'company' | 'department' | 'project'>('company')
const convTargetCircle = ref('')
const convTargetLevel = ref<SecurityLevel>('internal')
const convTarget = computed<
  { kind: 'company' | 'department' | 'project'; circle: string; level: SecurityLevel } | undefined
>(() => {
  if (convTargetKind.value === 'company' && convTargetLevel.value === 'internal') return undefined
  const circle = convTargetCircle.value.trim()
  if (convTargetKind.value !== 'company' && !circle) return undefined
  return { kind: convTargetKind.value, circle: circle || 'company', level: convTargetLevel.value }
})
// 當前預覽實際使用的目標（凍結——覆蓋寫回同一供給目錄，不隨後續改選漂移）。
let lastRunTarget: { kind: 'company' | 'department' | 'project'; circle: string; level: SecurityLevel } | null = null
function cleanupStaging(): void {
  if (!stagingId) return
  const s = stagingId
  stagingId = null
  void api.post('/api/factories/upload/cleanup', { staging: s }).catch(() => {})
}

async function uploadFiles(files: File[]): Promise<UploadResult> {
  cleanupStaging()
  const form = new FormData()
  for (const f of files) form.append('files', f, f.name)
  const r = await api.upload<UploadResult>('/api/factories/upload', form)
  stagingId = r.staging
  skippedFiles.value = r.skipped.map((s) => ({ name: s.name, reason: tL10n({ code: s.reason, params: s.detail ? { detail: s.detail } : undefined }) }))
  return r
}

function resetResult(): void {
  preview.value = null
  overwriteRes.value = null
  errorMsg.value = null
  skippedFiles.value = []
  selectedSample.value = 0
  selectedFile.value = null
}

// ── 選檔／拖放入口 ──
const fileInput = ref<HTMLInputElement | null>(null)
const pickTarget = ref<string | null>(null) // 'auto' 或類型 id
function pickFiles(target: string): void {
  pickTarget.value = target
  fileInput.value?.click()
}
function onPickChange(ev: Event): void {
  const files = Array.from((ev.target as HTMLInputElement).files ?? [])
  ;(ev.target as HTMLInputElement).value = ''
  if (!files.length || !pickTarget.value) return
  const target = pickTarget.value
  if (target === 'auto') void onAuto(files)
  else void doRun(target, files)
}
function onDrop(ev: DragEvent, target: string): void {
  hovered.value = null
  const files = Array.from(ev.dataTransfer?.files ?? [])
  if (!files.length) return
  if (target === 'auto') void onAuto(files)
  else void doRun(target, files)
}
function dragOver(target: string): void {
  hovered.value = target
}
function dragLeave(): void {
  hovered.value = null
}

// ── 轉換（run）：轉換＋立即寫入＋回預覽（有 target＝寫進供給目錄並立即入圖）──
async function doRun(factoryId: string, files: File[]): Promise<void> {
  busy.value = factoryId
  resetResult()
  try {
    const up = await uploadFiles(files)
    if (!up.paths.length) {
      errorMsg.value = t('factories.nothingUploaded')
      return
    }
    const body: Record<string, unknown> = { factory: factoryId, paths: up.paths }
    if (convTarget.value) body.target = convTarget.value
    const r = await api.post<PreviewResult>('/api/factories/run', body)
    preview.value = r
    lastRunTarget = convTarget.value ?? null
    syncEditedFromSample()
    cleanupStaging()
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    errorMsg.value = fmtError(e)
    cleanupStaging()
  } finally {
    busy.value = null
  }
}

function syncEditedFromSample(): void {
  const s = currentPages.value[selectedSample.value]
  editedMd.value = s ? s.markdown : ''
}

// ── 自動分類：高／中信心直接跑，低信心跳確認框 ──
interface ClassifyItem {
  path: string
  chosen: string
  reason: string
}
const classifyOpen = ref(false)
const classifyItems = ref<ClassifyItem[]>([])
const classifyBusy = ref(false)

async function onAuto(files: File[]): Promise<void> {
  busy.value = 'auto'
  resetResult()
  try {
    const up = await uploadFiles(files)
    if (!up.paths.length) {
      errorMsg.value = t('factories.nothingUploaded')
      return
    }
    const cls = await api.post<FileClassification[]>('/api/factories/classify', { paths: up.paths })
    const autoCls = cls.filter((c) => c.factory && c.confidence !== 'low')
    const confirmCls = cls.filter((c) => c.factory && c.confidence === 'low')
    const unsupported = cls.filter((c) => !c.factory)

    const groups: Record<string, string[]> = {}
    for (const c of autoCls) (groups[c.factory] ??= []).push(c.path)
    const parts = await runGroups(groups)
    preview.value = parts.length || unsupported.length ? mergePreview(parts, unsupported.length) : null
    lastRunTarget = convTarget.value ?? null

    if (confirmCls.length) {
      classifyItems.value = confirmCls.map((c) => ({
        path: c.path,
        chosen: c.factory || types.value[types.value.length - 1]?.id || '',
        reason: c.reason,
      }))
      classifyOpen.value = true // 暫存留著——確認框跑完才清
    } else {
      cleanupStaging()
    }
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    errorMsg.value = fmtError(e)
    cleanupStaging()
  } finally {
    busy.value = null
  }
}

/** 依分組跑各工廠；單群失敗不中斷其餘，轉成含 errors 的結果。有 target 時逐群帶入
 *  （capture 群伺服器端忽略 target）。 */
async function runGroups(groups: Record<string, string[]>): Promise<PreviewResult[]> {
  const results: PreviewResult[] = []
  for (const [f, paths] of Object.entries(groups)) {
    try {
      const body: Record<string, unknown> = { factory: f, paths }
      if (convTarget.value) body.target = convTarget.value
      results.push(await api.post<PreviewResult>('/api/factories/run', body))
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) throw e
      const detail = fmtError(e)
      const err = { code: 'factory.fileError', params: { file: f, detail } }
      results.push({ factory: f, summary: err, sample: [], total: 0, written: [], errors: [err], files: [] })
    }
  }
  return results
}

/** 把多個工廠的 PreviewResult 合併成一個（factory="auto"）餵預覽面板。
 *  scope/source 取自任一帶落點的部分（同目標轉換 → 同一供給 scope）。 */
function mergePreview(parts: PreviewResult[], skippedCount = 0): PreviewResult {
  const sample = parts.flatMap((p) => p.sample).slice(0, 10)
  const files = parts.flatMap((p) => p.files)
  const written = parts.flatMap((p) => p.written)
  const errors = parts.flatMap((p) => p.errors)
  const total = parts.reduce((n, p) => n + p.total, 0)
  const code = skippedCount > 0 ? 'factories.classify.autoSummarySkipped' : 'factories.classify.autoSummary'
  const scoped = parts.find((p) => p.scope)
  return {
    factory: 'auto',
    summary: { code, params: { n: String(written.length), skipped: String(skippedCount) } },
    sample,
    total,
    written,
    errors,
    files,
    scope: scoped?.scope,
    source: scoped?.source,
  }
}

/** 確認框：依使用者選定的工廠分組跑，結果併入現有預覽。 */
async function confirmClassify(): Promise<void> {
  classifyOpen.value = false
  if (!classifyItems.value.length) return
  classifyBusy.value = true
  busy.value = 'auto'
  try {
    const groups: Record<string, string[]> = {}
    for (const it of classifyItems.value) (groups[it.chosen] ??= []).push(it.path)
    const parts = await runGroups(groups)
    if (parts.length) {
      preview.value = preview.value ? mergePreview([preview.value, ...parts]) : mergePreview(parts)
    }
    if (!lastRunTarget && convTarget.value) lastRunTarget = convTarget.value
    cleanupStaging()
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    errorMsg.value = fmtError(e)
    cleanupStaging()
  } finally {
    classifyBusy.value = false
    busy.value = null
    classifyItems.value = []
  }
}

// ── 覆蓋寫入（預覽後編輯過的頁）——有 target 的預覽寫回同一供給目錄 ──
async function doOverwrite(): Promise<void> {
  const s = currentPages.value[selectedSample.value]
  if (!s) return
  busy.value = 'overwrite'
  try {
    const body: Record<string, unknown> = {
      pages: [{ slug: s.slug, target_dir: s.target_dir, markdown: editedMd.value }],
    }
    if (preview.value?.scope && lastRunTarget) body.target = lastRunTarget
    overwriteRes.value = await api.post<WriteResult>('/api/factories/write-pages', body)
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    errorMsg.value = fmtError(e)
  } finally {
    busy.value = null
  }
}

// ── 「+ 新增」撰寫器 ──
const editorOpen = ref(false)
const editorFactory = ref('')
const editorMd = ref('')
const editorSlug = ref<string | null>(null)
const editorResult = ref<AuthoredResult | null>(null)
const editorError = ref<string | null>(null)
const editorBusy = ref(false)

// C13c：撰寫目標（圈子×等級；company+internal＝預設公司層，不送 target）
const editorTargetKind = ref<'company' | 'department' | 'project'>('company')
const editorTargetCircle = ref('')
const editorTargetLevel = ref<SecurityLevel>('internal')
const editorTarget = computed<
  { kind: 'company' | 'department' | 'project'; circle: string; level: SecurityLevel } | undefined
>(() => {
  if (editorTargetKind.value === 'company' && editorTargetLevel.value === 'internal') return undefined
  const circle = editorTargetCircle.value.trim()
  if (editorTargetKind.value !== 'company' && !circle) return undefined
  return { kind: editorTargetKind.value, circle: circle || 'company', level: editorTargetLevel.value }
})

function openEditor(f: string): void {
  editorFactory.value = f
  // 類型專屬範本缺漏（未知／自訂 pack 類型）→ 以 id 動態組通用範本。
  const generic = `---\ntype: ${f}\ntitle: ''\ntags: [${f}]\n---\n\n# \n\n`
  editorMd.value = tOr(`factories.templates.${f}`, generic)
  editorSlug.value = null
  editorResult.value = null
  editorError.value = null
  editorTargetKind.value = 'company'
  editorTargetCircle.value = ''
  editorTargetLevel.value = 'internal'
  editorOpen.value = true
}

async function saveEditor(): Promise<void> {
  editorError.value = null
  editorBusy.value = true
  try {
    const r = await api.post<AuthoredResult>('/api/factories/save-authored', {
      factory: editorFactory.value,
      markdown: editorMd.value,
      existing_slug: editorSlug.value,
      target: editorTarget.value,
    })
    editorSlug.value = r.slug // 之後存檔覆蓋同檔
    editorMd.value = r.enriched_markdown // 反映 LLM 補的 wikilink
    editorResult.value = r
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return
    editorError.value = fmtError(e)
  } finally {
    editorBusy.value = false
  }
}

function closeEditor(): void {
  editorOpen.value = false
}

onMounted(() => {
  void loadTypes()
})
onBeforeUnmount(() => {
  cleanupStaging()
})
</script>

<template>
  <div>
    <ErrorBox :message="errorMsg ?? undefined" />
    <h2 class="sec">{{ t('factories.title') }}</h2>
    <p class="muted">{{ t('factories.desc') }}</p>

    <div v-if="v2Hint" class="hintwarn">{{ v2Hint }}</div>

    <!-- 轉換寫入目標（C13c 圈子×等級）：套用於拖放轉換與自動分類；預設＝企業預設來源 -->
    <div class="targetbar">
      <span class="targetlabel">{{ t('factories.targetBar') }}</span>
      <select v-model="convTargetLevel" class="sel" :disabled="busy !== null">
        <option value="internal">internal</option>
        <option value="confidential">confidential</option>
        <option value="secret">secret</option>
        <option value="public">public</option>
      </select>
      <select v-model="convTargetKind" class="sel" :disabled="busy !== null">
        <option value="company">company</option>
        <option value="department">department</option>
        <option value="project">project</option>
      </select>
      <input
        v-model="convTargetCircle"
        class="circleinput"
        :placeholder="t('factories.editor.targetCircle')"
      />
      <span class="muted small hinttext">
        {{ selectedInfo?.pipeline === 'capture' ? t('factories.targetCaptureNote') : t('factories.targetHintConvert') }}
      </span>
    </div>

    <div class="layout">
      <div class="left">
        <!-- 自動分類：丟任何支援格式，程式判斷歸屬 -->
        <div
          class="card auto"
          :class="{ hovered: hovered === 'auto', busy: busy === 'auto' }"
          @dragover.prevent="dragOver('auto')"
          @dragleave="dragLeave()"
          @drop.prevent="onDrop($event, 'auto')"
        >
          <div class="cardhead">
            <span class="badge auto">✦</span>
            <span class="cardtitle">{{ t('factories.defs.auto.title') }}</span>
          </div>
          <div class="muted small">{{ t('factories.packLabel', { pack: packName ?? '?' }) }}</div>
          <button type="button" class="dz" :disabled="busy !== null" @click="pickFiles('auto')">
            {{ hovered === 'auto' ? t('factories.dropActive') : t('factories.defs.auto.hint') }}
          </button>
        </div>

        <!-- 類型 chips：點選決定右側工作區目標 -->
        <div class="chips">
          <button
            v-for="tp in types"
            :key="tp.id"
            type="button"
            class="chip"
            :class="{ active: selected === tp.id }"
            @click="selected = tp.id"
          >
            <span class="chipbadge">{{ tp.id.slice(0, 1).toUpperCase() }}</span>
            {{ typeTitle(tp.id) }}
          </button>
        </div>
      </div>

      <!-- 選中類型的工作區 -->
      <div
        v-if="selectedInfo"
        :key="selectedInfo.id"
        class="card work"
        :class="{ hovered: hovered === selectedInfo.id, busy: busy === selectedInfo.id }"
        @dragover.prevent="dragOver(selectedInfo.id)"
        @dragleave="dragLeave()"
        @drop.prevent="onDrop($event, selectedInfo.id)"
      >
        <div class="cardhead">
          <span class="badge">{{ selectedInfo.id.slice(0, 1).toUpperCase() }}</span>
          <span class="min">
            <span class="cardtitle">
              {{ typeTitle(selectedInfo.id) }}
              <span
                v-if="selectedInfo.pipeline === 'capture'"
                :title="t('factories.defs.note.noGraphHint')"
                class="nograph"
                >{{ t('factories.defs.note.noGraph') }}</span
              >
            </span>
            <span class="muted small">{{ t('factories.accept') }}{{ selectedInfo.extensions.join(' / ').toUpperCase() }}</span>
          </span>
          <button
            type="button"
            class="iconbtn"
            :title="t('factories.addTooltip', { title: typeTitle(selectedInfo.id) })"
            @click="openEditor(selectedInfo.id)"
          >
            +
          </button>
        </div>
        <button
          type="button"
          class="dz grow"
          :disabled="busy !== null"
          @click="pickFiles(selectedInfo.id)"
        >
          {{ hovered === selectedInfo.id ? t('factories.dropActive') : t('factories.dropHint') }}
        </button>
        <div class="muted small">
          {{ t('factories.output') }} <code>{{ typeTarget(selectedInfo.id) }}</code>
        </div>
      </div>
    </div>

    <!-- 隱藏檔案選擇器（拖放區點擊用） -->
    <input ref="fileInput" type="file" multiple hidden :accept="acceptOf(pickTarget ?? 'auto')" @change="onPickChange" />

    <div v-if="skippedFiles.length" class="skipped">
      <div v-for="(s, i) in skippedFiles" :key="i">{{ s.name }}：{{ s.reason }}</div>
    </div>

    <!-- 預覽 / 結果 -->
    <section v-if="busy || preview || errorMsg" class="panel">
      <div v-if="busy && !preview" class="muted">
        {{ t('factories.converting') }}
      </div>
      <template v-else-if="preview">
        <div class="sumrow">
          <span class="sumok">✓ {{ tL10n(preview.summary) }}</span>
        </div>
        <div v-if="preview.scope" class="okline small">
          {{ t('factories.editor.scopeSource', { scope: preview.scope, source: preview.source ?? '' }) }}
        </div>

        <div v-if="preview.errors.length" class="perrors">
          <div v-for="(e, i) in preview.errors" :key="i">{{ tL10n(e) }}</div>
        </div>

        <!-- 多檔批次：檔案清單（點選進入單檔預覽） -->
        <div v-if="preview.files.length > 1 && selectedFile === null" class="mb2">
          <div class="muted small mb2">{{ t('factories.filesProcessed') }}</div>
          <div class="filelist">
            <button
              v-for="(f, i) in preview.files"
              :key="i"
              type="button"
              class="filerow"
              @click="selectedFile = i; selectedSample = 0; syncEditedFromSample()"
            >
              <span :class="f.ok ? 'ok' : 'bad'">{{ f.ok ? '✓' : '✗' }}</span>
              <span class="mono grow truncate">{{ f.path }}</span>
              <span class="muted small">{{ t('factories.pagesN', { n: f.pages.length }) }}</span>
            </button>
          </div>
        </div>

        <!-- 單檔預覽／編輯；或多檔選了某檔後 -->
        <template v-else>
          <div v-if="preview.files.length > 1" class="mb2">
            <button type="button" class="linkbtn" @click="selectedFile = null">
              ← {{ t('factories.backToList') }}
            </button>
            <span class="mono muted small">{{ preview.files[selectedFile ?? 0]?.path }}</span>
          </div>

          <div v-if="currentPages.length > 1" class="mb2 rowwrap">
            <span class="muted small">{{ t('factories.previewN') }}</span>
            <select v-model.number="selectedSample" class="sel">
              <option v-for="(s, i) in currentPages" :key="i" :value="i">{{ s.slug }}</option>
            </select>
            <span class="muted small">{{ t('factories.pageEditable') }}</span>
          </div>

          <div v-if="currentPages.length" class="mb2 rowbetween">
            <code class="small">{{ currentPages[selectedSample]?.target_dir }}/{{ currentPages[selectedSample]?.slug }}.md</code>
            <button type="button" class="primary" :disabled="busy !== null" @click="doOverwrite">
              {{ t('factories.overwrite') }}
            </button>
          </div>
          <textarea
            v-if="currentPages.length"
            v-model="editedMd"
            spellcheck="false"
            class="mdedit"
          ></textarea>
        </template>
        <div v-if="overwriteRes" class="okline">
          ✓ {{ t('factories.overwrittenN', { n: overwriteRes.written.length }) }}
        </div>
        <div class="muted small">{{ t('factories.noSyncHint') }}</div>
      </template>
    </section>
    <div v-else class="muted emptyline">{{ t('factories.empty') }}</div>

    <!-- 「+ 新增」撰寫器彈窗 -->
    <div v-if="editorOpen" class="modal" @click.self="closeEditor">
      <div class="dialog">
        <div class="dialoghead">
          <span class="cardtitle">{{ t('factories.editorTitle', { factory: editorFactory }) }}</span>
          <button type="button" class="iconbtn" @click="closeEditor">×</button>
        </div>

        <div class="dialoghint">
          <span v-if="editorResult">
            {{ t('factories.editorWritten') }}<code>{{ editorResult.target_dir }}/{{ editorResult.slug }}.md</code>
            <span v-if="editorResult.enriched" class="oktext">
              {{ t('factories.editorEnriched', { n: editorResult.names_count }) }}
            </span>
            <span v-else class="warntext">{{ t('factories.editorNotEnriched') }}</span>
            <span v-if="editorResult.used_fallback" class="warntext">{{ t('factories.editorFallback') }}</span>
            <span v-if="editorResult.scope" class="oktext">
              {{ t('factories.editor.scopeSource', { scope: editorResult.scope, source: editorResult.source ?? '' }) }}
            </span>
          </span>
          <span v-else>{{ t('factories.editorHint') }}</span>
        </div>

        <!-- C13c：寫入目標（圈子×等級；預設公司層） -->
        <div class="targetrow">
          <span class="muted small">{{ t('factories.editor.targetLevel') }}</span>
          <select v-model="editorTargetLevel" class="sel">
            <option value="internal">internal</option>
            <option value="confidential">confidential</option>
            <option value="secret">secret</option>
            <option value="public">public</option>
          </select>
          <select v-model="editorTargetKind" class="sel">
            <option value="company">company</option>
            <option value="department">department</option>
            <option value="project">project</option>
          </select>
          <input
            v-model="editorTargetCircle"
            class="circleinput"
            :placeholder="t('factories.editor.targetCircle')"
          />
        </div>
        <div class="dialoghint muted">{{ t('factories.editor.targetHint') }}</div>

        <textarea v-model="editorMd" spellcheck="false" class="mdedit grow"></textarea>

        <div v-if="editorError" class="perrors">{{ editorError }}</div>

        <div class="dialogfoot">
          <button type="button" class="ghost" @click="closeEditor">{{ t('factories.close') }}</button>
          <button type="button" class="primary" :disabled="editorBusy" @click="saveEditor">
            {{ editorBusy ? t('factories.editorEnriching') : t('factories.editorSave', { mode: editorSlug ? t('factories.editorSaveOverwrite') : t('factories.editorSaveNew') }) }}
          </button>
        </div>
      </div>
    </div>

    <!-- 自動分類確認框（低信心檔案） -->
    <div v-if="classifyOpen" class="modal" @click.self="classifyOpen = false">
      <div class="dialog narrow">
        <div class="dialoghead">
          <span class="cardtitle">{{ t('factories.classify.confirmTitle') }}</span>
          <button type="button" class="iconbtn" @click="classifyOpen = false">×</button>
        </div>
        <div class="dialoghint">{{ t('factories.classify.confirmHint') }}</div>
        <div class="confirmlist">
          <div v-for="(it, i) in classifyItems" :key="i" class="confirmrow">
            <span class="min">
              <span class="mono small truncate">{{ it.path }}</span>
              <span class="muted small">{{ it.reason }}</span>
            </span>
            <select v-model="it.chosen" class="sel">
              <option v-for="fo in factoryOptions" :key="fo" :value="fo">{{ fo }}</option>
            </select>
          </div>
        </div>
        <div class="dialogfoot">
          <button type="button" class="ghost" @click="classifyOpen = false">{{ t('factories.close') }}</button>
          <button type="button" class="primary" :disabled="classifyBusy" @click="confirmClassify">
            {{ t('factories.classify.confirmRun') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sec { margin: 1.4rem 0 0.4rem; font-size: 1.05rem; }
.layout { display: flex; gap: 0.9rem; margin-top: 0.9rem; flex-wrap: wrap; align-items: flex-start; }
.left { display: flex; flex-direction: column; gap: 0.9rem; flex: 0 0 19rem; min-width: 16rem; }
.card {
  border: 1px solid var(--border); border-radius: var(--radius); background: var(--surface);
  padding: 0.9rem; display: flex; flex-direction: column; gap: 0.5rem;
}
.card.auto { border-color: color-mix(in srgb, var(--accent) 35%, var(--border)); }
.card.hovered { border: 2px solid var(--accent); background: color-mix(in srgb, var(--accent) 8%, var(--surface)); }
.card.busy { border: 2px solid var(--danger); }
.card.work { flex: 1 1 22rem; min-height: 13rem; }
.cardhead { display: flex; align-items: center; gap: 0.6rem; }
.min { display: flex; flex-direction: column; min-width: 0; flex: 1; }
.cardtitle { font-weight: 600; }
.badge, .chipbadge {
  display: inline-flex; align-items: center; justify-content: center;
  width: 1.7rem; height: 1.7rem; border-radius: 0.45rem; flex: 0 0 auto;
  background: color-mix(in srgb, var(--accent) 12%, var(--surface));
  color: var(--accent); font-weight: 700; font-size: 0.85rem;
}
.badge.auto { font-size: 1rem; }
.dz {
  border: 1px dashed var(--border); border-radius: 0.5rem; background: transparent;
  padding: 1.1rem 0.75rem; cursor: pointer; color: var(--muted); font-size: 0.9rem;
}
.dz:hover:not(:disabled) { border-color: var(--accent); color: var(--text); background: color-mix(in srgb, var(--accent) 6%, var(--surface)); }
.dz:disabled { opacity: 0.55; cursor: wait; }
.dz.grow { flex: 1; }
.chips { display: flex; flex-wrap: wrap; gap: 0.35rem; border: 1px solid var(--border); border-radius: var(--radius); background: var(--surface); padding: 0.7rem; }
.chip {
  display: inline-flex; align-items: center; gap: 0.35rem; border: 1px solid var(--border);
  border-radius: 999px; padding: 0.2rem 0.65rem 0.2rem 0.25rem; background: transparent;
  color: var(--muted); font-size: 0.78rem; cursor: pointer;
}
.chip:hover { color: var(--text); background: color-mix(in srgb, var(--accent) 7%, var(--surface)); }
.chip.active { border-color: var(--accent); color: var(--text); background: color-mix(in srgb, var(--accent) 12%, var(--surface)); }
.chipbadge { width: 1.25rem; height: 1.25rem; font-size: 0.68rem; border-radius: 999px; }
.iconbtn {
  width: 1.7rem; height: 1.7rem; border: 1px solid var(--border); border-radius: 0.45rem;
  background: transparent; color: var(--muted); font-size: 1.05rem; line-height: 1; cursor: pointer; flex: 0 0 auto;
}
.iconbtn:hover { color: var(--text); background: color-mix(in srgb, var(--accent) 10%, var(--surface)); }
.nograph {
  margin-left: 0.3rem; border-radius: 0.25rem; padding: 0.05rem 0.35rem;
  background: color-mix(in srgb, var(--danger) 12%, var(--surface));
  color: var(--danger); font-size: 0.68rem; font-weight: 400;
}
.skipped { margin-top: 0.8rem; padding: 0.6rem 0.8rem; border: 1px solid var(--border); border-left: 3px solid var(--danger); border-radius: 0.4rem; background: var(--surface); font-size: 0.82rem; color: var(--muted); }
.targetbar {
  display: flex; align-items: center; gap: 0.45rem; flex-wrap: wrap;
  margin-top: 0.9rem; padding: 0.55rem 0.8rem;
  border: 1px solid var(--border); border-radius: var(--radius); background: var(--surface);
}
.targetlabel { font-weight: 600; font-size: 0.85rem; }
.hinttext { flex: 1 1 12rem; min-width: 10rem; }
.panel { margin-top: 1rem; padding: 1rem; border: 1px solid var(--border); border-radius: var(--radius); background: var(--surface); display: flex; flex-direction: column; gap: 0.55rem; }
.sumrow { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
.sumok { font-weight: 600; color: var(--ok); font-size: 0.92rem; }
.perrors { padding: 0.5rem 0.7rem; border-radius: 0.4rem; background: color-mix(in srgb, var(--danger) 8%, var(--surface)); color: var(--danger); font-size: 0.8rem; }
.filelist { border: 1px solid var(--border); border-radius: 0.5rem; overflow: hidden; }
.filerow { display: flex; width: 100%; align-items: center; gap: 0.5rem; padding: 0.45rem 0.7rem; background: transparent; border: 0; border-bottom: 1px solid var(--border); font-size: 0.8rem; text-align: left; cursor: pointer; }
.filerow:last-child { border-bottom: 0; }
.filerow:hover { background: color-mix(in srgb, var(--accent) 6%, var(--surface)); }
.ok { color: var(--ok); } .bad { color: var(--danger); }
.rowwrap { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
.rowbetween { display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; flex-wrap: wrap; }
.linkbtn { background: none; border: 0; color: var(--muted); cursor: pointer; font-size: 0.8rem; padding: 0; }
.linkbtn:hover { color: var(--text); }
.sel { padding: 0.25rem 0.45rem; border: 1px solid var(--border); border-radius: 0.4rem; background: var(--surface); color: var(--text); font-size: 0.8rem; }
.circleinput { width: 9rem; padding: 0.25rem 0.45rem; border: 1px solid var(--border); border-radius: 0.4rem; background: var(--surface); font-size: 0.8rem; }
.mdedit { width: 100%; min-height: 16rem; resize: vertical; border: 1px solid var(--border); border-radius: 0.5rem; background: var(--surface); color: var(--text); padding: 0.7rem; font-family: ui-monospace, monospace; font-size: 0.8rem; line-height: 1.55; }
.mdedit.grow { flex: 1; min-height: 18rem; }
.okline { color: var(--ok); font-size: 0.82rem; }
.emptyline { margin-top: 1rem; }
.mb2 { margin-bottom: 0.5rem; }
.mono { font-family: ui-monospace, monospace; }
.truncate { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.grow { flex: 1; }
.small { font-size: 0.8rem; }
.oktext { color: var(--ok); margin-left: 0.4rem; font-size: 0.78rem; }
.warntext { color: var(--danger); margin-left: 0.4rem; font-size: 0.78rem; }
.modal { position: fixed; inset: 0; z-index: 40; display: flex; align-items: center; justify-content: center; background: rgba(0, 0, 0, 0.55); padding: 1rem; }
.dialog { display: flex; flex-direction: column; width: 100%; max-width: 46rem; max-height: 88vh; border: 1px solid var(--border); border-radius: var(--radius); background: var(--surface); box-shadow: 0 18px 45px rgba(0, 0, 0, 0.25); }
.dialog.narrow { max-width: 36rem; }
.dialoghead { display: flex; align-items: center; justify-content: space-between; padding: 0.7rem 1.1rem; border-bottom: 1px solid var(--border); }
.dialoghint { padding: 0.5rem 1.1rem; border-bottom: 1px solid var(--border); font-size: 0.78rem; color: var(--muted); }
.targetrow { display: flex; align-items: center; gap: 0.45rem; flex-wrap: wrap; padding: 0.55rem 1.1rem 0.2rem; }
.dialog .targetrow + .dialoghint { border-bottom: 1px solid var(--border); }
.dialogfoot { display: flex; align-items: center; justify-content: flex-end; gap: 0.5rem; padding: 0.75rem 1.1rem; border-top: 1px solid var(--border); }
.confirmlist { flex: 1; overflow: auto; padding: 0.75rem 1.1rem; display: flex; flex-direction: column; gap: 0.5rem; }
.confirmrow { display: flex; align-items: center; gap: 0.6rem; border: 1px solid var(--border); border-radius: 0.5rem; padding: 0.5rem 0.7rem; }
.hintwarn { margin-top: 0.8rem; padding: 0.6rem 0.8rem; border: 1px solid color-mix(in srgb, var(--danger) 35%, var(--border)); border-radius: 0.5rem; background: color-mix(in srgb, var(--danger) 7%, var(--surface)); color: var(--danger); font-size: 0.85rem; }
button.primary { width: auto; padding: 0.45rem 1rem; }
button.ghost { border: 0; background: transparent; color: var(--muted); cursor: pointer; font-size: 0.85rem; padding: 0.45rem 0.7rem; }
button.ghost:hover { color: var(--text); }
button.primary:disabled { opacity: 0.55; cursor: wait; }
</style>
