import * as React from 'react';
import {
  AlertTriangle,
  Archive,
  ArrowUpFromLine,
  Check,
  Cloud,
  DatabaseBackup,
  Download,
  Gauge,
  KeyRound,
  LockKeyhole,
  RefreshCw,
  Save,
  ShieldCheck,
  Sparkles,
  UserCog,
  Users,
  X,
} from 'lucide-react';
import type {
  ApiKey,
  ApiStatus,
  AppInfo,
  BackupInfo,
  CreatedApiKey,
  PermissionDef,
  Role,
  Settings,
  UpdateInfo,
  User,
} from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { toastError, useToastStore } from '@/store/toast';
import { cn, formatBytes, formatDate, roleLabels } from '@/utils';
import { backupFilter, pickFile, saveFile } from '@/utils/dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardHeader, EmptyState, Spinner } from '@/components/ui/card';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { CopyButton } from '@/components/ui/misc';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input, Select, Textarea } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { Tabs } from '@/components/ui/tabs';

const roles: Role[] = ['super_admin', 'test_lead', 'test_engineer', 'viewer'];

export function SettingsPage() {
  const toast = useToastStore();
  const can = useAuthStore((state) => state.can);
  const canUserManage = can('user_manage');
  const canApiKey = can('api_key_manage');
  const [tab, setTab] = React.useState('resource');
  const [form, setForm] = React.useState<Settings | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [api, setApi] = React.useState<ApiStatus | null>(null);
  const [apiKeys, setApiKeys] = React.useState<ApiKey[]>([]);
  const [users, setUsers] = React.useState<User[]>([]);
  const [permissions, setPermissions] = React.useState<PermissionDef[]>([]);
  const [backups, setBackups] = React.useState<BackupInfo[]>([]);
  const [appInfo, setAppInfo] = React.useState<AppInfo | null>(null);
  const [updateInfo, setUpdateInfo] = React.useState<UpdateInfo | null>(null);
  const [callbackToken, setCallbackToken] = React.useState('');
  const [syncPassword, setSyncPassword] = React.useState('');
  const [checkingUpdate, setCheckingUpdate] = React.useState(false);
  const [apiBusy, setApiBusy] = React.useState(false);
  const [keyEditor, setKeyEditor] = React.useState(false);
  const [userEditor, setUserEditor] = React.useState<User | 'new' | null>(null);
  const [deleteUser, setDeleteUser] = React.useState<User | null>(null);
  const [revokeKey, setRevokeKey] = React.useState<ApiKey | null>(null);
  const [backupCreate, setBackupCreate] = React.useState(false);
  const [restorePath, setRestorePath] = React.useState<string | null>(null);
  const [syncAction, setSyncAction] = React.useState<'push' | 'pull' | null>(null);
  const [passwordForm, setPasswordForm] = React.useState({ old: '', next: '', confirm: '' });
  const [passwordBusy, setPasswordBusy] = React.useState(false);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const settings = await ipc.settingsGet();
      setForm(settings);
      const results = await Promise.allSettled([
        ipc.apiStatus(),
        ipc.backupList(),
        ipc.permissionMatrix(),
        ipc.appInfo(),
        canApiKey ? ipc.apiKeyList() : Promise.resolve([] as ApiKey[]),
        canUserManage ? ipc.userList() : Promise.resolve([] as User[]),
      ]);
      if (results[0].status === 'fulfilled') setApi(results[0].value as ApiStatus);
      if (results[1].status === 'fulfilled') setBackups(results[1].value as BackupInfo[]);
      if (results[2].status === 'fulfilled') setPermissions(results[2].value as PermissionDef[]);
      if (results[3].status === 'fulfilled') setAppInfo(results[3].value as AppInfo);
      if (results[4].status === 'fulfilled') setApiKeys(results[4].value as ApiKey[]);
      if (results[5].status === 'fulfilled') setUsers(results[5].value as User[]);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : '设置加载失败');
      toastError(err);
    } finally {
      setLoading(false);
    }
  }, [canApiKey, canUserManage]);

  React.useEffect(() => {
    void load();
  }, [load]);

  const update = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setForm((current) => (current ? { ...current, [key]: value } : current));

  const save = async () => {
    if (!form) return;
    setSaving(true);
    try {
      const updated = await ipc.settingsUpdate({
        maxConcurrentBrowsers: form.maxConcurrentBrowsers,
        idleTimeoutMin: form.idleTimeoutMin,
        memoryLimitMb: form.memoryLimitMb,
        memoryEnforce: form.memoryEnforce,
        autoRestartOnCrash: form.autoRestartOnCrash,
        apiEnabled: form.apiEnabled,
        apiHost: form.apiHost,
        apiPort: form.apiPort,
        chromiumPath: form.chromiumPath?.trim() || null,
        chromiumLaunchArgs: form.chromiumLaunchArgs,
        defaultStartUrl: form.defaultStartUrl,
        detectionServiceUrl: form.detectionServiceUrl,
        allowSensitiveExport: form.allowSensitiveExport,
        callbackType: form.callbackType,
        callbackUrl: form.callbackUrl?.trim() || null,
        callbackToken: callbackToken || undefined,
        syncEnabled: form.syncEnabled,
        syncKind: form.syncKind,
        syncEndpoint: form.syncEndpoint?.trim() || null,
        syncUsername: form.syncUsername?.trim() || null,
        syncPassword: syncPassword || undefined,
        syncRemotePath: form.syncRemotePath,
        closeToTray: form.closeToTray,
        globalShortcut: form.globalShortcut,
        launchAtLogin: form.launchAtLogin,
        logLevel: form.logLevel,
        downloadManifestUrl: form.downloadManifestUrl?.trim() || null,
      });
      setForm(updated);
      setCallbackToken('');
      setSyncPassword('');
      toast.success('设置已保存', 'API 与快捷键变更可能需要重启面板后完全生效');
      try {
        setApi(await ipc.apiStatus());
        setAppInfo(await ipc.appInfo());
      } catch {
        // status is best effort
      }
      await useAuthStore.getState().refresh();
    } catch (err) {
      toastError(err);
    } finally {
      setSaving(false);
    }
  };

  if (loading || !form) {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-xs text-content-weak">
        {error ? <AlertTriangle className="h-4 w-4 text-danger" /> : <Spinner />}
        {error ?? '加载系统设置…'}
      </div>
    );
  }

  return (
    <div className="page-pad space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-[10px] font-semibold uppercase tracking-widest text-content-hint">System / Administration</p>
          <h1 className="mt-1 text-xl font-semibold tracking-tight text-content-main">系统设置</h1>
          <p className="mt-1 max-w-2xl text-xs leading-5 text-content-weak">
            资源上限、API 服务、团队权限、加密备份与同步、系统集成。所有变更写入审计日志。
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" onClick={() => void load()}>
            <RefreshCw className="h-4 w-4" /> 重新加载
          </Button>
          <Button variant="primary" loading={saving} onClick={() => void save()}>
            <Save className="h-4 w-4" /> 保存设置
          </Button>
        </div>
      </div>

      <Tabs
        value={tab}
        onValueChange={setTab}
        items={[
          { value: 'resource', label: '资源管控' },
          { value: 'api', label: 'API 服务' },
          { value: 'team', label: '团队与权限' },
          { value: 'storage', label: '备份与同步' },
          { value: 'system', label: '系统集成' },
          { value: 'about', label: '关于' },
        ]}
      />

      {tab === 'resource' ? (
        <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_340px]">
          <Card>
            <CardHeader title="并发与回收" description="超出资源上限时拒绝新建浏览器实例，并在审计日志中记录。" />
            <CardBody className="space-y-4">
              <div className="grid grid-cols-2 gap-3">
                <Field label="最大并发浏览器" hint="1 - 100；文档验收目标为稳定 30 个">
                  <Input
                    type="number"
                    min={1}
                    max={100}
                    value={form.maxConcurrentBrowsers}
                    onChange={(event) => update('maxConcurrentBrowsers', Number(event.target.value))}
                  />
                </Field>
                <Field label="闲置自动回收（分钟）" hint="0 表示不回收">
                  <Input
                    type="number"
                    min={0}
                    max={1440}
                    value={form.idleTimeoutMin}
                    onChange={(event) => update('idleTimeoutMin', Number(event.target.value))}
                  />
                </Field>
                <Field label="单环境内存告警阈值（MB）">
                  <Input
                    type="number"
                    min={128}
                    value={form.memoryLimitMb}
                    onChange={(event) => update('memoryLimitMb', Number(event.target.value))}
                  />
                </Field>
                <div className="flex items-end pb-1.5">
                  <Switch
                    checked={form.memoryEnforce}
                    onCheckedChange={(value) => update('memoryEnforce', value)}
                    label="超限自动关闭"
                    description="关闭时仅告警，不结束进程。"
                  />
                </div>
              </div>
              <div className="rounded-md border border-border bg-surface-secondary p-3">
                <Switch
                  checked={form.autoRestartOnCrash}
                  onCheckedChange={(value) => update('autoRestartOnCrash', value)}
                  label="进程崩溃后自动重启（全局）"
                  description="单环境开关可覆盖；10 分钟内最多自动重启 3 次，避免崩溃循环。"
                />
              </div>
              <div className="rounded-md border border-border p-3 text-[11px] leading-5 text-content-weak">
                <p className="flex items-center gap-1.5 font-medium text-content-main"><Gauge className="h-3.5 w-3.5 text-primary" /> 监控策略</p>
                后台每 5 秒轮询所有 Chromium 进程树，汇总 CPU / 内存；超过单环境阈值触发系统通知与面板告警，写入审计日志。全局并发上限在启动前检查，达到上限返回明确错误。
              </div>
            </CardBody>
          </Card>
          <div className="space-y-3">
            <Card>
              <CardHeader title="当前资源" description="实时数据来自进程树采样" />
              <CardBody className="space-y-2 text-xs">
                <InfoRow label="最大并发" value={`${form.maxConcurrentBrowsers}`} />
                <InfoRow label="闲置回收" value={form.idleTimeoutMin ? `${form.idleTimeoutMin} 分钟` : '关闭'} />
                <InfoRow label="内存阈值" value={`${form.memoryLimitMb} MB`} />
                <InfoRow label="超限动作" value={form.memoryEnforce ? '自动关闭' : '仅告警'} />
                <InfoRow label="崩溃重启" value={form.autoRestartOnCrash ? '全局开启' : '按环境配置'} />
              </CardBody>
            </Card>
            <Card>
              <CardBody className="text-[11px] leading-5 text-content-weak">
                资源上限只限制由 FinEnv 拉起的实例；手动启动的 Chrome 或面板重启前遗留的实例会在启动时自动接管并计入并发。
              </CardBody>
            </Card>
          </div>
        </div>
      ) : null}

      {tab === 'api' ? (
        <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_380px]">
          <Card>
            <CardHeader
              title="内置 REST API"
              description="Axum 独立端口，默认仅监听 127.0.0.1；所有接口使用 API Key 认证并受角色权限约束。"
              actions={
                <Button
                  variant="secondary"
                  size="sm"
                  loading={apiBusy}
                  onClick={async () => {
                    setApiBusy(true);
                    try {
                      setApi(await ipc.apiRestart());
                      toast.success('API 服务已重启');
                    } catch (err) {
                      toastError(err);
                    } finally {
                      setApiBusy(false);
                    }
                  }}
                >
                  <RefreshCw className="h-3.5 w-3.5" /> 重启服务
                </Button>
              }
            />
            <CardBody className="space-y-4">
              <Switch
                checked={form.apiEnabled}
                onCheckedChange={(value) => update('apiEnabled', value)}
                label="启用 HTTP API 服务"
                description="保存后自动启动/停止；关闭不影响已建立的面板操作。"
              />
              <div className="grid grid-cols-2 gap-3">
                <Field label="监听地址" hint="0.0.0.0 可被局域网访问，请谨慎">
                  <Input value={form.apiHost} onChange={(event) => update('apiHost', event.target.value)} />
                </Field>
                <Field label="监听端口">
                  <Input type="number" min={1024} max={65535} value={form.apiPort} onChange={(event) => update('apiPort', Number(event.target.value))} />
                </Field>
              </div>
              <div className="rounded-md border border-border bg-surface-secondary p-3">
                <p className="text-[10px] uppercase tracking-widest text-content-hint">服务状态</p>
                <div className="mt-1 flex items-center gap-2">
                  <span className={cn('h-2 w-2 rounded-full', api?.running ? 'bg-[var(--success)]' : 'bg-content-hint')} />
                  <span className="text-sm font-medium text-content-main">{api?.running ? '运行中' : '未运行'}</span>
                  <Badge tone={api?.running ? 'success' : 'neutral'}>{api?.message ?? '—'}</Badge>
                </div>
                <div className="mt-2 flex items-center gap-1">
                  <code className="min-w-0 flex-1 truncate font-mono text-[11px] text-content-main">{api?.baseUrl ?? `http://${form.apiHost}:${form.apiPort}/api/v1`}</code>
                  <CopyButton value={api?.baseUrl ?? `http://${form.apiHost}:${form.apiPort}/api/v1`} />
                </div>
                <p className="mt-1 text-[10px] text-content-hint">健康检查：GET /api/v1/health（无需认证）</p>
              </div>
            </CardBody>
          </Card>

          <div className="space-y-3">
            <Card>
              <CardHeader
                title="API 密钥"
                description={canApiKey ? '密钥仅在创建时显示一次' : '需要 API 密钥管理权限'}
                actions={canApiKey ? <Button variant="primary" size="sm" onClick={() => setKeyEditor(true)}><KeyRound className="h-3.5 w-3.5" /> 新建</Button> : null}
              />
              <CardBody className="space-y-2">
                {!canApiKey ? (
                  <p className="text-xs text-content-hint">当前角色无权查看 API 密钥。</p>
                ) : apiKeys.length === 0 ? (
                  <p className="text-xs text-content-hint">尚未创建 API 密钥。CI / SDK 调用前请先创建。</p>
                ) : (
                  apiKeys.map((key) => (
                    <div key={key.id} className="rounded-md border border-border p-2">
                      <div className="flex items-center justify-between gap-2">
                        <div className="min-w-0">
                          <p className="truncate text-xs font-medium text-content-main">{key.name}</p>
                          <p className="font-mono text-[10px] text-content-hint">{key.prefix}…</p>
                        </div>
                        <div className="flex items-center gap-1">
                          <Badge tone="outline">{roleLabels[key.role]}</Badge>
                          {canApiKey ? (
                            <Button variant="ghost" size="icon-sm" className="text-danger" onClick={() => setRevokeKey(key)}>
                              <X className="h-3.5 w-3.5" />
                            </Button>
                          ) : null}
                        </div>
                      </div>
                      <p className="mt-1 text-[10px] text-content-hint">
                        {key.lastUsedAt ? `最近使用 ${formatDate(key.lastUsedAt)}` : '从未使用'} · 创建于 {formatDate(key.createdAt)}
                      </p>
                    </div>
                  ))
                )}
              </CardBody>
            </Card>
            <Card>
              <CardBody className="text-[11px] leading-5 text-content-weak">
                SDK 使用 <code className="font-mono">Authorization: Bearer finenv_xxx</code> 或{' '}
                <code className="font-mono">X-API-Key</code>。删除/导出等接口需额外{' '}
                <code className="font-mono">X-FinEnv-Confirm: true</code>。完整接口见 <code className="font-mono">docs/api-docs.md</code>。
              </CardBody>
            </Card>
          </div>
        </div>
      ) : null}

      {tab === 'team' ? (
        <div className="space-y-4">
          <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
            <Card>
              <CardHeader
                title="团队成员"
                description="角色直接决定 IPC 与 API 权限；最后一名启用的超级管理员不可删除或降级。"
                actions={canUserManage ? <Button variant="primary" size="sm" onClick={() => setUserEditor('new')}><UserCog className="h-3.5 w-3.5" /> 新增成员</Button> : null}
              />
              <CardBody>
                {!canUserManage ? (
                  <p className="py-6 text-center text-xs text-content-hint">当前角色无用户管理权限。</p>
                ) : users.length === 0 ? (
                  <EmptyState icon={<Users className="h-5 w-5" />} title="暂无其他成员" />
                ) : (
                  <div className="space-y-2">
                    {users.map((user) => (
                      <div key={user.id} className="flex flex-wrap items-center gap-2 rounded-md border border-border p-2.5">
                        <div className="flex h-8 w-8 items-center justify-center rounded-full bg-primary-light text-xs font-semibold text-primary">
                          {user.displayName.slice(0, 1)}
                        </div>
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-xs font-medium text-content-main">{user.displayName} <span className="font-mono text-[10px] text-content-hint">@{user.username}</span></p>
                          <p className="text-[10px] text-content-hint">{user.lastLoginAt ? `最近登录 ${formatDate(user.lastLoginAt)}` : '从未登录'}</p>
                        </div>
                        <Badge tone={user.enabled ? 'primary' : 'neutral'}>{roleLabels[user.role]}</Badge>
                        {!user.enabled ? <Badge tone="danger">已禁用</Badge> : null}
                        <Button variant="ghost" size="sm" onClick={() => setUserEditor(user)}>编辑</Button>
                        <Button variant="ghost" size="icon-sm" className="text-danger" onClick={() => setDeleteUser(user)}><X className="h-3.5 w-3.5" /></Button>
                      </div>
                    ))}
                  </div>
                )}
              </CardBody>
            </Card>

            <Card>
              <CardHeader title="修改我的密码" description="修改后当前会话保持登录" />
              <CardBody className="space-y-3">
                <Field label="当前密码">
                  <Input type="password" value={passwordForm.old} onChange={(event) => setPasswordForm((current) => ({ ...current, old: event.target.value }))} />
                </Field>
                <Field label="新密码" hint="至少 8 个字符">
                  <Input type="password" value={passwordForm.next} onChange={(event) => setPasswordForm((current) => ({ ...current, next: event.target.value }))} />
                </Field>
                <Field label="确认新密码" error={passwordForm.confirm && passwordForm.confirm !== passwordForm.next ? '两次输入不一致' : undefined}>
                  <Input type="password" value={passwordForm.confirm} onChange={(event) => setPasswordForm((current) => ({ ...current, confirm: event.target.value }))} />
                </Field>
                <Button
                  variant="primary"
                  className="w-full"
                  loading={passwordBusy}
                  disabled={!passwordForm.old || passwordForm.next.length < 8 || passwordForm.next !== passwordForm.confirm}
                  onClick={async () => {
                    setPasswordBusy(true);
                    try {
                      await ipc.authChangePassword(passwordForm.old, passwordForm.next);
                      setPasswordForm({ old: '', next: '', confirm: '' });
                      toast.success('密码已修改');
                    } catch (error) {
                      toastError(error);
                    } finally {
                      setPasswordBusy(false);
                    }
                  }}
                >
                  <LockKeyhole className="h-4 w-4" /> 更新密码
                </Button>
              </CardBody>
            </Card>
          </div>

          <Card>
            <CardHeader title="权限矩阵" description="后端对每个 Tauri 命令与 HTTP 接口执行相同的权限检查" />
            <div className="table-wrap">
              <table className="data-table">
                <thead>
                  <tr>
                    <th>权限</th>
                    {roles.map((role) => <th key={role} className="text-center">{roleLabels[role]}</th>)}
                    <th>说明</th>
                  </tr>
                </thead>
                <tbody>
                  {permissions.map((permission) => (
                    <tr key={permission.key}>
                      <td className="text-xs font-medium text-content-main">{permission.label}</td>
                      {roles.map((role) => (
                        <td key={role} className="text-center">
                          {permission.roles.includes(role) ? <Check className="mx-auto h-4 w-4 text-[var(--success)]" /> : <X className="mx-auto h-3.5 w-3.5 text-content-hint" />}
                        </td>
                      ))}
                      <td><p className="max-w-[420px] text-[11px] text-content-weak">{permission.description}</p></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </Card>

          <Card>
            <CardBody className="flex flex-wrap items-center justify-between gap-3 text-xs">
              <div className="flex items-center gap-2 text-content-weak">
                <ShieldCheck className="h-4 w-4 text-primary" />
                高危导出权限开关当前为：
                <Badge tone={form.allowSensitiveExport ? 'success' : 'danger'}>
                  {form.allowSensitiveExport ? '允许（二次确认）' : '已全局关闭'}
                </Badge>
              </div>
              <Switch
                checked={form.allowSensitiveExport}
                onCheckedChange={(value) => update('allowSensitiveExport', value)}
                label="允许导出密码 / Cookie / 环境包"
              />
            </CardBody>
          </Card>
        </div>
      ) : null}

      {tab === 'storage' ? (
        <div className="space-y-4">
          <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_400px]">
            <Card>
              <CardHeader
                title="本地备份"
                description="SQLite 一致性快照 + 元数据（可选 profile），可整体加密；一键恢复会覆盖当前数据库。"
                actions={
                  <Button variant="primary" size="sm" onClick={() => setBackupCreate(true)}>
                    <DatabaseBackup className="h-3.5 w-3.5" /> 创建备份
                  </Button>
                }
              />
              <CardBody>
                {backups.length === 0 ? (
                  <EmptyState icon={<Archive className="h-5 w-5" />} title="暂无本地备份" description="升级前建议创建一份包含 profile 的备份。" />
                ) : (
                  <div className="space-y-2">
                    {backups.map((backup) => (
                      <div key={backup.path} className="flex flex-wrap items-center gap-2 rounded-md border border-border p-2.5">
                        <DatabaseBackup className="h-4 w-4 text-primary" />
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-xs font-medium text-content-main">{backup.name}</p>
                          <p className="text-[10px] text-content-hint">{formatBytes(backup.size)} · {formatDate(backup.createdAt)} · {backup.encrypted ? '加密' : '未加密'}</p>
                        </div>
                        <Button variant="secondary" size="sm" onClick={() => setRestorePath(backup.path)}>恢复</Button>
                      </div>
                    ))}
                  </div>
                )}
                <Button
                  variant="secondary"
                  className="mt-3 w-full"
                  onClick={async () => {
                    try {
                      const files = await pickFile(backupFilter());
                      if (files?.[0]) setRestorePath(files[0]);
                    } catch (error) {
                      toastError(error);
                    }
                  }}
                >
                  <Download className="h-4 w-4" /> 从文件恢复备份
                </Button>
              </CardBody>
            </Card>

            <Card>
              <CardHeader title="备份策略" />
              <CardBody className="space-y-2 text-[11px] leading-5 text-content-weak">
                <p>· 升级前自动创建快照（设置变更与版本更新前）。</p>
                <p>· 备份包包含 database.sqlite、master.salt 与 meta.json；勾选后包含 profiles。</p>
                <p>· 加密备份使用 PBKDF2-SHA256 + AES-256-GCM，口令不落盘。</p>
                <p>· 跨设备恢复时，字段加密密钥仍绑定本机设备信息；建议同时迁移 profile 或重新录入敏感账号。</p>
                <p>· 恢复操作会先强制回收全部浏览器进程，并要求输入 RESTORE 二次确认。</p>
              </CardBody>
            </Card>
          </div>

          <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_400px]">
            <Card>
              <CardHeader title="可选加密云同步" description="零知识：同步包在本机整体加密后再上传，服务端无法解密。" />
              <CardBody className="space-y-3">
                <Switch
                  checked={form.syncEnabled}
                  onCheckedChange={(value) => update('syncEnabled', value)}
                  label="启用云同步"
                  description="默认关闭；敏感数据默认不上云。"
                />
                <div className="grid grid-cols-2 gap-3">
                  <Field label="同步类型">
                    <Select value={form.syncKind} onChange={(event) => update('syncKind', event.target.value)}>
                      <option value="webdav">WebDAV</option>
                      <option value="folder">本地/挂载目录</option>
                    </Select>
                  </Field>
                  <Field label="远端文件名">
                    <Input value={form.syncRemotePath} onChange={(event) => update('syncRemotePath', event.target.value)} />
                  </Field>
                  <Field label={form.syncKind === 'folder' ? '同步目录' : 'WebDAV 地址'} className="col-span-2">
                    <Input value={form.syncEndpoint ?? ''} onChange={(event) => update('syncEndpoint', event.target.value)} placeholder={form.syncKind === 'folder' ? 'D:\\FinEnvSync' : 'https://dav.example.com/finenv/'} />
                  </Field>
                  <Field label="用户名">
                    <Input value={form.syncUsername ?? ''} onChange={(event) => update('syncUsername', event.target.value)} disabled={form.syncKind === 'folder'} />
                  </Field>
                  <Field label={form.syncPasswordSet ? '新密码（留空不修改）' : '密码'}>
                    <div className="flex gap-2">
                      <Input
                        type="password"
                        value={syncPassword}
                        onChange={(event) => setSyncPassword(event.target.value)}
                        placeholder={form.syncPasswordSet ? '••••••••' : ''}
                        disabled={form.syncKind === 'folder'}
                      />
                      {form.syncPasswordSet ? (
                        <Button
                          variant="secondary"
                          onClick={async () => {
                            try {
                              const updated = await ipc.settingsUpdate({ syncPassword: '' });
                              setForm(updated);
                              setSyncPassword('');
                              toast.success('同步密码已清除');
                            } catch (error) {
                              toastError(error);
                            }
                          }}
                        >
                          清除
                        </Button>
                      ) : null}
                    </div>
                  </Field>
                </div>
                {form.syncLastAt ? <p className="text-[10px] text-content-hint">上次同步：{formatDate(form.syncLastAt)}</p> : null}
                <div className="flex flex-wrap gap-2">
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={async () => {
                      try {
                        const message = await ipc.syncTest(syncPassword || undefined);
                        toast.success('连接成功', message);
                      } catch (error) {
                        toastError(error);
                      }
                    }}
                  >
                    <Cloud className="h-3.5 w-3.5" /> 测试连接
                  </Button>
                  <Button variant="secondary" size="sm" onClick={() => setSyncAction('push')}>
                    <ArrowUpFromLine className="h-3.5 w-3.5" /> 上传
                  </Button>
                  <Button variant="secondary" size="sm" onClick={() => setSyncAction('pull')}>
                    <Download className="h-3.5 w-3.5" /> 下载并恢复
                  </Button>
                </div>
                <p className="rounded-md border border-border bg-surface-secondary p-2 text-[10px] leading-4 text-content-hint">
                  同步包始终使用操作时输入的口令加密；请妥善保管，口令丢失将无法恢复。开启同步前请先保存上方配置。
                </p>
              </CardBody>
            </Card>

            <Card>
              <CardHeader title="存储与安全" />
              <CardBody className="space-y-2 text-[11px] leading-5 text-content-weak">
                <p>· 数据库：SQLite WAL，敏感字段 AES-256-GCM 单独加密。</p>
                <p>· 主密钥：HKDF-SHA256(设备标识 + 随机盐)，盐保存在数据目录。</p>
                <p>· 审计日志：追加式 + 哈希链，数据库触发器禁止修改。</p>
                <p>· 云同步：PBKDF2-SHA256(210k) + AES-256-GCM，服务端零知识。</p>
                <p>· Cookie：浏览器停止时自动写入加密缓存表。</p>
              </CardBody>
            </Card>
          </div>
        </div>
      ) : null}

      {tab === 'system' ? (
        <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_380px]">
          <div className="space-y-4">
            <Card>
              <CardHeader
                title="浏览器内核"
                description="优先级：环境级路径 > 全局路径 > CHROME_PATH > chromium-bin/ > 系统 Chrome / Edge"
                actions={
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={async () => {
                      try {
                        const info = await ipc.chromiumDetect();
                        setAppInfo((current) =>
                          current
                            ? {
                                ...current,
                                chromiumPath: info.path,
                                chromiumVersion: info.version,
                              }
                            : current,
                        );
                        if (info.path) {
                          update('chromiumPath', info.path);
                          toast.success('已填入检测到的 Chromium', info.path);
                        } else {
                          toast.warning('未检测到 Chromium', '请手动指定 chrome.exe 路径');
                        }
                      } catch (error) {
                        toastError(error);
                      }
                    }}
                  >
                    <Sparkles className="h-3.5 w-3.5" /> 自动检测
                  </Button>
                }
              />
              <CardBody className="space-y-3">
                <Field label="全局 Chromium 路径" hint="留空使用自动检测；环境级设置优先">
                  <Input value={form.chromiumPath ?? ''} onChange={(event) => update('chromiumPath', event.target.value)} placeholder="C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe" />
                </Field>
                <div className="rounded-md border border-border bg-surface-secondary p-2 text-[11px]">
                  <p className="text-content-hint">当前检测结果</p>
                  <p className="mt-0.5 break-all font-mono text-[10px] text-content-main">{appInfo?.chromiumPath ?? '未检测到'}</p>
                  <p className="text-[10px] text-content-hint">{appInfo?.chromiumVersion ?? '版本未知'}</p>
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <Field label="默认启动地址" hint="留空或 about:blank 时打开 FinEnv 内置环境信息启动页">
                    <Input value={form.defaultStartUrl} onChange={(event) => update('defaultStartUrl', event.target.value)} placeholder="about:blank" />
                  </Field>
                  <Field label="指纹检测服务 URL" hint="检测时新开标签页访问">
                    <Input value={form.detectionServiceUrl} onChange={(event) => update('detectionServiceUrl', event.target.value)} />
                  </Field>
                </div>
              </CardBody>
            </Card>

            <Card>
              <CardHeader
                title="浏览器启动参数"
                description="默认参数由 FinEnv 固定；全局附加参数应用到所有环境，环境页中的同名参数优先。"
                actions={
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => update('chromiumLaunchArgs', [])}
                  >
                    <RefreshCw className="h-3.5 w-3.5" /> 恢复默认
                  </Button>
                }
              />
              <CardBody className="space-y-4">
                <div className="rounded-md border border-border bg-surface-secondary p-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <p className="text-xs font-medium text-content-main">默认启动参数（只读）</p>
                    <Badge tone="outline">{appInfo?.chromiumDefaultArgs.length ?? 0} 项</Badge>
                  </div>
                  <div className="mt-2 max-h-48 overflow-auto rounded border border-border/70 bg-surface-base p-2 font-mono text-[10px] leading-5 text-content-weak">
                    {(appInfo?.chromiumDefaultArgs ?? []).map((arg) => (
                      <div key={arg} className="break-all">
                        {arg}
                      </div>
                    ))}
                  </div>
                  <p className="mt-2 text-[10px] leading-5 text-content-hint">
                    每个环境还会自动追加 profile 目录、随机调试端口、代理、窗口尺寸、语言、Hosts 与扩展参数；这些隔离参数不可覆盖。
                  </p>
                </div>
                <Field
                  label="附加启动参数"
                  hint="每行一个，使用 --flag 或 --flag=value；全局参数与环境参数取并集，环境同名参数优先。"
                >
                  <Textarea
                    value={form.chromiumLaunchArgs.join('\n')}
                    onChange={(event) => update('chromiumLaunchArgs', event.target.value.split('\n'))}
                    rows={6}
                    spellCheck={false}
                    placeholder={'--autoplay-policy=no-user-gesture-required\n--disable-gpu'}
                  />
                </Field>
              </CardBody>
            </Card>

            <Card>
              <CardHeader title="系统集成" description="托盘、全局快捷键、开机启动与关闭行为" />
              <CardBody className="space-y-4">
                <div className="grid grid-cols-2 gap-3">
                  <Field label="全局快捷键" hint="例如 CmdOrCtrl+Shift+F">
                    <Input value={form.globalShortcut} onChange={(event) => update('globalShortcut', event.target.value)} />
                  </Field>
                  <Field label="日志级别">
                    <Select value={form.logLevel} onChange={(event) => update('logLevel', event.target.value)}>
                      {['trace', 'debug', 'info', 'warn', 'error'].map((level) => (
                        <option key={level} value={level}>{level}</option>
                      ))}
                    </Select>
                  </Field>
                </div>
                <div className="grid gap-3 md:grid-cols-2">
                  <div className="rounded-md border border-border bg-surface-secondary p-3">
                    <Switch checked={form.closeToTray} onCheckedChange={(value) => update('closeToTray', value)} label="关闭窗口时最小化到托盘" description="浏览器实例不受影响。" />
                  </div>
                  <div className="rounded-md border border-border bg-surface-secondary p-3">
                    <Switch checked={form.launchAtLogin} onCheckedChange={(value) => update('launchAtLogin', value)} label="开机自动启动" description="写入当前用户的自启动项。" />
                  </div>
                </div>
                <p className="text-[10px] text-content-hint">快捷键修改后需重启 FinEnv 才能重新注册；托盘菜单提供一键回收全部浏览器。</p>
              </CardBody>
            </Card>

            <Card>
              <CardHeader title="回调与更新" description="截图 / 录屏 / 验证码可推送到禅道、Jira 或自研测试平台" />
              <CardBody className="space-y-3">
                <div className="grid grid-cols-2 gap-3">
                  <Field label="回调类型">
                    <Select value={form.callbackType} onChange={(event) => update('callbackType', event.target.value)}>
                      <option value="generic">通用 JSON</option>
                      <option value="zentao">禅道</option>
                      <option value="jira">Jira</option>
                    </Select>
                  </Field>
                  <Field label="回调地址">
                    <Input value={form.callbackUrl ?? ''} onChange={(event) => update('callbackUrl', event.target.value)} placeholder="https://hooks.example.com/finenv" />
                  </Field>
                  <Field label="回调 Token" hint={form.callbackTokenSet ? '已保存，留空不修改' : '可选，Bearer Token'} className="col-span-2">
                    <div className="flex gap-2">
                      <Input type="password" value={callbackToken} onChange={(event) => setCallbackToken(event.target.value)} placeholder={form.callbackTokenSet ? '••••••••' : ''} />
                      {form.callbackTokenSet ? (
                        <Button
                          variant="secondary"
                          onClick={async () => {
                            try {
                              const updated = await ipc.settingsUpdate({ callbackToken: '' });
                              setForm(updated);
                              toast.success('回调 Token 已清除');
                            } catch (error) {
                              toastError(error);
                            }
                          }}
                        >
                          清除
                        </Button>
                      ) : null}
                    </div>
                  </Field>
                  <Field label="更新清单地址" hint="JSON: { version, url, notes }" className="col-span-2">
                    <Input value={form.downloadManifestUrl ?? ''} onChange={(event) => update('downloadManifestUrl', event.target.value)} placeholder="https://download.example.com/finenv/latest.json" />
                  </Field>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <Button
                    variant="secondary"
                    size="sm"
                    loading={checkingUpdate}
                    onClick={async () => {
                      setCheckingUpdate(true);
                      try {
                        const info = await ipc.updateCheck();
                        setUpdateInfo(info);
                        if (info.updateAvailable) toast.info('发现新版本', `${info.latestVersion}：${info.notes ?? ''}`);
                        else toast.success('已是最新版本', info.currentVersion);
                      } catch (error) {
                        toastError(error);
                      } finally {
                        setCheckingUpdate(false);
                      }
                    }}
                  >
                    <RefreshCw className="h-3.5 w-3.5" /> 检查更新
                  </Button>
                  {updateInfo ? (
                    <div className="text-[11px] text-content-weak">
                      {updateInfo.updateAvailable ? (
                        <span className="flex flex-wrap items-center gap-1">
                          新版本 <span className="font-mono text-content-main">{updateInfo.latestVersion}</span>
                          {updateInfo.downloadUrl ? <CopyButton value={updateInfo.downloadUrl} label="复制下载地址" size="sm" /> : null}
                        </span>
                      ) : (
                        <span>{updateInfo.message}</span>
                      )}
                    </div>
                  ) : null}
                </div>
              </CardBody>
            </Card>
          </div>

          <div className="space-y-3">
            <Card>
              <CardHeader title="安全开关" />
              <CardBody className="space-y-3">
                <Switch
                  checked={form.allowSensitiveExport}
                  onCheckedChange={(value) => update('allowSensitiveExport', value)}
                  label="允许高危导出"
                  description="关闭后所有密码、Cookie、环境包导出接口均拒绝。"
                />
                <div className="rounded-md border border-border p-2 text-[10px] leading-4 text-content-hint">
                  IPC 能力白名单仅包含窗口与文件对话框；前端无法调用文件系统、Shell 或任意 HTTP。
                </div>
              </CardBody>
            </Card>
            <Card>
              <CardHeader title="系统信息" />
              <CardBody className="space-y-2 text-[11px]">
                <InfoRow label="版本" value={appInfo?.version ?? '—'} mono />
                <InfoRow label="系统" value={`${appInfo?.os ?? '—'} / ${appInfo?.arch ?? '—'}`} />
                <InfoRow label="API" value={appInfo?.apiBaseUrl ?? '—'} mono />
                <InfoRow label="数据目录" value={appInfo?.dataDir ?? '—'} mono wrap />
                <InfoRow label="数据库" value={appInfo?.dbPath ?? '—'} mono wrap />
              </CardBody>
            </Card>
          </div>
        </div>
      ) : null}

      {tab === 'about' ? (
        <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_380px]">
          <Card>
            <CardHeader
              title={`FinEnv 指纹环境 V${appInfo?.version ?? '—'}`}
              description="多账号独立浏览器环境管理工具 · Tauri 2.0 + Chromium 隔离进程"
            />
            <CardBody className="space-y-3 text-xs leading-6 text-content-weak">
              <p>
                FinEnv 面向研发测试团队，为每个测试账号绑定一套隔离浏览器环境，持久保留登录态、Cookie、缓存与指纹配置，支持手动调试与自动化平台 API 对接。
              </p>
              <div className="grid gap-2 md:grid-cols-2">
                {[
                  ['独立进程', '每个环境独立 user-data-dir / 随机调试端口'],
                  ['指纹一致性', 'UA / Screen / Timezone / Canvas / WebGL / Audio / CH'],
                  ['自动化', 'REST API + JS / Python SDK + CDP'],
                  ['资源管控', '并发上限 / 内存告警 / 闲置回收 / 崩溃日志'],
                  ['安全', 'AES-256-GCM / 不可篡改审计 / 高危确认'],
                  ['主题', '亮暗 / 自定义主色 / 密度 / 导入导出'],
                ].map(([title, text]) => (
                  <div key={title} className="rounded-md border border-border p-2">
                    <p className="font-medium text-content-main">{title}</p>
                    <p className="mt-0.5 text-[11px] text-content-hint">{text}</p>
                  </div>
                ))}
              </div>
              <div className="rounded-md border border-border bg-surface-secondary p-3">
                <p className="flex items-center gap-1.5 font-medium text-content-main"><ShieldCheck className="h-4 w-4 text-primary" /> 首次运行账号</p>
                <p className="mt-1 font-mono text-[11px]">admin / FinEnv@2026</p>
                <p className="mt-1 text-[10px] text-content-hint">请立即在「团队与权限」中修改密码；可通过 FIN_ENV_ADMIN_USER / FIN_ENV_ADMIN_PASSWORD 环境变量覆盖初始值。</p>
              </div>
              <p className="text-[10px] text-content-hint">
                文档：docs/fin-env-requirement.md · docs/fin-env-tech.md · docs/api-docs.md。SDK：sdk/js · sdk/python。
              </p>
            </CardBody>
          </Card>
          <Card>
            <CardHeader title="运行信息" />
            <CardBody className="space-y-2 text-[11px]">
              <InfoRow label="应用版本" value={appInfo?.version ?? '—'} mono />
              <InfoRow label="API 地址" value={appInfo?.apiBaseUrl ?? '—'} mono />
              <InfoRow label="Chromium" value={appInfo?.chromiumVersion ?? '未检测'} />
              <InfoRow label="数据目录" value={appInfo?.dataDir ?? '—'} mono wrap />
              <InfoRow label="数据库" value={appInfo?.dbPath ?? '—'} mono wrap />
            </CardBody>
          </Card>
        </div>
      ) : null}

      <ApiKeyCreateDialog
        open={keyEditor}
        onOpenChange={setKeyEditor}
        onCreated={async () => {
          if (canApiKey) setApiKeys(await ipc.apiKeyList());
        }}
      />
      <UserEditorDialog
        open={!!userEditor}
        onOpenChange={(value) => !value && setUserEditor(null)}
        user={userEditor === 'new' ? null : userEditor}
        onSaved={async () => {
          setUserEditor(null);
          if (canUserManage) setUsers(await ipc.userList());
          await useAuthStore.getState().refresh();
        }}
      />
      <ConfirmDialog
        open={!!deleteUser}
        onOpenChange={(value) => !value && setDeleteUser(null)}
        title="删除团队成员"
        description={`将删除「${deleteUser?.displayName ?? ''}」的登录账号；其历史审计记录保留。`}
        confirmLabel="删除成员"
        onConfirm={async () => {
          if (!deleteUser) return;
          try {
            await ipc.userDelete(deleteUser.id);
            setDeleteUser(null);
            setUsers(await ipc.userList());
            toast.success('成员已删除');
          } catch (error) {
            toastError(error);
          }
        }}
      />
      <ConfirmDialog
        open={!!revokeKey}
        onOpenChange={(value) => !value && setRevokeKey(null)}
        title="吊销 API 密钥"
        description={`吊销「${revokeKey?.name ?? ''}」后，使用该密钥的 CI / SDK 将立即失去访问权限。`}
        confirmLabel="吊销密钥"
        onConfirm={async () => {
          if (!revokeKey) return;
          try {
            await ipc.apiKeyRevoke(revokeKey.id);
            setRevokeKey(null);
            setApiKeys(await ipc.apiKeyList());
            toast.success('API 密钥已吊销');
          } catch (error) {
            toastError(error);
          }
        }}
      />
      <BackupCreateDialog
        open={backupCreate}
        onOpenChange={setBackupCreate}
        onCreated={async () => {
          setBackupCreate(false);
          setBackups(await ipc.backupList());
        }}
      />
      <BackupRestoreDialog
        path={restorePath}
        onClose={() => setRestorePath(null)}
        onRestored={async () => {
          setRestorePath(null);
          await useAuthStore.getState().initialize();
        }}
      />
      <SyncActionDialog
        mode={syncAction}
        onClose={() => setSyncAction(null)}
        onDone={async () => {
          setSyncAction(null);
          if (syncAction === 'pull') await useAuthStore.getState().initialize();
          else if (form) {
            try {
              setForm(await ipc.settingsGet());
            } catch {
              // ignore
            }
          }
        }}
      />
    </div>
  );
}

