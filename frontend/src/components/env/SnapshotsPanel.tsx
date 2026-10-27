import * as React from 'react';
import { Camera, Clock3, RotateCcw, Save, Trash2 } from 'lucide-react';
import type { EnvironmentView, FingerprintSnapshot } from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { toastError, useToastStore } from '@/store/toast';
import { formatDate } from '@/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, EmptyState, Spinner } from '@/components/ui/card';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input } from '@/components/ui/input';

export function SnapshotsPanel({
  env,
  onUpdated,
}: {
  env: EnvironmentView;
  onUpdated: (env: EnvironmentView) => void;
}) {
  const toast = useToastStore();
  const canEdit = useAuthStore((state) => state.can('edit_fingerprint_proxy'));
  const [snapshots, setSnapshots] = React.useState<FingerprintSnapshot[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [createOpen, setCreateOpen] = React.useState(false);
  const [name, setName] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  const [restoreTarget, setRestoreTarget] = React.useState<FingerprintSnapshot | null>(null);
  const [deleteTarget, setDeleteTarget] = React.useState<FingerprintSnapshot | null>(null);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      setSnapshots(await ipc.snapshotList(env.id));
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, [env.id]);

  React.useEffect(() => {
    void load();
  }, [load]);

  const create = async () => {
    setBusy(true);
    try {
      const snapshot = await ipc.snapshotCreate(env.id, name.trim() || undefined);
      toast.success('快照已创建', snapshot.name);
      setCreateOpen(false);
      setName('');
      await load();
      onUpdated(await ipc.envGet(env.id));
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold text-content-main">指纹快照</h3>
          <p className="mt-0.5 text-xs text-content-weak">
            保存当前全套指纹参数；复现 Bug 或切回历史配置时可一键还原。还原会立即应用到运行中的环境。
          </p>
        </div>
        {canEdit ? (
          <Button variant="primary" size="sm" onClick={() => setCreateOpen(true)}>
            <Camera className="h-3.5 w-3.5" /> 保存当前快照
          </Button>
        ) : null}
      </div>

      <Card className="overflow-hidden">
        {loading ? (
          <div className="flex justify-center py-12"><Spinner /></div>
        ) : snapshots.length === 0 ? (
          <EmptyState
            icon={<Save className="h-5 w-5" />}
            title="暂无指纹快照"
            description="在调整指纹前先保存一份快照，后续可对比或快速回滚。"
            action={
              canEdit ? (
                <Button variant="primary" size="sm" onClick={() => setCreateOpen(true)}>
                  保存第一份快照
                </Button>
              ) : undefined
            }
          />
        ) : (
          <div className="table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>快照名称</th>
                  <th>指纹摘要</th>
                  <th>创建时间</th>
                  <th>状态</th>
                  <th className="text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {snapshots.map((snapshot) => (
                  <tr key={snapshot.id}>
                    <td>
                      <p className="text-sm font-medium text-content-main">{snapshot.name}</p>
                      <p className="mt-0.5 font-mono text-[10px] text-content-hint">{snapshot.id.slice(0, 8)}…</p>
                    </td>
                    <td>
                      <div className="max-w-[420px] space-y-0.5 text-xs">
                        <p className="text-content-main">
                          {snapshot.fingerprint.clientHints.platform} · {snapshot.fingerprint.timezone} ·{' '}
                          {snapshot.fingerprint.screen.width}×{snapshot.fingerprint.screen.height}
                        </p>
                        <p className="truncate font-mono text-[10px] text-content-hint" title={snapshot.fingerprint.userAgent}>
                          {snapshot.fingerprint.userAgent}
                        </p>
                        <p className="text-[10px] text-content-hint">
                          WebGL: {snapshot.fingerprint.webgl.vendor} · Canvas {snapshot.fingerprint.canvas.mode} · Audio{' '}
                          {snapshot.fingerprint.audio.mode}
                        </p>
                      </div>
                    </td>
                    <td className="text-xs text-content-weak">{formatDate(snapshot.createTime)}</td>
                    <td>
                      {env.fingerprintSnapshotId === snapshot.id ? (
                        <Badge tone="success">当前生效</Badge>
                      ) : (
                        <Badge tone="neutral">历史快照</Badge>
                      )}
                    </td>
                    <td>
                      <div className="flex items-center justify-end gap-1">
                        {canEdit ? (
                          <>
                            <Button variant="ghost" size="sm" onClick={() => setRestoreTarget(snapshot)}>
                              <RotateCcw className="h-3.5 w-3.5" /> 还原
                            </Button>
                            <Button variant="ghost" size="icon-sm" className="text-danger" onClick={() => setDeleteTarget(snapshot)}>
                              <Trash2 className="h-3.5 w-3.5" />
                            </Button>
                          </>
                        ) : (
                          <span className="text-[11px] text-content-hint">只读</span>
                        )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <div className="flex items-start gap-2 text-[11px] text-content-hint">
        <Clock3 className="mt-0.5 h-3.5 w-3.5" />
        快照仅保存指纹配置，不包含 Cookie 与登录态；浏览器 profile 由环境目录独立持久化。
      </div>

      <Dialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        title="保存指纹快照"
        description="将当前环境的 UA、屏幕、时区、Canvas/WebGL/Audio、字体、Client Hints 与插件配置完整保存。"
        width="max-w-md"
        footer={
          <>
            <Button variant="secondary" onClick={() => setCreateOpen(false)} disabled={busy}>取消</Button>
            <Button variant="primary" loading={busy} onClick={() => void create()}>保存快照</Button>
          </>
        }
      >
        <Field label="快照名称" hint="留空自动使用当前时间">
          <Input value={name} onChange={(event) => setName(event.target.value)} autoFocus placeholder={`快照 ${new Date().toLocaleString()}`} />
        </Field>
      </Dialog>

      <ConfirmDialog
        open={!!restoreTarget}
        onOpenChange={(value) => !value && setRestoreTarget(null)}
        title="还原指纹快照"
        description={`将「${restoreTarget?.name ?? ''}」覆盖当前指纹配置；运行中的浏览器会立即应用新的 UA/时区等参数。`}
        confirmLabel="还原并应用"
        tone="primary"
        onConfirm={async () => {
          if (!restoreTarget) return;
          try {
            const updated = await ipc.snapshotRestore(restoreTarget.id);
            onUpdated(updated);
            setRestoreTarget(null);
            toast.success('快照已还原', restoreTarget.name);
            await load();
          } catch (error) {
            toastError(error);
          }
        }}
      />

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(value) => !value && setDeleteTarget(null)}
        title="删除快照"
        description={`将永久删除「${deleteTarget?.name ?? ''}」，不影响当前指纹。`}
        confirmLabel="删除"
        onConfirm={async () => {
          if (!deleteTarget) return;
          try {
            await ipc.snapshotDelete(deleteTarget.id);
            setDeleteTarget(null);
            toast.success('快照已删除');
            await load();
            onUpdated(await ipc.envGet(env.id));
          } catch (error) {
            toastError(error);
          }
        }}
      />
    </div>
  );
}
