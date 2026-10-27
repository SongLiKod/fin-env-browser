import * as React from 'react';
import {
  Camera,
  Code2,
  FolderOpen,
  Globe2,
  MonitorPlay,
  RefreshCw,
  RotateCw,
  Square,
  Zap,
} from 'lucide-react';
import { ipc } from '@/ipc/client';
import type { EnvironmentView } from '@/ipc/types';
import { useEnvStore } from '@/store/env';
import { toastError, useToastStore } from '@/store/toast';
import { cn } from '@/utils';
import { Badge, StatusDot } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input, Textarea } from '@/components/ui/input';

type BatchAction =
  | 'open'
  | 'close'
  | 'restart'
  | 'navigate'
  | 'reload'
  | 'execute'
  | 'screenshot';

type RowStatus = 'pending' | 'running' | 'success' | 'failed';

interface ResultRow {
  envId: string;
  name: string;
  status: RowStatus;
  message: string;
  screenshotPath?: string;
}

const ACTIONS: Array<{
  value: BatchAction;
  label: string;
  description: string;
  icon: React.ReactNode;
}> = [
  {
    value: 'open',
    label: '批量打开',
    description: '按并发上限启动选中的环境',
    icon: <MonitorPlay className="h-4 w-4" />,
  },
  {
    value: 'close',
    label: '批量关闭',
    description: '结束浏览器进程，保留 profile',
    icon: <Square className="h-4 w-4" />,
  },
  {
    value: 'restart',
    label: '批量重启',
    description: '先关闭再重新启动环境',
    icon: <RotateCw className="h-4 w-4" />,
  },
  {
    value: 'navigate',
    label: '批量访问',
    description: '让所有选中环境打开同一 URL',
    icon: <Globe2 className="h-4 w-4" />,
  },
  {
    value: 'reload',
    label: '批量刷新',
    description: '刷新当前页面（忽略缓存关闭）',
    icon: <RefreshCw className="h-4 w-4" />,
  },
  {
    value: 'execute',
    label: '批量执行脚本',
    description: '在每个环境执行同一段 JS',
    icon: <Code2 className="h-4 w-4" />,
  },
  {
    value: 'screenshot',
    label: '批量截图',
    description: '保存 PNG 到截图目录',
    icon: <Camera className="h-4 w-4" />,
  },
];

const CONCURRENCY = 3;

function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  return '操作失败';
}

