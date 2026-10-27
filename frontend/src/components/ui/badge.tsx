import * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '@/utils';

const badgeVariants = cva(
  'inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium leading-5',
  {
    variants: {
      tone: {
        neutral: 'border-border bg-surface-tertiary text-content-weak',
        primary: 'border-transparent bg-primary-light text-primary',
        success: 'border-transparent bg-[color-mix(in_srgb,var(--success)_14%,transparent)] text-[var(--success)]',
        warning: 'border-transparent bg-[color-mix(in_srgb,var(--warning)_16%,transparent)] text-[var(--warning)]',
        danger: 'border-transparent bg-[color-mix(in_srgb,var(--danger)_14%,transparent)] text-[var(--danger)]',
        outline: 'border-border-strong bg-transparent text-content-weak',
      },
    },
    defaultVariants: { tone: 'neutral' },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, tone, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ tone }), className)} {...props} />;
}

export function StatusDot({
  tone = 'muted',
  pulse = false,
  className,
}: {
  tone?: 'success' | 'warning' | 'danger' | 'muted' | 'primary';
  pulse?: boolean;
  className?: string;
}) {
  const color =
    tone === 'success'
      ? 'var(--success)'
      : tone === 'warning'
        ? 'var(--warning)'
        : tone === 'danger'
          ? 'var(--danger)'
          : tone === 'primary'
            ? 'var(--color-primary)'
            : 'var(--text-hint)';
  return (
    <span
      className={cn('inline-block h-2 w-2 shrink-0 rounded-full', pulse && 'status-dot-running', className)}
      style={{ background: color }}
    />
  );
}
