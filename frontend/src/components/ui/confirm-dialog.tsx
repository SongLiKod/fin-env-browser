import * as React from 'react';
import { AlertTriangle } from 'lucide-react';
import { Button } from './button';
import { Dialog } from './dialog';
import { Input } from './input';

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel = '确认',
  cancelLabel = '取消',
  tone = 'danger',
  requiredText,
  busy = false,
  onConfirm,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: React.ReactNode;
  description?: React.ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: 'danger' | 'primary';
  requiredText?: string;
  busy?: boolean;
  onConfirm: () => void | Promise<void>;
  children?: React.ReactNode;
}) {
  const [value, setValue] = React.useState('');
  React.useEffect(() => {
    if (open) setValue('');
  }, [open]);
  const blocked = !!requiredText && value !== requiredText;
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      width="max-w-md"
      title={
        <span className="flex items-center gap-2">
          <span className="flex h-8 w-8 items-center justify-center rounded-full bg-[color-mix(in_srgb,var(--danger)_12%,transparent)] text-danger">
            <AlertTriangle className="h-4 w-4" />
          </span>
          {title}
        </span>
      }
      description={description}
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            {cancelLabel}
          </Button>
          <Button
            variant={tone === 'danger' ? 'danger' : 'primary'}
            loading={busy}
            disabled={blocked}
            onClick={() => void onConfirm()}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        {children}
        {requiredText ? (
          <div>
            <p className="mb-2 text-xs text-content-weak">
              此操作不可撤销。请输入 <span className="font-mono font-semibold text-content-main">{requiredText}</span>{' '}
              以确认。
            </p>
            <Input
              value={value}
              onChange={(event) => setValue(event.target.value)}
              placeholder={requiredText}
              autoFocus
            />
          </div>
        ) : null}
      </div>
    </Dialog>
  );
}
