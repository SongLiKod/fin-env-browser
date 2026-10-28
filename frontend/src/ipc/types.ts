export type Role = 'super_admin' | 'test_lead' | 'test_engineer' | 'viewer';

export type PermissionKey =
  | 'view_list'
  | 'open_close'
  | 'create_env'
  | 'edit_env'
  | 'delete_env'
  | 'clone_env'
  | 'edit_fingerprint_proxy'
  | 'export_sensitive'
  | 'audit_view'
  | 'settings_manage'
  | 'user_manage'
  | 'api_key_manage'
  | 'manage_env_members';

export interface User {
  id: string;
  username: string;
  displayName: string;
  role: Role;
  enabled: boolean;
  createdAt: string;
  lastLoginAt: string | null;
}

export interface SessionInfo {
  authenticated: boolean;
  user: User | null;
  permissions: PermissionKey[];
}

export interface PermissionDef {
  key: PermissionKey;
  label: string;
  description: string;
  roles: Role[];
}

export interface BrandVersion {
  brand: string;
  version: string;
}

export interface MimeTypeInfo {
  mimeType: string;
  description: string;
  suffixes: string;
}

export interface PluginInfo {
  name: string;
  filename: string;
  description: string;
  mimeTypes: MimeTypeInfo[];
}

export interface ScreenFingerprint {
  width: number;
  height: number;
  availWidth: number;
  availHeight: number;
  colorDepth: number;
  pixelRatio: number;
}

export interface Fingerprint {
  version: number;
  kernel: string;
  userAgent: string;
  platform: string;
  vendor: string;
  languages: string[];
  timezone: string;
  hardwareConcurrency: number;
  deviceMemory: number;
  doNotTrack: boolean;
  colorScheme: 'light' | 'dark' | 'no-preference';
  touchPoints: number;
  screen: ScreenFingerprint;
  canvas: { mode: 'noise' | 'off'; seed: number };
  webgl: { vendor: string; renderer: string; seed: number };
  audio: { mode: 'noise' | 'off'; seed: number };
  fonts: string[];
  webrtc: { mode: 'disable' | 'proxy' | 'public'; publicIp: string | null };
  clientHints: {
    brands: BrandVersion[];
    fullVersionList: BrandVersion[];
    platform: string;
    platformVersion: string;
    architecture: string;
    bitness: string;
    model: string;
    mobile: boolean;
  };
  plugins: PluginInfo[];
}

export interface FingerprintIssue {
  field: string;
  message: string;
  severity: 'error' | 'warning';
}

export interface FingerprintValidation {
  valid: boolean;
  errors: FingerprintIssue[];
  warnings: FingerprintIssue[];
}

export interface FingerprintCheck {
  name: string;
  expected: string;
  actual: string;
  passed: boolean;
  weight: number;
  hint: string;
}

export interface FingerprintReport {
  id: string;
  envId: string;
  score: number;
  checks: FingerprintCheck[];
  observed: Record<string, unknown>;
  configured: Record<string, unknown>;
  screenshotPath: string | null;
  externalUrl: string;
  createdAt: string;
}

export type EnvStatus = 'active' | 'disabled' | 'archived';

