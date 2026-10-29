import * as React from 'react';
import { cn } from '@/utils';

export function Card({
  className,
  children,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={cn('card', className)} {...props}>
      {children}
    </div>
  );
}

export function CardHeader({
  title,
  description,
  actions,
  className,
}: {
  title: React.ReactNode;
  description?: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn('flex items-start justify-between gap-3 border-b border-border px-4 py-3', className)}>
      <div className="min-w-0">
        <h3 className="text-sm font-semibold text-content-main">{title}</h3>
        {description ? <p className="mt-0.5 text-xs text-content-weak">{description}</p> : null}
      </div>
      {actions ? <div className="flex shrink-0 items-center gap-1.5">{actions}</div> : null}
    </div>
  );
}

export function CardBody({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  return <div className={cn('p-4', className)}>{children}</div>;
}

export function Section({
  title,
  description,
  actions,
  children,
  className,
}: {
  title: string;
  description?: string;
  actions?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <section className={cn('space-y-3', className)}>
      <div className="flex flex-wrap items-end justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold text-content-main">{title}</h3>
          {description ? <p className="mt-0.5 text-xs text-content-weak">{description}</p> : null}
        </div>
        {actions ? <div className="flex items-center gap-2">{actions}</div> : null}
      </div>
      {children}
    </section>
  );
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  className,
}: {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn('flex flex-col items-center justify-center px-6 py-12 text-center', className)}>
      {icon ? (
        <div className="mb-3 flex h-11 w-11 items-center justify-center rounded-full bg-surface-tertiary text-content-weak">
          {icon}
        </div>
      ) : null}
      <p className="text-sm font-medium text-content-main">{title}</p>
      {description ? <p className="mt-1 max-w-sm text-xs leading-5 text-content-weak">{description}</p> : null}
      {action ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}

export function Spinner({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        'inline-block h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent',
        className,
      )}
    />
  );
}

export function StatCard({
  label,
  value,
  hint,
  icon,
  tone,
}: {
  label: string;
  value: React.ReactNode;
  hint?: React.ReactNode;
  icon?: React.ReactNode;
  tone?: 'primary' | 'success' | 'warning' | 'neutral';
}) {
  const iconClass =
    tone === 'success'
      ? 'bg-[color-mix(in_srgb,var(--success)_12%,transparent)] text-[var(--success)]'
      : tone === 'warning'
        ? 'bg-[color-mix(in_srgb,var(--warning)_14%,transparent)] text-[var(--warning)]'
        : tone === 'primary'
          ? 'bg-primary-light text-primary'
          : 'bg-surface-tertiary text-content-weak';
  return (
    <Card className="stat-card flex items-center gap-3 p-4">
      {icon ? <div className={cn('stat-icon flex h-10 w-10 items-center justify-center rounded-xl', iconClass)}>{icon}</div> : null}
      <div className="relative z-[1] min-w-0">
        <p className="text-xs font-medium tracking-wide text-content-weak">{label}</p>
        <p className="mt-0.5 truncate text-[22px] font-semibold tracking-tight text-content-main tabular-nums">{value}</p>
        {hint ? <p className="mt-0.5 truncate text-[11px] text-content-hint">{hint}</p> : null}
      </div>
    </Card>
  );
}
