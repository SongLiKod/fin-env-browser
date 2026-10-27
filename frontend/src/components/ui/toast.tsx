import { AlertCircle, CheckCircle2, Info, TriangleAlert, X } from 'lucide-react';
import { cn } from '@/utils';
import { useToastStore, type ToastTone } from '@/store/toast';

const toneStyles: Record<ToastTone, { icon: typeof Info; className: string }> = {
  success: { icon: CheckCircle2, className: 'text-[var(--success)]' },
  error: { icon: AlertCircle, className: 'text-danger' },
  warning: { icon: TriangleAlert, className: 'text-[var(--warning)]' },
  info: { icon: Info, className: 'text-primary' },
};

export function ToastViewport() {
  const items = useToastStore((state) => state.items);
  const dismiss = useToastStore((state) => state.dismiss);
  return (
    <div className="pointer-events-none fixed bottom-5 right-5 z-[60] flex w-[360px] max-w-[calc(100vw-40px)] flex-col gap-2">
      {items.map((item) => {
        const tone = toneStyles[item.tone];
        const Icon = tone.icon;
        return (
          <div
            key={item.id}
            className="pointer-events-auto flex animate-slide-up items-start gap-3 rounded-lg border border-border bg-surface-base p-3 shadow-pop"
          >
            <Icon className={cn('mt-0.5 h-4 w-4 shrink-0', tone.className)} />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium text-content-main">{item.title}</p>
              {item.description ? (
                <p className="mt-0.5 break-words text-xs leading-5 text-content-weak">{item.description}</p>
              ) : null}
            </div>
            <button
              type="button"
              className="text-content-hint transition-colors hover:text-content-main"
              onClick={() => dismiss(item.id)}
              aria-label="关闭提示"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
