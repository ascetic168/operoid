import { Channel, invoke } from "@tauri-apps/api/core";
import i18n from "@/i18n";

/** Rust `L10n`（代碼 + 具名參數；對應 src-tauri/src/i18n.rs）。 */
export interface L10n {
  code: string;
  params?: Record<string, string>;
}

/** 寬鬆的 t：Rust 代碼是動態字串，繞過 vue-i18n 的嚴格 key 型別檢查。 */
type LooseT = (key: string, params?: Record<string, unknown>) => string;
const gt: LooseT = i18n.global.t as LooseT;

/** 把 Rust `L10n` 翻成當前語言字串。null/undefined → 空字串。 */
export function tL10n(m: L10n | null | undefined): string {
  if (!m) return "";
  return m.params ? gt(m.code, m.params) : gt(m.code);
}

/**
 * 統一格式化 invoke 拒絕值。Rust `AppError` 序列化為 `{code,params}` → 用 vue-i18n 翻譯；
 * 其餘（舊式字串、JS 錯誤）→ `String(e)`。
 */
export function formatError(e: unknown): string {
  if (
    e &&
    typeof e === "object" &&
    "code" in e &&
    typeof (e as { code: unknown }).code === "string"
  ) {
    const { code, params } = e as L10n;
    return params ? gt(code, params) : gt(code);
  }
  return String(e);
}

/** Rust 端 AppInfo（對應 src-tauri/src/lib.rs 的 AppInfo）。 */
export interface AppInfo {
  name: string;
  version: string;
  gbrain_home: string;
  notes_repo_default: string;
  gbrain_exe_default: string;
}

export const appInfo = (): Promise<AppInfo> => invoke<AppInfo>("app_info");
export const ping = (): Promise<string> => invoke<string>("ping");

// ---- Prerequisites check ----

export interface DepStatus {
  name: string;
  available: boolean;
  /** 版本字串（語言中性）；找不到時為 null。 */
  detail: string | null;
  /** 安裝說明（L10n 代碼）。 */
  install_hint: L10n;
  url: string;
}

export const checkPrerequisites = (): Promise<DepStatus[]> =>
  invoke<DepStatus[]>("check_prerequisites");

/** 用系統預設瀏覽器開 URL(tauri-plugin-shell open;需 shell:allow-open)。 */
export async function openUrl(url: string): Promise<void> {
  const { open } = await import("@tauri-apps/plugin-shell");
  await open(url);
}

// ---- Config ----

export interface LlmEndpoint {
  provider: string;
  model: string;
  base_url: string;
  has_api_key: boolean;
}

export interface GBrainConfigView {
  home: string;
  config_path: string;
  exists: boolean;
  raw: unknown;
  chat_model: string | null;
  /** models.default（file-plane 殘值；真正生效值在 DB plane，見 tiers）。 */
  models_default: string | null;
  embedding_model: string | null;
  embedding_dimensions: number | null;
  schema_pack: string | null;
  engine: string | null;
  database_path: string | null;
  provider_base_urls: Record<string, string>;
  /** v0.42 tier 路由：四層各自的有效模型（DB-plane 優先，否則 file/default）。 */
  tiers: TierModelsView;
  /** 每個 tier 的來源："db" | "file" | "default"。 */
  tier_source: TierSourceView;
  /** DB plane 正在蓋過 file plane 的 model/tier 鍵（前端亮警告）。 */
  db_overrides: string[];
  llm_endpoint: LlmEndpoint | null;
  llm_error: L10n | null;
}

/** 四個 tier 的有效模型值。 */
export interface TierModelsView {
  utility: string | null;
  reasoning: string | null;
  deep: string | null;
  subagent: string | null;
}

/** 每個 tier 的來源標記（"db" | "file" | "default"）。 */
export interface TierSourceView {
  utility: string;
  reasoning: string;
  deep: string;
  subagent: string;
}

