// @front/api-client——Operoid 企業前端 API 用戶端（遠端化 R4）。
// token **記憶體持有**（不落 storage——重整即重登，v1 取捨；完整方案隨 C12b 完整版）。
// 401 → 清 session＋通知監聽者（router 導回登入）；403 → ForbiddenError（頁面呈現無權限）。
// 錯誤體形狀與後端契約一致：{ code, params }。

export interface ApiErrorBody {
  code: string
  params?: Record<string, string>
}

export class ApiError extends Error {
  constructor(
    public code: string,
    public status: number,
    public params?: Record<string, string>,
  ) {
    super(code + ' (' + status + ')')
    this.name = 'ApiError'
  }
}

export class OfflineError extends Error {
  constructor() {
    super('server.offline')
    this.name = 'OfflineError'
  }
}

export interface Principal {
  id: string
  display_name: string
  roles: string[]
}

export interface LoginResponse {
  token: string
  token_id: string
  expires_at: string | null
  principal: Principal
  must_change_password: boolean
}

let baseUrl = ''
let token: string | null = null
let principal: Principal | null = null
const unauthorizedListeners = new Set<() => void>()

/** 設定伺服器位址（同源部署傳空字串即可；dev 走 vite proxy）。 */
export function configureServer(url: string): void {
  baseUrl = (url || '').replace(/\/+$/, '')
}

export function session(): { token: string; principal: Principal } | null {
  return token && principal ? { token, principal } : null
}

export function setSession(t: string, p: Principal): void {
  token = t
  principal = p
}

export function clearSession(): void {
  token = null
  principal = null
}

/** 401 時的通知（router 註冊後導回登入頁）。回傳解除函式。 */
export function onUnauthorized(fn: () => void): () => void {
  unauthorizedListeners.add(fn)
  return () => unauthorizedListeners.delete(fn)
}

/** 角色層級（鏡像伺服器端：admin ⊃ manager ⊃ user——伺服器端 403 才是真相）。 */
export function hasRole(roles: string[], required: 'user' | 'manager' | 'admin'): boolean {
  const has = (r: string) => roles.includes(r)
  if (required === 'admin') return has('admin')
  if (required === 'manager') return has('manager') || has('admin')
  return has('user') || has('manager') || has('admin')
}

async function safeBody(resp: Response): Promise<ApiErrorBody | null> {
  try {
    return (await resp.json()) as ApiErrorBody
  } catch {
    return null
  }
}

async function handle<T>(resp: Response): Promise<T> {
  if (resp.status === 401) {
    const had = token !== null
    clearSession()
    if (had) unauthorizedListeners.forEach((f) => f())
    throw new ApiError('auth.unauthorized', 401)
  }
  if (resp.status === 403) {
    const b = await safeBody(resp)
    throw new ApiError(b?.code ?? 'auth.forbidden', 403, b?.params)
  }
  if (!resp.ok) {
    const b = await safeBody(resp)
    throw new ApiError(b?.code ?? 'server.internal', resp.status, b?.params)
  }
  if (resp.status === 204) return undefined as T
  const ct = resp.headers.get('content-type') ?? ''
  if (!ct.includes('application/json')) return undefined as T
  return (await resp.json()) as T
}

