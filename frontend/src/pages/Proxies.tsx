import * as React from 'react';
import {
  CheckCircle2,
  CircleOff,
  Copy,
  Download,
  Globe2,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Server,
  ShieldCheck,
  Trash2,
  Users,
  Wifi,
  XCircle,
} from 'lucide-react';
import type { EnvironmentView, Proxy, ProxyCheckResult } from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { useEnvStore } from '@/store/env';
import { toastError, useToastStore } from '@/store/toast';
import { copyText, formatDateShort } from '@/utils';
import { Badge, StatusDot } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, EmptyState, Spinner, StatCard } from '@/components/ui/card';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input, Select } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { Tooltip } from '@/components/ui/tooltip';

export function ProxiesPage() {
  const toast = useToastStore();
  const canEdit = useAuthStore((state) => state.can('edit_fingerprint_proxy'));
  const envs = useEnvStore((state) => state.envs);
  const [proxies, setProxies] = React.useState<Proxy[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [search, setSearch] = React.useState('');
  const [editor, setEditor] = React.useState<Proxy | 'new' | null>(null);
  const [deleteTarget, setDeleteTarget] = React.useState<Proxy | null>(null);
  const [checkingId, setCheckingId] = React.useState<string | null>(null);
  const [checkingAll, setCheckingAll] = React.useState(false);
  const [results, setResults] = React.useState<Record<string, ProxyCheckResult>>({});
  const [assignTarget, setAssignTarget] = React.useState<Proxy | null>(null);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      setProxies(await ipc.proxyList());
      if (useEnvStore.getState().envs.length === 0) await useEnvStore.getState().refresh(true);
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    void load();
  }, [load]);

  const boundCount = (proxyId: string) => envs.filter((env) => env.proxyId === proxyId).length;

  const filtered = proxies.filter((proxy) => {
    const query = search.trim().toLowerCase();
    if (!query) return true;
    return [proxy.name, proxy.host, proxy.username, proxy.exitIp ?? ''].some((value) =>
      value.toLowerCase().includes(query),
    );
  });

  const check = async (proxy: Proxy) => {
    setCheckingId(proxy.id);
    try {
      const result = await ipc.proxyCheck(proxy.id);
      setResults((current) => ({ ...current, [proxy.id]: result }));
      if (result.ok) toast.success(`代理 ${proxy.name} 连通`, `${result.exitIp ?? '未知出口'} · ${result.latencyMs ?? '—'}ms`);
      else toast.error(`代理 ${proxy.name} 检测失败`, result.message);
      setProxies(await ipc.proxyList());
    } catch (error) {
      toastError(error);
    } finally {
      setCheckingId(null);
    }
  };

  const checkAll = async () => {
    setCheckingAll(true);
    let ok = 0;
    try {
      for (const proxy of proxies.filter((item) => item.enabled)) {
        try {
          const result = await ipc.proxyCheck(proxy.id);
          setResults((current) => ({ ...current, [proxy.id]: result }));
          if (result.ok) ok += 1;
        } catch {
          // Keep checking the rest; the summary reports successes.
        }
      }
      toast.success('批量检测完成', `${ok}/${proxies.filter((item) => item.enabled).length} 个代理可用`);
      setProxies(await ipc.proxyList());
    } finally {
      setCheckingAll(false);
    }
  };

  const statsCountries = new Set(proxies.map((proxy) => proxy.exitCountry).filter(Boolean)).size;

  return (
    <div className="page-pad space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-[10px] font-semibold uppercase tracking-widest text-content-hint">Network / Proxy Pool</p>
          <h1 className="mt-1 text-xl font-semibold tracking-tight text-content-main">代理池</h1>
          <p className="mt-1 max-w-2xl text-xs leading-5 text-content-weak">
            管理 HTTP / HTTPS / SOCKS5 代理，检测 TCP 连通性、出口 IP、国家与时区；支持批量绑定到环境。
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="secondary" onClick={() => void checkAll()} loading={checkingAll} disabled={proxies.length === 0}>
            <RefreshCw className="h-4 w-4" /> 全部检测
          </Button>
          <Button variant="secondary" onClick={() => void load()}>
            <RefreshCw className="h-4 w-4" /> 刷新
          </Button>
          {canEdit ? (
            <Button variant="primary" onClick={() => setEditor('new')}>
              <Plus className="h-4 w-4" /> 新增代理
            </Button>
          ) : null}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatCard label="代理总数" value={proxies.length} hint={`${proxies.filter((p) => p.enabled).length} 个启用`} icon={<Server className="h-4 w-4" />} tone="primary" />
        <StatCard label="已绑定环境" value={envs.filter((env) => env.proxyId).length} hint={`${envs.length} 个环境`} icon={<Users className="h-4 w-4" />} />
        <StatCard label="可用出口" value={Object.values(results).filter((r) => r.ok).length} hint="本次会话检测结果" icon={<Wifi className="h-4 w-4" />} tone="success" />
        <StatCard label="出口国家" value={statsCountries} hint="按最近一次检测统计" icon={<Globe2 className="h-4 w-4" />} />
      </div>

      <Card className="overflow-hidden">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border p-4">
          <div className="relative w-full max-w-sm">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-content-hint" />
            <Input className="pl-9" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索代理名称、主机、出口 IP…" />
          </div>
          <p className="text-[11px] text-content-hint">密码使用 AES-256-GCM 加密保存，列表不返回明文。</p>
        </div>

        {loading ? (
          <div className="flex justify-center py-16"><Spinner /></div>
        ) : filtered.length === 0 ? (
          <EmptyState
            icon={<Server className="h-5 w-5" />}
            title={proxies.length === 0 ? '代理池为空' : '没有匹配的代理'}
            description={proxies.length === 0 ? '添加代理后，可为每个环境独立绑定，并进行 IP / 时区对齐。' : undefined}
            action={
              canEdit && proxies.length === 0 ? (
                <Button variant="primary" size="sm" onClick={() => setEditor('new')}>
                  <Plus className="h-3.5 w-3.5" /> 新增代理
                </Button>
              ) : undefined
            }
          />
        ) : (
          <div className="table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>代理</th>
                  <th>协议 / 地址</th>
                  <th>认证</th>
                  <th>状态</th>
                  <th>出口信息</th>
                  <th>延迟</th>
                  <th>绑定环境</th>
                  <th className="text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((proxy) => {
                  const result = results[proxy.id];
                  const ok = result?.ok ?? (proxy.exitIp ? true : null);
                  return (
                    <tr key={proxy.id}>
                      <td>
                        <div className="flex items-center gap-2">
                          <StatusDot tone={proxy.enabled ? 'success' : 'muted'} pulse={proxy.enabled && !!ok} />
                          <div>
                            <p className="text-sm font-medium text-content-main">{proxy.name}</p>
                            <p className="font-mono text-[10px] text-content-hint">{proxy.id.slice(0, 8)}…</p>
                          </div>
                        </div>
                      </td>
                      <td>
                        <div className="flex items-center gap-1">
                          <Badge tone="outline">{proxy.protocol.toUpperCase()}</Badge>
                          <span className="font-mono text-[11px] text-content-main">{proxy.host}:{proxy.port}</span>
                          <Tooltip content="复制代理地址">
                            <Button variant="ghost" size="icon-sm" onClick={() => void copyText(`${proxy.protocol}://${proxy.host}:${proxy.port}`)}>
                              <Copy className="h-3 w-3" />
                            </Button>
                          </Tooltip>
                        </div>
                      </td>
                      <td className="text-xs text-content-weak">
                        {proxy.username ? (
                          <span className="font-mono">{proxy.username}</span>
                        ) : (
                          <span className="text-content-hint">无</span>
                        )}
                        {proxy.username ? (
                          <Badge tone={proxy.hasPassword ? 'success' : 'warning'} className="ml-1">
                            {proxy.hasPassword ? '密码已存' : '无密码'}
                          </Badge>
                        ) : null}
                      </td>
                      <td>
                        {proxy.enabled ? (
                          ok === true ? (
                            <Badge tone="success"><CheckCircle2 className="h-3 w-3" /> 可用</Badge>
                          ) : ok === false ? (
                            <Badge tone="danger"><XCircle className="h-3 w-3" /> 失败</Badge>
                          ) : (
                            <Badge tone="neutral">未检测</Badge>
                          )
                        ) : (
                          <Badge tone="neutral"><CircleOff className="h-3 w-3" /> 已停用</Badge>
                        )}
                      </td>
                      <td>
                        {proxy.exitIp ? (
                          <div className="space-y-0.5 text-[11px]">
                            <p className="font-mono text-content-main">{proxy.exitIp}</p>
                            <p className="text-content-hint">
                              {proxy.exitCountry ?? '—'} {proxy.exitTimezone ? `· ${proxy.exitTimezone}` : ''}
                            </p>
                            <p className="text-[10px] text-content-hint">{proxy.lastCheckAt ? formatDateShort(proxy.lastCheckAt) : ''}</p>
                          </div>
                        ) : (
                          <span className="text-xs text-content-hint">{result?.message ?? '尚未检测'}</span>
                        )}
                      </td>
                      <td className="text-xs tabular-nums text-content-weak">
                        {proxy.latencyMs != null ? `${proxy.latencyMs}ms` : '—'}
                      </td>
                      <td>
                        <Badge tone={boundCount(proxy.id) > 0 ? 'primary' : 'neutral'}>{boundCount(proxy.id)}</Badge>
                      </td>
                      <td>
                        <div className="flex items-center justify-end gap-1">
                          <Button variant="ghost" size="sm" loading={checkingId === proxy.id} onClick={() => void check(proxy)}>
                            <Wifi className="h-3.5 w-3.5" /> 检测
                          </Button>
                          {canEdit ? (
                            <>
                              <Button variant="ghost" size="sm" onClick={() => setAssignTarget(proxy)}>
                                <Users className="h-3.5 w-3.5" /> 分配
                              </Button>
                              <Button variant="ghost" size="icon-sm" onClick={() => setEditor(proxy)}>
                                <Pencil className="h-3.5 w-3.5" />
                              </Button>
                              <Button variant="ghost" size="icon-sm" className="text-danger" onClick={() => setDeleteTarget(proxy)}>
                                <Trash2 className="h-3.5 w-3.5" />
                              </Button>
                            </>
                          ) : null}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <div className="grid gap-3 md:grid-cols-2">
        <Card className="p-4 text-xs text-content-weak">
          <p className="flex items-center gap-1.5 font-medium text-content-main"><ShieldCheck className="h-4 w-4 text-primary" /> 检测与对齐</p>
          <p className="mt-1.5 leading-5">
            检测先做 TCP 握手，再通过代理请求出口 IP 服务，返回国家、时区与城市。环境详情中可一键按时区与语言对齐指纹。
          </p>
        </Card>
        <Card className="p-4 text-xs text-content-weak">
          <p className="flex items-center gap-1.5 font-medium text-content-main"><Download className="h-4 w-4 text-primary" /> 批量绑定</p>
          <p className="mt-1.5 leading-5">
            在代理行点击「分配」可勾选多个环境批量绑定；运行中的环境需要重启后代理才会生效。
          </p>
        </Card>
      </div>

      <ProxyEditorDialog
        open={!!editor}
        onOpenChange={(value) => !value && setEditor(null)}
        proxy={editor === 'new' ? null : editor}
        onSaved={() => {
          setEditor(null);
          void load();
        }}
      />

      <AssignEnvsDialog
        open={!!assignTarget}
        onOpenChange={(value) => !value && setAssignTarget(null)}
        proxy={assignTarget}
        envs={envs}
        onDone={() => {
          void load();
          void useEnvStore.getState().refresh(false);
        }}
      />

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(value) => !value && setDeleteTarget(null)}
        title="删除代理"
        description={`将删除「${deleteTarget?.name ?? ''}」并从 ${deleteTarget ? boundCount(deleteTarget.id) : 0} 个环境解绑。运行中的环境需要重启。`}
        confirmLabel="删除代理"
        onConfirm={async () => {
          if (!deleteTarget) return;
          try {
            await ipc.proxyDelete(deleteTarget.id, true);
            setDeleteTarget(null);
            toast.success('代理已删除');
            await load();
          } catch (error) {
            toastError(error);
          }
        }}
      />
    </div>
  );
}

function ProxyEditorDialog({
  open,
  onOpenChange,
  proxy,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  proxy: Proxy | null;
  onSaved: () => void;
}) {
  const toast = useToastStore();
  const [busy, setBusy] = React.useState(false);
  const [form, setForm] = React.useState({
    name: '',
    protocol: 'http' as Proxy['protocol'],
    host: '',
    port: '8080',
    username: '',
    password: '',
    enabled: true,
  });

  React.useEffect(() => {
    if (!open) return;
    setForm({
      name: proxy?.name ?? '',
      protocol: proxy?.protocol ?? 'http',
      host: proxy?.host ?? '',
      port: proxy ? String(proxy.port) : '8080',
      username: proxy?.username ?? '',
      password: '',
      enabled: proxy?.enabled ?? true,
    });
  }, [open, proxy]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={proxy ? '编辑代理' : '新增代理'}
      description="代理密码立即加密；编辑时留空表示保持原密码。"
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>取消</Button>
          <Button
            variant="primary"
            loading={busy}
            onClick={async () => {
              if (!form.name.trim() || !form.host.trim()) {
                toast.warning('请填写代理名称与主机');
                return;
              }
              setBusy(true);
              try {
                if (proxy) {
                  await ipc.proxyUpdate(proxy.id, {
                    name: form.name.trim(),
                    protocol: form.protocol,
                    host: form.host.trim(),
                    port: Number(form.port),
                    username: form.username,
                    password: form.password || undefined,
                    enabled: form.enabled,
                  });
                  toast.success('代理已更新');
                } else {
                  await ipc.proxyCreate({
                    name: form.name.trim(),
                    protocol: form.protocol,
                    host: form.host.trim(),
                    port: Number(form.port),
                    username: form.username || null,
                    password: form.password || null,
                    enabled: form.enabled,
                  });
                  toast.success('代理已创建');
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
        <Field label="代理名称" required className="col-span-2">
          <Input value={form.name} onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))} placeholder="例如：上海测试出口" autoFocus />
        </Field>
        <Field label="协议">
          <Select value={form.protocol} onChange={(event) => setForm((current) => ({ ...current, protocol: event.target.value as Proxy['protocol'] }))}>
            <option value="http">HTTP</option>
            <option value="https">HTTPS</option>
            <option value="socks5">SOCKS5</option>
          </Select>
        </Field>
        <Field label="端口" required>
          <Input type="number" min={1} max={65535} value={form.port} onChange={(event) => setForm((current) => ({ ...current, port: event.target.value }))} />
        </Field>
        <Field label="主机" required className="col-span-2">
          <Input value={form.host} onChange={(event) => setForm((current) => ({ ...current, host: event.target.value }))} placeholder="proxy.example.com 或 10.0.0.8" />
        </Field>
        <Field label="用户名">
          <Input value={form.username} onChange={(event) => setForm((current) => ({ ...current, username: event.target.value }))} autoComplete="off" />
        </Field>
        <Field label={proxy?.hasPassword ? '新密码（留空不修改）' : '密码'}>
          <Input type="password" value={form.password} onChange={(event) => setForm((current) => ({ ...current, password: event.target.value }))} autoComplete="new-password" placeholder={proxy?.hasPassword ? '••••••••' : ''} />
        </Field>
        <div className="col-span-2 rounded-md border border-border bg-surface-secondary p-3">
          <Switch checked={form.enabled} onCheckedChange={(value) => setForm((current) => ({ ...current, enabled: value }))} label="启用该代理" description="停用后绑定环境将无法打开，已打开进程不受影响。" />
        </div>
      </div>
    </Dialog>
  );
}

function AssignEnvsDialog({
  open,
  onOpenChange,
  proxy,
  envs,
  onDone,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  proxy: Proxy | null;
  envs: EnvironmentView[];
  onDone: () => void;
}) {
  const toast = useToastStore();
  const [selected, setSelected] = React.useState<string[]>([]);
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    if (!open || !proxy) return;
    setSelected(envs.filter((env) => env.proxyId === proxy.id).map((env) => env.id));
  }, [open, proxy, envs]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="分配代理到环境"
      description={`将「${proxy?.name ?? ''}」绑定到选中的环境；已绑定的环境可取消。`}
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>取消</Button>
          <Button
            variant="primary"
            loading={busy}
            onClick={async () => {
              if (!proxy) return;
              setBusy(true);
              try {
                const previouslyBound = envs
                  .filter((env) => env.proxyId === proxy.id && !selected.includes(env.id))
                  .map((env) => env.id);
                if (previouslyBound.length > 0) {
                  await ipc.proxyAssign(previouslyBound, null);
                }
                const count = await ipc.proxyAssign(selected, proxy.id);
                toast.success('代理分配完成', `${count} 个环境已更新${previouslyBound.length ? `，${previouslyBound.length} 个环境已解绑` : ''}`);
                onDone();
                onOpenChange(false);
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            保存分配
          </Button>
        </>
      }
    >
      <div className="mb-2 flex items-center justify-between text-[11px] text-content-hint">
        <span>已选 {selected.length} 个环境</span>
        <button type="button" className="text-primary" onClick={() => setSelected(selected.length === envs.length ? [] : envs.map((env) => env.id))}>
          {selected.length === envs.length ? '取消全选' : '全选'}
        </button>
      </div>
      <div className="max-h-72 space-y-1 overflow-y-auto rounded-md border border-border p-2">
        {envs.map((env) => (
          <label key={env.id} className="flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-xs hover:bg-surface-secondary">
            <input
              type="checkbox"
              className="accent-[var(--color-primary)]"
              checked={selected.includes(env.id)}
              onChange={(event) =>
                setSelected((current) => event.target.checked ? [...current, env.id] : current.filter((id) => id !== env.id))
              }
            />
            <span className="min-w-0 flex-1 truncate">{env.name}</span>
            {env.proxyId === proxy?.id ? <Badge tone="primary">已绑定</Badge> : null}
            {env.runtime ? <Badge tone="success">运行中</Badge> : null}
          </label>
        ))}
      </div>
      <p className="mt-2 text-[10px] text-content-hint">保存后运行中的环境需重启浏览器，代理才会生效。</p>
    </Dialog>
  );
}