export interface AppConfig {
  notes_repo_path: string;
  gbrain_exe_path: string;
  gbrain_home_override: string | null;
  brains: BrainEntry[];
  active_brain_id: string | null;
  active_source_id: string | null;
  auto_sync: boolean;
  sync_no_pull: boolean;
  llm_temperature: number;
  llm_max_tokens: number;
  locale: string | null;
  recent_claude_cwds: string[];
  claude_terminal: string | null;
  claude_terminal_template: string | null;
  agent_os_enabled: boolean;
}

export const getGbrainConfig = (): Promise<GBrainConfigView> =>
  invoke<GBrainConfigView>("get_gbrain_config");
export const saveGbrainConfigRaw = (raw: unknown): Promise<void> =>
  invoke<void>("save_gbrain_config_raw", { rawJson: raw });

/** 設單一 model/tier 鍵（走 DB plane via gbrain config set）。 */
export const setGbrainModel = (key: string, value: string): Promise<void> =>
  invoke<void>("set_gbrain_model", { key, value });
/** 單一模型同步到全部 tier + chat_model + models.default/think。 */
export const setGbrainModelsAll = (model: string): Promise<void> =>
  invoke<void>("set_gbrain_models_all", { model });
/** 從 DB plane 移除單一 model/tier 鍵（讓 file/default 生效）。 */
export const unsetGbrainModel = (key: string): Promise<void> =>
  invoke<void>("unset_gbrain_model", { key });
/** 清除所有 DB-plane model/tier 覆寫（修復用）。 */
export const clearDbOverrides = (): Promise<void> =>
  invoke<void>("clear_db_overrides");
/** 設 provider_base_url（直寫檔案；base_url=null 移除覆寫）。 */
export const setProviderBaseUrl = (
  provider: string,
  baseUrl: string | null,
): Promise<void> => invoke<void>("set_provider_base_url", { provider, baseUrl });

export const getAppConfig = (): Promise<AppConfig> => invoke<AppConfig>("get_app_config");
export const saveAppConfig = (config: AppConfig): Promise<void> =>
  invoke<void>("save_app_config", { config });

/** 內建終端機 profile（已偵測可用性）。 */
export interface TerminalInfo {
  id: string;
  label: string;
  available: boolean;
}
/** `claude_code_status` 回傳：claude/gbrain 就緒狀態 + 可用終端機清單。 */
export interface ClaudeStatus {
  claude_installed: boolean;
  claude_version: string | null;
  gbrain_exe: string;
  gbrain_ready: boolean;
  terminals: TerminalInfo[];
}
export const claudeCodeStatus = (): Promise<ClaudeStatus> =>
  invoke<ClaudeStatus>("claude_code_status");
/** 以所選腦 + cwd + 終端機啟動 Claude Code（帶 gbrain MCP）。terminal="custom" 時用 template。 */
export const claudeCodeLaunch = (
  brainId: string | null,
  cwd: string,
  terminal: string | null,
  template: string | null,
): Promise<void> =>
  invoke<void>("claude_code_launch", { brainId, cwd, terminal, template });
/** 設定介面語言覆寫（null = 回到自動偵測）。回傳實際生效的 locale。 */
export const setLocale = (locale: string | null): Promise<string | null> =>
  invoke<string | null>("set_locale", { locale });

// ---- Operations (gbrain CLI, streamed via Channel) ----

export interface CliLine {
  stream: string; // "stdout" | "stderr" | "step"
  text: string;
}

export interface OpResult {
  success: boolean;
  exit_code: number | null;
  note: L10n | null;
}

export type OpName =
  | "stats"
  | "sync"
  | "extract"
  | "embed"
  | "ask"
  | "think"
  | "doctor"
  | "orphans"
  | "storage"
  | "graph-query";

/** 跑一個 gbrain 操作，逐行串流到 onLine；Promise 解析為最終結果。 */
export async function runOp(
  op: OpName,
  arg: string | null,
  onLine: (line: CliLine) => void,
): Promise<OpResult> {
  const ch = new Channel<CliLine>();
  ch.onmessage = onLine;
  return invoke<OpResult>("op_run", { onEvent: ch, op, arg });
}

