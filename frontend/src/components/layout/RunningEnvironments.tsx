import * as React from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { ArrowUpRight, MonitorPlay, RefreshCw, Square } from 'lucide-react';
import { ipc } from '@/ipc/client';
import type { EnvironmentView } from '@/ipc/types';
import { useEnvStore } from '@/store/env';
import { toastError } from '@/store/toast';
import { cn } from '@/utils';
import { envBorderColor, envBorderWidth, envIpSummary, hexToRgba } from '@/utils/environment';

export function RunningEnvironments() {
  const navigate = useNavigate();
  const envs = useEnvStore((state) => state.envs);
  const replace = useEnvStore((state) => state.replace);
  const clearRuntime = useEnvStore((state) => state.clearRuntime);
  const [checking, setChecking] = React.useState<Set<string>>(new Set());
  const [closing, setClosing] = React.useState<Set<string>>(new Set());

  const running = envs.filter((env) => env.runtime);

  React.useEffect(() => {
    let active = true;
    const load = async () => {
      try {
        const current = await ipc.envList({ runningOnly: true });
        if (!active) return;
        current.forEach((env) => replace(env));
      } catch {
        // Best effort; the runtime event stream is the primary source.
      }
    };
    void load();
    const timer = window.setInterval(() => void load(), 5000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [replace]);

  if (running.length === 0) return null;

  const checkIp = async (env: EnvironmentView) => {
    setChecking((current) => new Set(current).add(env.id));
    try {
      await ipc.envCheckIp(env.id);
      replace(await ipc.envGet(env.id));
    } catch (error) {
      toastError(error);
    } finally {
      setChecking((current) => {
        const next = new Set(current);
        next.delete(env.id);
        return next;
      });
    }
  };

  const close = async (env: EnvironmentView) => {
    setClosing((current) => new Set(current).add(env.id));
    try {
      await ipc.browserClose(env.id);
      clearRuntime(env.id);
    } catch (error) {
      toastError(error);
    } finally {
      setClosing((current) => {
        const next = new Set(current);
        next.delete(env.id);
        return next;
      });
    }
  };

  return (
    <div className="flex shrink-0 items-center gap-3 border-b border-border bg-surface-base/80 px-4 py-2 backdrop-blur">
      <span className="hidden shrink-0 items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wide text-content-hint sm:flex">
        <MonitorPlay className="h-3.5 w-3.5 text-[var(--success)]" />
        运行中
      </span>
      <div className="flex min-w-0 flex-1 items-center gap-2 overflow-x-auto pb-0.5 [scrollbar-width:thin]">
        {running.map((env) => {
          const color = envBorderColor(env);
          const width = envBorderWidth(env);
          const isChecking = checking.has(env.id);
          const isClosing = closing.has(env.id);
          return (
            <div
              key={env.id}
              className="flex shrink-0 items-center gap-2 rounded-md border border-border px-2.5 py-1 shadow-sm"
              style={{
                borderLeftColor: color,
                borderLeftWidth: width,
                backgroundColor: hexToRgba(color, 0.06),
              }}
            >
              <span className="h-1.5 w-1.5 shrink-0 rounded-full" style={{ backgroundColor: color }} />
              <button
                type="button"
                className="max-w-[150px] truncate text-[11px] font-medium text-content-main hover:text-primary"
                title={env.name}
                onClick={() => navigate(`/envs/${env.id}`)}
              >
                {env.name}
              </button>
              {env.project || env.testEnv ? (
                <span className="hidden max-w-[100px] truncate text-[10px] text-content-hint md:inline">
                  {env.project || env.testEnv}
                </span>
              ) : null}
              <span className="inline-flex items-center gap-1 rounded bg-surface-base/70 px-1.5 py-0.5 font-mono text-[10px] text-content-weak">
                {env.ipAddress ? envIpSummary(env) : 'IP 检测中…'}
              </span>
              <span className="hidden text-[10px] tabular-nums text-content-hint lg:inline">PID {env.runtime?.pid}</span>
              <button
                type="button"
                className={cn('text-content-hint hover:text-primary', isChecking && 'pointer-events-none')}
                title="重新检测出口 IP"
                onClick={() => void checkIp(env)}
              >
                <RefreshCw className={cn('h-3 w-3', isChecking && 'animate-spin')} />
              </button>
              <Link
                to={`/envs/${env.id}`}
                className="text-content-hint hover:text-primary"
                title="打开环境详情"
              >
                <ArrowUpRight className="h-3 w-3" />
              </Link>
              <button
                type="button"
                className={cn('text-content-hint hover:text-danger', isClosing && 'pointer-events-none')}
                title="关闭浏览器"
                onClick={() => void close(env)}
              >
                <Square className="h-3 w-3" />
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
