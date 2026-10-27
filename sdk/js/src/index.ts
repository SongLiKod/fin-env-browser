export interface FinEnvOptions {
  baseUrl?: string;
  apiKey: string;
  fetch?: typeof fetch;
  timeoutMs?: number;
}

export interface EnvironmentSummary {
  id: string;
  name: string;
  status: string;
  project: string;
  testEnv: string;
  tags: string[];
  proxyId: string | null;
  fingerprint: Fingerprint;
  runtime: BrowserRuntime | null;
  [key: string]: unknown;
}

export interface Fingerprint {
  userAgent: string;
  platform: string;
  languages: string[];
  timezone: string;
  screen: { width: number; height: number; availWidth: number; availHeight: number; colorDepth: number; pixelRatio: number };
  canvas: { mode: string; seed: number };
  webgl: { vendor: string; renderer: string; seed: number };
  audio: { mode: string; seed: number };
  fonts: string[];
  webrtc: { mode: string; publicIp: string | null };
  clientHints: Record<string, unknown>;
  plugins: unknown[];
  [key: string]: unknown;
}

export interface BrowserRuntime {
  envId: string;
  pid: number;
  debugPort: number;
  wsEndpoint: string;
  status: string;
  startedAt: string;
  lastActiveAt: string;
  cpuPercent: number;
  memoryMb: number;
  currentUrl: string;
  recording: boolean;
  harRecording: boolean;
  warnings: string[];
}

export interface CookieRecord {
  name: string;
  value: string;
  domain: string;
  path: string;
  secure?: boolean;
  httpOnly?: boolean;
  session?: boolean;
  expires?: number | null;
  sameSite?: string | null;
}

export interface ScreenshotResult {
  envId: string;
  path: string;
  dataUrl: string;
  width: number;
  height: number;
  capturedAt: string;
}

export interface CdpInfo {
  envId: string;
  debugPort: number;
  wsEndpoint: string;
  httpEndpoint: string;
  running: boolean;
  playwrightExample: string;
}

export interface OpenOptions {
  url?: string;
  preScript?: string;
  headless?: boolean;
  waitUntil?: string;
}

export interface ProxyCheckResult {
  proxyId: string;
  ok: boolean;
  latencyMs: number | null;
  exitIp: string | null;
  country: string | null;
  timezone: string | null;
  city: string | null;
  message: string;
  checkedAt: string;
}

export interface AuditEntry {
  id: string;
  actor: string;
  action: string;
  envId: string | null;
  account: string | null;
  project: string | null;
  caseId: string | null;
  detail: string;
  createdAt: string;
  prevHash: string;
  hash: string;
}

export interface PageResult {
  result: unknown;
}

type Query = Record<string, string | number | boolean | null | undefined>;

export class FinEnvError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = 'FinEnvError';
    this.status = status;
    this.code = code;
  }
}

/**
 * Thin HTTP SDK for the FinEnv local API. All requests use JSON except the
 * screenshot endpoint, which returns image/png by default.
 */
export class FinEnvClient {
  readonly baseUrl: string;
  private readonly apiKey: string;
  private readonly fetchImpl: typeof fetch;
  private readonly timeoutMs: number;

  constructor(options: FinEnvOptions) {
    this.baseUrl = (options.baseUrl ?? 'http://127.0.0.1:17321/api/v1').replace(/\/$/, '');
    this.apiKey = options.apiKey;
    this.fetchImpl = options.fetch ?? globalThis.fetch;
    this.timeoutMs = options.timeoutMs ?? 30_000;
    if (!this.fetchImpl) {
      throw new FinEnvError(0, 'fetch_unavailable', '当前运行时没有可用的 fetch，请升级 Node.js 或传入 fetch 实现');
    }
  }

  static fromEnv(options: Partial<FinEnvOptions> = {}): FinEnvClient {
    const env =
      (globalThis as { process?: { env?: Record<string, string | undefined> } }).process?.env ?? {};
    const apiKey = options.apiKey ?? env.FINENV_API_KEY;
    if (!apiKey) throw new FinEnvError(0, 'missing_api_key', '缺少 FINENV_API_KEY');
    return new FinEnvClient({
      ...options,
      apiKey,
      baseUrl: options.baseUrl ?? env.FINENV_BASE_URL ?? 'http://127.0.0.1:17321/api/v1',
    });
  }

  private url(path: string, query?: Query): string {
    const url = new URL(`${this.baseUrl}${path.startsWith('/') ? path : `/${path}`}`);
    if (query) {
      for (const [key, value] of Object.entries(query)) {
        if (value !== undefined && value !== null && value !== '') {
          url.searchParams.set(key, String(value));
        }
      }
    }
    return url.toString();
  }

