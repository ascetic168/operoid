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
  let resp: Response | null = null;
  // 啟動競態緩解：GUI 剛帶起 oserver 時首次連線可能被拒——1s 後重試一次。
  for (let attempt = 0; attempt < 2; attempt++) {
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
      break;
    } catch {
      if (attempt === 0) {
        await new Promise((r) => setTimeout(r, 1000));
        continue;
      }
      // 重試仍失敗：服務真的不在（未啟動／中途退出）
      clearServerInfoCache();
      throw { code: "server.offline" };
    }
  }
  if (!resp) {
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

// ───────────────── F1 串流：員工級回合 delta 流（SSE） ─────────────────

/** 對話回合的即時 delta（SSE `event: delta`；對齊 ocore `TurnDelta` serde 標籤）。
 *  best-effort 契約：最終 Message{Out}（輪詢）才是事實源，delta 丟失僅觀感問題。
 *  v0.5.8 過程幀：thinking_delta（模型思考流增量）、tool_call_start/end（工具呼叫
 *  進行中→完成對，args 截 400 字）——客戶端做單行流動 ticker 與錯誤自動展開。 */
export type TurnDelta =
  | { kind: "step_start"; employee_id: string; task_id: string; step: number }
  | { kind: "text_delta"; employee_id: string; task_id: string; step: number; text: string }
  | {
      kind: "text_end";
      employee_id: string;
      task_id: string;
      step: number;
      final_reply: boolean;
    }
  | { kind: "thinking_delta"; employee_id: string; task_id: string; step: number; text: string }
  | {
      kind: "tool_call_start";
      employee_id: string;
      task_id: string;
      step: number;
      name: string;
      args: string;
    }
  | {
      kind: "tool_call_end";
      employee_id: string;
      task_id: string;
      step: number;
      name: string;
      status: string;
      ms: number;
    }
  | { kind: "turn_end"; employee_id: string; task_id: string }
  | { kind: "resync"; lost?: number };

interface TicketResponse {
  ticket: string;
  ttl_secs: number;
}

/** 員工級回合 delta 流：簽票（agentFetch 帶 Bearer）→ EventSource 連本地 oserver；
 *  斷線 3s 後以**新票**重連（服務存續期間）；401/403（token 失效）即停——交由
 *  輪詢的既有錯誤處理呈現。回傳 close()。 */
export function openTurnStream(
  employeeId: string,
  onDelta: (delta: TurnDelta) => void,
  onDown?: (err: unknown) => void,
): () => void {
  let closed = false;
  let es: EventSource | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const stop = () => {
    if (es) {
      es.close();
      es = null;
    }
  };

  const connect = async () => {
    while (!closed) {
      try {
        const info = await serverInfo();
        const { ticket } = await agentFetch<TicketResponse>("/api/stream/ticket", {
          method: "POST",
        });
        if (closed) return;
        await new Promise<void>((resolve) => {
          const url =
            `http://127.0.0.1:${info.port}/api/employees/` +
            encodeURIComponent(employeeId) +
            "/stream?ticket=" +
            ticket;
          es = new EventSource(url);
          es.addEventListener("delta", (ev) => {
            try {
              onDelta(JSON.parse((ev as MessageEvent).data) as TurnDelta);
            } catch {
              /* 非 JSON 幀：略過 */
            }
          });
          es.onerror = () => {
            stop();
            resolve();
          };
        });
      } catch (e) {
        onDown?.(e);
        // token 失效（401/403 以 {code,params} 形 reject）：停止重連——輪詢面會呈現。
        const code = (e as { code?: string } | null)?.code ?? "";
        if (code.includes("auth")) return;
      }
      if (closed) break;
      await new Promise((r) => {
        timer = setTimeout(r, 3000);
      });
    }
  };
  void connect();
  return () => {
    closed = true;
    if (timer) clearTimeout(timer);
    stop();
  };
}
