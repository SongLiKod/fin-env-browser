import * as React from 'react';
import { Copy, Check } from 'lucide-react';
import { copyText } from '@/utils';
import { Button } from './button';

export function CopyButton({
  value,
  label,
  size = 'icon-sm',
}: {
  value: string;
  label?: string;
  size?: 'icon-sm' | 'sm';
}) {
  const [copied, setCopied] = React.useState(false);
  return (
    <Button
      variant="ghost"
      size={size}
      title="复制"
      onClick={async () => {
        const ok = await copyText(value);
        if (ok) {
          setCopied(true);
          window.setTimeout(() => setCopied(false), 1600);
        }
      }}
    >
      {copied ? <Check className="h-3.5 w-3.5 text-[var(--success)]" /> : <Copy className="h-3.5 w-3.5" />}
      {label ? <span>{label}</span> : null}
    </Button>
  );
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  className,
}: {
  options: Array<{ value: T; label: React.ReactNode }>;
  value: T;
  onChange: (value: T) => void;
  className?: string;
}) {
  return (
    <div className={className}>
      <div className="inline-flex rounded-md border border-border bg-surface-tertiary p-0.5">
        {options.map((option) => (
          <button
            key={option.value}
            type="button"
            onClick={() => onChange(option.value)}
            className={
              option.value === value
                ? 'rounded-[5px] bg-surface-base px-2.5 py-1 text-xs font-medium text-content-main shadow-sm'
                : 'rounded-[5px] px-2.5 py-1 text-xs font-medium text-content-weak hover:text-content-main'
            }
          >
            {option.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export function Progress({ value, tone = 'primary' }: { value: number; tone?: 'primary' | 'warning' | 'danger' }) {
  const color = tone === 'danger' ? 'var(--danger)' : tone === 'warning' ? 'var(--warning)' : 'var(--color-primary)';
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface-tertiary">
      <div
        className="h-full rounded-full transition-all"
        style={{ width: `${Math.max(0, Math.min(100, value))}%`, background: color }}
      />
    </div>
  );
}