function InfoRow({ label, value, mono, wrap }: { label: string; value: string; mono?: boolean; wrap?: boolean }) {
  return (
    <div className="flex items-start justify-between gap-3">
      <span className="shrink-0 text-content-weak">{label}</span>
      <span className={cn('text-right text-content-main', mono && 'font-mono text-[10px]', !wrap && 'truncate')}>{value}</span>
    </div>
  );
}

function ApiKeyCreateDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: () => void;
}) {
  const [name, setName] = React.useState('');
  const [role, setRole] = React.useState<Role>('test_lead');
  const [created, setCreated] = React.useState<CreatedApiKey | null>(null);
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setName('');
      setRole('test_lead');
      setCreated(null);
    }
  }, [open]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={created ? 'API 密钥已创建' : '创建 API 密钥'}
      description={created ? '密钥仅显示一次，请立即保存到 CI Secret 中。' : '密钥继承所选角色的权限，建议为 CI 使用最小权限。'}
      width="max-w-lg"
      footer={
        created ? (
          <Button variant="primary" onClick={() => onOpenChange(false)}>我已保存</Button>
        ) : (
          <>
            <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>取消</Button>
            <Button
              variant="primary"
              loading={busy}
              disabled={!name.trim()}
              onClick={async () => {
                setBusy(true);
                try {
                  const key = await ipc.apiKeyCreate(name.trim(), role);
                  setCreated(key);
                  onCreated();
                } catch (error) {
                  toastError(error);
                } finally {
                  setBusy(false);
                }
              }}
            >
              创建密钥
            </Button>
          </>
        )
      }
    >
      {created ? (
        <div className="space-y-3">
          <div className="rounded-md border border-border bg-surface-secondary p-3">
            <p className="text-[10px] text-content-hint">Secret</p>
            <div className="mt-1 flex items-center gap-2">
              <code className="min-w-0 flex-1 break-all font-mono text-xs">{created.secret}</code>
              <CopyButton value={created.secret} label="复制" size="sm" />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2 text-xs">
            <InfoRow label="名称" value={created.name} />
            <InfoRow label="角色" value={roleLabels[created.role]} />
            <InfoRow label="前缀" value={created.prefix} mono />
            <InfoRow label="创建时间" value={formatDate(created.createdAt)} />
          </div>
          <p className="rounded-md bg-[color-mix(in_srgb,var(--warning)_10%,transparent)] p-2 text-[11px] text-[var(--warning)]">
            关闭后无法再次查看完整密钥；如遗失请吊销并重新创建。
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          <Field label="密钥名称" required>
            <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：GitLab CI - 订单回归" autoFocus />
          </Field>
          <Field label="授予角色">
            <Select value={role} onChange={(event) => setRole(event.target.value as Role)}>
              {roles.map((item) => (
                <option key={item} value={item}>{roleLabels[item]}</option>
              ))}
            </Select>
          </Field>
          <p className="text-[11px] text-content-hint">
            测试工程师可打开/关闭环境、截图、修改指纹；导出 Cookie/账号需要测试负责人及以上角色。
          </p>
        </div>
      )}
    </Dialog>
  );
}

