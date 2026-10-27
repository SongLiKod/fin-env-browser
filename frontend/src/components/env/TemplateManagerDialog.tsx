import * as React from 'react';
import {
  ArrowLeft,
  Boxes,
  Globe2,
  Layers,
  LayoutTemplate,
  Pencil,
  Play,
  Plus,
  Trash2,
} from 'lucide-react';
import { ipc } from '@/ipc/client';
import type {
  BatchCreateResult,
  EnvironmentTemplate,
  Group,
  Proxy,
} from '@/ipc/types';
import { useAuthStore } from '@/store/auth';
import { toastError, useToastStore } from '@/store/toast';
import { formatDateShort, testEnvLabels } from '@/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Dialog } from '@/components/ui/dialog';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { Field, Input, Select, Textarea } from '@/components/ui/input';

type Mode =
  | { kind: 'list' }
  | { kind: 'batch'; template: EnvironmentTemplate }
  | { kind: 'edit'; template: EnvironmentTemplate };

const proxyModeLabel: Record<string, string> = {
  none: '直连',
  fixed: '固定代理',
  pool: '代理池轮询',
};

function previewName(prefix: string, fallback: string, index: number) {
  return `${prefix.trim() || fallback} ${String(index).padStart(2, '0')}`;
}

export function TemplateManagerDialog({
  open,
  onOpenChange,
  groups,
  proxies,
  onChanged,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  groups: Group[];
  proxies: Proxy[];
  onChanged: () => void;
}) {
  const toast = useToastStore();
  const can = useAuthStore((state) => state.can);
  const [mode, setMode] = React.useState<Mode>({ kind: 'list' });
  const [templates, setTemplates] = React.useState<EnvironmentTemplate[]>([]);
  const [loading, setLoading] = React.useState(false);
  const [batchCount, setBatchCount] = React.useState('5');
  const [batchPrefix, setBatchPrefix] = React.useState('');
  const [batchStart, setBatchStart] = React.useState('1');
  const [batchBusy, setBatchBusy] = React.useState(false);
  const [batchResult, setBatchResult] = React.useState<BatchCreateResult | null>(null);
  const [editName, setEditName] = React.useState('');
  const [editDescription, setEditDescription] = React.useState('');
  const [editProxyMode, setEditProxyMode] = React.useState<'none' | 'fixed' | 'pool'>('none');
  const [editProxyId, setEditProxyId] = React.useState('');
  const [editBusy, setEditBusy] = React.useState(false);
  const [deleteTarget, setDeleteTarget] = React.useState<EnvironmentTemplate | null>(null);
  const [deleteBusy, setDeleteBusy] = React.useState(false);

  const groupNames = React.useMemo(
    () => new Map(groups.map((group) => [group.id, group.name])),
    [groups],
  );
  const proxyNames = React.useMemo(
    () => new Map(proxies.map((proxy) => [proxy.id, proxy.name])),
    [proxies],
  );

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      setTemplates(await ipc.templateList());
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    if (!open) return;
    setMode({ kind: 'list' });
    setBatchResult(null);
    void load();
  }, [open, load]);

  const startBatch = (template: EnvironmentTemplate) => {
    setBatchPrefix(template.name);
    setBatchCount('5');
    setBatchStart('1');
    setBatchResult(null);
    setMode({ kind: 'batch', template });
  };

  const startEdit = (template: EnvironmentTemplate) => {
    setEditName(template.name);
    setEditDescription(template.description);
    setEditProxyMode(
      template.proxyMode === 'fixed' || template.proxyMode === 'pool'
        ? template.proxyMode
        : 'none',
    );
    setEditProxyId(template.proxyId ?? '');
    setMode({ kind: 'edit', template });
  };

  const submitBatch = async (event: React.FormEvent) => {
    event.preventDefault();
    if (mode.kind !== 'batch') return;
    const count = Number(batchCount);
    if (!Number.isInteger(count) || count < 1 || count > 50) {
      toast.warning('创建数量需为 1-50');
      return;
    }
    setBatchBusy(true);
    setBatchResult(null);
    try {
      const result = await ipc.templateBatchCreate({
        templateId: mode.template.id,
        count,
        namePrefix: batchPrefix.trim() || null,
        startIndex: Number(batchStart) || 1,
      });
      setBatchResult(result);
      if (result.created.length > 0) {
        toast.success(
          '批量创建完成',
          `${result.created.length} 个环境已创建${result.errors.length ? `，${result.errors.length} 个失败` : ''}`,
        );
        onChanged();
        await load();
      } else {
        toast.warning('没有环境被创建', result.errors[0]);
      }
    } catch (error) {
      toastError(error);
    } finally {
      setBatchBusy(false);
    }
  };

  const submitEdit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (mode.kind !== 'edit') return;
    if (!editName.trim()) {
      toast.warning('模板名称不能为空');
      return;
    }
    if (editProxyMode === 'fixed' && !editProxyId) {
      toast.warning('请选择固定代理');
      return;
    }
    setEditBusy(true);
    try {
      const updated = await ipc.templateUpdate(mode.template.id, {
        name: editName.trim(),
        description: editDescription,
        proxyMode: editProxyMode,
        proxyId: editProxyMode === 'fixed' ? editProxyId : null,
      });
      setTemplates((current) =>
        current.map((template) => (template.id === updated.id ? updated : template)),
      );
      setMode({ kind: 'list' });
      toast.success('模板已更新', updated.name);
      onChanged();
    } catch (error) {
      toastError(error);
    } finally {
      setEditBusy(false);
    }
  };

  const confirmDelete = async () => {
    if (!deleteTarget) return;
    setDeleteBusy(true);
    try {
      await ipc.templateDelete(deleteTarget.id, true);
      setTemplates((current) => current.filter((template) => template.id !== deleteTarget.id));
      setDeleteTarget(null);
      toast.success('模板已删除', deleteTarget.name);
      onChanged();
    } catch (error) {
      toastError(error);
    } finally {
      setDeleteBusy(false);
    }
  };

  const count = Math.max(1, Math.min(50, Number(batchCount) || 1));
  const startIndex = Math.max(1, Number(batchStart) || 1);
  const previews = Array.from({ length: Math.min(count, 3) }, (_, index) =>
    previewName(batchPrefix, mode.kind === 'batch' ? mode.template.name : '', startIndex + index),
  );

  const title =
    mode.kind === 'batch'
      ? `批量创建 · ${mode.template.name}`
      : mode.kind === 'edit'
        ? '编辑模板'
        : '环境模板';
  const description =
    mode.kind === 'batch'
      ? '按模板生成多个独立环境，指纹、代理策略与扩展会自动复用。'
      : mode.kind === 'edit'
        ? '模板名称与说明可随时调整，不影响已创建的环境。'
        : '从环境保存的配置模板；批量创建时自动生成独立 profile 与指纹副本。';

  const footer =
    mode.kind === 'batch' ? (
      <>
        <Button variant="ghost" onClick={() => setMode({ kind: 'list' })} disabled={batchBusy}>
          <ArrowLeft className="h-4 w-4" /> 返回列表
        </Button>
        <span className="mr-auto text-[11px] text-content-hint">
          预计创建 {count} 个环境 · 序号 {startIndex} - {startIndex + count - 1}
        </span>
        <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={batchBusy}>
          关闭
        </Button>
        <Button
          variant="primary"
          type="submit"
          form="template-batch-form"
          loading={batchBusy}
        >
          <Plus className="h-4 w-4" /> 创建 {count} 个环境
        </Button>
      </>
    ) : mode.kind === 'edit' ? (
      <>
        <Button variant="secondary" onClick={() => setMode({ kind: 'list' })} disabled={editBusy}>
          取消
        </Button>
        <Button variant="primary" type="submit" form="template-edit-form" loading={editBusy}>
          保存模板
        </Button>
      </>
    ) : (
      <>
        <span className="mr-auto text-[11px] text-content-hint">
          在环境详情页点击「保存为模板」即可新增模板。
        </span>
        <Button variant="secondary" onClick={() => onOpenChange(false)}>
          关闭
        </Button>
      </>
    );

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={onOpenChange}
        title={title}
        description={description}
        width="max-w-3xl"
        footer={footer}
      >
        {mode.kind === 'list' ? (
          <div className="space-y-2.5">
            {loading && templates.length === 0 ? (
              <p className="py-12 text-center text-xs text-content-hint">正在加载模板…</p>
            ) : templates.length === 0 ? (
              <div className="rounded-lg border border-dashed border-border py-12 text-center">
                <LayoutTemplate className="mx-auto h-8 w-8 text-content-hint" />
                <p className="mt-3 text-sm font-medium text-content-main">还没有环境模板</p>
                <p className="mt-1 text-xs text-content-weak">
                  打开一个配置好的环境，点击「保存为模板」，之后即可批量派生环境。
                </p>
              </div>
            ) : (
              templates.map((template) => {
                const groupName = template.groupId ? groupNames.get(template.groupId) : null;
                return (
                  <div
                    key={template.id}
                    className="rounded-lg border border-border bg-surface-base p-3.5 shadow-sm"
                  >
                    <div className="flex flex-wrap items-start justify-between gap-2">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <span
                            className="h-2.5 w-2.5 rounded-full ring-1 ring-black/10"
                            style={{ backgroundColor: template.borderColor || 'var(--color-primary)' }}
                          />
                          <p className="truncate text-sm font-semibold text-content-main">
                            {template.name}
                          </p>
                          <Badge tone="outline">{template.fingerprint.clientHints.platform}</Badge>
                          {groupName ? <Badge tone="neutral">{groupName}</Badge> : null}
                        </div>
                        {template.description ? (
                          <p className="mt-1 line-clamp-2 text-xs text-content-weak">
                            {template.description}
                          </p>
                        ) : null}
                      </div>
                      <div className="flex shrink-0 items-center gap-1">
                        {can('create_env') ? (
                          <Button
                            variant="soft"
                            size="sm"
                            onClick={() => startBatch(template)}
                          >
                            <Play className="h-3.5 w-3.5" /> 批量创建
                          </Button>
                        ) : null}
                        {can('edit_env') ? (
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            title="编辑模板"
                            onClick={() => startEdit(template)}
                          >
                            <Pencil className="h-3.5 w-3.5" />
                          </Button>
                        ) : null}
                        {can('edit_env') ? (
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            className="text-danger"
                            title="删除模板"
                            onClick={() => setDeleteTarget(template)}
                          >
                            <Trash2 className="h-3.5 w-3.5" />
                          </Button>
                        ) : null}
                      </div>
                    </div>
                    <div className="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-content-hint">
                      <span className="flex items-center gap-1">
                        <Boxes className="h-3 w-3" />
                        {template.project || '未指定项目'}
                        {template.testEnv ? ` · ${testEnvLabels[template.testEnv] ?? template.testEnv}` : ''}
                      </span>
                      <span className="flex items-center gap-1">
                        <Globe2 className="h-3 w-3" />
                        {proxyModeLabel[template.proxyMode] ?? template.proxyMode}
                        {template.proxyMode === 'fixed' && template.proxyId
                          ? ` · ${proxyNames.get(template.proxyId) ?? '已删除代理'}`
                          : ''}
                      </span>
                      <span className="flex items-center gap-1">
                        <Layers className="h-3 w-3" />
                        {template.extensionIds.length} 个扩展
                      </span>
                      <span className="ml-auto">更新于 {formatDateShort(template.updatedAt)}</span>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        ) : mode.kind === 'batch' ? (
          <div className="space-y-4">
            <form id="template-batch-form" className="grid grid-cols-3 gap-3" onSubmit={submitBatch}>
              <Field label="创建数量" hint="1-50 个">
                <Input
                  type="number"
                  min={1}
                  max={50}
                  value={batchCount}
                  onChange={(event) => setBatchCount(event.target.value)}
                  autoFocus
                />
              </Field>
              <Field label="起始序号" hint="用于命名">
                <Input
                  type="number"
                  min={1}
                  max={999999}
                  value={batchStart}
                  onChange={(event) => setBatchStart(event.target.value)}
                />
              </Field>
              <Field label="环境名前缀" hint="留空则使用模板名">
                <Input
                  value={batchPrefix}
                  onChange={(event) => setBatchPrefix(event.target.value)}
                  placeholder={mode.template.name}
                />
              </Field>
            </form>
            <div className="rounded-lg border border-border bg-surface-secondary/60 p-3">
              <p className="text-[11px] font-medium text-content-weak">命名预览</p>
              <div className="mt-1.5 flex flex-wrap gap-1.5">
                {previews.map((name) => (
                  <Badge key={name} tone="neutral" className="font-mono">
                    {name}
                  </Badge>
                ))}
                {count > previews.length ? (
                  <span className="text-[11px] text-content-hint">
                    … 共 {count} 个
                  </span>
                ) : null}
              </div>
              <p className="mt-2 text-[11px] text-content-hint">
                模板配置：{proxyModeLabel[mode.template.proxyMode] ?? mode.template.proxyMode} ·{' '}
                {mode.template.extensionIds.length} 个扩展 ·{' '}
                {mode.template.tags.length ? mode.template.tags.map((tag) => `#${tag}`).join(' ') : '无标签'}
              </p>
            </div>
            {batchResult ? (
              <div
                className={
                  batchResult.errors.length
                    ? 'rounded-lg border border-[color-mix(in_srgb,var(--warning)_35%,transparent)] bg-[color-mix(in_srgb,var(--warning)_8%,transparent)] p-3'
                    : 'rounded-lg border border-[color-mix(in_srgb,var(--success)_35%,transparent)] bg-[color-mix(in_srgb,var(--success)_8%,transparent)] p-3'
                }
              >
                <p className="text-xs font-medium text-content-main">
                  已创建 {batchResult.created.length} 个环境
                  {batchResult.errors.length ? `，${batchResult.errors.length} 个失败` : ''}
                </p>
                {batchResult.created.length ? (
                  <p className="mt-1 text-[11px] text-content-weak">
                    {batchResult.created.map((env) => env.name).join('、')}
                  </p>
                ) : null}
                {batchResult.errors.length ? (
                  <ul className="mt-1.5 space-y-1 text-[11px] text-danger">
                    {batchResult.errors.map((error, index) => (
                      <li key={`${index}-${error}`}>{error}</li>
                    ))}
                  </ul>
                ) : null}
              </div>
            ) : (
              <p className="text-[11px] text-content-hint">
                每个环境都会获得独立的 profile 目录；代理池模式会按启用代理顺序轮询分配。
              </p>
            )}
          </div>
        ) : (
          <form id="template-edit-form" className="space-y-3" onSubmit={submitEdit}>
            <Field label="模板名称" required>
              <Input
                value={editName}
                onChange={(event) => setEditName(event.target.value)}
                autoFocus
              />
            </Field>
            <Field label="模板说明">
              <Textarea
                value={editDescription}
                onChange={(event) => setEditDescription(event.target.value)}
                placeholder="例如：订单中心测试账号标准配置"
              />
            </Field>
            <div className="grid grid-cols-2 gap-3">
              <Field label="代理策略">
                <Select
                  value={editProxyMode}
                  onChange={(event) =>
                    setEditProxyMode(event.target.value as 'none' | 'fixed' | 'pool')
                  }
                >
                  <option value="none">直连</option>
                  <option value="fixed">固定代理</option>
                  <option value="pool">代理池轮询</option>
                </Select>
              </Field>
              <Field
                label="固定代理"
                hint={editProxyMode === 'pool' ? '代理池按启用代理顺序轮询' : undefined}
              >
                <Select
                  value={editProxyId}
                  disabled={editProxyMode !== 'fixed'}
                  onChange={(event) => setEditProxyId(event.target.value)}
                  placeholder={editProxyMode === 'fixed' ? '选择代理' : '仅固定代理模式可用'}
                >
                  {proxies
                    .filter((proxy) => proxy.enabled)
                    .map((proxy) => (
                      <option key={proxy.id} value={proxy.id}>
                        {proxy.name} · {proxy.protocol}://{proxy.host}:{proxy.port}
                      </option>
                    ))}
                </Select>
              </Field>
            </div>
            <div className="rounded-lg border border-border bg-surface-secondary/60 p-3 text-[11px] text-content-hint">
              指纹：{mode.template.fingerprint.clientHints.platform} ·{' '}
              {mode.template.fingerprint.timezone} ·{' '}
              {mode.template.fingerprint.screen.width}×{mode.template.fingerprint.screen.height}
              <br />
              配置：{proxyModeLabel[mode.template.proxyMode] ?? mode.template.proxyMode} ·{' '}
              {mode.template.extensionIds.length} 个扩展 · 标识宽度 {mode.template.borderWidth}px
            </div>
          </form>
        )}
      </Dialog>

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(value) => !value && setDeleteTarget(null)}
        title="删除环境模板"
        description={`将删除模板「${deleteTarget?.name ?? ''}」。已由该模板创建的环境不受影响。`}
        confirmLabel="删除模板"
        requiredText="DELETE"
        busy={deleteBusy}
        onConfirm={confirmDelete}
      />
    </>
  );
}

export function SaveTemplateDialog({
  open,
  onOpenChange,
  env,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  env: { id: string; name: string };
  onSaved: (template: EnvironmentTemplate) => void;
}) {
  const toast = useToastStore();
  const [name, setName] = React.useState(`${env.name} 模板`);
  const [description, setDescription] = React.useState('');
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    if (!open) return;
    setName(`${env.name} 模板`);
    setDescription('');
  }, [open, env.name]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim()) {
      toast.warning('模板名称不能为空');
      return;
    }
    setBusy(true);
    try {
      const template = await ipc.templateCreateFromEnv({
        envId: env.id,
        name: name.trim(),
        description: description.trim() || null,
      });
      toast.success('已保存为模板', template.name);
      onSaved(template);
      onOpenChange(false);
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="保存为环境模板"
      description="保存当前环境的指纹、代理策略、扩展与业务字段，用于后续批量创建。"
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button variant="primary" type="submit" form="save-template-form" loading={busy}>
            <LayoutTemplate className="h-4 w-4" /> 保存模板
          </Button>
        </>
      }
    >
      <form id="save-template-form" className="space-y-3" onSubmit={submit}>
        <Field label="模板名称" required>
          <Input value={name} onChange={(event) => setName(event.target.value)} autoFocus />
        </Field>
        <Field label="模板说明">
          <Textarea
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder="描述该模板适用的项目、账号角色或测试场景"
          />
        </Field>
        <div className="rounded-lg border border-border bg-surface-secondary/60 p-3 text-[11px] text-content-hint">
          将保存：自洽指纹、标识色/宽度、业务字段、分组、固定代理、已启用扩展与资源限制。
        </div>
      </form>
    </Dialog>
  );
}