export interface Environment {
  id: string;
  name: string;
  groupId: string | null;
  status: EnvStatus;
  profileDir: string;
  proxyId: string | null;
  fingerprintSnapshotId: string | null;
  project: string;
  testEnv: string;
  accountRole: string;
  version: string;
  caseId: string;
  tags: string[];
  notes: string;
  kernel: string;
  chromiumPath: string | null;
  autoRestart: boolean;
  memoryLimitMb: number | null;
  idleTimeoutMin: number | null;
  fingerprint: Fingerprint;
  borderColor: string;
  borderWidth: number;
  ipAddress: string | null;
  ipCountry: string | null;
  ipCountryCode: string | null;
  ipCity: string | null;
  ipTimezone: string | null;
  ipSource: string | null;
  ipCheckedAt: string | null;
  createdAt: string;
  updatedAt: string;
  lastOpenedAt: string | null;
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

export interface Proxy {
  id: string;
  name: string;
  protocol: 'http' | 'https' | 'socks5';
  host: string;
  port: number;
  username: string;
  hasPassword: boolean;
  enabled: boolean;
  lastCheckAt: string | null;
  latencyMs: number | null;
  exitIp: string | null;
  exitCountry: string | null;
  exitTimezone: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface EnvironmentView extends Environment {
  proxy: Proxy | null;
  runtime: BrowserRuntime | null;
  accountCount: number;
  snapshotCount: number;
  accessLevel: 'view' | 'operate' | 'manage' | string;
}

export interface EnvironmentMember {
  userId: string;
  username: string;
  displayName: string;
  role: Role;
  enabled: boolean;
  accessLevel: 'view' | 'operate' | 'manage' | string;
  createdAt: string;
  updatedAt: string;
}

export interface EnvironmentMemberCandidate {
  userId: string;
  username: string;
  displayName: string;
  role: Role;
  enabled: boolean;
  accessLevel: 'view' | 'operate' | 'manage' | string | null;
}

export interface EnvironmentMemberInput {
  userId: string;
  accessLevel: 'view' | 'operate' | 'manage';
}

export interface EnvironmentIpInfo {
  envId: string;
  ok: boolean;
  ip: string | null;
  country: string | null;
  countryCode: string | null;
  city: string | null;
  timezone: string | null;
  source: 'proxy' | 'direct' | string;
  proxyId: string | null;
  latencyMs: number | null;
  message: string;
  checkedAt: string;
}

export interface EnvironmentTemplate {
  id: string;
  name: string;
  description: string;
  project: string;
  testEnv: string;
  accountRole: string;
  version: string;
  caseId: string;
  tags: string[];
  notes: string;
  kernel: string;
  chromiumPath: string | null;
  autoRestart: boolean;
  memoryLimitMb: number | null;
  idleTimeoutMin: number | null;
  fingerprint: Fingerprint;
  borderColor: string;
  borderWidth: number;
  groupId: string | null;
  proxyMode: 'none' | 'fixed' | 'pool' | string;
  proxyId: string | null;
  extensionIds: string[];
  createdAt: string;
  updatedAt: string;
}

export interface CreateTemplateFromEnvInput {
  envId: string;
  name: string;
  description?: string | null;
}

export interface UpdateTemplateInput {
  name?: string;
  description?: string;
  proxyMode?: 'none' | 'fixed' | 'pool';
  proxyId?: string | null;
}

export interface BatchCreateTemplateInput {
  templateId: string;
  count: number;
  namePrefix?: string | null;
  startIndex?: number | null;
}

export interface BatchCreateResult {
  created: Environment[];
  errors: string[];
}

export interface BroadcastStartInput {
  envIds: string[];
  masterEnvId?: string | null;
  url?: string | null;
  openMissing?: boolean | null;
}

export interface BroadcastStatus {
  active: boolean;
  id: string | null;
  masterEnvId: string | null;
  envIds: string[];
  paused: boolean;
  forwarded: number;
  startedAt: string | null;
  message: string;
  errors: string[];
}

export interface Group {
  id: string;
  name: string;
  color: string;
  createdAt: string;
}

export interface EnvironmentFilter {
  search?: string | null;
  groupId?: string | null;
  status?: string | null;
  project?: string | null;
  testEnv?: string | null;
  tag?: string | null;
  runningOnly?: boolean | null;
}

export interface CreateEnvironmentInput {
  name: string;
  groupId?: string | null;
  proxyId?: string | null;
  project?: string | null;
  testEnv?: string | null;
  accountRole?: string | null;
  version?: string | null;
  caseId?: string | null;
  tags?: string[];
  notes?: string | null;
  borderColor?: string | null;
  borderWidth?: number | null;
  kernel?: string | null;
  chromiumPath?: string | null;
  templateOs?: string | null;
  fingerprint?: Fingerprint | null;
  autoRestart?: boolean | null;
  memoryLimitMb?: number | null;
  idleTimeoutMin?: number | null;
}

export interface UpdateEnvironmentInput {
  name?: string;
  groupId?: string | null;
  status?: EnvStatus;
  proxyId?: string | null;
  project?: string;
  testEnv?: string;
  accountRole?: string;
  version?: string;
  caseId?: string;
  tags?: string[];
  notes?: string;
  borderColor?: string;
  borderWidth?: number;
  kernel?: string;
  chromiumPath?: string | null;
  autoRestart?: boolean;
  memoryLimitMb?: number | null;
  idleTimeoutMin?: number | null;
}

export type AccountStatus = 'pending' | 'in_use' | 'frozen' | 'deprecated';

export interface Account {
  id: string;
  envId: string;
  username: string;
  hasPassword: boolean;
  remark: string;
  owner: string;
  status: AccountStatus;
  createdAt: string;
  updatedAt: string;
}

export interface AccountInput {
  envId: string;
  username: string;
  password?: string | null;
  remark?: string | null;
  owner?: string | null;
  status?: AccountStatus | null;
}

export interface UpdateAccountInput {
  username?: string;
  password?: string;
  remark?: string;
  owner?: string;
  status?: AccountStatus;
}

export interface AccountExportRow {
  username: string;
  password: string;
  remark: string;
  owner: string;
  status: string;
}

export interface FingerprintSnapshot {
  id: string;
  envId: string;
  name: string;
  fingerprint: Fingerprint;
  createTime: string;
}

export interface ProxyInput {
  name: string;
  protocol: 'http' | 'https' | 'socks5';
  host: string;
  port: number;
  username?: string | null;
  password?: string | null;
  enabled?: boolean | null;
}

export interface UpdateProxyInput {
  name?: string;
  protocol?: 'http' | 'https' | 'socks5';
  host?: string;
  port?: number;
  username?: string;
  password?: string;
  enabled?: boolean;
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

export interface AlignProxyResult {
  environment: EnvironmentView;
  check: ProxyCheckResult;
}

export interface OpenOptions {
  url?: string | null;
  preScript?: string | null;
  waitUntil?: string | null;
  headless?: boolean | null;
}

export interface ScreenshotResult {
  envId: string;
  path: string;
  dataUrl: string;
  width: number;
  height: number;
  capturedAt: string;
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

export interface RecordingResult {
  envId: string;
  directory: string;
  frameCount: number;
  durationMs: number;
  manifestPath: string;
  startedAt: string;
  stoppedAt: string;
}

export interface CdpInfo {
  envId: string;
  debugPort: number;
  wsEndpoint: string;
  httpEndpoint: string;
  running: boolean;
  playwrightExample: string;
}

export interface ResourceSnapshot {
  instances: BrowserRuntime[];
  totalMemoryMb: number;
  maxConcurrent: number;
  runningCount: number;
  sampledAt: string;
  freeMemoryMb: number;
}

export interface AuditLog {
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

export interface AuditFilter {
  search?: string | null;
  envId?: string | null;
  action?: string | null;
  actor?: string | null;
  from?: string | null;
  to?: string | null;
  limit?: number | null;
  offset?: number | null;
}

export interface AuditVerifyResult {
  valid: boolean;
  checked: number;
  brokenAt: string | null;
  message: string;
}

export type ThemeMode = 'light' | 'dark' | 'auto';
export type ThemeDensity = 'compact' | 'comfortable';

export interface ThemeConfig {
  mode: ThemeMode;
  primaryColor: string;
  density: ThemeDensity;
  config: Record<string, unknown>;
  updatedAt: string;
}

export interface Settings {
  maxConcurrentBrowsers: number;
  idleTimeoutMin: number;
  memoryLimitMb: number;
  memoryEnforce: boolean;
  autoRestartOnCrash: boolean;
  apiEnabled: boolean;
  apiHost: string;
  apiPort: number;
  chromiumPath: string | null;
  defaultStartUrl: string;
  detectionServiceUrl: string;
  allowSensitiveExport: boolean;
  callbackType: string;
  callbackUrl: string | null;
  callbackTokenSet: boolean;
  syncEnabled: boolean;
  syncKind: string;
  syncEndpoint: string | null;
  syncUsername: string | null;
  syncPasswordSet: boolean;
  syncRemotePath: string;
  syncLastAt: string | null;
  closeToTray: boolean;
  globalShortcut: string;
  launchAtLogin: boolean;
  logLevel: string;
  downloadManifestUrl: string | null;
}

export interface UpdateSettingsInput {
  maxConcurrentBrowsers?: number;
  idleTimeoutMin?: number;
  memoryLimitMb?: number;
  memoryEnforce?: boolean;
  autoRestartOnCrash?: boolean;
  apiEnabled?: boolean;
  apiHost?: string;
  apiPort?: number;
  chromiumPath?: string | null;
  defaultStartUrl?: string;
  detectionServiceUrl?: string;
  allowSensitiveExport?: boolean;
  callbackType?: string;
  callbackUrl?: string | null;
  callbackToken?: string;
  syncEnabled?: boolean;
  syncKind?: string;
  syncEndpoint?: string | null;
  syncUsername?: string | null;
  syncPassword?: string;
  syncRemotePath?: string;
  closeToTray?: boolean;
  globalShortcut?: string;
  launchAtLogin?: boolean;
  logLevel?: string;
  downloadManifestUrl?: string | null;
}

export interface ApiKey {
  id: string;
  name: string;
  prefix: string;
  role: Role;
  enabled: boolean;
  createdAt: string;
  lastUsedAt: string | null;
}

export interface CreatedApiKey extends ApiKey {
  secret: string;
}

export interface ApiStatus {
  running: boolean;
  host: string;
  port: number;
  baseUrl: string;
  message: string;
}

export interface Extension {
  id: string;
  name: string;
  path: string;
  enabled: boolean;
  description: string;
  createdAt: string;
  assignedCount: number;
}

export interface UpdateExtensionInput {
  name?: string;
  description?: string;
  enabled?: boolean;
}

export interface HostsEntry {
  id: string;
  envId: string;
  host: string;
  ip: string;
  enabled: boolean;
}

export interface HostsEntryInput {
  host: string;
  ip: string;
  enabled?: boolean;
}

export interface MockRule {
  id: string;
  envId: string;
  pattern: string;
  method: string;
  statusCode: number;
  headers: Record<string, unknown>;
  body: string;
  delayMs: number;
  enabled: boolean;
}

export interface MockRuleInput {
  pattern: string;
  method?: string;
  statusCode?: number;
  headers?: Record<string, unknown>;
  body?: string;
  delayMs?: number;
  enabled?: boolean;
}

export interface CaptchaEvent {
  id: string;
  envId: string;
  envName: string;
  url: string;
  source: string;
  hint: string;
  screenshotPath: string | null;
  handled: boolean;
  createdAt: string;
}

export interface DashboardStats {
  envCount: number;
  accountCount: number;
  proxyCount: number;
  runningCount: number;
  snapshotCount: number;
  auditCount: number;
  totalMemoryMb: number;
  maxConcurrent: number;
  recentAudit: AuditLog[];
}

export interface AppInfo {
  version: string;
  dataDir: string;
  os: string;
  arch: string;
  chromiumPath: string | null;
  chromiumVersion: string | null;
  apiBaseUrl: string;
  dbPath: string;
}

export interface ChromiumDetection {
  path: string | null;
  version: string | null;
  message: string;
}

export interface UpdateInfo {
  updateAvailable: boolean;
  currentVersion: string;
  latestVersion: string | null;
  notes: string | null;
  downloadUrl: string | null;
  message: string;
}

export interface BackupInfo {
  path: string;
  name: string;
  size: number;
  encrypted: boolean;
  createdAt: string;
}

export interface SyncResult {
  ok: boolean;
  message: string;
  syncedAt: string;
  bytes: number;
}

export interface UserInput {
  username: string;
  displayName: string;
  role: Role;
  password: string;
  enabled?: boolean;
}

export interface UserUpdateInput {
  displayName?: string;
  role?: Role;
  password?: string;
  enabled?: boolean;
}

export class ApiError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = 'ApiError';
    this.code = code;
  }
}
