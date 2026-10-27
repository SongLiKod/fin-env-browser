import * as React from 'react';
import { Link } from 'react-router-dom';
import {
  CheckCheck,
  RefreshCw,
  ScanLine,
  ShieldQuestion,
  Camera,
  Clock3,
  ExternalLink,
} from 'lucide-react';
import type { CaptchaEvent } from '@/ipc/types';
import { ipc, onCaptcha } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { useEnvStore } from '@/store/env';
import { toastError, useToastStore } from '@/store/toast';
import { formatDateShort } from '@/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, EmptyState, Spinner, StatCard } from '@/components/ui/card';
import { Select } from '@/components/ui/input';

export function CaptchaPage() {
  const toast = useToastStore();
  const canOpen = useAuthStore((state) => state.can('open_close'));
  const envs = useEnvStore((state) => state.envs);
  const [events, setEvents] = React.useState<CaptchaEvent[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [envFilter, setEnvFilter] = React.useState('');
  const [showHandled, setShowHandled] = React.useState(false);
  const [captureEnv, setCaptureEnv] = React.useState('');
  const [capturing, setCapturing] = React.useState(false);
  const [updating, setUpdating] = React.useState<string | null>(null);
  const [previews, setPreviews] = React.useState<Record<string, string>>({});

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const next = await ipc.captchaList(envFilter || undefined);
      setEvents(next);
      if (useEnvStore.getState().envs.length === 0) await useEnvStore.getState().refresh(false);
      const missing = next.filter((event) => event.screenshotPath && !previews[event.id]);
      const loaded = await Promise.all(
        missing.map(async (event) => {
          try {
            const data = await ipc.readArtifact(event.screenshotPath as string);
            return [event.id, data] as const;
          } catch {
            return null;
          }
        }),
      );
      setPreviews((current) => {
        const merged = { ...current };
        loaded.forEach((item) => {
          if (item) merged[item[0]] = item[1];
        });
        return merged;
      });
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, [envFilter]);

  React.useEffect(() => {
    void load();
  }, [load]);

  React.useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    onCaptcha(() => {
      void load();
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [load]);

  const filtered = events.filter((event) => (showHandled ? true : !event.handled));
  const unhandled = events.filter((event) => !event.handled).length;
  const running = envs.filter((env) => env.runtime);

  const toggle = async (event: CaptchaEvent) => {
    setUpdating(event.id);
    try {
      await ipc.captchaHandle(event.id, !event.handled);
      setEvents((current) =>
        current.map((item) => (item.id === event.id ? { ...item, handled: !item.handled } : item)),
      );
    } catch (error) {
      toastError(error);
    } finally {
      setUpdating(null);
    }
  };

  const capture = async () => {
    const target = captureEnv || running[0]?.id;
    if (!target) {
      toast.warning('请先打开一个环境');
      return;
    }
    setCapturing(true);
    try {
      await ipc.captchaCapture(target);
      toast.success('已采集当前环境画面');
      await load();
    } catch (error) {
      toastError(error);
    } finally {
      setCapturing(false);
    }
  };

  return (
    <div className="page-pad space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-[10px] font-semibold uppercase tracking-widest text-content-hint">Testing / Captcha Aggregation</p>
          <h1 className="mt-1 text-xl font-semibold tracking-tight text-content-main">验证码聚合</h1>
          <p className="mt-1 max-w-2xl text-xs leading-5 text-content-weak">
            自动汇总所有运行环境的疑似验证码弹窗与手动采集画面，减少窗口切换；判定规则在页面内注入，不读取账号密码。
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Select className="w-[180px]" value={captureEnv} onChange={(event) => setCaptureEnv(event.target.value)} placeholder={running.length ? '选择运行中的环境' : '暂无运行环境'}>
            {running.map((env) => (
              <option key={env.id} value={env.id}>{env.name}</option>
            ))}
          </Select>
          <Button variant="secondary" loading={capturing} disabled={!canOpen || running.length === 0} onClick={() => void capture()}>
            <Camera className="h-4 w-4" /> 采集当前画面
          </Button>
          <Button variant="secondary" onClick={() => void load()}>
            <RefreshCw className="h-4 w-4" /> 刷新
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatCard label="待处理验证码" value={unhandled} hint="未标记处理的事件" icon={<ShieldQuestion className="h-4 w-4" />} tone="warning" />
        <StatCard label="全部事件" value={events.length} hint={`${events.filter((e) => e.handled).length} 条已处理`} icon={<ScanLine className="h-4 w-4" />} />
        <StatCard label="运行中环境" value={running.length} hint="自动监控范围" icon={<Clock3 className="h-4 w-4" />} tone="success" />
        <StatCard label="涉及环境" value={new Set(events.map((event) => event.envId)).size} hint="按环境去重" icon={<ExternalLink className="h-4 w-4" />} />
      </div>

      <Card className="overflow-hidden">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border p-4">
          <div className="flex flex-wrap items-center gap-2">
            <Select className="w-[180px]" value={envFilter} onChange={(event) => setEnvFilter(event.target.value)} placeholder="全部环境">
              {envs.map((env) => (
                <option key={env.id} value={env.id}>{env.name}</option>
              ))}
            </Select>
            <label className="flex items-center gap-1.5 text-xs text-content-weak">
              <input
                type="checkbox"
                className="accent-[var(--color-primary)]"
                checked={showHandled}
                onChange={(event) => setShowHandled(event.target.checked)}
              />
              显示已处理
            </label>
          </div>
          <p className="text-[11px] text-content-hint">
            {filtered.length} 条事件 · 自动刷新由 CDP binding 推送
          </p>
        </div>

        {loading ? (
          <div className="flex justify-center py-16"><Spinner /></div>
        ) : filtered.length === 0 ? (
          <EmptyState
            icon={<ShieldQuestion className="h-5 w-5" />}
            title="暂无验证码事件"
            description="打开环境并访问包含验证码的页面，FinEnv 会通过 DOM 监听与 CDP binding 自动捕获；也可手动采集当前画面。"
            action={
              canOpen && running.length > 0 ? (
                <Button variant="secondary" size="sm" loading={capturing} onClick={() => void capture()}>
                  <Camera className="h-3.5 w-3.5" /> 采集当前画面
                </Button>
              ) : undefined
            }
          />
        ) : (
          <div className="grid gap-3 p-4 md:grid-cols-2 xl:grid-cols-3">
            {filtered.map((event) => (
              <div key={event.id} className="overflow-hidden rounded-lg border border-border bg-surface-base">
                <div className="flex h-40 items-center justify-center border-b border-border bg-surface-tertiary">
                  {event.screenshotPath && previews[event.id] ? (
                    <img src={previews[event.id]} alt="验证码截图" className="h-full w-full object-cover" />
                  ) : event.screenshotPath ? (
                    <div className="flex flex-col items-center gap-1 p-3 text-center">
                      <Camera className="h-5 w-5 text-content-hint" />
                      <p className="max-w-full break-all text-[10px] text-content-hint">{event.screenshotPath}</p>
                    </div>
                  ) : (
                    <div className="flex flex-col items-center gap-1 text-content-hint">
                      <ShieldQuestion className="h-6 w-6" />
                      <span className="text-[10px]">无截图</span>
                    </div>
                  )}
                </div>
                <div className="space-y-2 p-3">
                  <div className="flex items-center justify-between gap-2">
                    <Link to={`/envs/${event.envId}`} className="min-w-0 truncate text-sm font-medium text-content-main hover:text-primary">
                      {event.envName}
                    </Link>
                    {event.handled ? <Badge tone="success">已处理</Badge> : <Badge tone="warning">待处理</Badge>}
                  </div>
                  <p className="max-h-12 overflow-hidden break-all text-[10px] leading-4 text-content-hint">{event.url || '当前页面'}</p>
                  <p className="line-clamp-2 text-[11px] leading-5 text-content-weak">{event.hint}</p>
                  <div className="flex items-center justify-between">
                    <span className="flex items-center gap-1 text-[10px] text-content-hint"><Clock3 className="h-3 w-3" /> {formatDateShort(event.createdAt)}</span>
                    <Button
                      variant={event.handled ? 'ghost' : 'secondary'}
                      size="sm"
                      loading={updating === event.id}
                      disabled={!canOpen}
                      onClick={() => void toggle(event)}
                    >
                      <CheckCheck className="h-3.5 w-3.5" />
                      {event.handled ? '标记未处理' : '标记已处理'}
                    </Button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
