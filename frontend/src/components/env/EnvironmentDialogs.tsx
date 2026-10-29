import * as React from 'react';
import { Download, FolderOpen, Layers, Plus, Trash2, Upload } from 'lucide-react';
import type {
  CreateEnvironmentInput,
  EnvironmentView,
  Group,
  Proxy,
} from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { parseTags } from '@/utils';
import {
  envPackageFilter,
  pickFile,
  saveFile,
} from '@/utils/dialog';
import { toastError, useToastStore } from '@/store/toast';
import { Button } from '@/components/ui/button';
import { ColorPicker } from '@/components/ui/color-picker';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input, Select, Textarea } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';

export function CreateEnvironmentDialog({
  open,
  onOpenChange,
  groups,
  proxies,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  groups: Group[];
  proxies: Proxy[];
  onCreated: (env: EnvironmentView) => void;
}) {
  const toast = useToastStore();
  const [busy, setBusy] = React.useState(false);
  const [form, setForm] = React.useState({
    name: '',
    groupId: '',
    proxyId: '',
    project: '',
    testEnv: '',
    accountRole: '',
    version: '',
    caseId: '',
    tags: '',
    notes: '',
    launchArgs: '',
    badgeText: '',
    borderColor: '',
    borderWidth: 3,
    templateOs: 'win11',
    autoRestart: false,
    memoryLimitMb: '',
    idleTimeoutMin: '',
  });

  const update = (key: keyof typeof form, value: string | boolean | number) =>
    setForm((current) => ({ ...current, [key]: value }));

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!form.name.trim()) {
      toast.warning('请填写环境名称');
      return;
    }
    setBusy(true);
    try {
      const input: CreateEnvironmentInput = {
        name: form.name.trim(),
        groupId: form.groupId || null,
        proxyId: form.proxyId || null,
        project: form.project,
        testEnv: form.testEnv,
        accountRole: form.accountRole,
        version: form.version,
        caseId: form.caseId,
        tags: parseTags(form.tags),
        notes: form.notes,
        chromiumLaunchArgs: form.launchArgs.split('\n'),
        taskbarBadgeText: form.badgeText,
        borderColor: form.borderColor,
        borderWidth: form.borderWidth,
        templateOs: form.templateOs,
        autoRestart: form.autoRestart,
        memoryLimitMb: form.memoryLimitMb ? Number(form.memoryLimitMb) : null,
        idleTimeoutMin: form.idleTimeoutMin ? Number(form.idleTimeoutMin) : null,
      };
      const env = await ipc.envCreate(input);
      toast.success('环境已创建', `${env.name} 已生成自洽指纹模板`);
      onCreated(env);
      onOpenChange(false);
      setForm((current) => ({ ...current, name: '', project: '', caseId: '', tags: '', notes: '', launchArgs: '', badgeText: '' }));
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
      title="新建指纹环境"
      description="自动生成一套自洽的 Chromium 指纹，创建后仍可精细调整。"
      width="max-w-2xl"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button variant="primary" loading={busy} type="submit" form="create-env-form">
            创建环境
          </Button>
        </>
      }
    >
      <form id="create-env-form" className="grid grid-cols-2 gap-3" onSubmit={submit}>
        <Field label="环境名称" required className="col-span-2">
          <Input
            value={form.name}
            autoFocus
            onChange={(event) => update('name', event.target.value)}
            placeholder="例如：订单中心 - 测试账号 A"
          />
        </Field>
        <Field label="环境标识色" hint="列表、详情、运行栏与浏览器窗口边框使用该颜色和粗细（Windows 窗口边框）" className="col-span-2">
          <ColorPicker
            value={form.borderColor}
            onChange={(value) => update('borderColor', value)}
            width={form.borderWidth}
            onWidthChange={(value) => update('borderWidth', value)}
          />
        </Field>
        <Field label="任务栏标识文字" hint="最多 4 个字符；留空时任务栏显示颜色圆点" className="col-span-2">
          <Input
            value={form.badgeText}
            onChange={(event) => update('badgeText', event.target.value)}
            maxLength={4}
            placeholder="例如：A1 / 支付"
          />
        </Field>
        <Field label="分组">
          <Select value={form.groupId} onChange={(event) => update('groupId', event.target.value)} placeholder="不分组">
            {groups.map((group) => (
              <option key={group.id} value={group.id}>
                {group.name}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="绑定代理">
          <Select value={form.proxyId} onChange={(event) => update('proxyId', event.target.value)} placeholder="直连">
            {proxies
              .filter((proxy) => proxy.enabled)
              .map((proxy) => (
                <option key={proxy.id} value={proxy.id}>
                  {proxy.name} · {proxy.protocol}://{proxy.host}:{proxy.port}
                </option>
              ))}
          </Select>
        </Field>
        <Field label="项目">
          <Input value={form.project} onChange={(event) => update('project', event.target.value)} placeholder="例如：交易中台" />
        </Field>
        <Field label="测试环境">
          <Select value={form.testEnv} onChange={(event) => update('testEnv', event.target.value)} placeholder="未指定">
            <option value="dev">dev · 开发</option>
            <option value="test">test · 测试</option>
            <option value="pre">pre · 预发</option>
            <option value="prod">prod · 生产</option>
          </Select>
        </Field>
        <Field label="账号角色">
          <Input value={form.accountRole} onChange={(event) => update('accountRole', event.target.value)} placeholder="例如：运营 / 审核员" />
        </Field>
        <Field label="版本号">
          <Input value={form.version} onChange={(event) => update('version', event.target.value)} placeholder="例如：V2.8.1" />
        </Field>
        <Field label="测试用例 ID">
          <Input value={form.caseId} onChange={(event) => update('caseId', event.target.value)} placeholder="例如：CASE-1024" />
        </Field>
        <Field label="指纹模板">
          <Select value={form.templateOs} onChange={(event) => update('templateOs', event.target.value)}>
            <option value="win11">Windows 11 · Chrome</option>
            <option value="win10">Windows 10 · Chrome</option>
            <option value="mac">macOS · Chrome</option>
            <option value="linux">Linux · Chrome</option>
          </Select>
        </Field>
        <Field label="标签" hint="逗号或空格分隔" className="col-span-2">
          <Input value={form.tags} onChange={(event) => update('tags', event.target.value)} placeholder="回归, 支付, P0" />
        </Field>
        <Field label="备注" className="col-span-2">
          <Textarea value={form.notes} onChange={(event) => update('notes', event.target.value)} placeholder="环境用途、注意事项…" />
        </Field>
        <Field
          label="环境启动参数"
          hint="每行一个 --flag / --flag=value；与全局参数取并集，同名参数以环境为准。"
          className="col-span-2"
        >
          <Textarea
            value={form.launchArgs}
            onChange={(event) => update('launchArgs', event.target.value)}
            rows={3}
            spellCheck={false}
            placeholder={'--autoplay-policy=no-user-gesture-required\n--disable-gpu'}
          />
        </Field>
        <Field label="内存上限 MB" hint="留空使用全局阈值">
          <Input
            type="number"
            min={128}
            value={form.memoryLimitMb}
            onChange={(event) => update('memoryLimitMb', event.target.value)}
            placeholder="1500"
          />
        </Field>
        <Field label="闲置回收 分钟" hint="留空使用全局阈值">
          <Input
            type="number"
            min={0}
            value={form.idleTimeoutMin}
            onChange={(event) => update('idleTimeoutMin', event.target.value)}
            placeholder="30"
          />
        </Field>
        <div className="col-span-2 rounded-md border border-border bg-surface-secondary p-3">
          <Switch
            checked={form.autoRestart}
            onCheckedChange={(value) => update('autoRestart', value)}
            label="进程崩溃后自动重启"
            description="仅在该环境进程意外退出时触发，10 分钟内最多 3 次。"
          />
        </div>
      </form>
    </Dialog>
  );
}

export function CloneEnvironmentDialog({
  open,
  onOpenChange,
  env,
  onCloned,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  env: EnvironmentView | null;
  onCloned: (env: EnvironmentView) => void;
}) {
  const toast = useToastStore();
  const [name, setName] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  React.useEffect(() => {
    if (env) setName(`${env.name} - 副本`);
  }, [env]);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="克隆环境"
      description="复制指纹、账号、快照与 Hosts/Mock 配置；浏览器 profile 与登录态不复制。"
      width="max-w-md"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button
            variant="primary"
            loading={busy}
            onClick={async () => {
              if (!env) return;
              setBusy(true);
              try {
                const cloned = await ipc.envClone(env.id, name.trim() || undefined);
                toast.success('环境已克隆', cloned.name);
                onCloned(cloned);
                onOpenChange(false);
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            克隆
          </Button>
        </>
      }
    >
      <Field label="新环境名称" required>
        <Input value={name} onChange={(event) => setName(event.target.value)} autoFocus />
      </Field>
    </Dialog>
  );
}

export function ExportEnvironmentsDialog({
  open,
  onOpenChange,
  envs,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  envs: EnvironmentView[];
}) {
  const toast = useToastStore();
  const [passphrase, setPassphrase] = React.useState('');
  const [includeProfiles, setIncludeProfiles] = React.useState(false);
  const [ack, setAck] = React.useState(false);
  const [busy, setBusy] = React.useState(false);

  const submit = async () => {
    if (passphrase.length < 6) {
      toast.warning('加密口令至少 6 个字符');
      return;
    }
    const path = await saveFile('finenv-environments.finenv', envPackageFilter());
    if (!path) return;
    setBusy(true);
    try {
      const count = await ipc.envExport(
        envs.map((env) => env.id),
        path,
        passphrase,
        includeProfiles,
        true,
      );
      toast.success('环境包已导出', `${count} 个环境 · ${path}`);
      onOpenChange(false);
      setPassphrase('');
      setAck(false);
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
      title="导出加密环境包"
      description={`将导出 ${envs.length} 个环境的指纹、账号密码、快照、Hosts/Mock。整个包使用口令加密。`}
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button variant="primary" loading={busy} disabled={!ack} onClick={() => void submit()}>
            <Download className="h-4 w-4" />
            选择位置并导出
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="加密口令" required hint="至少 6 个字符，迁移到其他设备时需要相同口令">
          <Input
            type="password"
            value={passphrase}
            onChange={(event) => setPassphrase(event.target.value)}
            placeholder="输入环境包口令"
          />
        </Field>
        <div className="rounded-md border border-border bg-surface-secondary p-3">
          <Switch
            checked={includeProfiles}
            onCheckedChange={setIncludeProfiles}
            label="包含浏览器 profile（登录态 / Cookie）"
            description="体积可能较大，且包含完整浏览器数据；仅在复现登录态时勾选。"
          />
        </div>
        <label className="flex items-start gap-2 text-xs leading-5 text-content-weak">
          <input
            type="checkbox"
            className="mt-0.5 accent-[var(--color-primary)]"
            checked={ack}
            onChange={(event) => setAck(event.target.checked)}
          />
          我了解环境包包含账号密码与 Cookie 数据，属于高危导出操作，将记录审计日志。
        </label>
        <div className="rounded-md border border-border p-2 text-xs text-content-weak">
          {envs.slice(0, 5).map((env) => (
            <div key={env.id} className="truncate py-0.5">
              · {env.name}
            </div>
          ))}
          {envs.length > 5 ? <div className="py-0.5 text-content-hint">… 共 {envs.length} 个</div> : null}
        </div>
      </div>
    </Dialog>
  );
}

export function ImportEnvironmentsDialog({
  open,
  onOpenChange,
  onImported,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImported: () => void;
}) {
  const toast = useToastStore();
  const [paths, setPaths] = React.useState<string[]>([]);
  const [passphrase, setPassphrase] = React.useState('');
  const [busy, setBusy] = React.useState(false);

  const choose = async () => {
    try {
      const files = await pickFile(envPackageFilter(), true);
      if (files) setPaths((current) => Array.from(new Set([...current, ...files])));
    } catch (error) {
      toastError(error);
    }
  };

  const submit = async () => {
    if (paths.length === 0) {
      toast.warning('请选择至少一个环境包');
      return;
    }
    if (!passphrase) {
      toast.warning('请输入环境包口令');
      return;
    }
    setBusy(true);
    try {
      const envs =
        paths.length === 1
          ? await ipc.envImport(paths[0], passphrase)
          : await ipc.envBulkImport(paths, passphrase);
      toast.success('导入完成', `${envs.length} 个环境已就绪`);
      setPaths([]);
      setPassphrase('');
      onImported();
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
      title="导入加密环境包"
      description="导入后会重新生成 ID 与 profile 目录，不会覆盖现有环境。"
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button variant="primary" loading={busy} onClick={() => void submit()}>
            <Upload className="h-4 w-4" />
            开始导入
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="space-y-2">
          <Button variant="secondary" onClick={() => void choose()}>
            <FolderOpen className="h-4 w-4" /> 选择 .finenv 包
          </Button>
          {paths.map((path) => (
            <div key={path} className="flex items-center gap-2 rounded-md border border-border bg-surface-secondary px-2.5 py-1.5 text-xs">
              <span className="min-w-0 flex-1 truncate" title={path}>
                {path}
              </span>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => setPaths((current) => current.filter((item) => item !== path))}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>
            </div>
          ))}
        </div>
        <Field label="环境包口令" required>
          <Input
            type="password"
            value={passphrase}
            onChange={(event) => setPassphrase(event.target.value)}
            placeholder="输入导出时设置的口令"
          />
        </Field>
      </div>
    </Dialog>
  );
}

export function AssignProxyDialog({
  open,
  onOpenChange,
  envs,
  proxies,
  onDone,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  envs: EnvironmentView[];
  proxies: Proxy[];
  onDone: () => void;
}) {
  const toast = useToastStore();
  const [proxyId, setProxyId] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  const running = envs.filter((env) => env.runtime).length;
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="批量分配代理"
      description={`将为 ${envs.length} 个环境绑定同一个代理或切换为直连。`}
      width="max-w-md"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button
            variant="primary"
            loading={busy}
            onClick={async () => {
              setBusy(true);
              try {
                const count = await ipc.proxyAssign(
                  envs.map((env) => env.id),
                  proxyId || null,
                );
                toast.success('代理分配完成', `${count} 个环境已更新${running ? '，运行中的环境需重启生效' : ''}`);
                onDone();
                onOpenChange(false);
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            确认分配
          </Button>
        </>
      }
    >
      <Field label="目标代理">
        <Select value={proxyId} onChange={(event) => setProxyId(event.target.value)} placeholder="直连（清除代理）">
          {proxies.map((proxy) => (
            <option key={proxy.id} value={proxy.id}>
              {proxy.name} · {proxy.protocol}://{proxy.host}:{proxy.port}
            </option>
          ))}
        </Select>
      </Field>
      {running > 0 ? (
        <p className="mt-3 rounded-md bg-[color-mix(in_srgb,var(--warning)_10%,transparent)] px-3 py-2 text-xs text-[var(--warning)]">
          其中 {running} 个环境正在运行，代理变更需要重启浏览器后生效。
        </p>
      ) : null}
    </Dialog>
  );
}

export function GroupManagerDialog({
  open,
  onOpenChange,
  groups,
  onChanged,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  groups: Group[];
  onChanged: () => void;
}) {
  const toast = useToastStore();
  const [name, setName] = React.useState('');
  const [color, setColor] = React.useState('#165DFF');
  const [busy, setBusy] = React.useState(false);
  const [deleting, setDeleting] = React.useState<Group | null>(null);

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={onOpenChange}
        title="分组管理"
        description="分组用于项目/团队维度的环境筛选，删除分组不会删除环境。"
        width="max-w-md"
      >
        <div className="space-y-3">
          <div className="flex items-end gap-2">
            <Field label="新分组名称" className="flex-1">
              <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：交易中台" />
            </Field>
            <input
              type="color"
              value={color}
              onChange={(event) => setColor(event.target.value)}
              className="h-[var(--control-height)] w-10 cursor-pointer rounded-md border border-border"
              title="分组颜色"
            />
            <Button
              variant="primary"
              loading={busy}
              onClick={async () => {
                if (!name.trim()) return;
                setBusy(true);
                try {
                  await ipc.groupCreate(name.trim(), color);
                  setName('');
                  toast.success('分组已创建');
                  onChanged();
                } catch (error) {
                  toastError(error);
                } finally {
                  setBusy(false);
                }
              }}
            >
              <Plus className="h-4 w-4" />
              添加
            </Button>
          </div>
          <div className="max-h-72 space-y-1.5 overflow-y-auto">
            {groups.length === 0 ? (
              <p className="py-6 text-center text-xs text-content-hint">暂无分组</p>
            ) : (
              groups.map((group) => (
                <div key={group.id} className="flex items-center gap-2 rounded-md border border-border px-3 py-2">
                  <span className="h-3 w-3 rounded-full" style={{ background: group.color }} />
                  <span className="min-w-0 flex-1 truncate text-sm">{group.name}</span>
                  <Button variant="ghost" size="icon-sm" onClick={() => setDeleting(group)}>
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                </div>
              ))
            )}
          </div>
          <div className="flex items-center gap-1.5 text-[11px] text-content-hint">
            <Layers className="h-3 w-3" /> 分组仅影响管理面板，不会写入被测页面。
          </div>
        </div>
      </Dialog>
      <ConfirmDialog
        open={!!deleting}
        onOpenChange={(value) => !value && setDeleting(null)}
        title="删除分组"
        description={`删除「${deleting?.name ?? ''}」后，其下环境将变为未分组。`}
        confirmLabel="删除分组"
        onConfirm={async () => {
          if (!deleting) return;
          try {
            await ipc.groupDelete(deleting.id);
            toast.success('分组已删除');
            setDeleting(null);
            onChanged();
          } catch (error) {
            toastError(error);
          }
        }}
      />
    </>
  );
}
