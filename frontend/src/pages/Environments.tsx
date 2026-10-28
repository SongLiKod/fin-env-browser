import * as React from 'react';
import { Link, useNavigate } from 'react-router-dom';
import {
  Activity,
  ArrowRight,
  Boxes,
  Camera,
  CheckSquare,
  Copy,
  Download,
  ExternalLink,
  Fingerprint,
  FolderKanban,
  Globe2,
  Layers,
  LayoutTemplate,
  MemoryStick,
  MonitorPlay,
  Plus,
  Radio,
  RefreshCw,
  Search,
  Square,
  Trash2,
  Upload,
  UserRound,
  Users,
  X,
  Zap,
} from 'lucide-react';
import { ipc } from '@/ipc/client';
import type {
  DashboardStats,
  EnvironmentView,
  Proxy,
  ScreenshotResult,
} from '@/ipc/types';
import { useAuthStore } from '@/store/auth';
import { useEnvStore } from '@/store/env';
import { toastError, useToastStore } from '@/store/toast';
import { cn, envStatusMeta, formatDateShort, relativeTime, testEnvLabels, truncate } from '@/utils';
import { envBorderColor, envBorderWidth, envAccessLabel, envHasAccess, envIpSummary, hexToRgba } from '@/utils/environment';
import { PageHeader } from '@/components/layout/PageHeader';
import { Badge, StatusDot } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, EmptyState, StatCard, Spinner } from '@/components/ui/card';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { Dialog } from '@/components/ui/dialog';
import { Input, Select } from '@/components/ui/input';
import { Tooltip } from '@/components/ui/tooltip';
import {
  AssignProxyDialog,
  CloneEnvironmentDialog,
  CreateEnvironmentDialog,
  ExportEnvironmentsDialog,
  GroupManagerDialog,
  ImportEnvironmentsDialog,
} from '@/components/env/EnvironmentDialogs';
import { BatchOperationsDialog } from '@/components/env/BatchOperationsDialog';
import { BroadcastControlDialog } from '@/components/env/BroadcastControlDialog';
import { TemplateManagerDialog } from '@/components/env/TemplateManagerDialog';

const PAGE_SIZE = 20;

