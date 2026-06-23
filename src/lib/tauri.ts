import { Channel, invoke } from "@tauri-apps/api/core";

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
  llm_error: string | null;
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
  auto_sync: boolean;
  sync_no_pull: boolean;
  factory_targets: FactoryTargets;
  llm_temperature: number;
  llm_max_tokens: number;
}

export const getGbrainConfig = (): Promise<GBrainConfigView> =>
  invoke<GBrainConfigView>("get_gbrain_config");
export const saveGbrainConfigRaw = (raw: unknown): Promise<void> =>
  invoke<void>("save_gbrain_config_raw", { rawJson: raw });
export const getAppConfig = (): Promise<AppConfig> => invoke<AppConfig>("get_app_config");
export const saveAppConfig = (config: AppConfig): Promise<void> =>
  invoke<void>("save_app_config", { config });

// ---- Operations (gbrain CLI, streamed via Channel) ----

export interface CliLine {
  stream: string; // "stdout" | "stderr" | "step"
  text: string;
}

export interface OpResult {
  success: boolean;
  exit_code: number | null;
  note: string | null;
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
  summary: string;
  sample: PreviewPage[];
  total: number;
  written: string[];
  errors: string[];
}

export interface WritePage {
  slug: string;
  target_dir: string;
  markdown: string;
}

export interface WriteResult {
  written: string[];
  errors: string[];
  note: string | null;
}

export type Factory = "people" | "companies" | "meeting" | "inbox";

/** 轉換 + 立即寫入 + 回傳預覽。 */
export const factoryRun = (factory: Factory, paths: string[]): Promise<PreviewResult> =>
  invoke<PreviewResult>("factory_run", { factory, paths });
/** 覆蓋寫入(預覽後編輯過的頁面)。 */
export const factoryWritePages = (pages: WritePage[]): Promise<WriteResult> =>
  invoke<WriteResult>("factory_write_pages", { pages });
export const extractCompaniesRun = (clean: boolean): Promise<WriteResult> =>
  invoke<WriteResult>("extract_companies_run", { clean });

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
): Promise<AuthoredResult> =>
  invoke<AuthoredResult>("factory_save_authored", {
    factory,
    markdown,
    existingSlug,
  });
