import * as React from 'react';
import { FolderPlus, Package, Plus, Puzzle, Trash2, X } from 'lucide-react';
import type { EnvironmentView, Extension } from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { pickDirectory } from '@/utils/dialog';
import { useAuthStore } from '@/store/auth';
import { useEnvStore } from '@/store/env';
import { toastError, useToastStore } from '@/store/toast';
import { formatDateShort } from '@/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, EmptyState, Spinner } from '@/components/ui/card';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input } from '@/components/ui/input';

export function ExtensionsPanel({ env }: { env: EnvironmentView }) {
  const toast = useToastStore();
  const canEdit = useAuthStore((state) => state.can('edit_env'));
  const allEnvs = useEnvStore((state) => state.envs);
  const [all, setAll] = React.useState<Extension[]>([]);
  const [assigned, setAssigned] = React.useState<Extension[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [addOpen, setAddOpen] = React.useState(false);
  const [path, setPath] = React.useState('');
  const [name, setName] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  const [deleteTarget, setDeleteTarget] = React.useState<Extension | null>(null);
  const [assignTarget, setAssignTarget] = React.useState<Extension | null>(null);
  const [assignEnvIds, setAssignEnvIds] = React.useState<string[]>([]);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const [global, current] = await Promise.all([ipc.extensionList(), ipc.envExtensions(env.id)]);
      setAll(global);
      setAssigned(current);
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, [env.id]);

  React.useEffect(() => {
    void load();
  }, [load]);

  const assignedIds = new Set(assigned.map((item) => item.id));

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold text-content-main">浏览器扩展</h3>
          <p className="mt-0.5 text-xs text-content-weak">
            为环境独立加载抓包、Mock、性能调试类扩展；通过 <code className="font-mono">--load-extension</code> 注入，需重启浏览器生效。
          </p>
        </div>
        {canEdit ? (
          <Button variant="primary" size="sm" onClick={() => setAddOpen(true)}>
            <Plus className="h-3.5 w-3.5" /> 安装扩展
          </Button>
        ) : null}
      </div>

      <Card className="overflow-hidden">
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div className="flex items-center gap-2">
            <Puzzle className="h-4 w-4 text-primary" />
            <span className="text-sm font-medium text-content-main">当前环境已分配</span>
            <Badge tone="primary">{assigned.length}</Badge>
          </div>
        </div>
        {loading ? (
          <div className="flex justify-center py-10"><Spinner /></div>
        ) : assigned.length === 0 ? (
          <EmptyState
            icon={<Package className="h-5 w-5" />}
            title="当前环境未分配扩展"
            description="安装一个本地解压的 Chrome 扩展目录，然后分配到此环境。"
          />
        ) : (
          <div className="divide-y divide-border">
            {assigned.map((extension) => (
              <div key={extension.id} className="flex items-center gap-3 px-4 py-3">
                <div className="flex h-8 w-8 items-center justify-center rounded-md bg-primary-light text-primary">
                  <Puzzle className="h-4 w-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium text-content-main">{extension.name}</p>
                  <p className="truncate font-mono text-[10px] text-content-hint" title={extension.path}>
                    {extension.path}
                  </p>
                  {extension.description ? <p className="truncate text-[11px] text-content-weak">{extension.description}</p> : null}
                </div>
                {canEdit ? (
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    onClick={async () => {
                      try {
                        await ipc.extensionUnassign(extension.id, [env.id]);
                        toast.success('已取消分配', env.runtime ? '重启环境后生效' : undefined);
                        await load();
                      } catch (error) {
                        toastError(error);
                      }
                    }}
                  >
                    <X className="h-3.5 w-3.5" />
                  </Button>
                ) : null}
              </div>
            ))}
          </div>
        )}
      </Card>

      <Card>
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div>
            <p className="text-sm font-medium text-content-main">扩展库</p>
            <p className="mt-0.5 text-xs text-content-hint">扩展文件仅引用本地目录，不复制到应用数据目录。</p>
          </div>
          <Badge tone="outline">{all.length} 个</Badge>
        </div>
        {all.length === 0 ? (
          <EmptyState icon={<Puzzle className="h-5 w-5" />} title="扩展库为空" description="选择一个包含 manifest.json 的解压扩展目录。" />
        ) : (
          <div className="divide-y divide-border">
            {all.map((extension) => (
              <div key={extension.id} className="flex flex-wrap items-center gap-2 px-4 py-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <p className="truncate text-sm text-content-main">{extension.name}</p>
                    {assignedIds.has(extension.id) ? <Badge tone="success">已分配</Badge> : <Badge tone="neutral">未分配</Badge>}
                  </div>
                  <p className="truncate font-mono text-[10px] text-content-hint" title={extension.path}>{extension.path}</p>
                  <p className="text-[10px] text-content-hint">安装于 {formatDateShort(extension.createdAt)}</p>
                </div>
                {canEdit ? (
                  <div className="flex items-center gap-1">
                    {assignedIds.has(extension.id) ? (
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={async () => {
                          try {
                            await ipc.extensionUnassign(extension.id, [env.id]);
                            toast.success('已从当前环境移除');
                            await load();
                          } catch (error) {
                            toastError(error);
                          }
                        }}
                      >
                        移除
                      </Button>
                    ) : (
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={async () => {
                          try {
                            await ipc.extensionAssign(extension.id, [env.id]);
                            toast.success('已分配到当前环境', env.runtime ? '重启环境后生效' : undefined);
                            await load();
                          } catch (error) {
                            toastError(error);
                          }
                        }}
                      >
                        分配到当前环境
                      </Button>
                    )}
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => {
                        setAssignTarget(extension);
                        setAssignEnvIds([env.id]);
                      }}
                    >
                      批量分配
                    </Button>
                    <Button variant="ghost" size="icon-sm" className="text-danger" onClick={() => setDeleteTarget(extension)}>
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                ) : null}
              </div>
            ))}
          </div>
        )}
      </Card>

      <div className="rounded-md border border-border bg-surface-secondary p-3 text-[11px] leading-5 text-content-weak">
        建议仅加载内部可信扩展。扩展运行在独立的 Chromium 子进程中，与主面板 IPC 权限隔离；但扩展本身仍可读取该环境页面的数据。
      </div>

      <Dialog
        open={addOpen}
        onOpenChange={setAddOpen}
        title="安装浏览器扩展"
        description="选择一个解压后的扩展目录（必须包含 manifest.json）。"
        width="max-w-lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setAddOpen(false)} disabled={busy}>取消</Button>
            <Button
              variant="primary"
              loading={busy}
              disabled={!path}
              onClick={async () => {
                setBusy(true);
                try {
                  const extension = await ipc.extensionAdd(path, name || undefined);
                  toast.success('扩展已安装', extension.name);
                  setAddOpen(false);
                  setPath('');
                  setName('');
                  await load();
                } catch (error) {
                  toastError(error);
                } finally {
                  setBusy(false);
                }
              }}
            >
              安装
            </Button>
          </>
        }
      >
        <div className="space-y-3">
          <Field label="扩展目录" required>
            <div className="flex gap-2">
              <Input value={path} readOnly placeholder="选择扩展目录" />
              <Button
                variant="secondary"
                onClick={async () => {
                  try {
                    const dir = await pickDirectory();
                    if (dir) setPath(dir);
                  } catch (error) {
                    toastError(error);
                  }
                }}
              >
                <FolderPlus className="h-4 w-4" /> 选择
              </Button>
            </div>
          </Field>
          <Field label="显示名称" hint="留空时使用 manifest.json 中的 name">
            <Input value={name} onChange={(event) => setName(event.target.value)} />
          </Field>
        </div>
      </Dialog>

      <Dialog
        open={!!assignTarget}
        onOpenChange={(value) => !value && setAssignTarget(null)}
        title="批量分配扩展"
        description={`选择需要加载「${assignTarget?.name ?? ''}」的环境，运行中的环境需重启后生效。`}
        width="max-w-lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setAssignTarget(null)}>取消</Button>
            <Button
              variant="primary"
              disabled={assignEnvIds.length === 0}
              onClick={async () => {
                if (!assignTarget) return;
                try {
                  await ipc.extensionAssign(assignTarget.id, assignEnvIds);
                  toast.success('扩展已分配', `${assignEnvIds.length} 个环境`);
                  setAssignTarget(null);
                  await load();
                } catch (error) {
                  toastError(error);
                }
              }}
            >
              分配 {assignEnvIds.length} 个环境
            </Button>
          </>
        }
      >
        <div className="max-h-72 space-y-1 overflow-y-auto rounded-md border border-border p-2">
          {allEnvs.map((item) => (
            <label key={item.id} className="flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-xs hover:bg-surface-secondary">
              <input
                type="checkbox"
                className="accent-[var(--color-primary)]"
                checked={assignEnvIds.includes(item.id)}
                onChange={(event) =>
                  setAssignEnvIds((current) =>
                    event.target.checked ? [...current, item.id] : current.filter((id) => id !== item.id),
                  )
                }
              />
              <span className="min-w-0 flex-1 truncate">{item.name}</span>
              {item.runtime ? <Badge tone="success">运行中</Badge> : null}
            </label>
          ))}
        </div>
      </Dialog>

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(value) => !value && setDeleteTarget(null)}
        title="删除扩展"
        description={`从扩展库删除「${deleteTarget?.name ?? ''}」，并解除所有环境分配。本地扩展目录不会被删除。`}
        confirmLabel="删除扩展"
        onConfirm={async () => {
          if (!deleteTarget) return;
          try {
            await ipc.extensionDelete(deleteTarget.id);
            setDeleteTarget(null);
            toast.success('扩展已删除');
            await load();
          } catch (error) {
            toastError(error);
          }
        }}
      />
    </div>
  );
}