export function BatchOperationsDialog({
  open,
  onOpenChange,
  envs,
  onCompleted,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  envs: EnvironmentView[];
  onCompleted: () => void;
}) {
  const toast = useToastStore();
  const [action, setAction] = React.useState<BatchAction>('open');
  const [url, setUrl] = React.useState('https://');
  const [script, setScript] = React.useState('document.title');
  const [busy, setBusy] = React.useState(false);
  const [results, setResults] = React.useState<ResultRow[]>([]);

  React.useEffect(() => {
    if (!open) return;
    setBusy(false);
    setResults([]);
    setAction('open');
    setUrl('https://');
    setScript('document.title');
  }, [open]);

  const completed = results.filter(
    (row) => row.status === 'success' || row.status === 'failed',
  ).length;
  const succeeded = results.filter((row) => row.status === 'success').length;
  const failed = results.filter((row) => row.status === 'failed').length;
  const selectedAction = ACTIONS.find((item) => item.value === action) ?? ACTIONS[0];

  const runOne = async (env: EnvironmentView) => {
    switch (action) {
      case 'open': {
        const runtime = await ipc.browserOpen(env.id, {});
        useEnvStore.getState().applyRuntime(runtime);
        return `PID ${runtime.pid} · 端口 ${runtime.debugPort}${
          runtime.warnings.length ? ` · ${runtime.warnings.join('；')}` : ''
        }`;
      }
      case 'close': {
        await ipc.browserClose(env.id);
        useEnvStore.getState().clearRuntime(env.id);
        return '已关闭';
      }
      case 'restart': {
        try {
          await ipc.browserClose(env.id);
          useEnvStore.getState().clearRuntime(env.id);
        } catch {
          // The environment may already be stopped; opening still applies.
        }
        const runtime = await ipc.browserOpen(env.id, {});
        useEnvStore.getState().applyRuntime(runtime);
        return `已重启 · PID ${runtime.pid}`;
      }
      case 'navigate': {
        const next = await ipc.browserNavigate(env.id, url.trim());
        return next || url.trim();
      }
      case 'reload': {
        await ipc.browserReload(env.id);
        return '页面已刷新';
      }
      case 'execute': {
        const value = await ipc.browserExecute(env.id, script);
        const text =
          typeof value === 'string' ? value : JSON.stringify(value ?? null);
        return text.length > 160 ? `${text.slice(0, 160)}…` : text;
      }
      case 'screenshot': {
        const shot = await ipc.browserScreenshot(env.id);
        return shot.path;
      }
      default:
        return '完成';
    }
  };

  const run = async () => {
    if (envs.length === 0) {
      toast.warning('请先选择环境');
      return;
    }
    if (action === 'navigate' && !/^https?:\/\//i.test(url.trim())) {
      toast.warning('请输入 http:// 或 https:// 地址');
      return;
    }
    if (action === 'execute' && !script.trim()) {
      toast.warning('请输入要执行的脚本');
      return;
    }
    const rows: ResultRow[] = envs.map((env) => ({
      envId: env.id,
      name: env.name,
      status: 'pending',
      message: '等待执行',
    }));
    setResults(rows);
    setBusy(true);
    const commit = (index: number, patch: Partial<ResultRow>) => {
      rows[index] = { ...rows[index], ...patch };
      setResults([...rows]);
    };
    let cursor = 0;
    const worker = async () => {
      while (cursor < envs.length) {
        const index = cursor++;
        const env = envs[index];
        commit(index, { status: 'running', message: '执行中…' });
        try {
          const message = await runOne(env);
          commit(index, {
            status: 'success',
            message,
            screenshotPath: action === 'screenshot' ? message : undefined,
          });
        } catch (error) {
          commit(index, { status: 'failed', message: errorMessage(error) });
        }
      }
    };
    await Promise.all(
      Array.from({ length: Math.min(CONCURRENCY, envs.length) }, () => worker()),
    );
    setBusy(false);
    onCompleted();
    const ok = rows.filter((row) => row.status === 'success').length;
    const bad = rows.filter((row) => row.status === 'failed').length;
    if (bad === 0) {
      toast.success('批量操作完成', `${ok} / ${rows.length} 个环境成功`);
    } else {
      toast.warning('批量操作完成', `${ok} 个成功，${bad} 个失败`);
    }
  };

  const screenshots = results.filter((row) => row.status === 'success' && row.screenshotPath);

  return (
    <Dialog
      open={open}
      onOpenChange={(value) => {
        if (busy && !value) return;
        onOpenChange(value);
      }}
      title="批量操作"
      description="对当前选中的环境执行一次性命令，最多同时处理 3 个环境；需要持续同步鼠标和键盘请使用「同步控制」。"
      width="max-w-3xl"
      footer={
        <>
          <span className="mr-auto text-[11px] text-content-hint">
            {envs.length} 个环境已选中
            {results.length ? ` · ${completed}/${results.length} 已执行` : ''}
          </span>
          {screenshots.length > 0 ? (
            <Button
              variant="secondary"
              onClick={() => {
                void ipc.openArtifactDir('screenshots').catch(toastError);
              }}
            >
              <FolderOpen className="h-4 w-4" /> 截图目录
            </Button>
          ) : null}
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            关闭
          </Button>
          <Button
            variant="primary"
            loading={busy}
            disabled={envs.length === 0}
            onClick={() => void run()}
          >
            <Zap className="h-4 w-4" /> 执行 {selectedAction.label}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
          {ACTIONS.map((item) => (
            <button
              key={item.value}
              type="button"
              disabled={busy}
              onClick={() => setAction(item.value)}
              className={cn(
                'rounded-lg border p-2.5 text-left transition-colors disabled:opacity-60',
                action === item.value
                  ? 'border-primary bg-primary-light text-primary'
                  : 'border-border bg-surface-base text-content-main hover:bg-surface-secondary',
              )}
            >
              <span className="flex items-center gap-1.5 text-xs font-medium">
                {item.icon}
                {item.label}
              </span>
              <span className="mt-1 block text-[10px] leading-4 text-content-hint">
                {item.description}
              </span>
            </button>
          ))}
        </div>

        {action === 'navigate' ? (
          <Field label="目标地址" hint="所有选中的环境都会导航到该地址">
            <Input
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="https://example.com"
              disabled={busy}
            />
          </Field>
        ) : null}

        {action === 'execute' ? (
          <Field label="JavaScript" hint="在执行环境的页面上下文中运行，返回值会显示在结果中">
            <Textarea
              value={script}
              onChange={(event) => setScript(event.target.value)}
              className="font-mono text-xs"
              disabled={busy}
            />
          </Field>
        ) : null}

        <div className="flex flex-wrap gap-1.5">
          {envs.slice(0, 12).map((env) => (
            <Badge key={env.id} tone={env.runtime ? 'success' : 'neutral'}>
              {env.name}
            </Badge>
          ))}
          {envs.length > 12 ? (
            <span className="self-center text-[11px] text-content-hint">
              +{envs.length - 12} 个
            </span>
          ) : null}
        </div>

        {results.length > 0 ? (
          <div className="max-h-[280px] space-y-1 overflow-y-auto rounded-lg border border-border">
            {results.map((row, index) => (
              <div
                key={`${row.envId}-${index}`}
                className="flex items-start gap-2 border-b border-border px-3 py-2 text-xs last:border-b-0"
              >
                <StatusDot
                  tone={
                    row.status === 'success'
                      ? 'success'
                      : row.status === 'failed'
                        ? 'danger'
                        : row.status === 'running'
                          ? 'primary'
                          : 'muted'
                  }
                  pulse={row.status === 'running'}
                  className="mt-1.5"
                />
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium text-content-main">{row.name}</p>
                  <p
                    className={cn(
                      'mt-0.5 break-all text-[11px]',
                      row.status === 'failed' ? 'text-danger' : 'text-content-hint',
                    )}
                  >
                    {row.message}
                  </p>
                </div>
                <span className="shrink-0 text-[10px] text-content-hint">
                  {row.status === 'success'
                    ? '成功'
                    : row.status === 'failed'
                      ? '失败'
                      : row.status === 'running'
                        ? '执行中'
                        : '等待'}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <div className="rounded-lg border border-dashed border-border px-3 py-8 text-center text-xs text-content-hint">
            选择动作后点击「执行」，每个环境的执行结果会显示在这里。
            {selectedAction.value === 'screenshot' ? ' 截图会保存到截图目录。' : ''}
          </div>
        )}

        {results.length > 0 && !busy ? (
          <p className="text-[11px] text-content-hint">
            成功 {succeeded} 个 · 失败 {failed} 个
            {failed > 0 ? ' · 请根据失败原因检查环境是否运行、URL 是否可访问或脚本是否合法。' : ''}
          </p>
        ) : null}
      </div>
    </Dialog>
  );
}
