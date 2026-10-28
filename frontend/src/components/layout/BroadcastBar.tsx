import * as React from 'react';
import { MousePointerClick, Pause, Play, Radio, Square } from 'lucide-react';
import { ipc, onBroadcastStatus } from '@/ipc/client';
import type { BroadcastStatus } from '@/ipc/types';
import { useEnvStore } from '@/store/env';
import { toastError, useToastStore } from '@/store/toast';
import { cn } from '@/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

export function BroadcastBar() {
  const toast = useToastStore();
  const envs = useEnvStore((state) => state.envs);
  const syncRuntimes = useEnvStore((state) => state.syncRuntimes);
  const [status, setStatus] = React.useState<BroadcastStatus | null>(null);
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    let active = true;
    const load = async () => {
      try {
        const next = await ipc.broadcastStatus();
        if (active) setStatus(next.active ? next : null);
      } catch {
        // Viewer roles or a transient restart; keep the bar hidden.
      }
    };
    void load();
    const timer = window.setInterval(() => void load(), 3000);
    let unlisten: (() => void) | undefined;
    void onBroadcastStatus((next) => {
      if (active) setStatus(next.active ? next : null);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      active = false;
      window.clearInterval(timer);
      unlisten?.();
    };
  }, []);

  if (!status) return null;
  const master = envs.find((env) => env.id === status.masterEnvId);

  const togglePause = async () => {
    setBusy(true);
    try {
      const next = await ipc.broadcastPause(!status.paused);
      setStatus(next.active ? next : null);
      toast.success(next.paused ? '同步控制已暂停' : '同步控制已恢复');
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    try {
      const next = await ipc.broadcastStop('顶部控制栏停止');
      setStatus(next.active ? next : null);
      toast.success('同步控制已停止');
      await syncRuntimes();
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-primary/30 bg-[color-mix(in_srgb,var(--color-primary)_7%,var(--bg-base))] px-4 py-2 text-xs">
      <span className="flex items-center gap-1.5 font-semibold text-primary">
        <span className="relative flex h-2 w-2">
          {!status.paused ? (
            <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary opacity-60" />
          ) : null}
          <span className="relative inline-flex h-2 w-2 rounded-full bg-primary" />
        </span>
        <Radio className="h-3.5 w-3.5" />
        {status.paused ? '同步控制已暂停' : '同步控制中'}
      </span>
      <span className="flex items-center gap-1 text-content-weak">
        <MousePointerClick className="h-3.5 w-3.5" />
        主控
        <strong className="font-medium text-content-main">
          {master?.name ?? status.masterEnvId?.slice(0, 8) ?? '—'}
        </strong>
      </span>
      <Badge tone="primary">{status.envIds.length} 个窗口</Badge>
      <span className="text-content-hint">已同步 {status.forwarded} 次输入</span>
      {status.errors.length > 0 ? (
        <Badge tone="warning" title={status.errors.join('\n')}>
          {status.errors.length} 个环境未加入
        </Badge>
      ) : null}
      <span className="hidden text-content-hint lg:inline">
        在「{master?.name ?? '主控'}」窗口中点击/输入/新建 Tab，其他窗口会同步操作
      </span>
      <div className="ml-auto flex items-center gap-1">
        <Button
          variant="ghost"
          size="sm"
          loading={busy}
          onClick={() => void togglePause()}
        >
          {status.paused ? <Play className="h-3.5 w-3.5" /> : <Pause className="h-3.5 w-3.5" />}
          {status.paused ? '恢复' : '暂停'}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className={cn('text-danger', busy && 'opacity-60')}
          loading={busy}
          onClick={() => void stop()}
        >
          <Square className="h-3.5 w-3.5" /> 停止同步
        </Button>
      </div>
    </div>
  );
}
