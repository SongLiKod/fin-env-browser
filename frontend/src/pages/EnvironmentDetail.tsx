import * as React from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  Camera,
  Copy,
  Download,
  Fingerprint,
  Globe2,
  KeyRound,
  LayoutTemplate,
  MonitorPlay,
  MonitorUp,
  Network,
  Radar,
  RefreshCw,
  ScrollText,
  Settings2,
  ShieldCheck,
  Square,
  Trash2,
  Users,
} from 'lucide-react';
import type {
  EnvironmentView,
  Group,
  Proxy,
} from '@/ipc/types';
import { ipc, onBrowserExit, onBrowserRuntime } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { useEnvStore } from '@/store/env';
import { toastError, useToastStore } from '@/store/toast';
import { cn, envStatusMeta, formatDate, relativeTime, testEnvLabels } from '@/utils';
import { envBorderColor, envBorderWidth, envAccessLabel, envHasAccess, envIpSummary, hexToRgba } from '@/utils/environment';
import { Badge, StatusDot } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, EmptyState, Spinner } from '@/components/ui/card';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { Tabs } from '@/components/ui/tabs';
import { Tooltip } from '@/components/ui/tooltip';
import {
  CloneEnvironmentDialog,
  ExportEnvironmentsDialog,
} from '@/components/env/EnvironmentDialogs';
import { OverviewPanel } from '@/components/env/OverviewPanel';
import { SaveTemplateDialog } from '@/components/env/TemplateManagerDialog';
import { FingerprintEditor } from '@/components/env/FingerprintEditor';
import { AccountsPanel } from '@/components/env/AccountsPanel';
import { SnapshotsPanel } from '@/components/env/SnapshotsPanel';
import { NetworkingPanel } from '@/components/env/NetworkingPanel';
import { ExtensionsPanel } from '@/components/env/ExtensionsPanel';
import { DetectionPanel } from '@/components/env/DetectionPanel';
import { DebugPanel } from '@/components/env/DebugPanel';
import { AccessPanel } from '@/components/env/AccessPanel';

const tabIds = ['overview', 'fingerprint', 'accounts', 'snapshots', 'network', 'extensions', 'access', 'detect', 'debug'] as const;
type TabId = (typeof tabIds)[number];