export function EnvironmentsPage() {
  const navigate = useNavigate();
  const toast = useToastStore();
  const can = useAuthStore((state) => state.can);
  const envs = useEnvStore((state) => state.envs);
  const groups = useEnvStore((state) => state.groups);
  const projects = useEnvStore((state) => state.projects);
  const tags = useEnvStore((state) => state.tags);
  const filter = useEnvStore((state) => state.filter);
  const setFilter = useEnvStore((state) => state.setFilter);
  const resetFilter = useEnvStore((state) => state.resetFilter);
  const refresh = useEnvStore((state) => state.refresh);
  const syncRuntimes = useEnvStore((state) => state.syncRuntimes);
  const loading = useEnvStore((state) => state.loading);

  const [search, setSearch] = React.useState(filter.search ?? '');
  const [selected, setSelected] = React.useState<Set<string>>(new Set());
  const [page, setPage] = React.useState(1);
  const [stats, setStats] = React.useState<DashboardStats | null>(null);
  const [proxies, setProxies] = React.useState<Proxy[]>([]);
  const [busyId, setBusyId] = React.useState<string | null>(null);
  const [ipBusyId, setIpBusyId] = React.useState<string | null>(null);
  const [recycleBusy, setRecycleBusy] = React.useState(false);
  const [showCreate, setShowCreate] = React.useState(false);
  const [showImport, setShowImport] = React.useState(false);
  const [showExport, setShowExport] = React.useState(false);
  const [showGroups, setShowGroups] = React.useState(false);
  const [showTemplates, setShowTemplates] = React.useState(false);
  const [showBatchOps, setShowBatchOps] = React.useState(false);
  const [showBroadcast, setShowBroadcast] = React.useState(false);
  const [showAssignProxy, setShowAssignProxy] = React.useState(false);
  const [cloneTarget, setCloneTarget] = React.useState<EnvironmentView | null>(null);
  const [deleteTarget, setDeleteTarget] = React.useState<EnvironmentView | null>(null);
  const [bulkDelete, setBulkDelete] = React.useState(false);
  const [recycleConfirm, setRecycleConfirm] = React.useState(false);
  const [screenshot, setScreenshot] = React.useState<ScreenshotResult | null>(null);

  const firstLoad = React.useRef(true);

  const loadStats = React.useCallback(async () => {
    try {
      const [data, proxyList] = await Promise.all([ipc.dashboard(), ipc.proxyList()]);
      setStats(data);
      setProxies(proxyList);
    } catch (error) {
      toastError(error);
    }
  }, []);

  React.useEffect(() => {
    const withFacets = firstLoad.current;
    firstLoad.current = false;
    void refresh(withFacets)
      .then(loadStats)
      .catch(toastError);
  }, [filter, refresh, loadStats]);

  React.useEffect(() => {
    const timer = window.setInterval(() => {
      void syncRuntimes();
    }, 5000);
    const statTimer = window.setInterval(() => {
      void loadStats();
    }, 15000);
    return () => {
      window.clearInterval(timer);
      window.clearInterval(statTimer);
    };
  }, [syncRuntimes, loadStats]);

  React.useEffect(() => {
    const timer = window.setTimeout(() => {
      if ((filter.search ?? '') !== search) setFilter({ search: search || null });
    }, 280);
    return () => window.clearTimeout(timer);
  }, [search, setFilter, filter.search]);

  React.useEffect(() => {
    setPage(1);
  }, [filter]);

  const selectedEnvs = envs.filter((env) => selected.has(env.id));
  const canOperateSelected = selectedEnvs.every((env) => envHasAccess(env, 'operate'));
  const canManageSelected = selectedEnvs.every((env) => envHasAccess(env, 'manage'));
  const running = envs.filter((env) => env.runtime);
  const totalMemory = running.reduce((total, env) => total + (env.runtime?.memoryMb ?? 0), 0);
  const totalAccounts = envs.reduce((total, env) => total + env.accountCount, 0);
  const pageCount = Math.max(1, Math.ceil(envs.length / PAGE_SIZE));
  const visible = envs.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);
  const allVisibleSelected = visible.length > 0 && visible.every((env) => selected.has(env.id));

  const toggleAll = () => {
    setSelected((current) => {
      const next = new Set(current);
      if (allVisibleSelected) visible.forEach((env) => next.delete(env.id));
      else visible.forEach((env) => next.add(env.id));
      return next;
    });
  };

  const openEnv = async (env: EnvironmentView) => {
    setBusyId(env.id);
    try {
      const runtime = await ipc.browserOpen(env.id, {});
      useEnvStore.getState().applyRuntime(runtime);
      toast.success('环境已启动', `${env.name} · PID ${runtime.pid} · 端口 ${runtime.debugPort}`);
      if (runtime.warnings.length > 0) {
        toast.warning('启动时存在告警', runtime.warnings.join('；'));
      }
      void refreshIp(env);
      await loadStats();
    } catch (error) {
      toastError(error);
    } finally {
      setBusyId(null);
    }
  };

  const closeEnv = async (env: EnvironmentView) => {
    setBusyId(env.id);
    try {
      await ipc.browserClose(env.id);
      useEnvStore.getState().clearRuntime(env.id);
      toast.success('环境已关闭', env.name);
      await loadStats();
    } catch (error) {
      toastError(error);
    } finally {
      setBusyId(null);
    }
  };

  const refreshIp = async (env: EnvironmentView) => {
    setIpBusyId(env.id);
    try {
      await ipc.envCheckIp(env.id);
      const next = await ipc.envGet(env.id);
      useEnvStore.getState().replace(next);
    } catch (error) {
      toastError(error);
    } finally {
      setIpBusyId(null);
    }
  };

  const capture = async (env: EnvironmentView) => {    setBusyId(env.id);
    try {
      const result = await ipc.browserScreenshot(env.id);
      setScreenshot(result);
    } catch (error) {
      toastError(error);
    } finally {
      setBusyId(null);
    }
  };

  const assignGroup = async (groupId: string) => {
    if (!groupId || selected.size === 0) return;
    try {
      const count = await ipc.envAssignGroup(Array.from(selected), groupId || null);
      toast.success('分组已更新', `${count} 个环境`);
      setSelected(new Set());
      await refresh(false);
    } catch (error) {
      toastError(error);
    }
  };

  return (
    <div className="page-pad space-y-4">
      <PageHeader
        eyebrow="Workspace / Environments"
        title="环境管理"
        description="每个测试账号绑定一套独立 profile 与自洽指纹；关闭面板后浏览器进程可继续保留并自动接管。"
        actions={
          <>
            {can('open_close') ? (
              <Button
                variant="secondary"
                onClick={() => setRecycleConfirm(true)}
                disabled={running.length === 0}
              >
                <Square className="h-4 w-4" /> 全部回收
              </Button>
            ) : null}
            <Button
              variant="secondary"
              onClick={async () => {
                try {
                  await refresh(false);
                  await loadStats();
                  await syncRuntimes();
                  toast.success('列表已刷新');
                } catch (error) {
                  toastError(error);
                }
              }}
            >
              <RefreshCw className={cn('h-4 w-4', loading && 'animate-spin')} /> 刷新
            </Button>
            {can('create_env') ? (
              <Button variant="secondary" onClick={() => setShowTemplates(true)}>
                <LayoutTemplate className="h-4 w-4" /> 模板
              </Button>
            ) : null}
            {can('create_env') ? (
              <>
                <Button variant="secondary" onClick={() => setShowImport(true)}>
                  <Upload className="h-4 w-4" /> 导入
                </Button>
                <Button variant="primary" onClick={() => setShowCreate(true)}>
                  <Plus className="h-4 w-4" /> 新建环境
                </Button>
              </>
            ) : null}
          </>
        }
      />

      <div className="grid grid-cols-2 gap-3 xl:grid-cols-5">
        <StatCard label="环境总数" value={envs.length} hint={`${stats?.snapshotCount ?? 0} 个指纹快照`} icon={<Boxes className="h-4 w-4" />} tone="primary" />
        <StatCard label="运行中" value={`${running.length} / ${stats?.maxConcurrent ?? '—'}`} hint="全局并发上限" icon={<Activity className="h-4 w-4" />} tone="success" />
        <StatCard label="账号台账" value={totalAccounts} hint="密码本地加密" icon={<Users className="h-4 w-4" />} />
        <StatCard label="实时内存" value={`${totalMemory} MB`} hint={`运行实例内存合计 · 上限 ${stats?.maxConcurrent ?? '—'} 个`} icon={<MemoryStick className="h-4 w-4" />} />
        <StatCard label="代理池" value={proxies.length} hint={`${envs.filter((env) => env.proxyId).length} 个环境已绑定`} icon={<Globe2 className="h-4 w-4" />} />
      </div>

      <Card className="overflow-hidden">
        <div className="space-y-2.5 border-b border-border bg-surface-secondary/40 p-4">
          <div className="flex flex-wrap items-center gap-2">
            <div className="relative min-w-[260px] flex-[1_1_320px]">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-content-hint" />
              <Input
                className="pl-9"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="搜索环境名称、项目、标签、用例 ID、版本…"
              />
              {search ? (
                <button
                  type="button"
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-content-hint hover:text-content-main"
                  onClick={() => setSearch('')}
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              ) : null}
            </div>
            <Select
              wrapperClassName="w-[130px]"
              value={filter.groupId ?? ''}
              placeholder="全部分组"
              onChange={(event) => setFilter({ groupId: event.target.value || null })}
            >
              {groups.map((group) => (
                <option key={group.id} value={group.id}>
                  {group.name}
                </option>
              ))}
            </Select>
            <Select
              wrapperClassName="w-[120px]"
              value={filter.status ?? ''}
              placeholder="全部状态"
              onChange={(event) => setFilter({ status: event.target.value || null })}
            >
              <option value="active">已启用</option>
              <option value="disabled">已停用</option>
              <option value="archived">已归档</option>
            </Select>
            <Select
              wrapperClassName="w-[140px]"
              value={filter.testEnv ?? ''}
              placeholder="全部测试环境"
              onChange={(event) => setFilter({ testEnv: event.target.value || null })}
            >
              {Object.entries(testEnvLabels).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </Select>
            <Select
              wrapperClassName="w-[130px]"
              value={filter.project ?? ''}
              placeholder="全部项目"
              onChange={(event) => setFilter({ project: event.target.value || null })}
            >
              {projects.map((project) => (
                <option key={project} value={project}>
                  {project}
                </option>
              ))}
            </Select>
            <Select
              wrapperClassName="w-[120px]"
              value={filter.tag ?? ''}
              placeholder="全部标签"
              onChange={(event) => setFilter({ tag: event.target.value || null })}
            >
              {tags.map((tag) => (
                <option key={tag} value={tag}>
                  {tag}
                </option>
              ))}
            </Select>
            <label className="flex h-[var(--control-height)] cursor-pointer items-center gap-1.5 rounded-md border border-border px-2.5 text-xs text-content-weak">
              <input
                type="checkbox"
                className="accent-[var(--color-primary)]"
                checked={!!filter.runningOnly}
                onChange={(event) => setFilter({ runningOnly: event.target.checked || null })}
              />
              仅运行中
            </label>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button variant="ghost" size="sm" onClick={toggleAll}>
              {allVisibleSelected ? <CheckSquare className="h-3.5 w-3.5" /> : <Square className="h-3.5 w-3.5" />}
              全选本页
            </Button>
            {selected.size > 0 ? (
              <>
                <span className="text-xs text-content-weak">已选 {selected.size} 个</span>
                {can('open_close') && canOperateSelected ? (
                  <Button variant="soft" size="sm" onClick={() => setShowBatchOps(true)}>
                    <Zap className="h-3.5 w-3.5" /> 批量操作
                  </Button>
                ) : null}
                {can('open_close') && canOperateSelected ? (
                  <Button
                    variant="soft"
                    size="sm"
                    disabled={selected.size < 2}
                    title={selected.size < 2 ? '至少选择 2 个环境' : '鼠标/键盘实时同步到多个窗口'}
                    onClick={() => setShowBroadcast(true)}
                  >
                    <Radio className="h-3.5 w-3.5" /> 同步控制
                  </Button>
                ) : null}
                {can('edit_env') && canManageSelected ? (
                  <Select
                    className="h-7 w-[140px] text-xs"
                    value=""
                    placeholder="移动到分组"
                    onChange={(event) => void assignGroup(event.target.value)}
                  >
                    {groups.map((group) => (
                      <option key={group.id} value={group.id}>
                        {group.name}
                      </option>
                    ))}
                  </Select>
                ) : null}
                {can('export_sensitive') ? (
                  <Button variant="ghost" size="sm" onClick={() => setShowExport(true)}>
                    <Download className="h-3.5 w-3.5" /> 批量导出
                  </Button>
                ) : null}
                {can('edit_fingerprint_proxy') && canManageSelected ? (
                  <Button variant="ghost" size="sm" onClick={() => setShowAssignProxy(true)}>
                    <Globe2 className="h-3.5 w-3.5" /> 分配代理
                  </Button>
                ) : null}
                {can('delete_env') && canManageSelected ? (
                  <Button variant="ghost" size="sm" className="text-danger" onClick={() => setBulkDelete(true)}>
                    <Trash2 className="h-3.5 w-3.5" /> 批量删除
                  </Button>
                ) : null}
                <Button variant="ghost" size="sm" onClick={() => setSelected(new Set())}>
                  <X className="h-3.5 w-3.5" /> 取消选择
                </Button>
              </>
            ) : (
              <span className="text-xs text-content-hint">按项目、测试环境、角色、标签与运行状态快速筛选</span>
            )}
            <div className="ml-auto flex items-center gap-1.5">
              <span className="text-[11px] text-content-hint">共 {envs.length} 个环境</span>
              <Button variant="ghost" size="sm" onClick={() => { resetFilter(); setSearch(''); }}>
                清除筛选
              </Button>
              {can('edit_env') ? (
                <Button variant="ghost" size="sm" onClick={() => setShowGroups(true)}>
                  <Layers className="h-3.5 w-3.5" /> 分组管理
                </Button>
              ) : null}
            </div>
          </div>
        </div>

        {loading && envs.length === 0 ? (
          <div className="flex items-center justify-center gap-2 py-20 text-xs text-content-weak">
            <Spinner /> 正在加载环境…
          </div>
        ) : envs.length === 0 ? (
          <EmptyState
            icon={<Fingerprint className="h-5 w-5" />}
            title="还没有指纹环境"
            description="新建一个环境，FinEnv 会自动生成自洽的 UA、屏幕、时区、语言与 WebGL/Audio/Canvas 指纹。"
            action={
              can('create_env') ? (
                <Button variant="primary" onClick={() => setShowCreate(true)}>
                  <Plus className="h-4 w-4" /> 新建环境
                </Button>
              ) : undefined
            }
          />
        ) : (
          <div className="table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th className="w-9">
                    <input
                      type="checkbox"
                      className="accent-[var(--color-primary)]"
                      checked={allVisibleSelected}
                      onChange={toggleAll}
                      aria-label="全选"
                    />
                  </th>
                  <th>环境</th>
                  <th>项目 / 用例</th>
                  <th>指纹</th>
                  <th>网络 / IP</th>
                  <th className="text-center">账号</th>
                  <th className="min-w-[150px]">运行状态</th>
                  <th>最近打开</th>
                  <th className="w-[170px] text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {visible.map((env) => {
                  const isRunning = !!env.runtime;
                  const status = envStatusMeta[env.status];
                  const accent = envBorderColor(env);
                  const accentWidth = envBorderWidth(env);
                  return (
                    <tr
                      key={env.id}
                      style={{
                        boxShadow: `inset ${accentWidth}px 0 0 ${accent}`,
                        backgroundColor: isRunning ? hexToRgba(accent, 0.035) : undefined,
                      }}
                    >
                      <td>
                        <input
                          type="checkbox"
                          className="accent-[var(--color-primary)]"
                          checked={selected.has(env.id)}
                          onChange={(event) =>
                            setSelected((current) => {
                              const next = new Set(current);
                              if (event.target.checked) next.add(env.id);
                              else next.delete(env.id);
                              return next;
                            })
                          }
                          aria-label={`选择 ${env.name}`}
                        />
                      </td>
                      <td>
                        <div className="flex items-start gap-2">
                          <span
                            className={cn(
                              'mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-md',
                              isRunning
                                ? 'bg-[color-mix(in_srgb,var(--success)_16%,transparent)] text-[var(--success)] ring-1 ring-[color-mix(in_srgb,var(--success)_45%,transparent)]'
                                : 'text-content-hint',
                            )}
                            title={isRunning ? `环境运行中 · PID ${env.runtime?.pid}` : undefined}
                          >
                            {isRunning ? (
                              <MonitorPlay className="h-3.5 w-3.5" />
                            ) : (
                              <StatusDot
                                tone={status.tone === 'success' ? 'primary' : 'muted'}
                                className="mt-0"
                              />
                            )}
                          </span>
                          <div className="min-w-0">
                            <div className="flex items-center gap-1.5">
                              <span
                                className="h-2 w-2 shrink-0 rounded-full ring-1 ring-black/10"
                                style={{ backgroundColor: accent }}
                                title={env.borderColor || '自动标识色'}
                              />
                              <Link to={`/envs/${env.id}`} className="block truncate text-sm font-medium text-content-main hover:text-primary">
                                {env.name}
                              </Link>
                            </div>
                            <div className="mt-1 flex flex-wrap items-center gap-1">
                              <Badge
                                tone={
                                  env.accessLevel === 'manage'
                                    ? 'success'
                                    : env.accessLevel === 'operate'
                                      ? 'primary'
                                      : 'neutral'
                                }
                              >
                                我的权限 · {envAccessLabel(env.accessLevel)}
                              </Badge>
                              {isRunning ? (
                                <span className="inline-flex items-center gap-1 rounded-full border border-[color-mix(in_srgb,var(--success)_38%,transparent)] bg-[color-mix(in_srgb,var(--success)_13%,transparent)] px-2 py-0.5 text-[10px] font-semibold text-[var(--success)]">
                                  <span className="relative flex h-1.5 w-1.5">
                                    <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[var(--success)] opacity-60" />
                                    <span className="relative inline-flex h-1.5 w-1.5 rounded-full bg-[var(--success)]" />
                                  </span>
                                  运行中
                                  <span className="font-mono text-[9px] font-normal opacity-80">
                                    PID {env.runtime?.pid}
                                  </span>
                                </span>
                              ) : (
                                <Badge tone={status.tone === 'warning' ? 'warning' : status.tone === 'muted' ? 'neutral' : 'primary'}>
                                  {status.label}
                                </Badge>
                              )}
                              {env.testEnv ? <Badge tone="outline">{testEnvLabels[env.testEnv] ?? env.testEnv}</Badge> : null}
                              {env.tags.slice(0, 2).map((tag) => (
                                <Badge key={tag} tone="neutral">
                                  #{tag}
                                </Badge>
                              ))}
                              {env.tags.length > 2 ? <span className="text-[10px] text-content-hint">+{env.tags.length - 2}</span> : null}
                            </div>
                          </div>
                        </div>
                      </td>
                      <td>
                        <div className="max-w-[190px] space-y-0.5 text-xs">
                          <p className="truncate text-content-main">{env.project || '未指定项目'}</p>
                          <p className="truncate text-content-hint">
                            {env.accountRole || '未指定角色'} {env.caseId ? `· ${env.caseId}` : ''}
                          </p>
                        </div>
                      </td>
                      <td>
                        <div className="max-w-[210px] space-y-0.5">
                          <p className="flex items-center gap-1 text-xs text-content-main">
                            <Fingerprint className="h-3 w-3 text-primary" />
                            {env.fingerprint.clientHints.platform} · {env.fingerprint.timezone}
                          </p>
                          <p className="truncate font-mono text-[10px] text-content-hint" title={env.fingerprint.userAgent}>
                            {truncate(env.fingerprint.userAgent.replace('Mozilla/5.0 ', ''), 42)}
                          </p>
                        </div>
                      </td>
                      <td>
                        <div className="group/ip max-w-[170px] space-y-0.5">
                          <p className="flex items-center gap-1 font-mono text-xs text-content-main" title={envIpSummary(env)}>
                            <Globe2 className="h-3 w-3 shrink-0 text-primary" />
                            <span className="truncate">{env.ipAddress ?? 'IP 待检测'}</span>
                            <button
                              type="button"
                              className="text-content-hint opacity-0 transition-opacity hover:text-primary group-hover/ip:opacity-100"
                              title="刷新出口 IP"
                              onClick={() => void refreshIp(env)}
                            >
                              <RefreshCw className={cn('h-3 w-3', ipBusyId === env.id && 'animate-spin')} />
                            </button>
                          </p>
                          <p className="truncate text-[10px] text-content-hint">
                            {env.proxy ? env.proxy.name : '直连'}
                            {env.ipCountry || env.ipCountryCode
                              ? ` · ${env.ipCountry ?? env.ipCountryCode}`
                              : env.proxy?.exitCountry
                                ? ` · ${env.proxy.exitCountry}`
                                : ''}
                          </p>
                        </div>
                      </td>
                      <td className="text-center">
                        <span className="inline-flex items-center gap-1 text-xs text-content-weak">
                          <UserRound className="h-3 w-3" /> {env.accountCount}
                        </span>
                      </td>
                      <td>
                        {env.runtime ? (
                          <div className="space-y-0.5 text-[11px]">
                            <p className="flex items-center gap-1 text-[var(--success)]">
                              <StatusDot tone="success" pulse /> PID {env.runtime.pid}
                            </p>
                            <p className="whitespace-nowrap text-content-hint">
                              CPU {env.runtime.cpuPercent.toFixed(1)}% · {env.runtime.memoryMb} MB
                            </p>
                            <p className="max-w-[190px] truncate text-content-hint" title={env.runtime.currentUrl}>
                              {env.runtime.currentUrl || 'about:blank'}
                            </p>
                          </div>
                        ) : (
                          <span className="text-xs text-content-hint">未运行</span>
                        )}
                      </td>
                      <td>
                        <div className="text-xs text-content-weak">
                          <p>{relativeTime(env.lastOpenedAt)}</p>
                          <p className="text-[10px] text-content-hint">{formatDateShort(env.updatedAt)}</p>
                        </div>
                      </td>
                      <td>
                        <div className="flex items-center justify-end gap-1">
                          {can('open_close') && envHasAccess(env, 'operate') ? (
                            isRunning ? (
                              <Button variant="ghost" size="icon-sm" title="关闭环境" loading={busyId === env.id} onClick={() => void closeEnv(env)}>
                                <Square className="h-3.5 w-3.5" />
                              </Button>
                            ) : (
                              <Button variant="ghost" size="icon-sm" title="打开环境" loading={busyId === env.id} onClick={() => void openEnv(env)}>
                                <MonitorPlay className="h-3.5 w-3.5" />
                              </Button>
                            )
                          ) : null}
                          {can('open_close') && envHasAccess(env, 'operate') ? (
                            <Tooltip content={isRunning ? '截图' : '启动后可截图'}>
                              <Button
                                variant="ghost"
                                size="icon-sm"
                                disabled={!isRunning}
                                onClick={() => void capture(env)}
                              >
                                <Camera className="h-3.5 w-3.5" />
                              </Button>
                            </Tooltip>
                          ) : null}
                          <Button variant="ghost" size="icon-sm" title="环境详情" onClick={() => navigate(`/envs/${env.id}`)}>
                            <ArrowRight className="h-3.5 w-3.5" />
                          </Button>
                          {can('clone_env') && envHasAccess(env, 'operate') ? (
                            <Button variant="ghost" size="icon-sm" title="克隆" onClick={() => setCloneTarget(env)}>
                              <Copy className="h-3.5 w-3.5" />
                            </Button>
                          ) : null}
                          {can('delete_env') && envHasAccess(env, 'manage') ? (
                            <Button variant="ghost" size="icon-sm" title="删除" className="text-danger" onClick={() => setDeleteTarget(env)}>
                              <Trash2 className="h-3.5 w-3.5" />
                            </Button>
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

        {envs.length > PAGE_SIZE ? (
          <div className="flex items-center justify-between border-t border-border px-4 py-2.5 text-xs text-content-weak">
            <span>
              第 {page} / {pageCount} 页 · 共 {envs.length} 个环境
            </span>
            <div className="flex items-center gap-1">
              <Button variant="ghost" size="sm" disabled={page === 1} onClick={() => setPage((p) => Math.max(1, p - 1))}>
                上一页
              </Button>
              <Button variant="ghost" size="sm" disabled={page === pageCount} onClick={() => setPage((p) => Math.min(pageCount, p + 1))}>
                下一页
              </Button>
            </div>
          </div>
        ) : null}
      </Card>

      <div className="grid gap-3 lg:grid-cols-2">
        <Card className="p-4">
          <div className="flex items-center gap-2 text-sm font-semibold text-content-main">
            <FolderKanban className="h-4 w-4 text-primary" /> 最近操作
          </div>
          <div className="mt-2 space-y-1">
            {stats?.recentAudit?.slice(0, 4).map((log) => (
              <div key={log.id} className="flex items-center gap-2 rounded-md px-1 py-1 text-[11px]">
                <span className="min-w-0 flex-1 truncate text-content-weak">{log.detail}</span>
                <span className="shrink-0 text-content-hint">{formatDateShort(log.createdAt)}</span>
              </div>
            ))}
            {!stats?.recentAudit?.length ? <p className="py-3 text-xs text-content-hint">暂无审计记录</p> : null}
          </div>
        </Card>
        <Card className="p-4">
          <div className="flex items-center gap-2 text-sm font-semibold text-content-main">
            <Globe2 className="h-4 w-4 text-primary" /> 快捷开始
          </div>
          <div className="mt-2 grid grid-cols-2 gap-2">
            <button type="button" className="rounded-md border border-border px-3 py-2 text-left text-xs hover:bg-surface-secondary" onClick={() => navigate('/proxies')}>
              <span className="font-medium text-content-main">检测代理池</span>
              <span className="mt-0.5 block text-[10px] text-content-hint">TCP + 出口 IP + 时区</span>
            </button>
            <button type="button" className="rounded-md border border-border px-3 py-2 text-left text-xs hover:bg-surface-secondary" onClick={() => navigate('/captcha')}>
              <span className="font-medium text-content-main">查看验证码</span>
              <span className="mt-0.5 block text-[10px] text-content-hint">汇总所有运行环境</span>
            </button>
          </div>
        </Card>
      </div>

      <CreateEnvironmentDialog
        open={showCreate}
        onOpenChange={setShowCreate}
        groups={groups}
        proxies={proxies}
        onCreated={(env) => {
          useEnvStore.getState().replace(env);
          void refresh(false);
          void loadStats();
          navigate(`/envs/${env.id}`);
        }}
      />
      <ImportEnvironmentsDialog open={showImport} onOpenChange={setShowImport} onImported={() => { void refresh(true); void loadStats(); }} />
      <ExportEnvironmentsDialog open={showExport} onOpenChange={setShowExport} envs={selectedEnvs} />
      <AssignProxyDialog open={showAssignProxy} onOpenChange={setShowAssignProxy} envs={selectedEnvs} proxies={proxies} onDone={() => void refresh(false)} />
      <GroupManagerDialog open={showGroups} onOpenChange={setShowGroups} groups={groups} onChanged={() => void refresh(true)} />
      <TemplateManagerDialog
        open={showTemplates}
        onOpenChange={setShowTemplates}
        groups={groups}
        proxies={proxies}
        onChanged={() => {
          void refresh(true);
          void loadStats();
        }}
      />
      <BatchOperationsDialog
        open={showBatchOps}
        onOpenChange={setShowBatchOps}
        envs={selectedEnvs}
        onCompleted={() => {
          void refresh(false);
          void loadStats();
          void syncRuntimes();
        }}
      />
      <BroadcastControlDialog
        open={showBroadcast}
        onOpenChange={setShowBroadcast}
        envs={selectedEnvs}
        onChanged={() => {
          void refresh(false);
          void loadStats();
          void syncRuntimes();
        }}
      />
      <CloneEnvironmentDialog
        open={!!cloneTarget}
        onOpenChange={(value) => !value && setCloneTarget(null)}
        env={cloneTarget}
        onCloned={(env) => {
          useEnvStore.getState().replace(env);
          void refresh(false);
          void loadStats();
          setCloneTarget(null);
        }}
      />
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(value) => !value && setDeleteTarget(null)}
        title="删除环境"
        description={`将永久删除「${deleteTarget?.name ?? ''}」及其账号、快照、Hosts/Mock 与浏览器 profile。审计日志保留。`}
        confirmLabel="删除环境"
        requiredText="DELETE"
        onConfirm={async () => {
          if (!deleteTarget) return;
          try {
            await ipc.envDelete(deleteTarget.id, true);
            useEnvStore.getState().remove(deleteTarget.id);
            setDeleteTarget(null);
            toast.success('环境已删除');
            await loadStats();
          } catch (error) {
            toastError(error);
          }
        }}
      />
      <ConfirmDialog
        open={bulkDelete}
        onOpenChange={setBulkDelete}
        title="批量删除环境"
        description={`将永久删除选中的 ${selected.size} 个环境及其全部数据，无法撤销。`}
        confirmLabel="批量删除"
        requiredText="DELETE"
        onConfirm={async () => {
          try {
            const count = await ipc.envBulkDelete(Array.from(selected), true);
            setSelected(new Set());
            setBulkDelete(false);
            toast.success('批量删除完成', `${count} 个环境`);
            await refresh(false);
            await loadStats();
          } catch (error) {
            toastError(error);
          }
        }}
      />
      <ConfirmDialog
        open={recycleConfirm}
        onOpenChange={setRecycleConfirm}
        title="强制回收全部浏览器"
        description={`将强制结束当前 ${running.length} 个浏览器进程（任务树），未保存的页面状态会丢失。`}
        confirmLabel="全部回收"
        busy={recycleBusy}
        onConfirm={async () => {
          setRecycleBusy(true);
          try {
            const count = await ipc.browserForceRecycleAll();
            toast.success('已回收浏览器进程', `${count} 个实例`);
            setRecycleConfirm(false);
            await syncRuntimes();
            await loadStats();
          } catch (error) {
            toastError(error);
          } finally {
            setRecycleBusy(false);
          }
        }}
      />
      <Dialog
        open={!!screenshot}
        onOpenChange={(value) => !value && setScreenshot(null)}
        title="环境截图"
        description={screenshot ? `${screenshot.width} × ${screenshot.height} · ${formatDateShort(screenshot.capturedAt)}` : undefined}
        width="max-w-4xl"
        footer={
          <>
            <Button variant="secondary" onClick={() => setScreenshot(null)}>
              关闭
            </Button>
            {screenshot ? (
              <Button
                variant="primary"
                onClick={() => {
                  const link = document.createElement('a');
                  link.href = screenshot.dataUrl;
                  link.download = `finenv-${screenshot.envId}.png`;
                  link.click();
                }}
              >
                <Download className="h-4 w-4" /> 下载 PNG
              </Button>
            ) : null}
          </>
        }
      >
        {screenshot ? (
          <img src={screenshot.dataUrl} alt="环境截图" className="max-h-[65vh] w-full rounded-md border border-border object-contain" />
        ) : null}
        {screenshot ? (
          <p className="mt-2 flex items-center gap-1 text-[11px] text-content-hint">
            <ExternalLink className="h-3 w-3" /> 文件路径：{screenshot.path}
          </p>
        ) : null}
      </Dialog>
    </div>
  );
}