// ---- Factories (drag-drop → convert → preview → write) ----

export interface PreviewPage {
  slug: string;
  target_dir: string;
  name: string;
  markdown: string;
}

/** 一個輸入檔的處理結果(檔案層級)。前端 >1 檔時顯示清單。 */
export interface ProcessedFile {
  path: string;
  ok: boolean;
  message: L10n | null;
  pages: PreviewPage[];
}

export interface PreviewResult {
  factory: string;
  summary: L10n;
  sample: PreviewPage[];
  total: number;
  written: string[];
  errors: L10n[];
  files: ProcessedFile[];
}

export interface WritePage {
  slug: string;
  target_dir: string;
  markdown: string;
}

export interface WriteResult {
  written: string[];
  errors: L10n[];
  note: L10n | null;
}

/**
 * 工廠／自動分類目標。輸出目錄寫死（people/companies/meetings/concepts/projects，
 * inbox 走 gbrain capture 不寫檔）。v0.42 起 gbrain 的 DIR_PATTERN 不再是丟棄閘
 * （#2576），非白名單目錄也能成邊，故目錄名不再需要可配置。
 */
export type Factory = "people" | "companies" | "meeting" | "inbox" | "concepts" | "projects";

/** 轉換 + 立即寫入 + 回傳預覽。target_repo=來源 repo 路徑（未給則用 app notes_repo_path）。 */
export const factoryRun = (
  factory: Factory,
  paths: string[],
  targetRepo: string | null,
): Promise<PreviewResult> =>
  invoke<PreviewResult>("factory_run", { factory, paths, targetRepo });

/** `factory_open_dir` 回傳：以什麼方式開啟了目錄。 */
export interface OpenDirResult {
  /** "vscode" | "filemanager" */
  opened_with: string;
  /** 開啟的目錄絕對路徑 */
  path: string;
}
/** 點工廠卡圖示：以 VS Code 開該工廠目錄；沒裝則以系統預設檔案管理員開啟。inbox 開 GBRAIN_HOME。 */
export const factoryOpenDir = (
  factory: Factory,
  targetRepo: string | null,
): Promise<OpenDirResult> =>
  invoke<OpenDirResult>("factory_open_dir", { factory, targetRepo });
/** 覆蓋寫入(預覽後編輯過的頁面)。 */
export const factoryWritePages = (
  pages: WritePage[],
  targetRepo: string | null,
): Promise<WriteResult> =>
  invoke<WriteResult>("factory_write_pages", { pages, targetRepo });
export const extractCompaniesRun = (
  clean: boolean,
  targetRepo: string | null,
): Promise<WriteResult> =>
  invoke<WriteResult>("extract_companies_run", { clean, targetRepo });

export interface AuthoredResult {
  slug: string;
  target_dir: string;
  path: string;
  used_fallback: boolean;
  enriched_markdown: string;
  names_count: number;
  enriched: boolean;
}

/** 手寫編輯器存檔:首次用 title 當檔名,之後覆蓋同檔。 */
export const factorySaveAuthored = (
  factory: Factory,
  markdown: string,
  existingSlug: string | null,
  targetRepo: string | null,
): Promise<AuthoredResult> =>
  invoke<AuthoredResult>("factory_save_authored", {
    factory,
    markdown,
    existingSlug,
    targetRepo,
  });

// ---- Factory auto-classify（統一入口：丟任意檔 → 程式判斷歸屬）----

export type Confidence = "high" | "medium" | "low";
export type ClassifySource = "extension" | "heuristic" | "llm";

/** Rust `FileClassification`（對應 src-tauri/src/classifier.rs）。factory 空字串 = 不支援。 */
export interface FileClassification {
  path: string;
  factory: string;
  confidence: Confidence;
  reason: string;
  source: ClassifySource;
}