function UserEditorDialog({
  open,
  onOpenChange,
  user,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  user: User | null;
  onSaved: () => void;
}) {
  const toast = useToastStore();
  const [busy, setBusy] = React.useState(false);
  const [form, setForm] = React.useState({
    username: '',
    displayName: '',
    role: 'test_engineer' as Role,
    password: '',
    enabled: true,
  });

  React.useEffect(() => {
    if (!open) return;
    setForm({
      username: user?.username ?? '',
      displayName: user?.displayName ?? '',
      role: user?.role ?? 'test_engineer',
      password: '',
      enabled: user?.enabled ?? true,
    });
  }, [open, user]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={user ? '编辑团队成员' : '新增团队成员'}
      description={user ? '密码留空表示不修改。' : '用户名创建后不可修改。'}
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>取消</Button>
          <Button
            variant="primary"
            loading={busy}
            onClick={async () => {
              if (!form.username.trim() || !form.displayName.trim()) {
                toast.warning('请填写用户名与显示名称');
                return;
              }
              if (!user && form.password.length < 8) {
                toast.warning('初始密码至少 8 个字符');
                return;
              }
              setBusy(true);
              try {
                if (user) {
                  await ipc.userUpdate(user.id, {
                    displayName: form.displayName.trim(),
                    role: form.role,
                    password: form.password || undefined,
                    enabled: form.enabled,
                  });
                  toast.success('成员已更新');
                } else {
                  await ipc.userCreate({
                    username: form.username.trim(),
                    displayName: form.displayName.trim(),
                    role: form.role,
                    password: form.password,
                    enabled: form.enabled,
                  });
                  toast.success('成员已创建');
                }
                onSaved();
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            保存
          </Button>
        </>
      }
    >
      <div className="grid grid-cols-2 gap-3">
        <Field label="用户名" required>
          <Input value={form.username} onChange={(event) => setForm((current) => ({ ...current, username: event.target.value }))} disabled={!!user} autoFocus={!user} />
        </Field>
        <Field label="显示名称" required>
          <Input value={form.displayName} onChange={(event) => setForm((current) => ({ ...current, displayName: event.target.value }))} />
        </Field>
        <Field label="角色">
          <Select value={form.role} onChange={(event) => setForm((current) => ({ ...current, role: event.target.value as Role }))}>
            {roles.map((role) => <option key={role} value={role}>{roleLabels[role]}</option>)}
          </Select>
        </Field>
        <Field label={user ? '新密码（留空不修改）' : '初始密码'} hint="至少 8 个字符">
          <Input type="password" value={form.password} onChange={(event) => setForm((current) => ({ ...current, password: event.target.value }))} />
        </Field>
        <div className="col-span-2 rounded-md border border-border bg-surface-secondary p-3">
          <Switch checked={form.enabled} onCheckedChange={(value) => setForm((current) => ({ ...current, enabled: value }))} label="启用该成员" />
        </div>
      </div>
    </Dialog>
  );
}

function BackupCreateDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: () => void;
}) {
  const toast = useToastStore();
  const [path, setPath] = React.useState('');
  const [passphrase, setPassphrase] = React.useState('');
  const [includeProfiles, setIncludeProfiles] = React.useState(true);
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setPath('');
      setPassphrase('');
      setIncludeProfiles(true);
    }
  }, [open]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="创建本地备份"
      description="包含 SQLite 一致性快照与元数据；可选择包含浏览器 profile（登录态）。"
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>取消</Button>
          <Button
            variant="primary"
            loading={busy}
            onClick={async () => {
              setBusy(true);
              try {
                const info = await ipc.backupCreate(path || undefined, passphrase || undefined, includeProfiles);
                toast.success('备份已创建', `${info.name} · ${formatBytes(info.size)}`);
                onCreated();
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            <DatabaseBackup className="h-4 w-4" /> 创建备份
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="保存路径" hint="留空保存到数据目录 backups/">
          <div className="flex gap-2">
            <Input value={path} onChange={(event) => setPath(event.target.value)} placeholder="自动生成" />
            <Button
              variant="secondary"
              onClick={async () => {
                try {
                  const target = await saveFile(`finenv-${new Date().toISOString().slice(0, 10)}.finenvbackup`, backupFilter());
                  if (target) setPath(target);
                } catch (error) {
                  toastError(error);
                }
              }}
            >
              选择
            </Button>
          </div>
        </Field>
        <Field label="加密口令" hint="留空则不加密；跨设备恢复建议加密">
          <Input type="password" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} />
        </Field>
        <div className="rounded-md border border-border bg-surface-secondary p-3">
          <Switch checked={includeProfiles} onCheckedChange={setIncludeProfiles} label="包含浏览器 profile" description="体积较大，包含 Cookie 与登录态；建议升级前备份。" />
        </div>
      </div>
    </Dialog>
  );
}

