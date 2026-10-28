import * as React from 'react';
import {
  CheckCheck,
  FolderOpen,
  FolderPlus,
  Globe2,
  Package,
  Pencil,
  Puzzle,
  Trash2,
  Users,
  X,
} from 'lucide-react';
import type { EnvironmentView, Extension } from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { pickDirectory } from '@/utils/dialog';
import { useAuthStore } from '@/store/auth';
import { toastError, useToastStore } from '@/store/toast';
import { cn, formatDateShort } from '@/utils';
import { envHasAccess } from '@/utils/environment';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, EmptyState, Spinner } from '@/components/ui/card';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input, Textarea } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';

export function ExtensionsPanel({ env }: { env: EnvironmentView }) {
  const toast = useToastStore();
  const canEdit = useAuthStore((state) => state.can('edit_env')) && envHasAccess(env, 'manage');
  const [envs, setEnvs] = React.useState<EnvironmentView[]>([]);
  const [all, setAll] = React.useState<Extension[]>([]);
  const [assigned, setAssigned] = React.useState<Extension[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [addOpen, setAddOpen] = React.useState(false);
  const [addMode, setAddMode] = React.useState<'online' | 'local'>('online');
  const [storeSource, setStoreSource] = React.useState('');
  const [path, setPath] = React.useState('');
  const [name, setName] = React.useState('');
  const [assignCurrent, setAssignCurrent] = React.useState(true);
  const [busy, setBusy] = React.useState(false);
  const [deleteTarget, setDeleteTarget] = React.useState<Extension | null>(null);
  const [assignTarget, setAssignTarget] = React.useState<Extension | null>(null);
  const [assignEnvIds, setAssignEnvIds] = React.useState<string[]>([]);
  const [applyAllTarget, setApplyAllTarget] = React.useState<Extension | null>(null);
  const [editTarget, setEditTarget] = React.useState<Extension | null>(null);
  const [editName, setEditName] = React.useState('');
  const [editDescription, setEditDescription] = React.useState('');

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const [global, current, allEnvs] = await Promise.all([
        ipc.extensionList(),
        ipc.envExtensions(env.id),
        ipc.envList({}),
      ]);
      setAll(global);
      setAssigned(current);
      setEnvs(allEnvs);
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

  const resetAdd = () => {
    setStoreSource('');
    setPath('');
    setName('');
    setAssignCurrent(true);
    setAddMode('online');
  };

  const closeAdd = (open: boolean) => {
    setAddOpen(open);
    if (!open) resetAdd();
  };

  const install = async () => {
    setBusy(true);
    try {
      let extension: Extension;
      if (addMode === 'online') {
        if (!storeSource.trim()) {
          toast.warning('请输入 Chrome 应用商店链接或扩展 ID');
          return;
        }
        extension = await ipc.extensionInstallOnline(
          storeSource.trim(),
          assignCurrent ? [env.id] : [],
        );
        toast.success('在线扩展已安装', assignCurrent ? '已分配到当前环境' : '可在扩展库中分配');
      } else {
        if (!path) {
          toast.warning('请选择扩展目录');
          return;
        }
        extension = await ipc.extensionAdd(path, name.trim() || undefined);
        if (assignCurrent) {
          await ipc.extensionAssign(extension.id, [env.id]);
        }
        toast.success('本地扩展已安装', assignCurrent ? '已分配到当前环境' : '可在扩展库中分配');
      }
      closeAdd(false);
      await load();
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  const openStore = async () => {
    if (!storeSource.trim()) {
      toast.warning('请先粘贴商店链接或扩展 ID');
      return;
    }
    try {
      await ipc.extensionOpenStore(env.id, storeSource.trim());
      toast.info('已在浏览器打开商店', '点击 Chrome 的「添加至 Chrome」完成原生安装');
    } catch (error) {
      toastError(error);
    }
  };

  const toggleEnabled = async (extension: Extension, enabled: boolean) => {
    try {
      await ipc.extensionUpdate(extension.id, { enabled });
      toast.success(enabled ? '扩展已启用' : '扩展已停用', env.runtime ? '重启浏览器后生效' : undefined);
      await load();
    } catch (error) {
      toastError(error);
    }
  };

  const openFolder = async (extension: Extension) => {
    try {
      await ipc.extensionOpenFolder(extension.id);
    } catch (error) {
      toastError(error);
    }
  };

  const applyAll = async (extension: Extension) => {
    try {
      const count = await ipc.extensionAssignAll(extension.id);
      toast.success('已应用到全部环境', `${count} 个环境，运行中的环境需重启后生效`);
      setApplyAllTarget(null);
      await load();
    } catch (error) {
      toastError(error);
    }
  };

  const openEdit = (extension: Extension) => {
    setEditTarget(extension);
    setEditName(extension.name);
    setEditDescription(extension.description);
  };

  const saveEdit = async () => {
    if (!editTarget) return;
    try {
      await ipc.extensionUpdate(editTarget.id, {
        name: editName.trim(),
        description: editDescription,
      });
      toast.success('扩展信息已更新');
      setEditTarget(null);
      await load();
    } catch (error) {
      toastError(error);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold text-content-main">浏览器扩展</h3>
          <p className="mt-0.5 text-xs text-content-weak">
            从 Chrome 应用商店在线安装，或加载本地解压目录；安装后进入全局扩展库，可分配到任意/全部环境。
          </p>
        </div>
        {canEdit ? (
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="primary"
              size="sm"
              onClick={() => {
                setAddMode('online');
                setAddOpen(true);
              }}
            >
              <Globe2 className="h-3.5 w-3.5" /> 在线安装
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => {
                setAddMode('local');
                setAddOpen(true);
              }}
            >
              <FolderPlus className="h-3.5 w-3.5" /> 本地安装
            </Button>
          </div>
        ) : null}
      </div>

      <Card className="overflow-hidden">
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div className="flex items-center gap-2">
            <Puzzle className="h-4 w-4 text-primary" />
            <span className="text-sm font-medium text-content-main">当前环境已分配</span>
            <Badge tone="primary">{assigned.length}</Badge>
          </div>
          {env.runtime ? <span className="text-[10px] text-content-hint">修改扩展后需重启浏览器生效</span> : null}
        </div>
        {loading ? (
          <div className="flex justify-center py-10"><Spinner /></div>
        ) : assigned.length === 0 ? (
          <EmptyState
            icon={<Package className="h-5 w-5" />}
            title="当前环境未分配扩展"
            description="从下方扩展库选择扩展，或在线安装一个新扩展。"
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
                {!extension.enabled ? <Badge tone="warning">已停用</Badge> : null}
                {canEdit ? (
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    title="移除当前环境"
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
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-4 py-3">
          <div>
            <p className="text-sm font-medium text-content-main">扩展库</p>
            <p className="mt-0.5 text-xs text-content-hint">
              全局扩展，任何环境都可以选择；下载的在线扩展保存在应用数据目录。
            </p>
          </div>
          <Badge tone="outline">{all.length} 个 · {envs.length} 个环境</Badge>
        </div>
        {all.length === 0 ? (
          <EmptyState
            icon={<Puzzle className="h-5 w-5" />}
            title="扩展库为空"
            description="在线安装 Chrome 应用商店扩展，或选择包含 manifest.json 的本地目录。"
          />
        ) : (
          <div className="divide-y divide-border">
            {all.map((extension) => {
              const assignedHere = assignedIds.has(extension.id);
              const allAssigned = envs.length > 0 && extension.assignedCount >= envs.length;
              return (
                <div key={extension.id} className={cn('flex flex-wrap items-center gap-3 px-4 py-3', !extension.enabled && 'opacity-60')}>
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-1.5">
                      <p className="truncate text-sm text-content-main">{extension.name}</p>
                      {extension.enabled ? <Badge tone="success">启用</Badge> : <Badge tone="warning">停用</Badge>}
                      {allAssigned ? <Badge tone="primary">全部环境</Badge> : <Badge tone="neutral">{extension.assignedCount} 个环境</Badge>}
                      {assignedHere ? <Badge tone="outline">当前环境</Badge> : null}
                    </div>
                    <p className="truncate font-mono text-[10px] text-content-hint" title={extension.path}>{extension.path}</p>
                    <p className="text-[10px] text-content-hint">
                      {extension.description || '未填写描述'} · 安装于 {formatDateShort(extension.createdAt)}
                    </p>
                  </div>
                  {canEdit ? (
                    <div className="flex flex-wrap items-center justify-end gap-1.5">
                      <div className="mr-1">
                        <Switch
                          checked={extension.enabled}
                          onCheckedChange={(enabled) => void toggleEnabled(extension, enabled)}
                        />
                      </div>
                      {assignedHere ? (
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
                          移除当前
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
                          分配到当前
                        </Button>
                      )}
                      <Button
                        variant="secondary"
                        size="sm"
                        disabled={allAssigned}
                        onClick={() => setApplyAllTarget(extension)}
                        title="应用到所有环境"
                      >
                        <Users className="h-3.5 w-3.5" /> {allAssigned ? '已应用全部' : '应用到全部'}
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        title="批量分配"
                        onClick={() => {
                          setAssignTarget(extension);
                          setAssignEnvIds([env.id]);
                        }}
                      >
                        <CheckCheck className="h-3.5 w-3.5" />
                      </Button>
                      <Button variant="ghost" size="icon-sm" title="打开扩展目录" onClick={() => void openFolder(extension)}>
                        <FolderOpen className="h-3.5 w-3.5" />
                      </Button>
                      <Button variant="ghost" size="icon-sm" title="编辑扩展信息" onClick={() => openEdit(extension)}>
                        <Pencil className="h-3.5 w-3.5" />
                      </Button>
                      <Button variant="ghost" size="icon-sm" className="text-danger" title="删除扩展" onClick={() => setDeleteTarget(extension)}>
                        <Trash2 className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  ) : null}
                </div>
              );
            })}
          </div>
        )}
      </Card>

      <div className="rounded-md border border-border bg-surface-secondary p-3 text-[11px] leading-5 text-content-weak">
        在线安装会从 Chrome 应用商店下载 CRX 并解压到扩展库，再由 <code className="font-mono">--load-extension</code> 注入。
        Chrome 137+ 品牌版已移除该启动参数；若扩展未加载，请使用「在浏览器中打开商店」手动添加，或使用 Chromium / Chrome for Testing。
      </div>

      <Dialog
        open={addOpen}
        onOpenChange={closeAdd}
        title="安装浏览器扩展"
        description="在线安装或选择本地解压目录；需要包含 manifest.json。"
        width="max-w-xl"
        footer={
          <>
            <Button variant="secondary" onClick={() => closeAdd(false)} disabled={busy}>取消</Button>
            <Button
              variant="primary"
              loading={busy}
              disabled={addMode === 'online' ? !storeSource.trim() : !path}
              onClick={() => void install()}
            >
              {addMode === 'online' ? '在线安装' : '安装本地扩展'}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-1 rounded-lg border border-border bg-surface-secondary p-1">
            <button
              type="button"
              className={cn(
                'flex items-center justify-center gap-1.5 rounded-md px-3 py-2 text-xs font-medium transition-colors',
                addMode === 'online' ? 'bg-surface-base text-content-main shadow-sm' : 'text-content-weak hover:text-content-main',
              )}
              onClick={() => setAddMode('online')}
            >
              <Globe2 className="h-3.5 w-3.5" /> 在线安装
            </button>
            <button
              type="button"
              className={cn(
                'flex items-center justify-center gap-1.5 rounded-md px-3 py-2 text-xs font-medium transition-colors',
                addMode === 'local' ? 'bg-surface-base text-content-main shadow-sm' : 'text-content-weak hover:text-content-main',
              )}
              onClick={() => setAddMode('local')}
            >
              <FolderPlus className="h-3.5 w-3.5" /> 本地目录
            </button>
          </div>

          {addMode === 'online' ? (
            <>
              <Field
                label="Chrome 应用商店链接 / 扩展 ID"
                required
                hint="支持 chromewebstore.google.com 链接或 32 位扩展 ID，例如 dhdgffkkebhmkfjojejmpbldmpobfkfo"
              >
                <Input
                  value={storeSource}
                  onChange={(event) => setStoreSource(event.target.value)}
                  placeholder="https://chromewebstore.google.com/detail/.../dhdgffkkebhmkfjojejmpbldmpobfkfo"
                />
              </Field>
              <div className="flex flex-wrap items-center gap-2">
                <Button variant="secondary" size="sm" onClick={() => void openStore()}>
                  <Globe2 className="h-3.5 w-3.5" /> 在浏览器中打开商店
                </Button>
                <span className="text-[10px] text-content-hint">品牌版 Chrome 137+ 建议用此方式手动安装</span>
              </div>
              <div className="rounded-md border border-border bg-surface-secondary p-3 text-[11px] leading-5 text-content-weak">
                在线安装会下载 CRX、校验包结构并解压到 <code className="font-mono">extensions/store</code>，随后作为全局扩展加入扩展库。
                需本机能访问 Chrome 应用商店。
              </div>
            </>
          ) : (
            <>
              <Field label="扩展目录" required>
                <div className="flex gap-2">
                  <Input value={path} readOnly placeholder="选择包含 manifest.json 的解压目录" />
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
            </>
          )}

          <div className="rounded-md border border-border p-3">
            <Switch
              checked={assignCurrent}
              onCheckedChange={setAssignCurrent}
              label="安装后分配到当前环境"
              description={`${env.name} · 也可稍后在扩展库中应用到全部环境`}
            />
          </div>
        </div>
      </Dialog>

      <Dialog
        open={!!editTarget}
        onOpenChange={(value) => !value && setEditTarget(null)}
        title="编辑扩展"
        description="修改扩展在扩展库中的显示信息，不会修改扩展源文件。"
        width="max-w-lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setEditTarget(null)}>取消</Button>
            <Button variant="primary" disabled={!editName.trim()} onClick={() => void saveEdit()}>保存</Button>
          </>
        }
      >
        <div className="space-y-3">
          <Field label="显示名称" required>
            <Input value={editName} onChange={(event) => setEditName(event.target.value)} />
          </Field>
          <Field label="描述">
            <Textarea value={editDescription} onChange={(event) => setEditDescription(event.target.value)} />
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
        <div className="mb-2 flex items-center justify-between text-xs">
          <span className="text-content-hint">已选 {assignEnvIds.length} / {envs.length}</span>
          <div className="flex gap-1">
            <Button variant="ghost" size="sm" onClick={() => setAssignEnvIds(envs.map((item) => item.id))}>
              全选
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setAssignEnvIds([])}>
              清空
            </Button>
          </div>
        </div>
        <div className="max-h-72 space-y-1 overflow-y-auto rounded-md border border-border p-2">
          {envs.map((item) => (
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
              {item.id === env.id ? <Badge tone="outline">当前</Badge> : null}
            </label>
          ))}
        </div>
      </Dialog>

      <ConfirmDialog
        open={!!applyAllTarget}
        onOpenChange={(value) => !value && setApplyAllTarget(null)}
        title="应用到全部环境"
        description={`将「${applyAllTarget?.name ?? ''}」分配到全部 ${envs.length} 个环境；运行中的环境需重启浏览器后生效。`}
        confirmLabel="应用到全部"
        onConfirm={async () => {
          if (applyAllTarget) await applyAll(applyAllTarget);
        }}
      />

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(value) => !value && setDeleteTarget(null)}
        title="删除扩展"
        description={`从扩展库删除「${deleteTarget?.name ?? ''}」，并解除所有环境分配。在线下载的扩展目录会一并删除，本地目录不受影响。`}
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