/** 逐檔判斷歸屬工廠（副檔名/特徵優先，模糊才用 LLM；無 key 退回純規則）。 */
export const factoryClassify = (paths: string[]): Promise<FileClassification[]> =>
  invoke<FileClassification[]>("factory_classify", { paths });

// ---- Brains management (多腦 + 每腦多來源) ----

export const DEFAULT_BRAIN_ID = "__default__";

export interface BrainEntry {
  id: string;
  name: string;
  gbrain_home: string | null; // null = 預設腦(~/.gbrain)
}

export interface BrainsList {
  brains: BrainEntry[];
  active_id: string | null;
  active_dot_gbrain: string | null;
}

export interface GbrainSource {
  id: string;
  name: string;
  /** federated 或剛建立、尚未綁定本地目錄的來源為 null */
  local_path: string | null;
  federated: boolean;
  page_count: number;
  last_sync_at: string | null;
}

export interface AddBrainReq {
  name: string;
  gbrain_home: string | null;
  create: boolean;
  embedding_model?: string;
  embedding_dimensions?: number;
  chat_model?: string;
}

export const brainsList = (): Promise<BrainsList> => invoke<BrainsList>("brains_list");
export const brainsAdd = (req: AddBrainReq): Promise<BrainEntry> =>
  invoke<BrainEntry>("brains_add", { req });
export const brainsRemove = (id: string): Promise<void> => invoke<void>("brains_remove", { id });
export const brainsSetActive = (id: string): Promise<void> =>
  invoke<void>("brains_set_active", { id });
export const brainsSetActiveSource = (sourceId: string | null): Promise<void> =>
  invoke<void>("brains_set_active_source", { sourceId: sourceId });
export const brainSources = (brainId: string): Promise<GbrainSource[]> =>
  invoke<GbrainSource[]>("brain_sources", { brainId });
export const brainSourceAdd = (
  brainId: string,
  sourceId: string,
  path: string,
): Promise<void> =>
  invoke<void>("brain_source_add", { req: { brain_id: brainId, source_id: sourceId, path } });
export const brainSourceRemove = (brainId: string, sourceId: string): Promise<void> =>
  invoke<void>("brain_source_remove", { req: { brain_id: brainId, source_id: sourceId } });

// ---- Note view（點擊 wikilink → 該 .md 轉 HTML 用預設瀏覽器開啟）----

export interface NoteViewResult {
  title: string;
}

/** 把 wikilink 指向的筆記轉成 HTML 並以系統預設瀏覽器開啟。
 *  `target` 為 `[[...]]` 內文（如 `people/JLin` 或 `people/JLin|JLin`）。 */
export const openNote = (target: string): Promise<NoteViewResult> =>
  invoke<NoteViewResult>("open_note", { target });

/** 同步某腦：scope "all" | "one"（one 需 sourceId）。逐行串流。 */
export async function brainSync(
  brainId: string,
  scope: "all" | "one",
  sourceId: string | null,
  onLine: (line: CliLine) => void,
): Promise<OpResult> {
  const ch = new Channel<CliLine>();
  ch.onmessage = onLine;
  return invoke<OpResult>("brain_sync", {
    onEvent: ch,
    brainId,
    scope,
    sourceId: sourceId,
  });
}

/** 綁定 default 來源路徑：確保 path 是 git repo（自動 init）→ gbrain sync --repo 綁定。 */
export async function brainBindSourcePath(
  brainId: string,
  path: string,
  onLine: (line: CliLine) => void,
): Promise<OpResult> {
  const ch = new Channel<CliLine>();
  ch.onmessage = onLine;
  return invoke<OpResult>("brain_bind_source_path", { onEvent: ch, brainId, path });
}

// ───────────────── Agent-OS（員工模板／實體管理）─────────────────

/** 預設 workspace（GUI 用；後端 `agent_ensure_workspace` 確保存在）。 */
export const AGENT_WS = "ws-default";

