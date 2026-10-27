import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  type Account,
  type AccountExportRow,
  type AccountInput,
  type AlignProxyResult,
  type ApiKey,
  type ApiStatus,
  type AppInfo,
  type AuditFilter,
  type AuditLog,
  type AuditVerifyResult,
  type BackupInfo,
  type BrowserRuntime,
  type CaptchaEvent,
  type CdpInfo,
  type ChromiumDetection,
  type CookieRecord,
  type CreateEnvironmentInput,
  type CreatedApiKey,
  type DashboardStats,
  type EnvironmentFilter,
  type EnvironmentIpInfo,
  type EnvironmentView,
  type Extension,
  type Fingerprint,
  type FingerprintReport,
  type FingerprintSnapshot,
  type FingerprintValidation,
  type Group,
  type HostsEntry,
  type HostsEntryInput,
  type MockRule,
  type MockRuleInput,
  type OpenOptions,
  type PermissionDef,
  type Proxy,
  type ProxyCheckResult,
  type ProxyInput,
  type RecordingResult,
  type ResourceSnapshot,
  type ScreenshotResult,
  type SessionInfo,
  type Settings,
  type SyncResult,
  type ThemeConfig,
  type UpdateAccountInput,
  type UpdateEnvironmentInput,
  type UpdateInfo,
  type UpdateProxyInput,
  type UpdateSettingsInput,
  type User,
  type UserInput,
  type UserUpdateInput,
  ApiError,
} from './types';

function normalizeError(error: unknown): ApiError {
  if (error instanceof ApiError) return error;
  if (typeof error === 'string') {
    try {
      const parsed = JSON.parse(error) as { code?: string; message?: string };
      if (parsed && typeof parsed === 'object') {
        return new ApiError(parsed.code ?? 'error', parsed.message ?? error);
      }
    } catch {
      return new ApiError('error', error);
    }
  }
  if (error && typeof error === 'object') {
    const value = error as { code?: unknown; message?: unknown };
    return new ApiError(
      typeof value.code === 'string' ? value.code : 'error',
      typeof value.message === 'string' ? value.message : '操作失败',
    );
  }
  return new ApiError('error', '操作失败，请查看日志');
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw normalizeError(error);
  }
}

