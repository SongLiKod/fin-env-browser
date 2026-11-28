import type { Proxy } from '@/ipc/types';
import { cn, formatDateShort } from '@/utils';
import { Badge } from '@/components/ui/badge';

export function proxyLocationText(proxy: Proxy): string {
  return (
    [proxy.exitCountry, proxy.lastCheckCity, proxy.exitTimezone].filter(Boolean).join(' · ') ||
    '尚未获取位置信息'
  );
}

export function proxyCheckTargetLabel(target: string | null | undefined): string {
  return target === 'github' ? 'GitHub' : 'Google + YouTube';
}

export function proxyOptionLabel(proxy: Proxy): string {
  const location = [proxy.exitCountry, proxy.lastCheckCity].filter(Boolean).join(' / ');
  const latency = proxy.latencyMs != null ? ` · ${proxy.latencyMs}ms` : '';
  return `${proxy.name} · ${location || '未检测位置'}${latency} · ${proxy.protocol}://${proxy.host}:${proxy.port}`;
}

export function ProxyLocationSummary({
  proxy,
  className,
}: {
  proxy: Proxy | null;
  className?: string;
}) {
  if (!proxy) {
    return (
      <div
        className={cn(
          'rounded-md border border-border bg-surface-secondary px-3 py-2 text-xs text-content-weak',
          className,
        )}
      >
        直连（未绑定代理）
      </div>
    );
  }

  const status = !proxy.enabled
    ? { tone: 'neutral' as const, label: '已停用' }
    : proxy.lastCheckOk === true
      ? { tone: 'success' as const, label: '可用' }
      : proxy.lastCheckOk === false
        ? { tone: 'danger' as const, label: '检测失败' }
        : { tone: 'neutral' as const, label: '未检测' };

  return (
    <div
      className={cn(
        'rounded-md border border-border bg-surface-secondary px-3 py-2',
        className,
      )}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-sm font-medium text-content-main">
          {proxy.exitIp ?? '未检测出口 IP'}
        </span>
        <Badge tone={status.tone}>{status.label}</Badge>
      </div>
      <p className="mt-1 text-[11px] text-content-weak">{proxyLocationText(proxy)}</p>
      <p className="mt-0.5 text-[10px] text-content-hint">
        {proxy.latencyMs != null ? `${proxy.latencyMs}ms` : '—'}
        {' · '}
        {proxy.lastCheckAt ? formatDateShort(proxy.lastCheckAt) : '未检测'}
        {proxy.lastCheckTarget ? ` · ${proxyCheckTargetLabel(proxy.lastCheckTarget)}` : ''}
        {proxy.lastCheckMessage ? ` · ${proxy.lastCheckMessage}` : ''}
      </p>
    </div>
  );
}
