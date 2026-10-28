import * as React from 'react';
import { CalendarClock, Cpu, FolderOpen, HardDrive, Save, Tags, UserCog } from 'lucide-react';
import type { EnvironmentView, Group } from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { toastError, useToastStore } from '@/store/toast';
import { formatDate, parseTags, testEnvLabels } from '@/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardHeader } from '@/components/ui/card';
import { ColorPicker } from '@/components/ui/color-picker';
import { CopyButton } from '@/components/ui/misc';
import { Field, Input, Select, Textarea } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';

export function OverviewPanel({
  env,
  groups,
  onUpdated,
}: {
  env: EnvironmentView;
  groups: Group[];
  onUpdated: (env: EnvironmentView) => void;
}) {
  const toast = useToastStore();
  const canEdit = useAuthStore((state) => state.can('edit_env'));
  const [saving, setSaving] = React.useState(false);
  const [form, setForm] = React.useState({
    name: env.name,
    groupId: env.groupId ?? '',
    status: env.status,
    project: env.project,
    testEnv: env.testEnv,
    accountRole: env.accountRole,
    version: env.version,
    caseId: env.caseId,
    tags: env.tags.join(', '),
    notes: env.notes,
    borderColor: env.borderColor,
    borderWidth: env.borderWidth ?? 3,
    autoRestart: env.autoRestart,
    memoryLimitMb: env.memoryLimitMb?.toString() ?? '',
    idleTimeoutMin: env.idleTimeoutMin?.toString() ?? '',
    chromiumPath: env.chromiumPath ?? '',
  });

  React.useEffect(() => {
    setForm({
      name: env.name,
      groupId: env.groupId ?? '',
      status: env.status,
      project: env.project,
      testEnv: env.testEnv,
      accountRole: env.accountRole,
      version: env.version,
      caseId: env.caseId,
      tags: env.tags.join(', '),
      notes: env.notes,
      borderColor: env.borderColor,
      borderWidth: env.borderWidth ?? 3,
      autoRestart: env.autoRestart,
      memoryLimitMb: env.memoryLimitMb?.toString() ?? '',
      idleTimeoutMin: env.idleTimeoutMin?.toString() ?? '',
      chromiumPath: env.chromiumPath ?? '',
    });
  }, [env.id, env.updatedAt]);

  const update = <K extends keyof typeof form>(key: K, value: (typeof form)[K]) =>
    setForm((current) => ({ ...current, [key]: value }));

  const save = async () => {
    if (!form.name.trim()) {
      toast.warning('环境名称不能为空');
      return;
    }
    setSaving(true);
    try {
      const updated = await ipc.envUpdate(env.id, {
        name: form.name.trim(),
        groupId: form.groupId || null,
        status: form.status,
        project: form.project,
        testEnv: form.testEnv,
        accountRole: form.accountRole,
        version: form.version,
        caseId: form.caseId,
        tags: parseTags(form.tags),
        notes: form.notes,
        borderColor: form.borderColor,
        borderWidth: form.borderWidth,
        autoRestart: form.autoRestart,
        memoryLimitMb: form.memoryLimitMb ? Number(form.memoryLimitMb) : null,
        idleTimeoutMin: form.idleTimeoutMin ? Number(form.idleTimeoutMin) : null,
        chromiumPath: form.chromiumPath.trim() || null,
      });
      onUpdated(updated);
      toast.success('环境信息已保存');
    } catch (error) {
      toastError(error);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_320px]">
      <Card>
        <CardHeader
          title="环境基础信息"
          description="项目、测试环境、角色、版本和用例 ID 用于筛选与审计关联。"
          actions={
            canEdit ? (
              <Button variant="primary" size="sm" loading={saving} onClick={() => void save()}>
                <Save className="h-3.5 w-3.5" /> 保存
              </Button>
            ) : null
          }
        />
        <CardBody>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
            <Field label="环境名称" required className="col-span-2 md:col-span-3">
              <Input value={form.name} onChange={(event) => update('name', event.target.value)} disabled={!canEdit} />
            </Field>
            <Field label="分组">
              <Select value={form.groupId} onChange={(event) => update('groupId', event.target.value)} disabled={!canEdit} placeholder="未分组">
                {groups.map((group) => (
                  <option key={group.id} value={group.id}>{group.name}</option>
                ))}
              </Select>
            </Field>
            <Field label="状态">
              <Select value={form.status} onChange={(event) => update('status', event.target.value as typeof form.status)} disabled={!canEdit}>
                <option value="active">已启用</option>
                <option value="disabled">已停用</option>
                <option value="archived">已归档</option>
              </Select>
            </Field>
            <Field label="项目">
              <Input value={form.project} onChange={(event) => update('project', event.target.value)} disabled={!canEdit} placeholder="交易中台" />
            </Field>
            <Field label="测试环境">
              <Select value={form.testEnv} onChange={(event) => update('testEnv', event.target.value)} disabled={!canEdit} placeholder="未指定">
                {Object.entries(testEnvLabels).map(([value, label]) => (
                  <option key={value} value={value}>{label}</option>
                ))}
              </Select>
            </Field>
            <Field label="账号角色">
              <Input value={form.accountRole} onChange={(event) => update('accountRole', event.target.value)} disabled={!canEdit} placeholder="运营 / 审核员" />
            </Field>
            <Field label="版本号">
              <Input value={form.version} onChange={(event) => update('version', event.target.value)} disabled={!canEdit} placeholder="V2.8.1" />
            </Field>
            <Field label="测试用例 ID">
              <Input value={form.caseId} onChange={(event) => update('caseId', event.target.value)} disabled={!canEdit} placeholder="CASE-1024" />
            </Field>
            <Field label="环境标识色" className="col-span-2 md:col-span-3" hint="列表、详情、运行栏与浏览器窗口边框使用该颜色和粗细（Windows 窗口边框）">
              <ColorPicker
                value={form.borderColor}
                onChange={(value) => update('borderColor', value)}
                width={form.borderWidth}
                onWidthChange={(value) => update('borderWidth', value)}
              />
            </Field>
            <Field label="标签" className="col-span-2 md:col-span-3" hint="逗号或空格分隔">              <Input value={form.tags} onChange={(event) => update('tags', event.target.value)} disabled={!canEdit} />
            </Field>
            <Field label="备注" className="col-span-2 md:col-span-3">
              <Textarea value={form.notes} onChange={(event) => update('notes', event.target.value)} disabled={!canEdit} />
            </Field>
            <Field label="内存上限 MB" hint="留空使用全局阈值">
              <Input type="number" min={128} value={form.memoryLimitMb} onChange={(event) => update('memoryLimitMb', event.target.value)} disabled={!canEdit} />
            </Field>
            <Field label="闲置回收 分钟" hint="留空使用全局阈值">
              <Input type="number" min={0} value={form.idleTimeoutMin} onChange={(event) => update('idleTimeoutMin', event.target.value)} disabled={!canEdit} />
            </Field>
            <Field label="Chromium 路径" hint="留空使用全局设置或内置内核">
              <Input value={form.chromiumPath} onChange={(event) => update('chromiumPath', event.target.value)} disabled={!canEdit} placeholder="C:\\...\\chrome.exe" />
            </Field>
            <div className="col-span-2 rounded-md border border-border bg-surface-secondary p-3 md:col-span-3">
              <Switch
                checked={form.autoRestart}
                onCheckedChange={(value) => update('autoRestart', value)}
                disabled={!canEdit}
                label="进程崩溃后自动重启"
                description="全局设置也可统一开启自动重启；单环境开关优先级更高。"
              />
            </div>
          </div>
        </CardBody>
      </Card>

      <div className="space-y-3">
        <Card>
          <CardHeader title="运行摘要" />
          <CardBody className="space-y-2 text-xs">
            <div className="flex items-center justify-between">
              <span className="text-content-weak">状态</span>
              {env.runtime ? (
                <Badge tone="success">运行中 · PID {env.runtime.pid}</Badge>
              ) : (
                <Badge tone="neutral">未运行</Badge>
              )}
            </div>
            <div className="flex items-center justify-between">
              <span className="text-content-weak">账号 / 快照</span>
              <span className="text-content-main">{env.accountCount} / {env.snapshotCount}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-content-weak">代理</span>
              <span className="max-w-[180px] truncate text-content-main">{env.proxy?.name ?? '直连'}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-content-weak">出口 IP</span>
              <span className="max-w-[180px] truncate font-mono text-content-main">{env.ipAddress ?? '未检测'}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-content-weak">最后打开</span>
              <span className="text-content-main">{env.lastOpenedAt ? formatDate(env.lastOpenedAt) : '从未'}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-content-weak">内存 / CPU</span>
              <span className="text-content-main">
                {env.runtime ? `${env.runtime.memoryMb} MB · ${env.runtime.cpuPercent.toFixed(1)}%` : '—'}
              </span>
            </div>
            {env.runtime ? (
              <div className="rounded-md border border-border bg-surface-secondary p-2">
                <p className="text-[10px] text-content-hint">当前页面</p>
                <p className="mt-0.5 break-all font-mono text-[10px] text-content-main">{env.runtime.currentUrl || 'about:blank'}</p>
              </div>
            ) : null}
          </CardBody>
        </Card>

        <Card>
          <CardHeader title="隔离详情" />
          <CardBody className="space-y-2.5 text-[11px]">
            <div>
              <p className="flex items-center gap-1 text-content-hint"><FolderOpen className="h-3 w-3" /> Profile 目录</p>
              <div className="mt-0.5 flex items-center gap-1">
                <p className="min-w-0 flex-1 break-all font-mono text-[10px] text-content-main">{env.profileDir}</p>
                <CopyButton value={env.profileDir} />
              </div>
            </div>
            <div>
              <p className="flex items-center gap-1 text-content-hint"><Cpu className="h-3 w-3" /> 内核</p>
              <p className="mt-0.5 text-content-main">Chromium · 独立进程 / 随机调试端口</p>
            </div>
            <div>
              <p className="flex items-center gap-1 text-content-hint"><HardDrive className="h-3 w-3" /> 环境 ID</p>
              <p className="mt-0.5 break-all font-mono text-[10px] text-content-main">{env.id}</p>
            </div>
            <div>
              <p className="flex items-center gap-1 text-content-hint"><CalendarClock className="h-3 w-3" /> 创建 / 更新</p>
              <p className="mt-0.5 text-content-main">{formatDate(env.createdAt)}</p>
              <p className="text-content-hint">{formatDate(env.updatedAt)}</p>
            </div>
            <div>
              <p className="flex items-center gap-1 text-content-hint"><Tags className="h-3 w-3" /> 标签</p>
              <div className="mt-1 flex flex-wrap gap-1">
                {env.tags.length ? env.tags.map((tag) => <Badge key={tag} tone="neutral">#{tag}</Badge>) : <span className="text-content-hint">无</span>}
              </div>
            </div>
            <div>
              <p className="flex items-center gap-1 text-content-hint"><UserCog className="h-3 w-3" /> 指纹快照</p>
              <p className="mt-0.5 font-mono text-[10px] text-content-main">{env.fingerprintSnapshotId ? env.fingerprintSnapshotId.slice(0, 12) : '未绑定快照'}</p>
            </div>
          </CardBody>
        </Card>
      </div>
    </div>
  );
}
