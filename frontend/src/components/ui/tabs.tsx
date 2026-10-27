import { cn } from '@/utils';

export interface TabItem {
  value: string;
  label: string;
  badge?: string | number;
}

export function Tabs({
  items,
  value,
  onValueChange,
  className,
}: {
  items: TabItem[];
  value: string;
  onValueChange: (value: string) => void;
  className?: string;
}) {
  return (
    <div className={cn('flex flex-wrap items-center gap-1 border-b border-border', className)}>
      {items.map((item) => {
        const active = item.value === value;
        return (
          <button
            key={item.value}
            type="button"
            onClick={() => onValueChange(item.value)}
            className={cn(
              'relative -mb-px flex items-center gap-1.5 border-b-2 px-3 py-2.5 text-sm font-medium transition-colors',
              active
                ? 'border-primary text-primary'
                : 'border-transparent text-content-weak hover:text-content-main',
            )}
          >
            {item.label}
            {item.badge !== undefined && item.badge !== '' ? (
              <span className="rounded-full bg-surface-tertiary px-1.5 py-0.5 text-[10px] text-content-weak">
                {item.badge}
              </span>
            ) : null}
          </button>
        );
      })}
    </div>
  );
}