export const ipc = {
  // auth & users
  authStatus: () => call<SessionInfo>('auth_status'),
  authLogin: (username: string, password: string) =>
    call<SessionInfo>('auth_login', { username, password }),
  authLogout: () => call<void>('auth_logout'),
  authChangePassword: (oldPassword: string, newPassword: string) =>
    call<void>('auth_change_password', { oldPassword, newPassword }),
  userList: () => call<User[]>('user_list'),
  userCreate: (input: UserInput) => call<User>('user_create', { input }),
  userUpdate: (id: string, input: UserUpdateInput) => call<User>('user_update', { id, input }),
  userDelete: (id: string) => call<void>('user_delete', { id }),
  permissionMatrix: () => call<PermissionDef[]>('permission_matrix'),

  // environments
  envList: (filter: EnvironmentFilter = {}) =>
    call<EnvironmentView[]>('env_list', { filter }),
  envGet: (id: string) => call<EnvironmentView>('env_get', { id }),
  envCheckIp: (id: string) => call<EnvironmentIpInfo>('env_check_ip', { id }),
  envCreate: (input: CreateEnvironmentInput) => call<EnvironmentView>('env_create', { input }),
  envUpdate: (id: string, input: UpdateEnvironmentInput) =>
    call<EnvironmentView>('env_update', { id, input }),
  envDelete: (id: string, confirm: boolean) => call<void>('env_delete', { id, confirm }),
  envBulkDelete: (ids: string[], confirm: boolean) =>
    call<number>('env_bulk_delete', { ids, confirm }),
  envClone: (id: string, name?: string) => call<EnvironmentView>('env_clone', { id, name }),
  envExport: (
    ids: string[],
    path: string,
    passphrase: string,
    includeProfiles: boolean,
    confirm: boolean,
  ) => call<number>('env_export', { ids, path, passphrase, includeProfiles, confirm }),
  envImport: (path: string, passphrase: string) =>
    call<EnvironmentView[]>('env_import', { path, passphrase }),
  envBulkImport: (paths: string[], passphrase: string) =>
    call<EnvironmentView[]>('env_bulk_import', { paths, passphrase }),
  envProjects: () => call<string[]>('env_projects'),
  envTags: () => call<string[]>('env_tags'),

  // groups
  groupList: () => call<Group[]>('group_list'),
  groupCreate: (name: string, color?: string) =>
    call<Group>('group_create', { input: { name, color } }),
  groupUpdate: (id: string, name: string, color?: string) =>
    call<void>('group_update', { id, input: { name, color } }),
  groupDelete: (id: string) => call<void>('group_delete', { id }),
  envAssignGroup: (ids: string[], groupId: string | null) =>
    call<number>('env_assign_group', { ids, groupId }),

  // accounts
  accountList: (envId: string) => call<Account[]>('account_list', { envId }),
  accountCreate: (input: AccountInput) => call<Account>('account_create', { input }),
  accountUpdate: (id: string, input: UpdateAccountInput) =>
    call<Account>('account_update', { id, input }),
  accountDelete: (id: string) => call<void>('account_delete', { id }),
  accountReveal: (id: string, confirm: boolean) =>
    call<string>('account_reveal', { id, confirm }),
  accountImport: (envId: string, rows: AccountExportRow[]) =>
    call<number>('account_import', { envId, rows }),
  accountImportFile: (envId: string, path: string, passphrase: string) =>
    call<number>('account_import_file', { envId, path, passphrase }),
  accountExport: (envIds: string[], path: string, passphrase: string, confirm: boolean) =>
    call<number>('account_export', { envIds, path, passphrase, confirm }),

  // fingerprints & snapshots
  fingerprintTemplate: (operatingSystem?: string) =>
    call<Fingerprint>('fingerprint_template', { operatingSystem }),
  fingerprintValidate: (fp: Fingerprint) =>
    call<FingerprintValidation>('fingerprint_validate', { fp }),
  fingerprintUpdate: (envId: string, fp: Fingerprint) =>
    call<EnvironmentView>('fingerprint_update', { envId, fp }),
  fingerprintDetect: (envId: string) =>
    call<FingerprintReport>('fingerprint_detect', { envId }),
  fingerprintReports: (envId: string) =>
    call<FingerprintReport[]>('fingerprint_reports', { envId }),
  snapshotList: (envId: string) =>
    call<FingerprintSnapshot[]>('snapshot_list', { envId }),
  snapshotCreate: (envId: string, name?: string) =>
    call<FingerprintSnapshot>('snapshot_create', { envId, name }),
  snapshotRestore: (id: string) =>
    call<EnvironmentView>('snapshot_restore', { id }),
  snapshotDelete: (id: string) => call<void>('snapshot_delete', { id }),

  // proxies
  proxyList: () => call<Proxy[]>('proxy_list'),
  proxyCreate: (input: ProxyInput) => call<Proxy>('proxy_create', { input }),
  proxyUpdate: (id: string, input: UpdateProxyInput) =>
    call<Proxy>('proxy_update', { id, input }),
  proxyDelete: (id: string, confirm: boolean) =>
    call<void>('proxy_delete', { id, confirm }),
  proxyCheck: (id: string) => call<ProxyCheckResult>('proxy_check', { id }),
  proxyAssign: (envIds: string[], proxyId: string | null) =>
    call<number>('proxy_assign', { envIds, proxyId }),
  proxyAlign: (envId: string) => call<AlignProxyResult>('proxy_align', { envId }),

  // browser
  browserOpen: (envId: string, options: OpenOptions = {}) =>
    call<BrowserRuntime>('browser_open', { envId, options }),
  browserClose: (envId: string) => call<void>('browser_close', { envId }),
  browserStatus: (envId: string) =>
    call<BrowserRuntime | null>('browser_status', { envId }),
  browserListRunning: () => call<BrowserRuntime[]>('browser_list_running'),
  browserScreenshot: (envId: string) =>
    call<ScreenshotResult>('browser_screenshot', { envId }),
  browserCookiesGet: (envId: string) =>
    call<CookieRecord[]>('browser_cookies_get', { envId }),
  browserCookiesSet: (envId: string, cookies: CookieRecord[]) =>
    call<number>('browser_cookies_set', { envId, cookies }),
  browserExecute: (envId: string, script: string) =>
    call<unknown>('browser_execute', { envId, script }),
  browserNavigate: (envId: string, url: string) =>
    call<string>('browser_navigate', { envId, url }),
  browserCdpInfo: (envId: string) => call<CdpInfo>('browser_cdp_info', { envId }),
  browserRecordingStart: (envId: string, intervalMs = 1000) =>
    call<RecordingResult>('browser_recording_start', { envId, intervalMs }),
  browserRecordingStop: (envId: string) =>
    call<RecordingResult>('browser_recording_stop', { envId }),
  browserHarStart: (envId: string) => call<void>('browser_har_start', { envId }),
  browserHarStop: (envId: string) => call<string>('browser_har_stop', { envId }),
  browserForceRecycleAll: () => call<number>('browser_force_recycle_all'),
  browserSwitchProxy: (envId: string, proxyId: string | null, restart: boolean) =>
    call<EnvironmentView>('browser_switch_proxy', { envId, proxyId, restart }),
  resourceSnapshot: () => call<ResourceSnapshot>('resource_snapshot'),

  // hosts / mocks / extensions / captcha
  hostsList: (envId: string) => call<HostsEntry[]>('hosts_list', { envId }),
  hostsSet: (envId: string, entries: HostsEntryInput[]) =>
    call<HostsEntry[]>('hosts_set', { envId, entries }),
  mockList: (envId: string) => call<MockRule[]>('mock_list', { envId }),
  mockSet: (envId: string, rules: MockRuleInput[]) =>
    call<MockRule[]>('mock_set', { envId, rules }),
  extensionList: () => call<Extension[]>('extension_list'),
  extensionAdd: (path: string, name?: string) =>
    call<Extension>('extension_add', { path, name }),
  extensionDelete: (id: string) => call<void>('extension_delete', { id }),
  extensionAssign: (extensionId: string, envIds: string[]) =>
    call<void>('extension_assign', { extensionId, envIds }),
  extensionUnassign: (extensionId: string, envIds: string[]) =>
    call<void>('extension_unassign', { extensionId, envIds }),
  envExtensions: (envId: string) => call<Extension[]>('env_extensions', { envId }),
  captchaList: (envId?: string) => call<CaptchaEvent[]>('captcha_list', { envId }),
  captchaHandle: (id: string, handled: boolean) =>
    call<void>('captcha_handle', { id, handled }),
  captchaCapture: (envId: string) =>
    call<string | null>('captcha_capture', { envId }),
  readArtifact: (path: string) => call<string>('read_artifact', { path }),

  // settings / api / theme / audit
  settingsGet: () => call<Settings>('settings_get'),
  settingsUpdate: (input: UpdateSettingsInput) =>
    call<Settings>('settings_update', { input }),
  apiStatus: () => call<ApiStatus>('api_status'),
  apiRestart: () => call<ApiStatus>('api_restart'),
  apiKeyList: () => call<ApiKey[]>('api_key_list'),
  apiKeyCreate: (name: string, role: string) =>
    call<CreatedApiKey>('api_key_create', { name, role }),
  apiKeyRevoke: (id: string) => call<void>('api_key_revoke', { id }),
  themeGet: () => call<ThemeConfig>('theme_get'),
  themeSave: (theme: ThemeConfig) => call<ThemeConfig>('theme_save', { theme }),
  themeExport: (path: string) => call<void>('theme_export', { path }),
  themeImport: (path: string) => call<ThemeConfig>('theme_import', { path }),
  auditList: (filter: AuditFilter = {}) => call<AuditLog[]>('audit_list', { filter }),
  auditVerify: () => call<AuditVerifyResult>('audit_verify'),
  auditExport: (path: string, format: 'json' | 'csv', filter: AuditFilter = {}) =>
    call<number>('audit_export', { path, format, filter }),

  // dashboard / app
  dashboard: () => call<DashboardStats>('dashboard'),
  appInfo: () => call<AppInfo>('app_info'),
  chromiumDetect: () => call<ChromiumDetection>('chromium_detect'),
  updateCheck: () => call<UpdateInfo>('update_check'),

  // backup & sync
  backupCreate: (path?: string, passphrase?: string, includeProfiles = false) =>
    call<BackupInfo>('backup_create', { path, passphrase, includeProfiles }),
  backupRestore: (
    path: string,
    passphrase: string | undefined,
    restoreProfiles: boolean,
    confirm: boolean,
  ) => call<string>('backup_restore', { path, passphrase, restoreProfiles, confirm }),
  backupList: () => call<BackupInfo[]>('backup_list'),
  syncTest: (password?: string) => call<string>('sync_test', { password }),
  syncPush: (passphrase: string) => call<SyncResult>('sync_push', { passphrase }),
  syncPull: (passphrase: string, confirm: boolean) =>
    call<SyncResult>('sync_pull', { passphrase, confirm }),
};

export function onBrowserRuntime(handler: (value: BrowserRuntime) => void): Promise<UnlistenFn> {
  return listen<BrowserRuntime>('browser://runtime', (event) => handler(event.payload));
}

export function onBrowserExit(
  handler: (value: { envId: string; pid?: number; code?: number; reason?: string }) => void,
): Promise<UnlistenFn> {
  return listen('browser://exit', (event) => handler(event.payload as never));
}

export function onCaptcha(handler: (value: CaptchaEvent) => void): Promise<UnlistenFn> {
  return listen<CaptchaEvent>('captcha://new', (event) => handler(event.payload));
}

export function onApiError(handler: (message: string) => void): Promise<UnlistenFn> {
  return listen<{ message: string }>('api://error', (event) => handler(event.payload.message));
}

export function onResourceAlert(
  handler: (value: { level: string; envId: string; message: string }) => void,
): Promise<UnlistenFn> {
  return listen('resource://alert', (event) => handler(event.payload as never));
}
