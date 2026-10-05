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
}

export interface StreamEvent {
  id: string
  employee_id: string
  kind: string
  detail: string
  created_at: string
}

/** SSE 事件流：短票換流；斷線自動以**新票**重連（session 存續期間）。回傳 close()。 */
export function openEventStream(
  onEvent: (payload: StreamEvent) => void,
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
          es = new EventSource(baseUrl + '/api/stream?ticket=' + ticket)
          es.addEventListener('event', (ev) => {
            try {
              onEvent(JSON.parse((ev as MessageEvent).data) as StreamEvent)
            } catch {
              onDown?.(ev)
            }
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
