import * as React from 'react';
import { KeyRound } from 'lucide-react';
import { ipc } from '@/ipc/client';
import { toastError, useToastStore } from '@/store/toast';
import { Button } from '@/components/ui/button';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input } from '@/components/ui/input';

export function PasswordDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const toast = useToastStore();
  const [oldPassword, setOldPassword] = React.useState('');
  const [newPassword, setNewPassword] = React.useState('');
  const [confirm, setConfirm] = React.useState('');
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setOldPassword('');
      setNewPassword('');
      setConfirm('');
    }
  }, [open]);

  const mismatch = confirm.length > 0 && confirm !== newPassword;
  const canSubmit = oldPassword.length > 0 && newPassword.length >= 8 && newPassword === confirm;

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="修改登录密码"
      description="密码使用 PBKDF2-SHA256 加盐哈希存储，修改后当前会话保持登录。"
      width="max-w-md"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button
            variant="primary"
            loading={busy}
            disabled={!canSubmit}
            onClick={async () => {
              setBusy(true);
              try {
                await ipc.authChangePassword(oldPassword, newPassword);
                toast.success('密码已修改', '下次登录请使用新密码');
                onOpenChange(false);
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            <KeyRound className="h-4 w-4" /> 更新密码
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="当前密码" required>
          <Input type="password" value={oldPassword} onChange={(event) => setOldPassword(event.target.value)} autoFocus />
        </Field>
        <Field label="新密码" required hint="至少 8 个字符">
          <Input type="password" value={newPassword} onChange={(event) => setNewPassword(event.target.value)} />
        </Field>
        <Field label="确认新密码" required error={mismatch ? '两次输入不一致' : undefined}>
          <Input type="password" value={confirm} onChange={(event) => setConfirm(event.target.value)} />
        </Field>
      </div>
    </Dialog>
  );
}
