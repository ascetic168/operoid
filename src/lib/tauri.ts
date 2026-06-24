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
  embedding_model: string | null;
  embedding_dimensions: number | null;
  schema_pack: string | null;
  engine: string | null;
  database_path: string | null;
  provider_base_urls: Record<string, string>;
  llm_endpoint: LlmEndpoint | null;
  llm_error: L10n | null;
}

export interface FactoryTargets {
  people: string;
  companies: string;
  meetings: string;
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
  factory_targets: FactoryTargets;
  llm_temperature: number;
  llm_max_tokens: number;
  locale: string | null;
}

export const getGbrainConfig = (): Promise<GBrainConfigView> =>
  invoke<GBrainConfigView>("get_gbrain_config");
export const saveGbrainConfigRaw = (raw: unknown): Promise<void> =>
  invoke<void>("save_gbrain_config_raw", { rawJson: raw });
export const getAppConfig = (): Promise<AppConfig> => invoke<AppConfig>("get_app_config");
export const saveAppConfig = (config: AppConfig): Promise<void> =>
  invoke<void>("save_app_config", { config });
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

export interface PreviewResult {
  factory: string;
  summary: L10n;
  sample: PreviewPage[];
  total: number;
  written: string[];
  errors: L10n[];
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

export type Factory = "people" | "companies" | "meeting" | "inbox";

/** 轉換 + 立即寫入 + 回傳預覽。target_repo=來源 repo 路徑（未給則用 app notes_repo_path）。 */
export const factoryRun = (
  factory: Factory,
  paths: string[],
  targetRepo: string | null,
): Promise<PreviewResult> =>
  invoke<PreviewResult>("factory_run", { factory, paths, targetRepo });
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
  local_path: string;
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
