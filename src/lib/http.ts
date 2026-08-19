/**
 * oserver HTTP 客戶端（P3 前後端分離）——agent-os API 的傳輸層。
 *
 * - 認證：Bearer token（`server_info` 指令取得，本機桌面殼注入——token 不出本機）。
 * - 錯誤形狀：服務端錯誤體 `{code, params}` 以**同形狀物件 reject**——
 *   與 invoke 的 AppError 一致，stores 的既有處理（String(e)／e.code）零改動。
 * - 斷線語意：`server_info` 失敗或 fetch 連不上 → reject `{code: "server.offline"}`。
 */
import { invoke } from "@tauri-apps/api/core";

export interface ServerInfo {
  port: number;
  token: string | null;
  running: boolean;
}

let cached: ServerInfo | null = null;

/** 取得本地服務資訊（殼層指令；快取）。失敗 → server.offline。 */
export async function serverInfo(): Promise<ServerInfo> {
  if (cached) return cached;
  try {
    cached = await invoke<ServerInfo>("server_info");
    return cached;
  } catch {
    throw { code: "server.offline" };
  }
}

/** 供測試／重連時清除快取。 */
export function clearServerInfoCache(): void {
  cached = null;
}

/** agent-os API 呼叫（GET/POST/...；path 以 /api 開頭）。`body` 為 JSON 化前的值。 */
export interface AgentFetchInit {
  method?: string;
  body?: unknown;
}

export async function agentFetch<T>(
  path: string,
  init?: AgentFetchInit,
): Promise<T> {
  const info = await serverInfo();
  if (!info.token) {
    throw { code: "server.offline" };
  }
  let resp: Response;
  try {
    const { body, method } = init ?? {};
    resp = await fetch(`http://127.0.0.1:${info.port}${path}`, {
      method,
      headers: {
        Authorization: `Bearer ${info.token}`,
        ...(body !== undefined ? { "Content-Type": "application/json" } : {}),
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  } catch {
    // fetch 網路層失敗（服務未啟動／中途退出）
    clearServerInfoCache();
    throw { code: "server.offline" };
  }
  if (!resp.ok) {
    // 錯誤體 {code, params} 同形狀 reject（與 invoke AppError 一致）
    let err: unknown = { code: "server.httpError", params: { status: String(resp.status) } };
    try {
      err = await resp.json();
    } catch {
      /* 保持 fallback 形狀 */
    }
    throw err;
  }
  if (resp.status === 204) return undefined as T;
  return (await resp.json()) as T;
}