  private async request<T>(
    method: string,
    path: string,
    options: { query?: Query; body?: unknown; headers?: Record<string, string>; raw?: boolean } = {},
  ): Promise<T> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);
    try {
      const response = await this.fetchImpl(this.url(path, options.query), {
        method,
        headers: {
          Authorization: `Bearer ${this.apiKey}`,
          Accept: options.raw ? '*/*' : 'application/json',
          ...(options.body !== undefined ? { 'Content-Type': 'application/json' } : {}),
          ...options.headers,
        },
        body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
        signal: controller.signal,
      });
      if (!response.ok) {
        let code = 'http_error';
        let message = `HTTP ${response.status}`;
        try {
          const payload = (await response.json()) as { error?: { code?: string; message?: string } };
          code = payload.error?.code ?? code;
          message = payload.error?.message ?? message;
        } catch {
          // non-JSON error body
        }
        throw new FinEnvError(response.status, code, message);
      }
      if (options.raw) return (await response.arrayBuffer()) as T;
      if (response.status === 204) return undefined as T;
      return (await response.json()) as T;
    } finally {
      clearTimeout(timer);
    }
  }

  health(): Promise<{ status: string; app: string; version: string; time: string }> {
    return this.request('GET', '/health');
  }

  stats(): Promise<Record<string, unknown>> {
    return this.request('GET', '/stats');
  }

  listEnvironments(filter: {
    search?: string;
    groupId?: string;
    status?: string;
    project?: string;
    testEnv?: string;
    tag?: string;
    runningOnly?: boolean;
  } = {}): Promise<EnvironmentSummary[]> {
    return this.request('GET', '/envs', { query: filter });
  }

  getEnvironment(id: string): Promise<EnvironmentSummary> {
    return this.request('GET', `/envs/${encodeURIComponent(id)}`);
  }

  createEnvironment(input: Record<string, unknown>): Promise<EnvironmentSummary> {
    return this.request('POST', '/envs', { body: input });
  }

  updateEnvironment(id: string, input: Record<string, unknown>): Promise<EnvironmentSummary> {
    return this.request('PATCH', `/envs/${encodeURIComponent(id)}`, { body: input });
  }

  deleteEnvironment(id: string): Promise<{ deleted: boolean; id: string }> {
    return this.request('DELETE', `/envs/${encodeURIComponent(id)}`, {
      headers: { 'X-FinEnv-Confirm': 'true' },
    });
  }

  openEnvironment(id: string, options: OpenOptions = {}): Promise<BrowserRuntime> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/open`, { body: options });
  }

  closeEnvironment(id: string): Promise<{ closed: boolean; id: string }> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/close`);
  }

  environmentStatus(id: string): Promise<{ running: boolean; runtime: BrowserRuntime | null }> {
    return this.request('GET', `/envs/${encodeURIComponent(id)}/status`);
  }

  /** Returns a PNG Blob in browsers / ArrayBuffer in Node. */
  screenshot(id: string): Promise<ArrayBuffer> {
    return this.request('GET', `/envs/${encodeURIComponent(id)}/screenshot`, { raw: true });
  }

  screenshotInfo(id: string): Promise<ScreenshotResult> {
    return this.request('GET', `/envs/${encodeURIComponent(id)}/screenshot`, { query: { format: 'json' } });
  }

  async saveScreenshot(id: string, path: string): Promise<string> {
    const bytes = new Uint8Array(await this.screenshot(id));
    const { writeFile } = await import('node:fs/promises');
    await writeFile(path, bytes);
    return path;
  }

  async getCookies(id: string): Promise<{ envId: string; envName: string; cookies: CookieRecord[] }> {
    return this.request('GET', `/envs/${encodeURIComponent(id)}/cookies`);
  }

  setCookies(id: string, cookies: CookieRecord[]): Promise<{ updated: number }> {
    return this.request('PUT', `/envs/${encodeURIComponent(id)}/cookies`, { body: { cookies } });
  }

  updateFingerprint(id: string, fingerprint: Fingerprint): Promise<EnvironmentSummary> {
    return this.request('PUT', `/envs/${encodeURIComponent(id)}/fingerprint`, { body: fingerprint });
  }

  detectFingerprint(id: string): Promise<Record<string, unknown>> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/detect`);
  }

  createSnapshot(id: string, name?: string): Promise<Record<string, unknown>> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/snapshot`, { body: { name } });
  }

  listSnapshots(id: string): Promise<Array<Record<string, unknown>>> {
    return this.request('GET', `/envs/${encodeURIComponent(id)}/snapshots`);
  }

  restoreSnapshot(id: string, snapshotId: string): Promise<EnvironmentSummary> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/restore-snapshot`, {
      body: { snapshotId },
    });
  }

  cdpInfo(id: string): Promise<CdpInfo> {
    return this.request('GET', `/envs/${encodeURIComponent(id)}/cdp`);
  }

  navigate(id: string, url: string): Promise<{ url: string }> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/navigate`, { body: { url } });
  }

  execute(id: string, script: string): Promise<PageResult> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/execute`, { body: { script } });
  }

  startRecording(id: string, intervalMs = 1000): Promise<Record<string, unknown>> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/recording/start`, {
      body: { intervalMs },
    });
  }

  stopRecording(id: string): Promise<Record<string, unknown>> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/recording/stop`);
  }

  startHar(id: string): Promise<Record<string, unknown>> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/har`);
  }

  exportHar(id: string): Promise<{ path: string; envId: string }> {
    return this.request('GET', `/envs/${encodeURIComponent(id)}/har`);
  }

  setEnvironmentProxy(id: string, proxyId: string | null, restart = false): Promise<Record<string, unknown>> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/proxy`, { body: { proxyId, restart } });
  }
  alignProxy(id: string): Promise<Record<string, unknown>> {
    return this.request('POST', `/envs/${encodeURIComponent(id)}/align-proxy`);
  }

  setHosts(id: string, entries: Array<{ host: string; ip: string; enabled?: boolean }>): Promise<unknown[]> {
    return this.request('PUT', `/envs/${encodeURIComponent(id)}/hosts`, { body: { entries } });
  }

  getHosts(id: string): Promise<unknown[]> {
    return this.request('GET', `/envs/${encodeURIComponent(id)}/hosts`);
  }

  setMocks(id: string, rules: unknown[]): Promise<unknown[]> {
    return this.request('PUT', `/envs/${encodeURIComponent(id)}/mocks`, { body: { rules } });
  }

  getMocks(id: string): Promise<unknown[]> {
    return this.request('GET', `/envs/${encodeURIComponent(id)}/mocks`);
  }

  listProxies(): Promise<unknown[]> {
    return this.request('GET', '/proxies');
  }

  createProxy(input: Record<string, unknown>): Promise<unknown> {
    return this.request('POST', '/proxies', { body: input });
  }

  updateProxy(id: string, input: Record<string, unknown>): Promise<unknown> {
    return this.request('PATCH', `/proxies/${encodeURIComponent(id)}`, { body: input });
  }

  deleteProxy(id: string): Promise<{ deleted: boolean; id: string }> {
    return this.request('DELETE', `/proxies/${encodeURIComponent(id)}`, {
      headers: { 'X-FinEnv-Confirm': 'true' },
    });
  }

  checkProxy(id: string): Promise<ProxyCheckResult> {
    return this.request('POST', `/proxies/${encodeURIComponent(id)}/check`);
  }

  listAudit(query: Record<string, string | number | undefined> = {}): Promise<AuditEntry[]> {
    return this.request('GET', '/audit-logs', { query });
  }

  verifyAudit(): Promise<{ valid: boolean; checked: number; brokenAt: string | null; message: string }> {
    return this.request('GET', '/audit-logs/verify');
  }

  /**
   * Connect Playwright to the environment CDP endpoint. Playwright is an
   * optional peer dependency; install `playwright` to use this helper.
   */
  async connectPlaywright(id: string): Promise<{
    browser: unknown;
    context: unknown;
    runtime: BrowserRuntime;
    cdp: CdpInfo;
  }> {
    await this.openEnvironment(id);
    const cdp = await this.cdpInfo(id);
    let chromium: { connectOverCDP: (endpoint: string) => Promise<{ contexts: () => Array<unknown> }> };
    try {
      const moduleName = 'playwright';
      const module = (await import(moduleName)) as unknown as {
        chromium: { connectOverCDP: (endpoint: string) => Promise<{ contexts: () => Array<unknown> }> };
      };
      chromium = module.chromium;
    } catch {
      throw new FinEnvError(
        0,
        'playwright_missing',
        '未安装 Playwright，请先执行 npm install playwright，或直接使用 cdp.httpEndpoint 连接',
      );
    }
    const browser = await chromium.connectOverCDP(cdp.httpEndpoint);
    const context = browser.contexts()[0];
    const runtime = await this.environmentStatus(id);
    if (!runtime.runtime) {
      throw new FinEnvError(500, 'runtime_missing', '环境已连接但运行状态丢失');
    }
    return { browser, context, runtime: runtime.runtime, cdp };
  }
}

export default FinEnvClient;
