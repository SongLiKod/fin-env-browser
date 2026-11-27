import * as React from 'react';
import {
  CheckCircle2,
  CircleOff,
  CloudDownload,
  Copy,
  FileText,
  Globe2,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Server,
  ShieldCheck,
  Trash2,
  Upload,
  Users,
  Wifi,
  XCircle,
} from 'lucide-react';
import type {
  EnvironmentView,
  Proxy,
  ProxyBatchProgress,
  ProxyCheckResult,
  ProxyDiscoveryInput,
  ProxyDiscoveryProgress,
  ProxyDiscoveryResult,
  ProxyFilter,
  ProxyImportOptions,
  ProxyImportPreview,
  ProxyPage,
  ProxyStats,
  ProxySubscription,
} from '@/ipc/types';
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
import { Field, Input, Select, Textarea } from '@/components/ui/input';
import { ProxyLocationSummary } from '@/components/ui/proxy-location';
import { Switch } from '@/components/ui/switch';
import { Tabs } from '@/components/ui/tabs';
import { Tooltip } from '@/components/ui/tooltip';

const PAGE_SIZE = 100;

type ProxyTab = 'list' | 'subscriptions';

export function ProxiesPage() {
  const toast = useToastStore();
  const canEdit = useAuthStore((state) => state.can('edit_fingerprint_proxy'));
  const envs = useEnvStore((state) => state.envs);
  const [tab, setTab] = React.useState<ProxyTab>('list');
  const [page, setPage] = React.useState(0);
  const [search, setSearch] = React.useState('');
  const [groupFilter, setGroupFilter] = React.useState('');
  const [sourceFilter, setSourceFilter] = React.useState('');
  const [statusFilter, setStatusFilter] = React.useState('');
  const [data, setData] = React.useState<ProxyPage>({ items: [], total: 0 });
  const [stats, setStats] = React.useState<ProxyStats>({
    total: 0,
    enabled: 0,
    available: 0,
    failed: 0,
    untested: 0,
    disabled: 0,
    subscriptions: 0,
  });
  const [subscriptions, setSubscriptions] = React.useState<ProxySubscription[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [selected, setSelected] = React.useState<string[]>([]);
  const [editor, setEditor] = React.useState<Proxy | 'new' | null>(null);
  const [assignTarget, setAssignTarget] = React.useState<Proxy | null>(null);
  const [deleteTarget, setDeleteTarget] = React.useState<Proxy | null>(null);
  const [bulkDeleteOpen, setBulkDeleteOpen] = React.useState(false);
  const [bulkMetaOpen, setBulkMetaOpen] = React.useState(false);
  const [cleanupOpen, setCleanupOpen] = React.useState(false);
  const [cleanupStatuses, setCleanupStatuses] = React.useState<string[]>(['disabled', 'fail']);
  const [importOpen, setImportOpen] = React.useState(false);
  const [discoveryOpen, setDiscoveryOpen] = React.useState(false);
  const [checkingId, setCheckingId] = React.useState<string | null>(null);
  const [job, setJob] = React.useState<ProxyBatchProgress | null>(null);
  const [jobResults, setJobResults] = React.useState<ProxyCheckResult[]>([]);
  const [jobOpen, setJobOpen] = React.useState(false);
  const [subEditor, setSubEditor] = React.useState<ProxySubscription | 'new' | null>(null);
  const [subDelete, setSubDelete] = React.useState<ProxySubscription | null>(null);
  const [subDeleteProxies, setSubDeleteProxies] = React.useState(false);
  const [refreshingSub, setRefreshingSub] = React.useState<string | null>(null);

  const filter = React.useMemo<ProxyFilter>(
    () => ({
      search: search.trim() || undefined,
      groupName: groupFilter.trim() || undefined,
      source: sourceFilter || undefined,
      status: statusFilter || undefined,
      limit: PAGE_SIZE,
      offset: page * PAGE_SIZE,
    }),
    [search, groupFilter, sourceFilter, statusFilter, page],
  );

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const [next, nextStats, nextSubs] = await Promise.all([
        ipc.proxyQuery(filter),
        ipc.proxyStats(),
        ipc.proxySubscriptionList(),
      ]);
      setData(next);
      setStats(nextStats);
      setSubscriptions(nextSubs);
      if (useEnvStore.getState().envs.length === 0) await useEnvStore.getState().refresh(true);
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, [filter]);

  React.useEffect(() => {
    void load();
  }, [load]);

  React.useEffect(() => {
    setSelected((current) => current.filter((id) => data.items.some((proxy) => proxy.id === id)));
  }, [data.items]);

  React.useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void ipc
      .onProxyBatch((progress) => {
        setJob(progress);
        if (progress.state !== 'running') {
          void ipc
            .proxyJobGet(progress.jobId)
            .then((view) => setJobResults(view.results))
            .catch(() => undefined);
          void load();
        }
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [load]);

  React.useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void ipc
      .onProxyChanged(() => {
        void load();
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [load]);

  React.useEffect(() => {
    if (!job || job.state !== 'running') return;
    const timer = window.setInterval(() => {
      void ipc
        .proxyJobGet(job.jobId)
        .then((view) => {
          setJob(view.progress);
          if (view.progress.state !== 'running') {
            setJobResults(view.results);
            void load();
            window.clearInterval(timer);
          }
        })
        .catch(() => undefined);
    }, 1800);
    return () => window.clearInterval(timer);
  }, [job?.jobId, job?.state, load]);

  const boundCount = (proxyId: string) => envs.filter((env) => env.proxyId === proxyId).length;
  const totalPages = Math.max(1, Math.ceil(data.total / PAGE_SIZE));
  const allVisibleSelected =
    data.items.length > 0 && data.items.every((proxy) => selected.includes(proxy.id));

  const resetPage = () => setPage(0);

  const checkOne = async (proxy: Proxy) => {
    setCheckingId(proxy.id);
    try {
      const result = await ipc.proxyCheck(proxy.id);
      if (result.ok) {
        toast.success(
          `代理 ${proxy.name} 连通`,
          `${result.exitIp ?? '未知出口'} · ${result.latencyMs ?? '—'}ms · ${result.message}`,
        );
      } else {
        toast.error(`代理 ${proxy.name} 检测失败`, result.message);
      }
      await load();
    } catch (error) {
      toastError(error);
    } finally {
      setCheckingId(null);
    }
  };

  const startBatch = async (input: Parameters<typeof ipc.proxyBatchCheck>[0]) => {
    try {
      const start = await ipc.proxyBatchCheck(input);
      setJobResults([]);
      setJob({
        jobId: start.jobId,
        kind: 'check',
        state: 'running',
        total: start.total,
        completed: 0,
        ok: 0,
        failed: 0,
        startedAt: new Date().toISOString(),
        finishedAt: null,
        message: `正在检测 0/${start.total}`,
        recent: [],
      });
      setJobOpen(true);
      setSelected([]);
      toast.info('批量检测已开始', `${start.total} 个代理`);
    } catch (error) {
      toastError(error);
    }
  };

  const runBatchCheck = () => {
    if (selected.length > 0) {
      void startBatch({ ids: selected, concurrency: 8 });
    } else {
      void startBatch({ filter: { ...filter, limit: undefined, offset: undefined }, onlyEnabled: true, concurrency: 8 });
    }
  };

  const toggleSelected = (id: string, checked: boolean) => {
    setSelected((current) => (checked ? [...current, id] : current.filter((value) => value !== id)));
  };

  const toggleAll = (checked: boolean) => {
    setSelected(checked ? data.items.map((proxy) => proxy.id) : []);
  };

  const deleteOne = async () => {
    if (!deleteTarget) return;
    try {
      await ipc.proxyDelete(deleteTarget.id, true);
      setDeleteTarget(null);
      toast.success('代理已删除');
      await load();
    } catch (error) {
      toastError(error);
    }
  };

  const bulkSetEnabled = async (enabled: boolean) => {
    try {
      const count = await ipc.proxyBatchSetEnabled(selected, enabled);
      toast.success(enabled ? '代理已启用' : '代理已停用', `${count} 个`);
      setSelected([]);
      await load();
    } catch (error) {
      toastError(error);
    }
  };

  const refreshSubscription = async (subscription: ProxySubscription) => {
    setRefreshingSub(subscription.id);
    try {
      const result = await ipc.proxySubscriptionRefresh(subscription.id);
      toast.success('订阅刷新完成', result.message);
      await load();
    } catch (error) {
      toastError(error);
      await load();
    } finally {
      setRefreshingSub(null);
    }
  };

  const checkSubscription = (subscription: ProxySubscription) => {
    void startBatch({ filter: { subscriptionId: subscription.id }, onlyEnabled: true, concurrency: 8 });
  };

  return (
    <div className="page-pad space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-[10px] font-semibold uppercase tracking-widest text-content-hint">
            Network / Proxy Pool
          </p>
          <h1 className="mt-1 text-xl font-semibold tracking-tight text-content-main">代理池</h1>
          <p className="mt-1 max-w-2xl text-xs leading-5 text-content-weak">
            管理 HTTP / HTTPS / SOCKS5 代理与订阅源，支持批量导入、批量检测、分组标签和停用节点清理。检测会同时验证 Google / YouTube 可达性。
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="secondary"
            onClick={() => (job?.state === 'running' ? setJobOpen(true) : runBatchCheck())}
            disabled={data.total === 0 && job?.state !== 'running'}
          >
            <RefreshCw className="h-4 w-4" />
            {job?.state === 'running' ? `检测中 ${job.completed}/${job.total}` : '批量检测'}
          </Button>
          {job && job.state !== 'running' ? (
            <Button variant="ghost" size="sm" onClick={() => setJobOpen(true)}>
              上次检测
            </Button>
          ) : null}
          <Button variant="secondary" onClick={() => void load()}>
            <RefreshCw className="h-4 w-4" /> 刷新
          </Button>
          {canEdit ? (
            <>
              <Button variant="secondary" onClick={() => setDiscoveryOpen(true)}>
                <CloudDownload className="h-4 w-4" /> 在线获取
              </Button>
              <Button variant="secondary" onClick={() => setImportOpen(true)}>
                <Upload className="h-4 w-4" /> 批量导入
              </Button>
              <Button variant="primary" onClick={() => setEditor('new')}>
                <Plus className="h-4 w-4" /> 新增代理
              </Button>
            </>
          ) : null}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-5">
        <StatCard label="代理总数" value={stats.total} hint={`${stats.enabled} 个启用`} icon={<Server className="h-4 w-4" />} tone="primary" />
        <StatCard label="可用" value={stats.available} hint="最近一次检测成功" icon={<Wifi className="h-4 w-4" />} tone="success" />
        <StatCard label="失败" value={stats.failed} hint="最近一次检测失败" icon={<XCircle className="h-4 w-4" />} tone="warning" />
        <StatCard label="未检测" value={stats.untested} hint={`${stats.disabled} 个停用`} icon={<CircleOff className="h-4 w-4" />} />
        <StatCard label="订阅源" value={stats.subscriptions} hint={`${envs.filter((env) => env.proxyId).length} 个环境已绑定`} icon={<Globe2 className="h-4 w-4" />} />
      </div>

      <Card className="overflow-hidden">
        <Tabs
          className="px-4"
          value={tab}
          onValueChange={(value) => setTab(value as ProxyTab)}
          items={[
            { value: 'list', label: '代理列表', badge: stats.total },
            { value: 'subscriptions', label: '订阅源', badge: subscriptions.length },
          ]}
        />
        {tab === 'list' ? (
          <>
            <div className="flex flex-wrap items-center gap-2 border-b border-border p-4">
              <div className="relative w-full min-w-[200px] flex-1 sm:max-w-xs">
                <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-content-hint" />
                <Input
                  className="pl-9"
                  value={search}
                  onChange={(event) => {
                    setSearch(event.target.value);
                    resetPage();
                  }}
                  placeholder="搜索名称、主机、出口 IP…"
                />
              </div>
              <Select
                wrapperClassName="w-32 shrink-0"
                value={groupFilter}
                onChange={(event) => {
                  setGroupFilter(event.target.value);
                  resetPage();
                }}
                placeholder="全部分组"
              >
                {[...new Set(data.items.map((proxy) => proxy.groupName).filter(Boolean))].map((group) => (
                  <option key={group} value={group}>
                    {group}
                  </option>
                ))}
              </Select>
              <Select
                wrapperClassName="w-28 shrink-0"
                value={sourceFilter}
                onChange={(event) => {
                  setSourceFilter(event.target.value);
                  resetPage();
                }}
                placeholder="全部来源"
              >
                <option value="manual">手工</option>
                <option value="import">导入</option>
                <option value="subscription">订阅</option>
                <option value="discovery">在线发现</option>
              </Select>
              <Select
                wrapperClassName="w-28 shrink-0"
                value={statusFilter}
                onChange={(event) => {
                  setStatusFilter(event.target.value);
                  resetPage();
                }}
                placeholder="全部状态"
              >
                <option value="ok">可用</option>
                <option value="fail">失败</option>
                <option value="untested">未检测</option>
                <option value="disabled">已停用</option>
              </Select>
              {canEdit ? (
                <Button
                  variant="ghost"
                  size="sm"
                  className="shrink-0"
                  onClick={() => {
                    setCleanupStatuses(['disabled', 'fail']);
                    setCleanupOpen(true);
                  }}
                >
                  <Trash2 className="h-3.5 w-3.5" /> 清理节点
                </Button>
              ) : null}
              <span
                className="ml-auto hidden shrink-0 items-center gap-1 text-[11px] text-content-hint lg:flex"
                title="密码使用 AES-256-GCM 加密保存，列表不返回明文。"
              >
                <ShieldCheck className="h-3.5 w-3.5" /> 加密存储
              </span>
            </div>

            {selected.length > 0 ? (
              <div className="flex flex-wrap items-center gap-2 border-b border-border bg-surface-secondary px-4 py-2">
                <span className="text-xs text-content-weak">已选 {selected.length} 个</span>
                {canEdit ? (
                  <>
                    <Button variant="ghost" size="sm" onClick={() => void bulkSetEnabled(true)}>
                      启用
                    </Button>
                    <Button variant="ghost" size="sm" onClick={() => void bulkSetEnabled(false)}>
                      停用
                    </Button>
                    <Button variant="ghost" size="sm" onClick={() => setBulkMetaOpen(true)}>
                      分组 / 标签
                    </Button>
                    <Button variant="ghost" size="sm" className="text-danger" onClick={() => setBulkDeleteOpen(true)}>
                      删除
                    </Button>
                  </>
                ) : null}
                <Button variant="ghost" size="sm" onClick={runBatchCheck}>
                  检测选中
                </Button>
                <Button variant="ghost" size="sm" onClick={() => setSelected([])}>
                  取消选择
                </Button>
              </div>
            ) : null}

            {loading ? (
              <div className="flex justify-center py-16">
                <Spinner />
              </div>
            ) : data.items.length === 0 ? (
              <EmptyState
                icon={<Server className="h-5 w-5" />}
                title={stats.total === 0 ? '代理池为空' : '没有匹配的代理'}
                description={
                  stats.total === 0
                    ? '批量导入代理或创建订阅源后，可为环境独立绑定并进行 IP / 时区对齐。'
                    : '调整搜索或筛选条件后重试。'
                }
                action={
                  canEdit && stats.total === 0 ? (
                    <div className="flex gap-2">
                      <Button variant="secondary" size="sm" onClick={() => setImportOpen(true)}>
                        <Upload className="h-3.5 w-3.5" /> 批量导入
                      </Button>
                      <Button variant="primary" size="sm" onClick={() => setEditor('new')}>
                        <Plus className="h-3.5 w-3.5" /> 新增代理
                      </Button>
                    </div>
                  ) : undefined
                }
              />
            ) : (
              <div className="table-wrap">
                <table className="data-table">
                  <thead>
                    <tr>
                      <th className="w-8">
                        <input
                          type="checkbox"
                          className="accent-[var(--color-primary)]"
                          checked={allVisibleSelected}
                          onChange={(event) => toggleAll(event.target.checked)}
                          aria-label="全选当前页"
                        />
                      </th>
                      <th>代理</th>
                      <th>协议 / 地址</th>
                      <th>来源 / 分组</th>
                      <th>状态</th>
                      <th>出口信息</th>
                      <th>延迟</th>
                      <th>绑定</th>
                      <th className="text-right">操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.items.map((proxy) => {
                      const ok = proxy.enabled ? proxy.lastCheckOk : null;
                      return (
                        <tr key={proxy.id}>
                          <td>
                            <input
                              type="checkbox"
                              className="accent-[var(--color-primary)]"
                              checked={selected.includes(proxy.id)}
                              onChange={(event) => toggleSelected(proxy.id, event.target.checked)}
                              aria-label={`选择 ${proxy.name}`}
                            />
                          </td>
                          <td>
                            <div className="flex items-center gap-2">
                              <StatusDot
                                tone={!proxy.enabled ? 'muted' : ok === true ? 'success' : ok === false ? 'danger' : 'muted'}
                                pulse={proxy.enabled && ok === true}
                              />
                              <div className="min-w-0">
                                <p className="truncate text-sm font-medium text-content-main">{proxy.name}</p>
                                <p className="font-mono text-[10px] text-content-hint">{proxy.id.slice(0, 8)}…</p>
                              </div>
                            </div>
                          </td>
                          <td>
                            <div className="flex items-center gap-1">
                              <Badge tone="outline">{proxy.protocol.toUpperCase()}</Badge>
                              <span className="font-mono text-[11px] text-content-main">
                                {proxy.host}:{proxy.port}
                              </span>
                              <Tooltip content="复制代理地址">
                                <Button
                                  variant="ghost"
                                  size="icon-sm"
                                  onClick={() => void copyText(`${proxy.protocol}://${proxy.host}:${proxy.port}`)}
                                >
                                  <Copy className="h-3 w-3" />
                                </Button>
                              </Tooltip>
                            </div>
                            {proxy.username ? (
                              <p className="mt-1 font-mono text-[10px] text-content-hint">{proxy.username}</p>
                            ) : null}
                          </td>
                          <td>
                            <div className="flex flex-wrap gap-1">
                              <Badge tone={proxy.source === 'subscription' || proxy.source === 'discovery' ? 'primary' : 'neutral'}>
                                {proxy.source === 'subscription'
                                  ? '订阅'
                                  : proxy.source === 'discovery'
                                    ? '在线发现'
                                    : proxy.source === 'import'
                                      ? '导入'
                                      : '手工'}
                              </Badge>
                              {proxy.groupName ? <Badge tone="outline">{proxy.groupName}</Badge> : null}
                              {proxy.tags.slice(0, 2).map((tag) => (
                                <Badge key={tag} tone="neutral">
                                  #{tag}
                                </Badge>
                              ))}
                              {proxy.tags.length > 2 ? <Badge tone="neutral">+{proxy.tags.length - 2}</Badge> : null}
                            </div>
                            {proxy.note ? <p className="mt-1 max-w-[180px] truncate text-[10px] text-content-hint">{proxy.note}</p> : null}
                          </td>
                          <td>
                            {!proxy.enabled ? (
                              <Badge tone="neutral">
                                <CircleOff className="h-3 w-3" />
                                {proxy.disabledReason === 'subscription_missing' ? '订阅缺失' : '已停用'}
                              </Badge>
                            ) : ok === true ? (
                              <Tooltip content={proxy.lastCheckMessage ?? '最近一次检测成功'}>
                                <Badge tone="success">
                                  <CheckCircle2 className="h-3 w-3" /> 可用
                                </Badge>
                              </Tooltip>
                            ) : ok === false ? (
                              <Tooltip content={proxy.lastCheckMessage ?? '检测失败'}>
                                <Badge tone="danger">
                                  <XCircle className="h-3 w-3" /> 失败
                                </Badge>
                              </Tooltip>
                            ) : (
                              <Badge tone="neutral">未检测</Badge>
                            )}
                            {proxy.failCount > 0 && proxy.enabled ? (
                              <p className="mt-1 text-[10px] text-content-hint">连续失败 {proxy.failCount} 次</p>
                            ) : null}
                          </td>
                          <td>
                            {proxy.exitIp ? (
                              <div className="space-y-0.5 text-[11px]">
                                <p className="font-mono text-content-main">{proxy.exitIp}</p>
                                <p className="text-content-hint">
                                  {proxy.exitCountry ?? '—'}
                                  {proxy.lastCheckCity ? ` · ${proxy.lastCheckCity}` : ''}
                                  {proxy.exitTimezone ? ` · ${proxy.exitTimezone}` : ''}
                                </p>
                                <p className="text-[10px] text-content-hint">
                                  {proxy.lastCheckAt ? formatDateShort(proxy.lastCheckAt) : ''}
                                </p>
                                {ok === false ? <p className="text-[10px] text-content-hint">上次成功出口</p> : null}
                              </div>
                            ) : (
                              <span className="text-xs text-content-hint">{proxy.lastCheckMessage ?? '尚未检测'}</span>
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
                              <Button
                                variant="ghost"
                                size="sm"
                                loading={checkingId === proxy.id}
                                onClick={() => void checkOne(proxy)}
                              >
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
                                  <Button
                                    variant="ghost"
                                    size="icon-sm"
                                    className="text-danger"
                                    onClick={() => setDeleteTarget(proxy)}
                                  >
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

            {data.total > 0 ? (
              <div className="flex items-center justify-between border-t border-border px-4 py-2 text-xs text-content-hint">
                <span>
                  共 {data.total} 条，第 {page + 1}/{totalPages} 页
                </span>
                <div className="flex gap-1">
                  <Button variant="ghost" size="sm" disabled={page === 0} onClick={() => setPage((value) => Math.max(0, value - 1))}>
                    上一页
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={page + 1 >= totalPages}
                    onClick={() => setPage((value) => value + 1)}
                  >
                    下一页
                  </Button>
                </div>
              </div>
            ) : null}
          </>
        ) : (
          <SubscriptionPanel
            subscriptions={subscriptions}
            loading={loading}
            canEdit={canEdit}
            refreshingId={refreshingSub}
            onRefresh={refreshSubscription}
            onCheck={checkSubscription}
            onCreate={() => setSubEditor('new')}
            onEdit={setSubEditor}
            onDelete={(subscription) => {
              setSubDeleteProxies(false);
              setSubDelete(subscription);
            }}
          />
        )}
      </Card>

      <div className="grid gap-3 md:grid-cols-2">
        <Card className="p-4 text-xs text-content-weak">
          <p className="flex items-center gap-1.5 font-medium text-content-main">
            <ShieldCheck className="h-4 w-4 text-primary" /> 检测与对齐
          </p>
          <p className="mt-1.5 leading-5">
            检测先做 TCP 握手，再通过代理请求出口 IP 服务，返回国家、时区与城市。批量检测在后台执行，可关闭面板后继续。
          </p>
        </Card>
        <Card className="p-4 text-xs text-content-weak">
          <p className="flex items-center gap-1.5 font-medium text-content-main">
            <Users className="h-4 w-4 text-primary" /> 批量绑定
          </p>
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

      <ProxyImportDialog
        open={importOpen}
        onOpenChange={setImportOpen}
        onImported={() => {
          void load();
        }}
      />

      <ProxyDiscoveryDialog
        open={discoveryOpen}
        onOpenChange={setDiscoveryOpen}
        canEdit={canEdit}
        onAdded={() => {
          void load();
        }}
      />

      <ProxyBatchPanel
        open={jobOpen}
        onOpenChange={setJobOpen}
        job={job}
        results={jobResults}
        onCancel={async () => {
          if (!job) return;
          try {
            await ipc.proxyJobCancel(job.jobId);
            toast.info('已请求取消检测');
          } catch (error) {
            toastError(error);
          }
        }}
        onRetryFailed={() => {
          const failedIds = (jobResults.length > 0 ? jobResults : job?.recent ?? [])
            .filter((item) => !item.ok)
            .map((item) => item.proxyId);
          if (failedIds.length === 0) {
            toast.info('没有失败项可重试');
            return;
          }
          void startBatch({ ids: failedIds, concurrency: 8 });
        }}
        onSelectWorking={() => {
          const ids = (jobResults.length > 0 ? jobResults : job?.recent ?? [])
            .filter((item) => item.ok)
            .map((item) => item.proxyId);
          if (ids.length === 0) {
            toast.info('没有可用节点可勾选');
            return;
          }
          setSelected(ids);
          setJobOpen(false);
          toast.success('已选择可用节点', `${ids.length} 个`);
        }}
      />

      <BulkMetaDialog
        open={bulkMetaOpen}
        onOpenChange={setBulkMetaOpen}
        count={selected.length}
        onApply={async (patch) => {
          try {
            const updated = await ipc.proxyBatchUpdate({ ids: selected, ...patch });
            toast.success('批量更新完成', `${updated} 个代理`);
            setBulkMetaOpen(false);
            setSelected([]);
            await load();
          } catch (error) {
            toastError(error);
          }
        }}
      />

      <PostDeleteDialog
        open={bulkDeleteOpen}
        onOpenChange={setBulkDeleteOpen}
        title={`批量删除 ${selected.length} 个代理`}
        description={`将删除选中的 ${selected.length} 个代理，并从绑定环境解绑。该操作不可撤销。`}
        onConfirm={async () => {
          try {
            const result = await ipc.proxyBatchDelete(selected, true);
            toast.success('代理已删除', `${result.deleted} 个，影响 ${result.affectedEnvs} 个环境`);
            setBulkDeleteOpen(false);
            setSelected([]);
            await load();
          } catch (error) {
            toastError(error);
          }
        }}
      />

      <PostDeleteDialog
        open={cleanupOpen}
        onOpenChange={setCleanupOpen}
        title="清理代理节点"
        description={`将删除选中状态的节点（${cleanupStatuses
          .map((status) => cleanupStatusLabel(status))
          .join('、') || '未选择'}），并从绑定环境解绑。该操作不可撤销。`}
        confirmLabel="清理节点"
        onConfirm={async () => {
          if (cleanupStatuses.length === 0) {
            toast.warning('请至少选择一种要清理的状态');
            return;
          }
          try {
            const result = await ipc.proxyBatchDeleteDisabled(
              { statuses: cleanupStatuses },
              true,
            );
            toast.success('代理节点已清理', `${result.deleted} 个，影响 ${result.affectedEnvs} 个环境`);
            setCleanupOpen(false);
            await load();
          } catch (error) {
            toastError(error);
          }
        }}
      >
        <div className="space-y-2">
          {[
            { value: 'disabled', label: '已停用', count: stats.disabled, hint: '手动停用或订阅缺失的节点' },
            { value: 'fail', label: '检测失败', count: stats.failed, hint: '已启用但最近一次检测失败' },
            { value: 'untested', label: '未检测', count: stats.untested, hint: '已启用且尚未执行过检测' },
          ].map((option) => (
            <label
              key={option.value}
              className="flex cursor-pointer items-start gap-2 rounded-md border border-border px-3 py-2 text-xs text-content-weak hover:bg-surface-secondary"
            >
              <input
                type="checkbox"
                className="mt-0.5 accent-[var(--color-primary)]"
                checked={cleanupStatuses.includes(option.value)}
                onChange={(event) =>
                  setCleanupStatuses((current) =>
                    event.target.checked
                      ? [...current, option.value]
                      : current.filter((value) => value !== option.value),
                  )
                }
              />
              <span className="min-w-0 flex-1">
                <span className="flex items-center justify-between gap-2">
                  <span className="font-medium text-content-main">{option.label}</span>
                  <Badge tone="neutral">{option.count}</Badge>
                </span>
                <span className="mt-0.5 block text-[10px] text-content-hint">{option.hint}</span>
              </span>
            </label>
          ))}
        </div>
      </PostDeleteDialog>

      <SubscriptionEditorDialog
        open={!!subEditor}
        onOpenChange={(value) => !value && setSubEditor(null)}
        subscription={subEditor === 'new' ? null : subEditor}
        onSaved={async (created, refreshNow) => {
          setSubEditor(null);
          await load();
          if (created && refreshNow) {
            await refreshSubscription(created);
          }
        }}
      />

      <PostDeleteDialog
        open={!!subDelete}
        onOpenChange={(value) => !value && setSubDelete(null)}
        title={`删除订阅「${subDelete?.name ?? ''}」`}
        description="默认保留该订阅下的代理并转为手工代理；运行中的环境不受影响。"
        confirmLabel="删除订阅"
        onConfirm={async () => {
          if (!subDelete) return;
          try {
            const result = await ipc.proxySubscriptionDelete(subDelete.id, subDeleteProxies, true);
            toast.success(
              '订阅已删除',
              subDeleteProxies
                ? `同时删除 ${result.deleted} 个代理，影响 ${result.affectedEnvs} 个环境`
                : '代理已保留并转为手工',
            );
            setSubDelete(null);
            await load();
          } catch (error) {
            toastError(error);
          }
        }}
      >
        <label className="flex items-center gap-2 text-xs text-content-weak">
          <input
            type="checkbox"
            className="accent-[var(--color-primary)]"
            checked={subDeleteProxies}
            onChange={(event) => setSubDeleteProxies(event.target.checked)}
          />
          同时删除该订阅下的 {subDelete?.proxyCount ?? 0} 个代理
        </label>
      </PostDeleteDialog>

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(value) => !value && setDeleteTarget(null)}
        title="删除代理"
        description={`将删除「${deleteTarget?.name ?? ''}」并从 ${deleteTarget ? boundCount(deleteTarget.id) : 0} 个环境解绑。运行中的环境需要重启。`}
        confirmLabel="删除代理"
        onConfirm={deleteOne}
      />
    </div>
  );
}

function SubscriptionPanel({
  subscriptions,
  loading,
  canEdit,
  refreshingId,
  onRefresh,
  onCheck,
  onCreate,
  onEdit,
  onDelete,
}: {
  subscriptions: ProxySubscription[];
  loading: boolean;
  canEdit: boolean;
  refreshingId: string | null;
  onRefresh: (subscription: ProxySubscription) => void;
  onCheck: (subscription: ProxySubscription) => void;
  onCreate: () => void;
  onEdit: (subscription: ProxySubscription) => void;
  onDelete: (subscription: ProxySubscription) => void;
}) {
  return (
    <div>
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border p-4">
        <p className="text-xs text-content-weak">
          订阅刷新会自动新增节点；源中消失的节点会停用并标记，不会自动删除。
        </p>
        {canEdit ? (
          <Button variant="primary" size="sm" onClick={onCreate}>
            <Plus className="h-3.5 w-3.5" /> 新增订阅
          </Button>
        ) : null}
      </div>
      {loading ? (
        <div className="flex justify-center py-16">
          <Spinner />
        </div>
      ) : subscriptions.length === 0 ? (
        <EmptyState
          icon={<Globe2 className="h-5 w-5" />}
          title="还没有订阅源"
          description="创建订阅后，可手动刷新或按周期自动同步代理节点。"
          action={
            canEdit ? (
              <Button variant="primary" size="sm" onClick={onCreate}>
                <Plus className="h-3.5 w-3.5" /> 新增订阅
              </Button>
            ) : undefined
          }
        />
      ) : (
        <div className="table-wrap">
          <table className="data-table">
            <thead>
              <tr>
                <th>订阅</th>
                <th>URL</th>
                <th>节点</th>
                <th>刷新状态</th>
                <th>自动刷新</th>
                <th className="text-right">操作</th>
              </tr>
            </thead>
            <tbody>
              {subscriptions.map((subscription) => (
                <tr key={subscription.id}>
                  <td>
                    <div className="flex items-center gap-2">
                      <StatusDot
                        tone={
                          !subscription.enabled
                            ? 'muted'
                            : subscription.lastRefreshStatus === 'error'
                              ? 'danger'
                              : subscription.lastRefreshStatus === 'ok'
                                ? 'success'
                                : 'muted'
                        }
                      />
                      <div>
                        <p className="text-sm font-medium text-content-main">{subscription.name}</p>
                        <p className="text-[10px] text-content-hint">
                          {subscription.groupName ? `${subscription.groupName} · ` : ''}
                          {subscription.defaultProtocol.toUpperCase()}
                        </p>
                      </div>
                    </div>
                  </td>
                  <td>
                    <div className="flex max-w-[320px] items-center gap-1">
                      <span className="truncate font-mono text-[10px] text-content-weak" title={subscription.url}>
                        {subscription.url}
                      </span>
                      <Button variant="ghost" size="icon-sm" onClick={() => void copyText(subscription.url)}>
                        <Copy className="h-3 w-3" />
                      </Button>
                    </div>
                  </td>
                  <td>
                    <Badge tone="primary">{subscription.proxyCount}</Badge>
                  </td>
                  <td>
                    <div className="space-y-0.5 text-[11px]">
                      <Badge tone={subscription.lastRefreshStatus === 'error' ? 'danger' : subscription.lastRefreshStatus === 'ok' ? 'success' : 'neutral'}>
                        {subscription.lastRefreshStatus === 'ok' ? '成功' : subscription.lastRefreshStatus === 'error' ? '失败' : '未刷新'}
                      </Badge>
                      {subscription.lastRefreshAt ? (
                        <p className="text-[10px] text-content-hint">{formatDateShort(subscription.lastRefreshAt)}</p>
                      ) : null}
                      {subscription.lastRefreshMessage ? (
                        <p className="max-w-[220px] truncate text-[10px] text-content-hint" title={subscription.lastRefreshMessage}>
                          {subscription.lastRefreshMessage}
                        </p>
                      ) : null}
                    </div>
                  </td>
                  <td className="text-xs text-content-weak">
                    {subscription.autoRefresh ? `每 ${formatInterval(subscription.refreshIntervalMin)}` : '关闭'}
                  </td>
                  <td>
                    <div className="flex items-center justify-end gap-1">
                      {canEdit ? (
                        <Button
                          variant="ghost"
                          size="sm"
                          loading={refreshingId === subscription.id}
                          onClick={() => onRefresh(subscription)}
                        >
                          <RefreshCw className="h-3.5 w-3.5" /> 刷新
                        </Button>
                      ) : null}
                      <Button variant="ghost" size="sm" onClick={() => onCheck(subscription)}>
                        <Wifi className="h-3.5 w-3.5" /> 检测
                      </Button>
                      {canEdit ? (
                        <>
                          <Button variant="ghost" size="icon-sm" onClick={() => onEdit(subscription)}>
                            <Pencil className="h-3.5 w-3.5" />
                          </Button>
                          <Button variant="ghost" size="icon-sm" className="text-danger" onClick={() => onDelete(subscription)}>
                            <Trash2 className="h-3.5 w-3.5" />
                          </Button>
                        </>
                      ) : null}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function ProxyDiscoveryDialog({
  open,
  onOpenChange,
  canEdit,
  onAdded,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  canEdit: boolean;
  onAdded: () => void;
}) {
  const toast = useToastStore();
  const [searchQuery, setSearchQuery] = React.useState('free proxy list');
  const [engines, setEngines] = React.useState<string[]>(['bing', 'duckduckgo']);
  const [githubQuery, setGithubQuery] = React.useState('free proxy list');
  const [friendLinks, setFriendLinks] = React.useState('');
  const [followLinks, setFollowLinks] = React.useState(true);
  const [protocol, setProtocol] = React.useState<'http' | 'https' | 'socks5'>('http');
  const [maxResults, setMaxResults] = React.useState('200');
  const [perSourceLimit, setPerSourceLimit] = React.useState('5000');
  const [result, setResult] = React.useState<ProxyDiscoveryResult | null>(null);
  const [selected, setSelected] = React.useState<string[]>([]);
  const [busy, setBusy] = React.useState(false);
  const [scanning, setScanning] = React.useState(false);
  const [logs, setLogs] = React.useState<string[]>([]);
  const [progress, setProgress] = React.useState<ProxyDiscoveryProgress | null>(null);
  const [cancelling, setCancelling] = React.useState(false);
  const scanIdRef = React.useRef<string | null>(null);
  const logRef = React.useRef<HTMLDivElement | null>(null);

  React.useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void ipc
      .onProxyDiscovery((event) => {
        if (scanIdRef.current !== event.scanId) return;
        const stamp = new Date().toLocaleTimeString();
        if (event.kind === 'log') {
          setLogs((current) => [...current.slice(-199), `[${stamp}] ${event.message}`]);
        } else if (event.kind === 'source' && event.source) {
          const source = event.source;
          setResult((current) => {
            if (!current) return current;
            const sources = current.sources.filter(
              (item) => !(item.kind === source.kind && item.target === source.target),
            );
            return { ...current, sources: [...sources, source] };
          });
        } else if (event.kind === 'candidate' && event.candidate) {
          const candidate = event.candidate;
          setResult((current) => {
            const base =
              current ?? {
                candidates: [],
                sources: [],
                scanned: 0,
                verified: 0,
                rejected: 0,
                errors: [],
              };
            if (base.candidates.some((item) => item.id === candidate.id)) return base;
            const candidates = [...base.candidates, candidate];
            return {
              ...base,
              candidates,
              verified: candidates.length,
            };
          });
          if (!candidate.duplicate) {
            setSelected((current) =>
              current.includes(candidate.id) ? current : [...current, candidate.id],
            );
          }
        } else if (event.kind === 'progress' && event.progress) {
          const next = event.progress;
          setProgress(next);
          setResult((current) =>
            current
              ? {
                  ...current,
                  scanned: next.scanned,
                  verified: Math.max(current.verified, next.verified),
                  rejected: next.rejected,
                }
              : current,
          );
        }
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  React.useEffect(() => {
    const node = logRef.current;
    if (node) node.scrollTop = node.scrollHeight;
  }, [logs]);
  const [groupName, setGroupName] = React.useState('');
  const [tags, setTags] = React.useState('');
  const [enabled, setEnabled] = React.useState(true);
  const [onDuplicate, setOnDuplicate] = React.useState<'skip' | 'update' | 'error'>('skip');
  const [includePublicLists, setIncludePublicLists] = React.useState(true);
  const [verify, setVerify] = React.useState(true);
  const [checks, setChecks] = React.useState<Record<string, ProxyCheckResult>>({});
  const [checking, setChecking] = React.useState<string[]>([]);

  React.useEffect(() => {
    if (!open) return;
    setResult(null);
    setSelected([]);
    setChecks({});
    setChecking([]);
    setLogs([]);
    setProgress(null);
    setCancelling(false);
    setScanning(false);
    scanIdRef.current = null;
    try {
      const saved = window.localStorage.getItem('finenv:proxy-discovery');
      if (!saved) return;
      const value = JSON.parse(saved) as Record<string, unknown>;
      if (typeof value.searchQuery === 'string') setSearchQuery(value.searchQuery);
      if (Array.isArray(value.engines)) setEngines(value.engines.filter((item): item is string => typeof item === 'string'));
      if (typeof value.githubQuery === 'string') setGithubQuery(value.githubQuery);
      if (typeof value.friendLinks === 'string') setFriendLinks(value.friendLinks);
      if (typeof value.followLinks === 'boolean') setFollowLinks(value.followLinks);
      if (value.protocol === 'http' || value.protocol === 'https' || value.protocol === 'socks5') setProtocol(value.protocol);
      if (typeof value.maxResults === 'string') setMaxResults(value.maxResults);
      if (typeof value.perSourceLimit === 'string') setPerSourceLimit(value.perSourceLimit);
      if (typeof value.includePublicLists === 'boolean') setIncludePublicLists(value.includePublicLists);
      if (typeof value.verify === 'boolean') setVerify(value.verify);
    } catch {
      // Ignore malformed local settings.
    }
  }, [open]);

  const saveSettings = () => {
    try {
      window.localStorage.setItem(
        'finenv:proxy-discovery',
        JSON.stringify({ searchQuery, engines, githubQuery, friendLinks, followLinks, protocol, maxResults, perSourceLimit, includePublicLists, verify }),
      );
    } catch {
      // Ignore storage failures.
    }
  };

  const buildInput = (): ProxyDiscoveryInput => ({
    searchQueries: searchQuery
      .split(/[\n,，]+/)
      .map((value) => value.trim())
      .filter(Boolean),
    engines,
    githubQueries: githubQuery.trim() ? [githubQuery.trim()] : [],
    friendLinks: friendLinks
      .split(/\r?\n/)
      .map((value) => value.trim())
      .filter(Boolean),
    followLinks,
    defaultProtocol: protocol,
    maxResults: Number(maxResults) || 200,
    perSourceLimit: Math.min(20000, Math.max(100, Number(perSourceLimit) || 5000)),
    includePublicLists,
    verify,
  });

  const scan = async () => {
    const input = buildInput();
    const hasSource =
      includePublicLists ||
      ((input.engines?.length ?? 0) > 0 && (input.searchQueries?.length ?? 0) > 0) ||
      (input.githubQueries?.length ?? 0) > 0 ||
      (input.friendLinks?.length ?? 0) > 0;
    if (!hasSource) {
      toast.warning('请至少填写一个来源', '公共列表、搜索引擎关键词、GitHub 关键词或友情链接');
      return;
    }
    setBusy(true);
    setScanning(true);
    setCancelling(false);
    setResult({
      candidates: [],
      sources: [],
      scanned: 0,
      verified: 0,
      rejected: 0,
      errors: [],
    });
    setProgress(null);
    setLogs([`[${new Date().toLocaleTimeString()}] 正在启动扫描…`]);
    setSelected([]);
    setChecks({});
    setChecking([]);
    saveSettings();
    const scanId = `scan-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    scanIdRef.current = scanId;
    try {
      const next = await ipc.proxyDiscoveryScan({ ...input, scanId });
      setResult(next);
      setSelected(
        next.candidates
          .filter((candidate) => !candidate.duplicate && (!verify || candidate.verified))
          .map((candidate) => candidate.id),
      );
      if (next.candidates.length === 0) {
        toast.warning('没有发现可用节点', next.errors[0] ?? '尝试更换关键词或友情链接');
      } else if (verify) {
        toast.success(
          '在线扫描完成',
          `验证可用 ${next.verified} 个，已过滤 ${next.rejected} 个不可用节点`,
        );
      } else {
        toast.success('在线扫描完成', `发现 ${next.candidates.length} 个候选节点`);
      }
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
      setScanning(false);
      setCancelling(false);
      if (scanIdRef.current === scanId) scanIdRef.current = null;
    }
  };

  const stopScan = async () => {
    const scanId = scanIdRef.current;
    if (!scanId) return;
    setCancelling(true);
    try {
      const accepted = await ipc.proxyDiscoveryCancel(scanId);
      if (accepted) {
        toast.info('已请求停止扫描', '正在结束当前检测，已找到的节点会保留');
      } else {
        toast.info('扫描已结束');
      }
    } catch (error) {
      toastError(error);
      setCancelling(false);
    }
  };

  const closeDialog = (value: boolean) => {
    if (!value && scanIdRef.current) {
      void ipc.proxyDiscoveryCancel(scanIdRef.current);
    }
    onOpenChange(value);
  };

  const addSelected = async () => {
    if (!canEdit || selected.length === 0) return;
    setBusy(true);
    try {
      const added = await ipc.proxyDiscoveryAdd({
        candidateIds: selected,
        groupName: groupName.trim() || undefined,
        tags: splitTags(tags),
        enabled,
        onDuplicate,
      });
      toast.success(
        '节点已添加',
        `新增 ${added.inserted}，更新 ${added.updated}，跳过 ${added.skipped}，失败 ${added.failed}`,
      );
      setSelected([]);
      onAdded();
      onOpenChange(false);
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  const checkCandidates = async (ids: string[]) => {
    const unique = ids.filter(
      (id, index) => ids.indexOf(id) === index && !checking.includes(id),
    );
    if (unique.length === 0) return;
    setChecking((current) => [...current, ...unique]);
    let cursor = 0;
    const worker = async () => {
      while (cursor < unique.length) {
        const id = unique[cursor];
        cursor += 1;
        try {
          const result = await ipc.proxyDiscoveryCheck(id);
          setChecks((current) => ({ ...current, [id]: result }));
        } catch {
          // Expired candidates are reported when adding; keep the row usable.
        } finally {
          setChecking((current) => current.filter((value) => value !== id));
        }
      }
    };
    await Promise.all(
      Array.from({ length: Math.min(6, unique.length) }, () => worker()),
    );
  };

  const toggleEngine = (engine: string, checked: boolean) => {
    setEngines((current) => (checked ? [...current, engine] : current.filter((value) => value !== engine)));
  };

  const selectWorking = () => {
    if (!result) return;
    const ids = result.candidates
      .filter((candidate) => {
        const check = checks[candidate.id];
        if (check) return check.ok;
        return candidate.verified || !verify;
      })
      .map((candidate) => candidate.id);
    setSelected(ids);
    if (ids.length > 0) {
      toast.success('已选择可用节点', `${ids.length} 个节点`);
    } else {
      toast.warning('没有可用节点', '先执行检测，或开启“只返回可用节点”后重新扫描');
    }
  };

  const toggleCandidate = (id: string, checked: boolean) => {
    setSelected((current) => (checked ? [...current, id] : current.filter((value) => value !== id)));
  };

  const duplicates = result?.candidates.filter((candidate) => candidate.duplicate).length ?? 0;
  const allSelected = !!result && result.candidates.length > 0 && result.candidates.every((candidate) => selected.includes(candidate.id));
  const tested = Object.keys(checks).length;
  const testedOk = Object.values(checks).filter((result) => result.ok).length;

  return (
    <Dialog
      open={open}
      onOpenChange={closeDialog}
      title="在线获取代理节点"
      description="从内置公开列表、搜索引擎、GitHub 和友情链接抓取公开页面，提取 HTTP/HTTPS/SOCKS5 节点；扫描后实测 Google / YouTube，只保留真正可用的节点。"
      width="max-w-5xl"
      footer={
        <>
          <Button variant="secondary" onClick={() => closeDialog(false)}>
            关闭
          </Button>
          {scanning ? (
            <Button variant="danger" onClick={() => void stopScan()} loading={cancelling}>
              停止扫描
            </Button>
          ) : (
            <Button variant="secondary" onClick={() => void scan()} disabled={busy}>
              <Search className="h-4 w-4" /> 开始扫描
            </Button>
          )}
          <Button
            variant="primary"
            onClick={() => void addSelected()}
            disabled={!canEdit || selected.length === 0 || scanning || busy}
          >
            添加选中 {selected.length > 0 ? `(${selected.length})` : ''}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="grid gap-3 lg:grid-cols-3">
          <div className="space-y-2 rounded-md border border-border p-3">
            <p className="flex items-center gap-1.5 text-xs font-medium text-content-main">
              <Search className="h-3.5 w-3.5" /> 搜索引擎
            </p>
            <Input
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="关键词，多个用逗号分隔"
            />
            <PromptChips
              items={['free proxy list', 'socks5 proxy list', 'fresh proxy list google', 'proxy list youtube']}
              onPick={setSearchQuery}
            />
            <div className="flex flex-wrap gap-3 text-[11px] text-content-weak">
              {[
                ['bing', 'Bing'],
                ['duckduckgo', 'DuckDuckGo'],
                ['google', 'Google'],
              ].map(([value, label]) => (
                <label key={value} className="flex items-center gap-1.5">
                  <input
                    type="checkbox"
                    className="accent-[var(--color-primary)]"
                    checked={engines.includes(value)}
                    onChange={(event) => toggleEngine(value, event.target.checked)}
                  />
                  {label}
                </label>
              ))}
            </div>
          </div>
          <div className="space-y-2 rounded-md border border-border p-3">
            <p className="flex items-center gap-1.5 text-xs font-medium text-content-main">
              <Server className="h-3.5 w-3.5" /> GitHub
            </p>
            <Input
              value={githubQuery}
              onChange={(event) => setGithubQuery(event.target.value)}
              placeholder="仓库搜索关键词"
            />
            <PromptChips
              items={['free proxy list', 'socks5 proxy list', 'proxy list google', 'proxy scraper daily']}
              onPick={setGithubQuery}
            />
            <p className="text-[10px] leading-4 text-content-hint">
              抓取搜索结果前 5 个仓库的 README；未配置 Token 时受 GitHub 匿名限流影响。
            </p>
          </div>
          <div className="space-y-2 rounded-md border border-border p-3">
            <p className="flex items-center gap-1.5 text-xs font-medium text-content-main">
              <Globe2 className="h-3.5 w-3.5" /> 友情链接
            </p>
            <Textarea
              className="min-h-[64px] font-mono text-[11px]"
              value={friendLinks}
              onChange={(event) => setFriendLinks(event.target.value)}
              placeholder={'每行一个页面 URL\nhttps://example.com/proxy-list'}
            />
          </div>
        </div>

        <div className="grid gap-3 rounded-md border border-border bg-surface-secondary p-3 md:grid-cols-2">
          <Switch
            checked={includePublicLists}
            onCheckedChange={setIncludePublicLists}
            label="内置公开代理列表"
            description="抓取 Proxifly / TheSpeedX 等公开列表；推荐开启，不依赖搜索关键词。"
          />
          <Switch
            checked={verify}
            onCheckedChange={setVerify}
            label="只返回真正可用的节点"
            description="TCP 预筛后再实测 Google / YouTube；候选较多时首次扫描可能需要 1-3 分钟。"
          />
        </div>

        <div className="flex flex-wrap items-end gap-3 rounded-md border border-border bg-surface-secondary p-3">
          <Field label="默认协议">
            <Select value={protocol} onChange={(event) => setProtocol(event.target.value as typeof protocol)}>
              <option value="http">HTTP</option>
              <option value="https">HTTPS</option>
              <option value="socks5">SOCKS5</option>
            </Select>
          </Field>
          <Field label="最大结果">
            <Input
              type="number"
              min={1}
              max={500}
              value={maxResults}
              onChange={(event) => setMaxResults(event.target.value)}
              className="w-24"
            />
          </Field>
          <Field label="单来源上限">
            <Input
              type="number"
              min={100}
              max={20000}
              step={500}
              value={perSourceLimit}
              onChange={(event) => setPerSourceLimit(event.target.value)}
              className="w-28"
              title="每个来源抓取的候选数量，默认 5000；数值越大扫描越慢"
            />
          </Field>
          <div className="pb-1.5">
            <Switch checked={followLinks} onCheckedChange={setFollowLinks} label="跟进页面链接" />
          </div>
          <Field label="新增分组">
            <Input value={groupName} onChange={(event) => setGroupName(event.target.value)} placeholder="可选" className="w-40" />
          </Field>
          <Field label="标签">
            <Input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="逗号分隔" className="w-40" />
          </Field>
          <Field label="重复策略">
            <Select value={onDuplicate} onChange={(event) => setOnDuplicate(event.target.value as typeof onDuplicate)}>
              <option value="skip">跳过</option>
              <option value="update">更新密码</option>
              <option value="error">视为错误</option>
            </Select>
          </Field>
          <div className="pb-1.5">
            <Switch checked={enabled} onCheckedChange={setEnabled} label="添加后启用" />
          </div>
        </div>

        {scanning || logs.length > 0 ? (
          <div className="rounded-md border border-border bg-surface-secondary">
            <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-3 py-1.5 text-[11px]">
              <span className="font-medium text-content-main">实时日志</span>
              <span className="text-content-hint">
                {progress
                  ? `${discoveryPhaseLabel(progress.phase)} · ${progress.message}`
                  : scanning
                    ? '准备中…'
                    : '扫描结束'}
              </span>
            </div>
            <div
              ref={logRef}
              className="max-h-36 overflow-y-auto px-3 py-2 font-mono text-[10px] leading-5 text-content-weak"
            >
              {logs.length === 0 ? (
                <p className="text-content-hint">等待日志…</p>
              ) : (
                logs.map((line, index) => (
                  <p key={`${index}-${line}`} className="whitespace-pre-wrap break-all">
                    {line}
                  </p>
                ))
              )}
            </div>
          </div>
        ) : null}

        {result ? (
          <div className="space-y-3">
            <div className="flex flex-wrap items-center gap-2 text-xs text-content-weak">
              <Badge tone="primary">候选 {result.candidates.length}</Badge>
              <Badge tone={duplicates > 0 ? 'warning' : 'neutral'}>已存在 {duplicates}</Badge>
              <Badge tone="neutral">扫描 {result.scanned}</Badge>
              {result.verified > 0 ? <Badge tone="success">已验证 {result.verified}</Badge> : null}
              {result.rejected > 0 ? <Badge tone="warning">过滤 {result.rejected}</Badge> : null}
              {tested > 0 ? <Badge tone="success">可用 {testedOk}/{tested}</Badge> : null}
              <span className="text-content-hint">已选 {selected.length}</span>
              <div className="ml-auto flex flex-wrap items-center gap-1">
                <Button
                  variant="ghost"
                  size="sm"
                  loading={checking.length > 0}
                  disabled={selected.length === 0}
                  onClick={() => void checkCandidates(selected)}
                >
                  <Wifi className="h-3.5 w-3.5" /> 检测选中
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={checking.length > 0 || result.candidates.length === 0}
                  onClick={() => void checkCandidates(result.candidates.map((candidate) => candidate.id))}
                >
                  <Wifi className="h-3.5 w-3.5" /> 全部检测
                </Button>
                <Button variant="secondary" size="sm" onClick={selectWorking}>
                  <CheckCircle2 className="h-3.5 w-3.5" /> 选择可用
                </Button>
                <button
                  type="button"
                  className="px-1 text-primary"
                  onClick={() => setSelected(allSelected ? [] : result.candidates.map((candidate) => candidate.id))}
                >
                  {allSelected ? '取消全选' : '全选'}
                </button>
              </div>
            </div>
            <div className="flex flex-wrap gap-1">
              {result.sources.map((source, index) => (
                <Tooltip key={`${source.kind}-${source.target}-${index}`} content={source.message || source.target}>
                  <Badge tone={source.ok ? (source.count > 0 ? 'success' : 'neutral') : 'danger'}>
                    {source.kind} · {source.count}
                  </Badge>
                </Tooltip>
              ))}
            </div>
            {result.errors.length > 0 ? (
              <div className="max-h-20 overflow-y-auto rounded-md border border-border bg-surface-secondary p-2 text-[10px] text-content-hint">
                {result.errors.slice(0, 10).map((error, index) => (
                  <p key={index}>{error}</p>
                ))}
              </div>
            ) : null}
            {result.candidates.length === 0 ? (
              <EmptyState
                icon={<CloudDownload className="h-5 w-5" />}
                title={scanning ? '正在扫描中…' : '没有发现可用节点'}
                description={
                  scanning
                    ? '每验证到一个可用节点都会立即加入结果列表。'
                    : '尝试更换关键词、放宽页面链接抓取，或添加新的友情链接。'
                }
              />
            ) : (
              <div className="table-wrap max-h-[320px] overflow-y-auto rounded-md border border-border">
                <table className="data-table">
                  <thead>
                    <tr>
                      <th className="w-8" />
                      <th>节点</th>
                      <th>来源</th>
                      <th>认证</th>
                      <th>状态</th>
                      <th className="text-right">检测</th>
                    </tr>
                  </thead>
                  <tbody>
                    {result.candidates.map((candidate) => {
                      const check = checks[candidate.id];
                      const latency = check?.latencyMs ?? candidate.latencyMs;
                      const exitIp = check?.exitIp ?? candidate.exitIp;
                      const verified = check?.ok ?? candidate.verified;
                      return (
                        <tr key={candidate.id}>
                          <td>
                            <input
                              type="checkbox"
                              className="accent-[var(--color-primary)]"
                              checked={selected.includes(candidate.id)}
                              onChange={(event) => toggleCandidate(candidate.id, event.target.checked)}
                            />
                          </td>
                          <td>
                            <span className="font-mono text-[11px] text-content-main">
                              {candidate.protocol}://{candidate.host}:{candidate.port}
                            </span>
                            <p className="max-w-[240px] truncate text-[10px] text-content-hint" title={candidate.sourceUrl}>
                              {candidate.name}
                            </p>
                            {verified ? (
                              <p className="text-[10px] text-[var(--success)]">
                                已验证
                                {latency != null ? ` · ${latency}ms` : ''}
                                {exitIp ? ` · ${exitIp}` : ''}
                              </p>
                            ) : null}
                          </td>
                          <td>
                            <Badge tone="outline">{candidate.source}</Badge>
                          </td>
                          <td className="text-[11px] text-content-weak">
                            {candidate.username || '无'}
                            {candidate.hasPassword ? <Badge tone="success" className="ml-1">密码</Badge> : null}
                          </td>
                          <td>
                            {candidate.duplicate ? <Badge tone="warning">已存在</Badge> : <Badge tone="success">新节点</Badge>}
                          </td>
                          <td>
                            <div className="flex items-center justify-end gap-1">
                              {check ? (
                                check.ok ? (
                                  <Tooltip
                                    content={`${check.exitIp ?? '连通'}${check.city ? ` · ${check.city}` : ''}${check.country ? ` · ${check.country}` : ''}`}
                                  >
                                    <Badge tone="success">
                                      {check.latencyMs != null ? `${check.latencyMs}ms` : '可用'}
                                    </Badge>
                                  </Tooltip>
                                ) : (
                                  <Tooltip content={check.message}>
                                    <Badge tone="danger">失败</Badge>
                                  </Tooltip>
                                )
                              ) : null}
                              <Button
                                variant="ghost"
                                size="sm"
                                loading={checking.includes(candidate.id)}
                                onClick={() => void checkCandidates([candidate.id])}
                              >
                                <Wifi className="h-3 w-3" /> 检测
                              </Button>
                            </div>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
            {!canEdit ? (
              <p className="text-[11px] text-content-hint">当前账号只有查看权限，无法添加节点。</p>
            ) : null}
          </div>
        ) : (
          <div className="rounded-md border border-dashed border-border p-8 text-center text-xs text-content-hint">
            选择来源后点击「开始扫描」。扫描只读取公开页面，不会执行页面脚本。
          </div>
        )}
      </div>
    </Dialog>
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
    groupName: '',
    tags: '',
    note: '',
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
      groupName: proxy?.groupName ?? '',
      tags: (proxy?.tags ?? []).join(', '),
      note: proxy?.note ?? '',
    });
  }, [open, proxy]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={proxy ? '编辑代理' : '新增代理'}
      description="代理密码立即加密；编辑时密码留空表示保持原密码。"
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
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
                const tags = splitTags(form.tags);
                if (proxy) {
                  await ipc.proxyUpdate(proxy.id, {
                    name: form.name.trim(),
                    protocol: form.protocol,
                    host: form.host.trim(),
                    port: Number(form.port),
                    username: form.username,
                    password: form.password || undefined,
                    enabled: form.enabled,
                    groupName: form.groupName.trim(),
                    tags,
                    note: form.note.trim(),
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
                    groupName: form.groupName.trim(),
                    tags,
                    note: form.note.trim(),
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
          <Input
            value={form.name}
            onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))}
            placeholder="例如：上海测试出口"
            autoFocus
          />
        </Field>
        <Field label="协议">
          <Select
            value={form.protocol}
            onChange={(event) =>
              setForm((current) => ({ ...current, protocol: event.target.value as Proxy['protocol'] }))
            }
          >
            <option value="http">HTTP</option>
            <option value="https">HTTPS</option>
            <option value="socks5">SOCKS5</option>
          </Select>
        </Field>
        <Field label="端口" required>
          <Input
            type="number"
            min={1}
            max={65535}
            value={form.port}
            onChange={(event) => setForm((current) => ({ ...current, port: event.target.value }))}
          />
        </Field>
        <Field label="主机" required className="col-span-2">
          <Input
            value={form.host}
            onChange={(event) => setForm((current) => ({ ...current, host: event.target.value }))}
            placeholder="proxy.example.com 或 10.0.0.8"
          />
        </Field>
        <Field label="用户名">
          <Input
            value={form.username}
            onChange={(event) => setForm((current) => ({ ...current, username: event.target.value }))}
            autoComplete="off"
          />
        </Field>
        <Field label={proxy?.hasPassword ? '新密码（留空不修改）' : '密码'}>
          <Input
            type="password"
            value={form.password}
            onChange={(event) => setForm((current) => ({ ...current, password: event.target.value }))}
            autoComplete="new-password"
            placeholder={proxy?.hasPassword ? '••••••••' : ''}
          />
        </Field>
        {form.protocol === 'socks5' && form.username.trim() ? (
          <p className="col-span-2 rounded-md border border-border bg-surface-secondary px-3 py-2 text-[11px] text-[var(--warning)]">
            Chromium 不支持 SOCKS5 用户名/密码认证，带认证的 SOCKS5 代理在浏览器中不可用；请改用 HTTP 代理或本地转发。
          </p>
        ) : null}
        <Field label="分组">
          <Input
            value={form.groupName}
            onChange={(event) => setForm((current) => ({ ...current, groupName: event.target.value }))}
            placeholder="例如：美国住宅"
          />
        </Field>
        <Field label="标签" hint="用逗号或空格分隔">
          <Input
            value={form.tags}
            onChange={(event) => setForm((current) => ({ ...current, tags: event.target.value }))}
            placeholder="us, residential"
          />
        </Field>
        <Field label="备注" className="col-span-2">
          <Input
            value={form.note}
            onChange={(event) => setForm((current) => ({ ...current, note: event.target.value }))}
            placeholder="可选"
          />
        </Field>
        <div className="col-span-2 rounded-md border border-border bg-surface-secondary p-3">
          <Switch
            checked={form.enabled}
            onCheckedChange={(value) => setForm((current) => ({ ...current, enabled: value }))}
            label="启用该代理"
            description="停用后绑定环境将无法打开，已打开进程不受影响。"
          />
        </div>
      </div>
    </Dialog>
  );
}

function ProxyImportDialog({
  open,
  onOpenChange,
  onImported,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImported: () => void;
}) {
  const toast = useToastStore();
  const [text, setText] = React.useState('');
  const [protocol, setProtocol] = React.useState<'http' | 'https' | 'socks5'>('http');
  const [enabled, setEnabled] = React.useState(true);
  const [groupName, setGroupName] = React.useState('');
  const [tags, setTags] = React.useState('');
  const [namePrefix, setNamePrefix] = React.useState('');
  const [onDuplicate, setOnDuplicate] = React.useState<'skip' | 'update' | 'error'>('skip');
  const [preview, setPreview] = React.useState<ProxyImportPreview | null>(null);
  const [busy, setBusy] = React.useState(false);

  const options = React.useMemo<ProxyImportOptions>(
    () => ({
      defaultProtocol: protocol,
      enabled,
      groupName: groupName.trim() || undefined,
      tags: splitTags(tags),
      namePrefix: namePrefix.trim() || undefined,
      onDuplicate,
    }),
    [protocol, enabled, groupName, tags, namePrefix, onDuplicate],
  );

  React.useEffect(() => {
    if (!open) {
      setPreview(null);
      setBusy(false);
      setText('');
    }
  }, [open]);

  const runPreview = async () => {
    if (!text.trim()) {
      toast.warning('请先粘贴或选择代理内容');
      return;
    }
    setBusy(true);
    try {
      setPreview(await ipc.proxyImportPreview({ text, options }));
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  const importCount = preview
    ? onDuplicate === 'update'
      ? preview.valid
      : Math.max(0, preview.valid - preview.duplicates)
    : 0;

  const commit = async () => {
    if (!preview || importCount === 0) {
      toast.warning('没有可导入的有效代理');
      return;
    }
    setBusy(true);
    try {
      const result = await ipc.proxyImportCommit({ text, options });
      toast.success(
        '批量导入完成',
        `新增 ${result.inserted}，更新 ${result.updated}，跳过 ${result.skipped}，失败 ${result.failed}`,
      );
      onImported();
      onOpenChange(false);
      setText('');
      setPreview(null);
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="批量导入代理"
      description="支持纯文本、Base64、URI、JSON 与常见 Clash HTTP/SOCKS5 节点；行尾 # 注释（如 #cn）会被忽略。提交时后端会重新解析。"
      width="max-w-4xl"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button variant="secondary" onClick={() => void runPreview()} loading={busy}>
            <FileText className="h-4 w-4" /> 解析预览
          </Button>
          <Button variant="primary" onClick={() => void commit()} loading={busy} disabled={!preview || importCount === 0}>
            导入 {importCount} 条
          </Button>
        </>
      }
    >
      <div className="grid gap-4 lg:grid-cols-[1.1fr_0.9fr]">
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-content-main">代理内容</span>
            <label className="cursor-pointer text-xs text-primary">
              选择文件
              <input
                type="file"
                className="hidden"
                accept=".txt,.csv,.json,.yaml,.yml"
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  if (!file) return;
                  void file.text().then(setText);
                  event.target.value = '';
                }}
              />
            </label>
          </div>
          <Textarea
            className="min-h-[320px] font-mono text-[11px]"
            value={text}
            onChange={(event) => {
              setText(event.target.value);
              setPreview(null);
            }}
            placeholder={`socks5://user:pass@1.2.3.4:1080\nsocks5://1.2.3.4:1080 #cn\n1.2.3.5:8080\n1.2.3.6:8080:user:pass`}
          />
          <div className="grid grid-cols-2 gap-3">
            <Field label="默认协议">
              <Select value={protocol} onChange={(event) => setProtocol(event.target.value as typeof protocol)}>
                <option value="http">HTTP</option>
                <option value="https">HTTPS</option>
                <option value="socks5">SOCKS5</option>
              </Select>
            </Field>
            <Field label="重复策略">
              <Select value={onDuplicate} onChange={(event) => setOnDuplicate(event.target.value as typeof onDuplicate)}>
                <option value="skip">跳过</option>
                <option value="update">更新密码</option>
                <option value="error">视为错误</option>
              </Select>
            </Field>
            <Field label="分组">
              <Input value={groupName} onChange={(event) => setGroupName(event.target.value)} placeholder="可选" />
            </Field>
            <Field label="标签" hint="逗号或空格分隔">
              <Input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="us, residential" />
            </Field>
            <Field label="名称前缀" hint="留空则使用 host:port">
              <Input value={namePrefix} onChange={(event) => setNamePrefix(event.target.value)} placeholder="US-" />
            </Field>
            <div className="flex items-end pb-1">
              <Switch checked={enabled} onCheckedChange={setEnabled} label="导入后启用" />
            </div>
          </div>
        </div>
        <div className="min-w-0 rounded-md border border-border bg-surface-secondary p-3">
          <p className="text-xs font-medium text-content-main">解析预览</p>
          {!preview ? (
            <p className="mt-6 text-center text-xs text-content-hint">点击「解析预览」查看统计和错误行。</p>
          ) : (
            <div className="mt-3 space-y-3">
              <div className="grid grid-cols-2 gap-2 text-[11px]">
                <PreviewStat label="有效" value={preview.valid} />
                <PreviewStat label="错误" value={preview.invalid} tone={preview.invalid ? 'danger' : undefined} />
                <PreviewStat label="重复" value={preview.duplicates} tone={preview.duplicates ? 'warning' : undefined} />
                <PreviewStat label="不支持" value={preview.unsupported} />
              </div>
              <div className="max-h-[260px] space-y-1 overflow-y-auto font-mono text-[10px]">
                {preview.rows.slice(0, 200).map((row, index) => (
                  <div key={`${row.line}-${index}`} className="flex items-center gap-2 rounded px-1 py-0.5">
                    <span className="w-7 shrink-0 text-content-hint">{row.line}</span>
                    <span className="min-w-0 flex-1 truncate text-content-main">
                      {row.protocol}://{row.host}:{row.port}
                      {row.username ? ` @${row.username}` : ''}
                    </span>
                    {row.duplicate ? <Badge tone="warning">{row.duplicateKind === 'in_db' ? '库内重复' : '批内重复'}</Badge> : null}
                  </div>
                ))}
                {preview.errors.slice(0, 30).map((error, index) => (
                  <div key={`err-${error.line}-${index}`} className="rounded bg-[color-mix(in_srgb,var(--danger)_10%,transparent)] px-1 py-0.5 text-danger">
                    第 {error.line} 行：{error.message}
                  </div>
                ))}
              </div>
              {preview.rows.length > 200 ? <p className="text-[10px] text-content-hint">仅预览前 200 行。</p> : null}
            </div>
          )}
        </div>
      </div>
    </Dialog>
  );
}

function PreviewStat({ label, value, tone }: { label: string; value: number; tone?: 'danger' | 'warning' }) {
  return (
    <div className="rounded border border-border bg-surface-base px-2 py-1.5">
      <p className="text-[10px] text-content-hint">{label}</p>
      <p className={tone === 'danger' ? 'text-danger' : tone === 'warning' ? 'text-[var(--warning)]' : 'text-content-main'}>
        {value}
      </p>
    </div>
  );
}

function ProxyBatchPanel({
  open,
  onOpenChange,
  job,
  results,
  onCancel,
  onRetryFailed,
  onSelectWorking,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  job: ProxyBatchProgress | null;
  results: ProxyCheckResult[];
  onCancel: () => void;
  onRetryFailed: () => void;
  onSelectWorking: () => void;
}) {
  if (!job) return null;
  const percent = job.total > 0 ? Math.round((job.completed / job.total) * 100) : 0;
  const failedResults = results.filter((item) => !item.ok).length;
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="批量检测"
      description={job.message}
      width="max-w-2xl"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)}>
            关闭
          </Button>
          {job.state === 'running' ? (
            <Button variant="danger" onClick={onCancel}>
              取消检测
            </Button>
          ) : (
            <>
              <Button variant="secondary" onClick={onRetryFailed}>
                重试失败项
              </Button>
              <Button variant="primary" onClick={onSelectWorking}>
                选择可用节点
              </Button>
            </>
          )}
        </>
      }
    >
      <div className="space-y-4">
        <div className="space-y-1.5">
          <div className="flex items-center justify-between text-xs text-content-weak">
            <span>
              {job.completed}/{job.total}
            </span>
            <span>
              成功 {job.ok} · 失败 {results.length > 0 ? failedResults : job.failed} · {percent}%
            </span>
          </div>
          <div className="h-2 overflow-hidden rounded-full bg-surface-tertiary">
            <div className="h-full rounded-full bg-primary transition-all" style={{ width: `${percent}%` }} />
          </div>
        </div>
        <div className="max-h-[320px] space-y-1 overflow-y-auto rounded-md border border-border p-2">
          {job.recent.length === 0 ? (
            <p className="py-6 text-center text-xs text-content-hint">等待检测结果…</p>
          ) : (
            [...job.recent].reverse().map((result) => (
              <div key={`${result.proxyId}-${result.checkedAt}`} className="flex items-center gap-2 rounded px-1 py-1 text-[11px]">
                <StatusDot tone={result.ok ? 'success' : 'danger'} />
                <span className="min-w-0 flex-1 truncate font-mono text-content-weak">{result.proxyId.slice(0, 8)}…</span>
                <span className={result.ok ? 'text-[var(--success)]' : 'text-danger'}>
                  {result.ok ? result.exitIp ?? '连通' : result.message}
                </span>
                <span className="w-14 text-right tabular-nums text-content-hint">
                  {result.latencyMs != null ? `${result.latencyMs}ms` : '—'}
                </span>
              </div>
            ))
          )}
        </div>
      </div>
    </Dialog>
  );
}

function BulkMetaDialog({
  open,
  onOpenChange,
  count,
  onApply,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  count: number;
  onApply: (patch: { groupName?: string; addTags?: string[]; removeTags?: string[]; note?: string }) => void;
}) {
  const [groupName, setGroupName] = React.useState('');
  const [addTags, setAddTags] = React.useState('');
  const [removeTags, setRemoveTags] = React.useState('');
  const [note, setNote] = React.useState('');
  React.useEffect(() => {
    if (!open) return;
    setGroupName('');
    setAddTags('');
    setRemoveTags('');
    setNote('');
  }, [open]);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="批量设置分组 / 标签"
      description={`将应用到选中的 ${count} 个代理；留空表示不修改。`}
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button
            variant="primary"
            onClick={() =>
              onApply({
                groupName: groupName.trim() || undefined,
                addTags: splitTags(addTags),
                removeTags: splitTags(removeTags),
                note: note.trim() || undefined,
              })
            }
          >
            应用
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="分组">
          <Input value={groupName} onChange={(event) => setGroupName(event.target.value)} placeholder="留空不修改" />
        </Field>
        <Field label="添加标签" hint="逗号或空格分隔">
          <Input value={addTags} onChange={(event) => setAddTags(event.target.value)} />
        </Field>
        <Field label="移除标签" hint="逗号或空格分隔">
          <Input value={removeTags} onChange={(event) => setRemoveTags(event.target.value)} />
        </Field>
        <Field label="备注">
          <Input value={note} onChange={(event) => setNote(event.target.value)} placeholder="留空不修改" />
        </Field>
      </div>
    </Dialog>
  );
}

function SubscriptionEditorDialog({
  open,
  onOpenChange,
  subscription,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  subscription: ProxySubscription | null;
  onSaved: (created: ProxySubscription | null, refreshNow: boolean) => void;
}) {
  const toast = useToastStore();
  const [busy, setBusy] = React.useState(false);
  const [form, setForm] = React.useState({
    name: '',
    url: '',
    headers: '',
    protocol: 'http' as ProxySubscription['defaultProtocol'],
    groupName: '',
    tags: '',
    enabled: true,
    autoRefresh: false,
    interval: '360',
  });
  const [customInterval, setCustomInterval] = React.useState('360');
  const [refreshNow, setRefreshNow] = React.useState(false);

  React.useEffect(() => {
    if (!open) return;
    const interval = String(subscription?.refreshIntervalMin ?? 360);
    const preset = ['5', '60', '360', '720', '1440'].includes(interval);
    setForm({
      name: subscription?.name ?? '',
      url: subscription?.url ?? '',
      headers: subscription?.headers ?? '',
      protocol: subscription?.defaultProtocol ?? 'http',
      groupName: subscription?.groupName ?? '',
      tags: (subscription?.tags ?? []).join(', '),
      enabled: subscription?.enabled ?? true,
      autoRefresh: subscription?.autoRefresh ?? false,
      interval: preset ? interval : 'custom',
    });
    setCustomInterval(interval);
    setRefreshNow(false);
  }, [open, subscription]);

  const save = async () => {
    if (!form.name.trim() || !form.url.trim()) {
      toast.warning('请填写订阅名称与 URL');
      return;
    }
    setBusy(true);
    try {
      let saved: ProxySubscription;
      if (subscription) {
        saved = await ipc.proxySubscriptionUpdate(subscription.id, {
          name: form.name.trim(),
          url: form.url.trim(),
          headers: form.headers.trim(),
          defaultProtocol: form.protocol,
          groupName: form.groupName.trim(),
          tags: splitTags(form.tags),
          enabled: form.enabled,
          autoRefresh: form.autoRefresh,
          refreshIntervalMin: Number(form.interval === 'custom' ? customInterval : form.interval),
        });
        toast.success('订阅已更新');
      } else {
        saved = await ipc.proxySubscriptionCreate({
          name: form.name.trim(),
          url: form.url.trim(),
          headers: form.headers.trim() || null,
          defaultProtocol: form.protocol,
          groupName: form.groupName.trim(),
          tags: splitTags(form.tags),
          enabled: form.enabled,
          autoRefresh: form.autoRefresh,
          refreshIntervalMin: Number(form.interval === 'custom' ? customInterval : form.interval),
        });
        toast.success('订阅已创建');
      }
      onSaved(subscription ? null : saved, refreshNow);
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={subscription ? '编辑订阅' : '新增订阅'}
      description="订阅 URL 与请求头可查看和复制；落库仍使用本机 AES 加密。"
      width="max-w-2xl"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button variant="primary" loading={busy} onClick={() => void save()}>
            保存
          </Button>
        </>
      }
    >
      <div className="grid grid-cols-2 gap-3">
        <Field label="订阅名称" required className="col-span-2">
          <Input value={form.name} onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))} autoFocus />
        </Field>
        <Field
          label="订阅 URL"
          required
          className="col-span-2"
          hint="支持 http/https；GitHub 文件页（/blob/）链接会自动转换为 raw 内容地址"
        >
          <Input value={form.url} onChange={(event) => setForm((current) => ({ ...current, url: event.target.value }))} placeholder="https://github.com/HankNovic/ProxyClean/blob/main/SOCKS5.txt" />
        </Field>
        <Field label="默认协议">
          <Select value={form.protocol} onChange={(event) => setForm((current) => ({ ...current, protocol: event.target.value as typeof form.protocol }))}>
            <option value="http">HTTP</option>
            <option value="https">HTTPS</option>
            <option value="socks5">SOCKS5</option>
          </Select>
        </Field>
        <Field label="分组">
          <Input value={form.groupName} onChange={(event) => setForm((current) => ({ ...current, groupName: event.target.value }))} placeholder="新节点默认分组" />
        </Field>
        <Field label="标签" hint="逗号或空格分隔">
          <Input value={form.tags} onChange={(event) => setForm((current) => ({ ...current, tags: event.target.value }))} />
        </Field>
        <Field label="刷新周期">
          <div className="flex gap-2">
            <Select
              value={form.interval}
              onChange={(event) => setForm((current) => ({ ...current, interval: event.target.value }))}
              disabled={!form.autoRefresh}
            >
              <option value="5">5 分钟</option>
              <option value="60">1 小时</option>
              <option value="360">6 小时</option>
              <option value="720">12 小时</option>
              <option value="1440">24 小时</option>
              <option value="custom">自定义</option>
            </Select>
            {form.interval === 'custom' ? (
              <Input
                type="number"
                min={5}
                max={10080}
                value={customInterval}
                onChange={(event) => setCustomInterval(event.target.value)}
                disabled={!form.autoRefresh}
                className="w-24"
                placeholder="分钟"
              />
            ) : null}
          </div>
        </Field>
        <Field label="请求头（JSON，可选）" className="col-span-2" hint='例如 {"Authorization":"Bearer ..."}'>
          <Textarea
            className="min-h-[80px] font-mono text-[11px]"
            value={form.headers}
            onChange={(event) => setForm((current) => ({ ...current, headers: event.target.value }))}
            placeholder="{}"
          />
        </Field>
        <div className="col-span-2 space-y-2 rounded-md border border-border bg-surface-secondary p-3">
          <Switch checked={form.enabled} onCheckedChange={(value) => setForm((current) => ({ ...current, enabled: value }))} label="启用该订阅" />
          <Switch
            checked={form.autoRefresh}
            onCheckedChange={(value) => setForm((current) => ({ ...current, autoRefresh: value }))}
            label="自动刷新"
            description="默认关闭；开启后仅在应用运行期间按周期刷新。"
          />
        </div>
        {!subscription ? (
          <label className="col-span-2 flex items-center gap-2 text-xs text-content-weak">
            <input
              type="checkbox"
              className="accent-[var(--color-primary)]"
              checked={refreshNow}
              onChange={(event) => setRefreshNow(event.target.checked)}
            />
            创建后立即刷新一次
          </label>
        ) : null}
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
  const [restartRunning, setRestartRunning] = React.useState(true);

  React.useEffect(() => {
    if (!open || !proxy) return;
    setSelected(envs.filter((env) => env.proxyId === proxy.id).map((env) => env.id));
    setRestartRunning(true);
  }, [open, proxy, envs]);

  const runningAffected = envs.filter(
    (env) =>
      env.runtime &&
      ((selected.includes(env.id) && env.proxyId !== proxy?.id) ||
        (!selected.includes(env.id) && env.proxyId === proxy?.id)),
  );

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="分配代理到环境"
      description={`将「${proxy?.name ?? ''}」绑定到选中的环境；已绑定的环境可取消。`}
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
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
                if (restartRunning && runningAffected.length > 0) {
                  const runningIds = new Set(runningAffected.map((env) => env.id));
                  const normalPreviouslyBound = previouslyBound.filter((id) => !runningIds.has(id));
                  const normalSelected = selected.filter((id) => !runningIds.has(id));
                  if (normalPreviouslyBound.length > 0) {
                    await ipc.proxyAssign(normalPreviouslyBound, null);
                  }
                  await ipc.proxyAssign(normalSelected, proxy.id);
                  for (const env of runningAffected) {
                    await ipc.browserSwitchProxy(
                      env.id,
                      selected.includes(env.id) ? proxy.id : null,
                      true,
                    );
                  }
                  toast.success(
                    '代理分配完成',
                    `${normalSelected.length + runningAffected.length} 个环境已更新，${runningAffected.length} 个运行中环境已重启生效`,
                  );
                } else {
                  if (previouslyBound.length > 0) {
                    await ipc.proxyAssign(previouslyBound, null);
                  }
                  const count = await ipc.proxyAssign(selected, proxy.id);
                  toast.success('代理分配完成', `${count} 个环境已更新${previouslyBound.length ? `，${previouslyBound.length} 个环境已解绑` : ''}`);
                }
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
      <ProxyLocationSummary proxy={proxy} className="mb-3" />
      <div className="mb-2 flex items-center justify-between text-[11px] text-content-hint">
        <span>已选 {selected.length} 个环境</span>
        <button
          type="button"
          className="text-primary"
          onClick={() => setSelected(selected.length === envs.length ? [] : envs.map((env) => env.id))}
        >
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
                setSelected((current) =>
                  event.target.checked ? [...current, env.id] : current.filter((id) => id !== env.id),
                )
              }
            />
            <span className="min-w-0 flex-1 truncate">{env.name}</span>
            {env.proxyId === proxy?.id ? <Badge tone="primary">已绑定</Badge> : null}
            {env.runtime ? <Badge tone="success">运行中</Badge> : null}
          </label>
        ))}
      </div>
      {runningAffected.length > 0 ? (
        <label className="mt-2 flex items-center gap-2 text-xs text-content-weak">
          <input
            type="checkbox"
            className="accent-[var(--color-primary)]"
            checked={restartRunning}
            onChange={(event) => setRestartRunning(event.target.checked)}
          />
          自动重启 {runningAffected.length} 个运行中的环境，使代理立即生效
        </label>
      ) : (
        <p className="mt-2 text-[10px] text-content-hint">保存后运行中的环境需重启浏览器，代理才会生效。</p>
      )}
    </Dialog>
  );
}

function PostDeleteDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel = '确认删除',
  onConfirm,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: React.ReactNode;
  description?: React.ReactNode;
  confirmLabel?: string;
  onConfirm: () => void | Promise<void>;
  children?: React.ReactNode;
}) {
  const [busy, setBusy] = React.useState(false);
  React.useEffect(() => {
    if (open) setBusy(false);
  }, [open]);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      description={description}
      width="max-w-md"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button
            variant="danger"
            loading={busy}
            onClick={async () => {
              setBusy(true);
              try {
                await onConfirm();
              } finally {
                setBusy(false);
              }
            }}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      {children ?? <p className="text-xs text-content-weak">此操作不可撤销。</p>}
    </Dialog>
  );
}

function splitTags(value: string): string[] {
  return value
    .split(/[,，\s]+/)
    .map((tag) => tag.trim())
    .filter(Boolean);
}

function cleanupStatusLabel(status: string): string {
  return status === 'disabled'
    ? '已停用'
    : status === 'fail'
      ? '检测失败'
      : status === 'untested'
        ? '未检测'
        : status;
}

function discoveryPhaseLabel(phase: string): string {
  return phase === 'fetch'
    ? '抓取来源'
    : phase === 'tcp'
      ? 'TCP 预筛'
      : phase === 'google'
        ? 'Google 探测'
        : phase === 'verify'
          ? '完整验证'
          : phase === 'done'
            ? '完成'
            : phase;
}

function PromptChips({ items, onPick }: { items: string[]; onPick: (value: string) => void }) {  return (
    <div className="flex flex-wrap gap-1">
      {items.map((item) => (
        <button
          key={item}
          type="button"
          className="rounded border border-border bg-surface-base px-1.5 py-0.5 text-[10px] text-content-weak hover:border-primary hover:text-primary"
          onClick={() => onPick(item)}
        >
          {item}
        </button>
      ))}
    </div>
  );
}

function formatInterval(minutes: number): string {
  if (minutes >= 1440 && minutes % 1440 === 0) return `${minutes / 1440} 天`;
  if (minutes >= 60 && minutes % 60 === 0) return `${minutes / 60} 小时`;
  return `${minutes} 分钟`;
}