export function EnvironmentDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const toast = useToastStore();
  const can = useAuthStore((state) => state.can);
  const [env, setEnv] = React.useState<EnvironmentView | null>(null);
  const [groups, setGroups] = React.useState<Group[]>([]);
  const [proxies, setProxies] = React.useState<Proxy[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);
  const [tab, setTab] = React.useState<TabId>('overview');
  const [busy, setBusy] = React.useState(false);
  const [shortcutBusy, setShortcutBusy] = React.useState(false);
  const [ipChecking, setIpChecking] = React.useState(false);
  const [detecting, setDetecting] = React.useState(false);
  const [showClone, setShowClone] = React.useState(false);
  const [showTemplate, setShowTemplate] = React.useState(false);
  const [showExport, setShowExport] = React.useState(false);
  const [showDelete, setShowDelete] = React.useState(false);
  const ipAutoChecked = React.useRef<string | null>(null);

  const applyEnv = React.useCallback((next: EnvironmentView) => {
    setEnv((current) => {
      if (!current) return next;
      return { ...next, runtime: next.runtime ?? current.runtime };
    });
    useEnvStore.getState().replace(next);
  }, []);

  const refresh = React.useCallback(async () => {
    if (!id) return;
    const next = await ipc.envGet(id);
    applyEnv(next);
  }, [id, applyEnv]);

  const refreshIp = React.useCallback(async (silent = false) => {
    if (!id) return;
    setIpChecking(true);
    try {
      await ipc.envCheckIp(id);
      const next = await ipc.envGet(id);
      applyEnv(next);
    } catch (error) {
      if (!silent) toastError(error);
    } finally {
      setIpChecking(false);
    }
  }, [id, applyEnv]);

  React.useEffect(() => {
    if (!id) return;
    let active = true;
    setLoading(true);
    void Promise.all([ipc.envGet(id), ipc.groupList(), ipc.proxyList(), ipc.envList()])
      .then(([environment, groupList, proxyList, envList]) => {
        if (!active) return;
        setEnv(environment);
        setGroups(groupList);
        setProxies(proxyList);
        const store = useEnvStore.getState();
        envList.forEach((item) => store.replace(item));
        setError(null);
      })
      .catch((err) => {
        if (!active) return;
        setError(err instanceof Error ? err.message : '环境加载失败');
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [id]);

  React.useEffect(() => {
    if (!env || ipAutoChecked.current === env.id) return;
    ipAutoChecked.current = env.id;
    const stale =
      !env.ipCheckedAt ||
      Number.isNaN(Date.parse(env.ipCheckedAt)) ||
      Date.now() - Date.parse(env.ipCheckedAt) > 30 * 60 * 1000;
    if (stale) void refreshIp(true);
  }, [env, refreshIp]);

  React.useEffect(() => {
    if (!id) return;
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const register = (promise: Promise<() => void>) =>
      promise
        .then((unlisten) => {
          if (disposed) unlisten();
          else unlisteners.push(unlisten);
        })
        .catch(() => undefined);
    register(
      onBrowserRuntime((runtime) => {
        if (runtime.envId === id) {
          setEnv((current) => (current ? { ...current, runtime } : current));
        }
      }),
    );
    register(
      onBrowserExit((event) => {
        if (event.envId === id) {
          setEnv((current) => (current ? { ...current, runtime: null } : current));
        }
      }),
    );
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [id]);

  const open = async () => {
    if (!env) return;
    setBusy(true);
    try {
      const runtime = await ipc.browserOpen(env.id, {});
      setEnv((current) => (current ? { ...current, runtime } : current));
      toast.success('环境已启动', `PID ${runtime.pid} · 端口 ${runtime.debugPort}`);
      if (runtime.warnings.length) toast.warning('启动告警', runtime.warnings.join('；'));
      void refreshIp(true);
      await refresh();
    } catch (err) {
      toastError(err);
    } finally {
      setBusy(false);
    }
  };

  const close = async () => {
    if (!env) return;
    setBusy(true);
    try {
      await ipc.browserClose(env.id);
      setEnv((current) => (current ? { ...current, runtime: null } : current));
      toast.success('环境已关闭');
    } catch (err) {
      toastError(err);
    } finally {
      setBusy(false);
    }
  };

  const createShortcut = async () => {
    if (!env) return;
    setShortcutBusy(true);
    try {
      const path = await ipc.envCreateShortcut(env.id);
      toast.success('桌面快捷方式已创建', path);
    } catch (error) {
      toastError(error);
    } finally {
      setShortcutBusy(false);
    }
  };

  const detect = async () => {
    if (!env) return;
    setDetecting(true);
    try {
      const report = await ipc.fingerprintDetect(env.id);
      toast.success('指纹检测完成', `评分 ${report.score}/100`);
      setTab('detect');
      await refresh();
    } catch (err) {
      toastError(err);
    } finally {
      setDetecting(false);
    }
  };

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-xs text-content-weak">
        <Spinner /> 加载环境详情…
      </div>
    );
  }

  if (error || !env) {
    return (
      <div className="page-pad">
        <Card>
          <EmptyState
            title="无法打开环境"
            description={error ?? '环境不存在或已被删除'}
            action={<Button variant="primary" onClick={() => navigate('/')}>返回环境列表</Button>}
          />
        </Card>
      </div>
    );
  }

  const status = envStatusMeta[env.status];
  const running = !!env.runtime;
  const accent = envBorderColor(env);
  const accentWidth = envBorderWidth(env);
  const canOperateEnv = envHasAccess(env, 'operate');
  const canManageEnv = envHasAccess(env, 'manage');
  const tabItems = [
    { value: 'overview', label: '概览' },
    ...(canManageEnv && can('edit_fingerprint_proxy')
      ? [{ value: 'fingerprint', label: '指纹配置' }]
      : []),
    { value: 'accounts', label: '账号台账', badge: env.accountCount || undefined },
    ...(canManageEnv && can('edit_fingerprint_proxy')
      ? [{ value: 'snapshots', label: '指纹快照', badge: env.snapshotCount || undefined }]
      : []),
    ...(canManageEnv && can('edit_fingerprint_proxy')
      ? [{ value: 'network', label: '网络与 Mock' }]
      : []),
    ...(canManageEnv && can('edit_env') ? [{ value: 'extensions', label: '扩展' }] : []),
    ...(canManageEnv && can('manage_env_members')
      ? [{ value: 'access', label: '访问成员' }]
      : []),
    ...(canOperateEnv && can('open_close') ? [{ value: 'detect', label: '检测报告' }] : []),
    ...(canOperateEnv && can('open_close')
      ? [{ value: 'debug', label: '调试与自动化' }]
      : []),
  ];

  return (
    <div className="page-pad space-y-4">
      <div
        className="flex flex-wrap items-start justify-between gap-3 rounded-lg border border-border bg-surface-base p-4"
        style={{ borderLeft: `${accentWidth}px solid ${accent}`, backgroundColor: hexToRgba(accent, 0.025) }}
      >
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="truncate text-xl font-semibold tracking-tight text-content-main">{env.name}</h1>
            {running ? (
              <Badge tone="success">
                <StatusDot tone="success" pulse /> 运行中
              </Badge>
            ) : (
              <Badge tone={status.tone === 'success' ? 'primary' : status.tone === 'warning' ? 'warning' : 'neutral'}>
                {status.label}
              </Badge>
            )}
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
            {env.testEnv ? <Badge tone="outline">{testEnvLabels[env.testEnv] ?? env.testEnv}</Badge> : null}
            {env.tags.map((tag) => <Badge key={tag} tone="neutral">#{tag}</Badge>)}
          </div>
          <p className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-content-weak">
            <span>{env.project || '未指定项目'}</span>
            <span>·</span>
            <span>{env.accountRole || '未指定角色'}</span>
            {env.caseId ? (
              <>
                <span>·</span>
                <span className="font-mono">{env.caseId}</span>
              </>
            ) : null}
            <span>·</span>
            <span>{env.proxy?.name ?? '直连'}</span>
            <span>·</span>
            <span className="font-mono">{env.ipAddress ?? 'IP 待检测'}</span>
            <span>·</span>
            <span>{env.accountCount} 个账号</span>
            <span>·</span>
            <span>最近打开 {relativeTime(env.lastOpenedAt)}</span>
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Tooltip content="刷新详情">
            <Button variant="ghost" size="icon" onClick={() => void refresh()}>
              <RefreshCw className="h-4 w-4" />
            </Button>
          </Tooltip>
          {can('open_close') && canOperateEnv ? (
            running ? (
              <Button variant="secondary" loading={busy} onClick={() => void close()}>
                <Square className="h-4 w-4" /> 关闭
              </Button>
            ) : (
              <Button variant="secondary" loading={busy} onClick={() => void open()}>
                <MonitorPlay className="h-4 w-4" /> 打开环境
              </Button>
            )
          ) : null}
          {can('open_close') && canOperateEnv ? (
            <Button variant="secondary" onClick={() => void detect()} loading={detecting}>
              <Radar className="h-4 w-4" /> 检测指纹
            </Button>
          ) : null}
          {can('open_close') && canOperateEnv ? (
            <Button variant="secondary" loading={shortcutBusy} onClick={() => void createShortcut()}>
              <MonitorUp className="h-4 w-4" /> 桌面快捷方式
            </Button>
          ) : null}
          {can('export_sensitive') ? (
            <Button variant="secondary" onClick={() => setShowExport(true)}>
              <Download className="h-4 w-4" /> 导出包
            </Button>
          ) : null}
          {can('create_env') && canOperateEnv ? (
            <Button variant="secondary" onClick={() => setShowTemplate(true)}>
              <LayoutTemplate className="h-4 w-4" /> 保存为模板
            </Button>
          ) : null}
          {can('clone_env') && canOperateEnv ? (
            <Button variant="secondary" onClick={() => setShowClone(true)}>
              <Copy className="h-4 w-4" /> 克隆
            </Button>
          ) : null}
          {can('delete_env') && canManageEnv ? (
            <Button variant="ghost" size="icon" className="text-danger" onClick={() => setShowDelete(true)}>
              <Trash2 className="h-4 w-4" />
            </Button>
          ) : null}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-5">
        <Card className="p-3">
          <p className="flex items-center gap-1 text-[10px] text-content-hint"><Users className="h-3 w-3" /> 账号台账</p>
          <p className="mt-1 text-lg font-semibold text-content-main">{env.accountCount}</p>
        </Card>
        <Card className="p-3">
          <p className="flex items-center gap-1 text-[10px] text-content-hint"><Camera className="h-3 w-3" /> 指纹快照</p>
          <p className="mt-1 text-lg font-semibold text-content-main">{env.snapshotCount}</p>
        </Card>
        <Card className="p-3">
          <p className="flex items-center gap-1 text-[10px] text-content-hint"><Fingerprint className="h-3 w-3" /> 指纹平台</p>
          <p className="mt-1 truncate text-xs font-medium text-content-main">{env.fingerprint.clientHints.platform} · {env.fingerprint.timezone}</p>
          <p className="truncate text-[10px] text-content-hint">{env.fingerprint.screen.width}×{env.fingerprint.screen.height} · DPR {env.fingerprint.screen.pixelRatio}</p>
        </Card>
        <Card className="p-3">
          <div className="flex items-center justify-between">
            <p className="flex items-center gap-1 text-[10px] text-content-hint"><Globe2 className="h-3 w-3" /> 出口 IP</p>
            <button
              type="button"
              title="刷新出口 IP"
              className="text-content-hint hover:text-primary"
              onClick={() => void refreshIp()}
            >
              <RefreshCw className={cn('h-3 w-3', ipChecking && 'animate-spin')} />
            </button>
          </div>
          <p className="mt-1 truncate font-mono text-xs font-medium text-content-main" title={envIpSummary(env)}>
            {env.ipAddress ?? (ipChecking ? '检测中…' : '未检测')}
          </p>
          <p className="truncate text-[10px] text-content-hint">
            {[env.ipCountry ?? env.ipCountryCode, env.ipCity].filter(Boolean).join(' · ') || (env.ipSource === 'proxy' ? '经代理' : env.ipSource === 'direct' ? '直连' : '点击刷新检测')}
            {env.ipCheckedAt ? ` · ${relativeTime(env.ipCheckedAt)}` : ''}
          </p>
        </Card>
        <Card className="p-3">
          <p className="flex items-center gap-1 text-[10px] text-content-hint"><ShieldCheck className="h-3 w-3" /> 运行状态</p>
          <p className="mt-1 truncate text-xs font-medium text-content-main">{running ? `PID ${env.runtime?.pid}` : '未运行'}</p>
          <p className="truncate text-[10px] text-content-hint">{running ? `${env.runtime?.memoryMb} MB · CPU ${env.runtime?.cpuPercent.toFixed(1)}%` : `更新于 ${formatDate(env.updatedAt)}`}</p>
        </Card>
      </div>

      <Tabs
        value={tab}
        onValueChange={(value) => setTab(value as TabId)}
        items={tabItems}
      />

      <div>
        {tab === 'overview' ? <OverviewPanel env={env} groups={groups} onUpdated={applyEnv} /> : null}
        {tab === 'fingerprint' && canManageEnv && can('edit_fingerprint_proxy') ? (
          <FingerprintEditor env={env} onSaved={applyEnv} />
        ) : null}
        {tab === 'accounts' ? (
          <AccountsPanel
            env={env}
            onCountChanged={(count) =>
              setEnv((current) => (current ? { ...current, accountCount: count } : current))
            }
          />
        ) : null}
        {tab === 'snapshots' && canManageEnv && can('edit_fingerprint_proxy') ? (
          <SnapshotsPanel env={env} onUpdated={applyEnv} />
        ) : null}
        {tab === 'network' && canManageEnv && can('edit_fingerprint_proxy') ? (
          <NetworkingPanel env={env} proxies={proxies} onUpdated={applyEnv} />
        ) : null}
        {tab === 'extensions' && canManageEnv && can('edit_env') ? (
          <ExtensionsPanel env={env} />
        ) : null}
        {tab === 'access' && canManageEnv && can('manage_env_members') ? (
          <AccessPanel env={env} />
        ) : null}
        {tab === 'detect' ? <DetectionPanel env={env} /> : null}
        {tab === 'debug' ? <DebugPanel env={env} onRefresh={refresh} /> : null}
      </div>

      <div className="flex flex-wrap items-center gap-3 border-t border-border pt-3 text-[10px] text-content-hint">
        <span className="inline-flex items-center gap-1"><Network className="h-3 w-3" /> 独立进程 / profile / 调试端口</span>
        <span className="inline-flex items-center gap-1"><KeyRound className="h-3 w-3" /> 账号密码与 Cookie 密文存储</span>
        <span className="inline-flex items-center gap-1"><ScrollText className="h-3 w-3" /> 所有高危操作写审计</span>
        <span className="inline-flex items-center gap-1"><Settings2 className="h-3 w-3" /> 主题仅作用于面板</span>
      </div>

      <CloneEnvironmentDialog
        open={showClone}
        onOpenChange={setShowClone}
        env={env}
        onCloned={(cloned) => {
          useEnvStore.getState().replace(cloned);
          setShowClone(false);
          navigate(`/envs/${cloned.id}`);
        }}
      />
      <SaveTemplateDialog
        open={showTemplate}
        onOpenChange={setShowTemplate}
        env={env}
        onSaved={() => undefined}
      />
      <ExportEnvironmentsDialog open={showExport} onOpenChange={setShowExport} envs={[env]} />
      <ConfirmDialog
        open={showDelete}
        onOpenChange={setShowDelete}
        title="删除环境"
        description={`将永久删除「${env.name}」及其 profile、账号、快照、Hosts/Mock 数据。审计日志保留。`}
        confirmLabel="删除环境"
        requiredText="DELETE"
        onConfirm={async () => {
          try {
            await ipc.envDelete(env.id, true);
            useEnvStore.getState().remove(env.id);
            toast.success('环境已删除');
            navigate('/');
          } catch (err) {
            toastError(err);
          }
        }}
      />
    </div>
  );
}
