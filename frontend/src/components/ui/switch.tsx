import { cn } from '@/utils';

export function Switch({
  checked,
  onCheckedChange,
  disabled,
  label,
  description,
}: {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  label?: string;
  description?: string;
}) {
  return (
    <label className="flex cursor-pointer items-start gap-3">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onCheckedChange(!checked)}
        className={cn(
          'relative mt-0.5 inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-light disabled:opacity-50',
          checked ? 'border-primary bg-primary' : 'border-border-strong bg-surface-tertiary',
        )}
      >
        <span
          className={cn(
            'inline-block h-4 w-4 transform rounded-full bg-[var(--switch-thumb)] shadow transition-transform',
            checked ? 'translate-x-4' : 'translate-x-0.5',
          )}
        />
      </button>
      {label || description ? (
        <span className="min-w-0">
          {label ? <span className="block text-sm font-medium text-content-main">{label}</span> : null}
          {description ? (
            <span className="mt-0.5 block text-xs leading-5 text-content-weak">{description}</span>
          ) : null}
        </span>
      ) : null}
    </label>
  );
}