function BackupRestoreDialog({
  path,
  onClose,
  onRestored,
}: {
  path: string | null;
  onClose: () => void;
  onRestored: () => Promise<void>;
}) {
  const toast = useToastStore();
  const [passphrase, setPassphrase] = React.useState('');
  const [restoreProfiles, setRestoreProfiles] = React.useState(true);
  const [confirmText, setConfirmText] = React.useState('');
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    if (path) {
      setPassphrase('');
      setRestoreProfiles(true);
      setConfirmText('');
    }
  }, [path]);

  return (
    <Dialog
      open={!!path}
      onOpenChange={(value) => !value && onClose()}
      title="恢复本地备份"
      description={path ?? ''}
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={onClose} disabled={busy}>取消</Button>
          <Button
            variant="danger"
            loading={busy}
            disabled={confirmText !== 'RESTORE' || !path}
            onClick={async () => {
              if (!path) return;
              setBusy(true);
              try {
                const message = await ipc.backupRestore(path, passphrase || undefined, restoreProfiles, true);
                toast.success('备份已恢复', message);
                await onRestored();
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            覆盖并恢复
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="rounded-md border border-[color-mix(in_srgb,var(--danger)_30%,transparent)] bg-[color-mix(in_srgb,var(--danger)_7%,transparent)] p-2 text-[11px] leading-5 text-danger">
          将强制关闭全部浏览器并用备份数据库覆盖当前数据；当前未备份的变更会丢失。
        </div>
        <Field label="备份口令" hint="未加密备份留空">
          <Input type="password" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} />
        </Field>
        <div className="rounded-md border border-border bg-surface-secondary p-3">
          <Switch checked={restoreProfiles} onCheckedChange={setRestoreProfiles} label="同时恢复浏览器 profile" description="若备份包包含 profiles 则覆盖本地环境目录。" />
        </div>
        <Field label='输入 "RESTORE" 确认' required>
          <Input value={confirmText} onChange={(event) => setConfirmText(event.target.value.toUpperCase())} placeholder="RESTORE" />
        </Field>
      </div>
    </Dialog>
  );
}

function SyncActionDialog({
  mode,
  onClose,
  onDone,
}: {
  mode: 'push' | 'pull' | null;
  onClose: () => void;
  onDone: () => Promise<void>;
}) {
  const toast = useToastStore();
  const [passphrase, setPassphrase] = React.useState('');
  const [confirmText, setConfirmText] = React.useState('');
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    if (mode) {
      setPassphrase('');
      setConfirmText('');
    }
  }, [mode]);

  return (
    <Dialog
      open={!!mode}
      onOpenChange={(value) => !value && onClose()}
      title={mode === 'pull' ? '下载并恢复同步包' : '上传加密同步包'}
      description={mode === 'pull' ? '将覆盖本地数据库并退出登录；请确认已保存重要数据。' : '同步包在本机整体加密后上传，服务端无法解密。'}
      width="max-w-md"
      footer={
        <>
          <Button variant="secondary" onClick={onClose} disabled={busy}>取消</Button>
          <Button
            variant={mode === 'pull' ? 'danger' : 'primary'}
            loading={busy}
            disabled={passphrase.length < 6 || (mode === 'pull' && confirmText !== 'SYNC')}
            onClick={async () => {
              if (!mode) return;
              setBusy(true);
              try {
                if (mode === 'push') {
                  const result = await ipc.syncPush(passphrase);
                  toast.success('同步包已上传', `${result.bytes} bytes`);
                } else {
                  const result = await ipc.syncPull(passphrase, true);
                  toast.success('同步包已恢复', result.message);
                }
                await onDone();
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            {mode === 'pull' ? '下载并覆盖' : '上传'}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="同步口令" required hint="至少 6 个字符，丢失将无法解密">
          <Input type="password" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} autoFocus />
        </Field>
        {mode === 'pull' ? (
          <Field label='输入 "SYNC" 确认覆盖' required>
            <Input value={confirmText} onChange={(event) => setConfirmText(event.target.value.toUpperCase())} placeholder="SYNC" />
          </Field>
        ) : null}
      </div>
    </Dialog>
  );
}