async function request<T>(method: string, path: string, body?: unknown, auth = true): Promise<T> {
  let resp: Response
  try {
    resp = await fetch(baseUrl + path, {
      method,
      headers: {
        ...(body !== undefined ? { 'content-type': 'application/json' } : {}),
        ...(auth && token ? { authorization: 'Bearer ' + token } : {}),
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    })
  } catch {
    throw new OfflineError()
  }
  return handle<T>(resp)
}

/** Bearer GET → Blob（來源 PDF 等 binary 面）——401/403 紀律與 request() 一致：
 *  401 清 session 並通知監聽者（導回登入），403 丟 ApiError 交頁面呈現。 */
async function requestBlob(path: string): Promise<Blob> {
  let resp: Response
  try {
    resp = await fetch(baseUrl + path, {
      headers: token ? { authorization: 'Bearer ' + token } : {},
    })
  } catch {
    throw new OfflineError()
  }
  if (resp.status === 401) {
    const had = token !== null
    clearSession()
    if (had) unauthorizedListeners.forEach((f) => f())
    throw new ApiError('auth.unauthorized', 401)
  }
  if (!resp.ok) {
    const b = await safeBody(resp)
    if (resp.status === 403) throw new ApiError(b?.code ?? 'auth.forbidden', 403, b?.params)
    throw new ApiError(b?.code ?? 'server.internal', resp.status, b?.params)
  }
  return resp.blob()
}

/** multipart 檔案上傳（工廠暫存）——瀏覽器自帶 boundary；401/403 契約與 request 一致。 */
export async function upload<T>(path: string, form: FormData): Promise<T> {
  let resp: Response
  try {
    resp = await fetch(baseUrl + path, {
      method: 'POST',
      headers: token ? { authorization: 'Bearer ' + token } : {},
      body: form,
    })
  } catch {
    throw new OfflineError()
  }
  return handle<T>(resp)
}

export const api = {
  login: async (login_name: string, password: string): Promise<LoginResponse> => {
    const r = await request<LoginResponse>('POST', '/api/auth/login', { login_name, password }, false)
    setSession(r.token, r.principal)
    return r
  },
  logout: async (): Promise<void> => {
    try {
      await request('POST', '/api/auth/logout')
    } finally {
      clearSession()
    }
  },
  /** 輪替換發 24h token（沿用記憶體中的 principal）。 */
  refresh: async (): Promise<void> => {
    const r = await request<{ token: string; token_id: string; expires_at: string | null }>(
      'POST',
      '/api/auth/refresh',
    )
    if (principal) setSession(r.token, principal)
  },
  changePassword: (old_password: string, new_password: string) =>
    request('POST', '/api/auth/password', { old_password, new_password }),
  streamTicket: () => request<{ ticket: string; ttl_secs: number }>('POST', '/api/stream/ticket'),
  get: <T>(path: string) => request<T>('GET', path),
  post: <T>(path: string, body?: unknown) => request<T>('POST', path, body),
  put: <T>(path: string, body?: unknown) => request<T>('PUT', path, body),
  patch: <T>(path: string, body?: unknown) => request<T>('PATCH', path, body),
  del: <T>(path: string) => request<T>('DELETE', path),
  getBlob: (path: string) => requestBlob(path),
  upload: <T>(path: string, form: FormData) => upload<T>(path, form),
}

export interface StreamEvent {
  id: string
  employee_id: string
  kind: string
  detail: string
  created_at: string
}

/** 對話回合的即時 delta（SSE `event: delta`；對齊 ocore `TurnDelta` serde 標籤）。
 *  best-effort 契約：最終 Message{Out}（輪詢）才是事實源，delta 丟失僅觀感問題。
 *  v0.5.8 過程幀：thinking_delta（模型思考流增量）、tool_call_start/end（工具呼叫
 *  進行中→完成對，args 截 400 字）——客戶端做單行流動 ticker 與錯誤自動展開。 */
export type TurnDelta =
  | { kind: 'step_start'; employee_id: string; task_id: string; step: number }
  | { kind: 'text_delta'; employee_id: string; task_id: string; step: number; text: string }
  | {
      kind: 'text_end'
      employee_id: string
      task_id: string
      step: number
      final_reply: boolean
    }
  | { kind: 'thinking_delta'; employee_id: string; task_id: string; step: number; text: string }
  | {
      kind: 'tool_call_start'
      employee_id: string
      task_id: string
      step: number
      name: string
      args: string
    }
  | {
      kind: 'tool_call_end'
      employee_id: string
      task_id: string
      step: number
      name: string
      status: string
      ms: number
    }
  | { kind: 'turn_end'; employee_id: string; task_id: string }
  | { kind: 'resync'; lost?: number }

/** 短票 SSE 泛用骨幹：簽票→EventSource→斷線 3s 後以**新票**重連（session 存續期間）；
 *  401/403（session 失效）即停。回傳 close()。 */
function openTicketedStream(
  path: string,
  eventName: string,
  onData: (data: string) => void,
  onDown?: (err: unknown) => void,
): () => void {
  let closed = false
  let es: EventSource | null = null
  let timer: ReturnType<typeof setTimeout> | null = null

  const stop = () => {
    if (es) {
      es.close()
      es = null
    }
  }

  const connect = async () => {
    while (!closed) {
      try {
        const { ticket } = await api.streamTicket()
        if (closed) return
        await new Promise<void>((resolve) => {
          es = new EventSource(baseUrl + path + '?ticket=' + ticket)
          es.addEventListener(eventName, (ev) => {
            onData((ev as MessageEvent).data as string)
          })
          es.onerror = () => {
            stop()
            resolve()
          }
        })
      } catch (e) {
        onDown?.(e)
        if (e instanceof ApiError && (e.status === 401 || e.status === 403)) return
      }
      if (closed) break
      await new Promise((r) => {
        timer = setTimeout(r, 3000)
      })
    }
  }
  void connect()
  return () => {
    closed = true
    if (timer) clearTimeout(timer)
    stop()
  }
}

/** SSE 事件流（EventsView/Home 刷新面）：短票換流；斷線自動以**新票**重連。回傳 close()。 */
export function openEventStream(
  onEvent: (payload: StreamEvent) => void,
  onDown?: (err: unknown) => void,
): () => void {
  return openTicketedStream(
    '/api/stream',
    'event',
    (data) => {
      try {
        onEvent(JSON.parse(data) as StreamEvent)
      } catch {
        onDown?.(data)
      }
    },
    onDown,
  )
}

/** 員工級回合 delta 流（F1 串流）：訂閱 `/api/employees/{id}/stream`（event: delta）。
 *  解析失敗的幀靜默略過（心跳／未知 kind 前向相容）。回傳 close()。 */
export function openTurnStream(
  employeeId: string,
  onDelta: (delta: TurnDelta) => void,
  onDown?: (err: unknown) => void,
): () => void {
  return openTicketedStream(
    '/api/employees/' + encodeURIComponent(employeeId) + '/stream',
    'delta',
    (data) => {
      try {
        onDelta(JSON.parse(data) as TurnDelta)
      } catch {
        /* 非 JSON 幀：略過 */
      }
    },
    onDown,
  )
}