export interface BrainRef {
  brain_id: string;
}
export type EmployeeState =
  | "created" | "idle" | "working" | "waiting" | "sleeping" | "paused" | "error";

export interface EmployeeTemplate {
  id: string;
  workspace_id: string;
  name: string;
  brain: BrainRef;
  role: string | null;
  created_at: string;
}

export interface Employee {
  id: string;
  workspace_id: string;
  name: string;
  brain: BrainRef;
  role: string | null;
  template_id: string | null;
  state: EmployeeState;
  created_at: string;
}

export interface IdResult {
  /** `{ template_id }` / `{ employee_id }` / `{ workspace_id }` */
  [key: string]: string;
}

export const agentEnsureWorkspace = (): Promise<{ workspace_id: string }> =>
  invoke<{ workspace_id: string }>("agent_ensure_workspace");
export const agentListTemplates = (workspaceId: string = AGENT_WS): Promise<EmployeeTemplate[]> =>
  invoke<EmployeeTemplate[]>("agent_list_templates", { workspaceId });
export const agentListEmployees = (workspaceId: string = AGENT_WS): Promise<Employee[]> =>
  invoke<Employee[]>("agent_list_employees", { workspaceId });
export const agentCreateTemplate = (
  name: string,
  brainId: string | null,
  role: string | null,
  workspaceId: string = AGENT_WS,
): Promise<{ template_id: string }> =>
  invoke<{ template_id: string }>("agent_create_template", { workspaceId, name, brainId, role });
export const agentDeployInstance = (
  templateId: string,
  instanceName: string,
): Promise<{ employee_id: string }> =>
  invoke<{ employee_id: string }>("agent_deploy_instance", { templateId, instanceName });
export const agentDeleteTemplate = (templateId: string): Promise<void> =>
  invoke<void>("agent_delete_template", { templateId });
export const agentDeleteEmployee = (employeeId: string): Promise<void> =>
  invoke<void>("agent_delete_employee", { employeeId });
export const agentRenameTemplate = (templateId: string, name: string): Promise<void> =>
  invoke<void>("agent_rename_template", { templateId, name });
export const agentRenameEmployee = (employeeId: string, name: string): Promise<void> =>
  invoke<void>("agent_rename_employee", { employeeId, name });
export const agentSendMessage = (
  employeeId: string,
  text: string,
  commitmentId: string | null = null,
): Promise<{ task_id: string }> =>
  invoke<{ task_id: string }>("agent_send_message", { employeeId, text, commitmentId });
export const agentClearMessages = (employeeId: string): Promise<void> =>
  invoke<void>("agent_clear_messages", { employeeId });
export interface WatchSnapshot {
  employee: Employee;
  llm_model: string | null;
  commitments: unknown[];
  proposals: { id: string; title: string; completion_condition: string; status: string }[];
  tasks: unknown[];
  artifacts: unknown[];
  memory: { notes: string[]; last_artifact_id: string | null } | null;
  events: { id: string; kind: string; detail: string; created_at: string }[];
  messages: {
    id: string;
    direction: "in" | "out";
    text: string;
    commitment_id: string | null;
    artifact_id: string | null;
    created_at: string;
  }[];
}
export const agentWatch = (employeeId: string): Promise<WatchSnapshot> =>
  invoke<WatchSnapshot>("agent_watch", { employeeId });
export const agentCreateCommitment = (
  employeeId: string,
  title: string,
  completionCondition: string,
): Promise<{ commitment_id: string }> =>
  invoke<{ commitment_id: string }>("agent_create_commitment", {
    employeeId,
    title,
    completionCondition,
  });
export const agentSatisfyCommitment = (commitmentId: string): Promise<void> =>
  invoke<void>("agent_satisfy_commitment", { commitmentId });
export const agentApproveCommitment = (commitmentId: string): Promise<void> =>
  invoke<void>("agent_approve_commitment", { commitmentId });
export const agentRejectCommitment = (commitmentId: string): Promise<void> =>
  invoke<void>("agent_reject_commitment", { commitmentId });
