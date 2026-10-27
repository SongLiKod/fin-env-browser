import * as React from 'react';
import {
  Download,
  Eye,
  FileKey2,
  KeyRound,
  Plus,
  Search,
  Trash2,
  Upload,
  UserRound,
} from 'lucide-react';
import type {
  Account,
  AccountExportRow,
  AccountStatus,
  EnvironmentView,
} from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { accountStatusMeta, cn, formatDateShort, parseCsvRows } from '@/utils';
import { accountPackageFilter, pickFile, saveFile } from '@/utils/dialog';
import { useAuthStore } from '@/store/auth';
import { toastError, useToastStore } from '@/store/toast';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, EmptyState, Spinner } from '@/components/ui/card';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { CopyButton } from '@/components/ui/misc';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input, Select, Textarea } from '@/components/ui/input';
import { Tooltip } from '@/components/ui/tooltip';

export function AccountsPanel({
  env,
  onCountChanged,
}: {
  env: EnvironmentView;
  onCountChanged?: (count: number) => void;
}) {
  const toast = useToastStore();
  const canEdit = useAuthStore((state) => state.can('edit_env'));
  const canExport = useAuthStore((state) => state.can('export_sensitive'));
  const [accounts, setAccounts] = React.useState<Account[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [search, setSearch] = React.useState('');
  const [editor, setEditor] = React.useState<Account | 'new' | null>(null);
  const [revealTarget, setRevealTarget] = React.useState<Account | null>(null);
  const [revealed, setRevealed] = React.useState<{ account: Account; password: string } | null>(null);
  const [deleteTarget, setDeleteTarget] = React.useState<Account | null>(null);
  const [importOpen, setImportOpen] = React.useState(false);
  const [exportOpen, setExportOpen] = React.useState(false);
  const [fileImportOpen, setFileImportOpen] = React.useState(false);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const list = await ipc.accountList(env.id);
      setAccounts(list);
      onCountChanged?.(list.length);
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, [env.id]);

  React.useEffect(() => {
    void load();
  }, [load]);

  const filtered = accounts.filter((account) => {
    const query = search.trim().toLowerCase();
    if (!query) return true;
    return [account.username, account.owner, account.remark].some((value) =>
      value.toLowerCase().includes(query),
    );
  });

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold text-content-main">账号台账</h3>
          <p className="mt-0.5 text-xs text-content-weak">
            账号密码使用 AES-256-GCM 加密落盘；查看明文与导出均写入审计日志。
          </p>
        </div>
        <div className="flex items-center gap-2">
          {canExport ? (
            <Button variant="secondary" size="sm" onClick={() => setExportOpen(true)} disabled={accounts.length === 0}>
              <Download className="h-3.5 w-3.5" /> 导出账号
            </Button>
          ) : null}
          {canEdit ? (
            <>
              <Button variant="secondary" size="sm" onClick={() => setFileImportOpen(true)}>
                <FileKey2 className="h-3.5 w-3.5" /> 导入账号包
              </Button>
              <Button variant="secondary" size="sm" onClick={() => setImportOpen(true)}>
                <Upload className="h-3.5 w-3.5" /> 批量导入
              </Button>
              <Button variant="primary" size="sm" onClick={() => setEditor('new')}>
                <Plus className="h-3.5 w-3.5" /> 新增账号
              </Button>
            </>
          ) : null}
        </div>
      </div>

      <div className="relative max-w-sm">
        <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-content-hint" />
        <Input className="pl-9" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索账号、负责人、备注…" />
      </div>

      <Card className="overflow-hidden">
        {loading ? (
          <div className="flex items-center justify-center gap-2 py-12 text-xs text-content-weak">
            <Spinner /> 加载账号…
          </div>
        ) : filtered.length === 0 ? (
          <EmptyState
            icon={<UserRound className="h-5 w-5" />}
            title={accounts.length === 0 ? '暂无账号记录' : '没有匹配的账号'}
            description={accounts.length === 0 ? '每个测试账号建议单独建立一条台账记录。' : undefined}
            action={
              canEdit && accounts.length === 0 ? (
                <Button variant="primary" size="sm" onClick={() => setEditor('new')}>
                  <Plus className="h-3.5 w-3.5" /> 新增账号
                </Button>
              ) : undefined
            }
          />
        ) : (
          <div className="table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>账号</th>
                  <th>密码</th>
                  <th>负责人</th>
                  <th>状态</th>
                  <th>备注</th>
                  <th>更新时间</th>
                  <th className="w-[130px] text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((account) => {
                  const meta = accountStatusMeta[account.status];
                  return (
                    <tr key={account.id}>
                      <td>
                        <div className="flex items-center gap-2">
                          <div className="flex h-7 w-7 items-center justify-center rounded-full bg-primary-light text-[11px] font-semibold uppercase text-primary">
                            {account.username.slice(0, 1)}
                          </div>
                          <span className="font-mono text-xs text-content-main">{account.username}</span>
                        </div>
                      </td>
                      <td>
                        {account.hasPassword ? (
                          <Badge tone="success">
                            <KeyRound className="h-3 w-3" /> 已加密
                          </Badge>
                        ) : (
                          <Badge tone="neutral">未设置</Badge>
                        )}
                      </td>
                      <td className="text-xs text-content-weak">{account.owner || '—'}</td>
                      <td>
                        <Badge tone={meta.tone === 'muted' ? 'neutral' : meta.tone}>{meta.label}</Badge>
                      </td>
                      <td>
                        <p className="max-w-[260px] truncate text-xs text-content-weak" title={account.remark}>
                          {account.remark || '—'}
                        </p>
                      </td>
                      <td className="text-xs text-content-hint">{formatDateShort(account.updatedAt)}</td>
                      <td>
                        <div className="flex items-center justify-end gap-1">
                          {canExport && account.hasPassword ? (
                            <Tooltip content="查看明文密码（审计）">
                              <Button variant="ghost" size="icon-sm" onClick={() => setRevealTarget(account)}>
                                <Eye className="h-3.5 w-3.5" />
                              </Button>
                            </Tooltip>
                          ) : null}
                          {canEdit ? (
                            <>
                              <Button variant="ghost" size="sm" onClick={() => setEditor(account)}>
                                编辑
                              </Button>
                              <Button variant="ghost" size="icon-sm" className="text-danger" onClick={() => setDeleteTarget(account)}>
                                <Trash2 className="h-3.5 w-3.5" />
                              </Button>
                            </>
                          ) : null}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <AccountEditorDialog
        open={!!editor}
        onOpenChange={(value) => !value && setEditor(null)}
        envId={env.id}
        account={editor === 'new' ? null : editor}
        onSaved={() => {
          setEditor(null);
          void load();
        }}
      />

      <ConfirmDialog
        open={!!revealTarget}
        onOpenChange={(value) => !value && setRevealTarget(null)}
        title="查看明文密码"
        description={`将解密「${revealTarget?.username ?? ''}」的密码并写入审计日志。`}
        confirmLabel="解密查看"
        requiredText="REVEAL"
        onConfirm={async () => {
          if (!revealTarget) return;
          try {
            const password = await ipc.accountReveal(revealTarget.id, true);
            setRevealed({ account: revealTarget, password });
            setRevealTarget(null);
          } catch (error) {
            toastError(error);
          }
        }}
      />

      <Dialog
        open={!!revealed}
        onOpenChange={(value) => !value && setRevealed(null)}
        title="账号明文密码"
        description={revealed ? `${revealed.account.username} · 本次查看已记录审计日志` : undefined}
        width="max-w-md"
        footer={
          <Button variant="secondary" onClick={() => setRevealed(null)}>
            关闭
          </Button>
        }
      >
        {revealed ? (
          <div className="flex items-center gap-2 rounded-md border border-border bg-surface-secondary p-3">
            <span className="min-w-0 flex-1 break-all font-mono text-sm">{revealed.password || '（空密码）'}</span>
            <CopyButton value={revealed.password} label="复制" size="sm" />
          </div>
        ) : null}
        <p className="mt-3 text-[11px] text-content-hint">请勿将密码粘贴到聊天工具或工单系统。</p>
      </Dialog>

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(value) => !value && setDeleteTarget(null)}
        title="删除账号记录"
        description={`删除「${deleteTarget?.username ?? ''}」的台账记录，不影响浏览器中的登录态。`}
        confirmLabel="删除"
        onConfirm={async () => {
          if (!deleteTarget) return;
          try {
            await ipc.accountDelete(deleteTarget.id);
            setDeleteTarget(null);
            toast.success('账号记录已删除');
            void load();
          } catch (error) {
            toastError(error);
          }
        }}
      />

      <AccountImportDialog open={importOpen} onOpenChange={setImportOpen} envId={env.id} onImported={() => void load()} />
      <AccountFileImportDialog open={fileImportOpen} onOpenChange={setFileImportOpen} envId={env.id} onImported={() => void load()} />
      <AccountExportDialog
        open={exportOpen}
        onOpenChange={setExportOpen}
        env={env}
        count={accounts.length}
      />
    </div>
  );
}

function AccountEditorDialog({
  open,
  onOpenChange,
  envId,
  account,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  envId: string;
  account: Account | null;
  onSaved: () => void;
}) {
  const toast = useToastStore();
  const [busy, setBusy] = React.useState(false);
  const [form, setForm] = React.useState({
    username: '',
    password: '',
    owner: '',
    remark: '',
    status: 'pending' as AccountStatus,
  });

  React.useEffect(() => {
    if (!open) return;
    setForm({
      username: account?.username ?? '',
      password: '',
      owner: account?.owner ?? '',
      remark: account?.remark ?? '',
      status: account?.status ?? 'pending',
    });
  }, [open, account]);

  const update = (key: keyof typeof form, value: string) =>
    setForm((current) => ({ ...current, [key]: value }));

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={account ? '编辑账号' : '新增账号'}
      description={account ? '密码留空表示保持原密码不变。' : '账号与密码仅用于本地测试台账，密码会立即加密。'}
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button
            variant="primary"
            loading={busy}
            onClick={async () => {
              if (!form.username.trim()) {
                toast.warning('账号不能为空');
                return;
              }
              setBusy(true);
              try {
                if (account) {
                  await ipc.accountUpdate(account.id, {
                    username: form.username.trim(),
                    password: form.password || undefined,
                    owner: form.owner,
                    remark: form.remark,
                    status: form.status,
                  });
                  toast.success('账号已更新');
                } else {
                  await ipc.accountCreate({
                    envId,
                    username: form.username.trim(),
                    password: form.password || null,
                    owner: form.owner,
                    remark: form.remark,
                    status: form.status,
                  });
                  toast.success('账号已创建');
                }
                onSaved();
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            保存
          </Button>
        </>
      }
    >
      <div className="grid grid-cols-2 gap-3">
        <Field label="账号" required className="col-span-2">
          <Input value={form.username} onChange={(event) => update('username', event.target.value)} autoFocus placeholder="登录账号 / 手机号 / 邮箱" />
        </Field>
        <Field label={account ? '新密码（留空不修改）' : '密码'} hint="AES-256-GCM 加密存储">
          <Input type="password" value={form.password} onChange={(event) => update('password', event.target.value)} placeholder={account?.hasPassword ? '••••••••' : '输入密码'} />
        </Field>
        <Field label="负责人">
          <Input value={form.owner} onChange={(event) => update('owner', event.target.value)} placeholder="测试同学姓名" />
        </Field>
        <Field label="状态">
          <Select value={form.status} onChange={(event) => update('status', event.target.value)}>
            {Object.entries(accountStatusMeta).map(([value, meta]) => (
              <option key={value} value={value}>
                {meta.label}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="备注" className="col-span-2">
          <Textarea value={form.remark} onChange={(event) => update('remark', event.target.value)} placeholder="用途、权限范围、注意事项…" />
        </Field>
      </div>
    </Dialog>
  );
}

function AccountImportDialog({
  open,
  onOpenChange,
  envId,
  onImported,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  envId: string;
  onImported: () => void;
}) {
  const toast = useToastStore();
  const [text, setText] = React.useState('username,password,owner,remark,status\n');
  const [busy, setBusy] = React.useState(false);
  const rows = React.useMemo(() => {
    const parsed = parseCsvRows(text);
    if (parsed.length === 0) return [];
    const header = parsed[0].map((cell) => cell.trim().toLowerCase());
    const hasHeader = header.includes('username') || header.includes('账号');
    const data = hasHeader ? parsed.slice(1) : parsed;
    const index = (names: string[], fallback: number) => {
      const found = names.map((name) => header.indexOf(name)).find((i) => i >= 0);
      return found ?? fallback;
    };
    return data
      .map((cells) => ({
        username: (cells[index(['username', '账号'], 0)] ?? '').trim(),
        password: cells[index(['password', '密码'], 1)] ?? '',
        owner: cells[index(['owner', '负责人'], 2)] ?? '',
        remark: cells[index(['remark', '备注'], 3)] ?? '',
        status: (cells[index(['status', '状态'], 4)] ?? 'pending').trim() || 'pending',
      }))
      .filter((row) => row.username);
  }, [text]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="批量导入账号"
      description="支持带表头或不带表头的 CSV：username,password,owner,remark,status"
      width="max-w-2xl"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button
            variant="primary"
            loading={busy}
            disabled={rows.length === 0}
            onClick={async () => {
              setBusy(true);
              try {
                const count = await ipc.accountImport(envId, rows as AccountExportRow[]);
                toast.success('账号导入完成', `${count} 条新记录`);
                onImported();
                onOpenChange(false);
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            导入 {rows.length} 条
          </Button>
        </>
      }
    >
      <Textarea className="min-h-[280px] font-mono text-[11px]" value={text} onChange={(event) => setText(event.target.value)} />
      <div className="mt-2 flex items-center justify-between text-[11px] text-content-hint">
        <span>状态可选 pending / in_use / frozen / deprecated，非法值按 pending 处理。</span>
        <span>{rows.length} 条有效记录</span>
      </div>
      {rows.length > 0 ? (
        <div className="mt-2 max-h-32 overflow-y-auto rounded-md border border-border p-2 font-mono text-[10px] text-content-weak">
          {rows.slice(0, 8).map((row, index) => (
            <div key={index} className="truncate">
              {row.username} · {row.password ? '***' : '空密码'} · {row.status}
            </div>
          ))}
          {rows.length > 8 ? <div>… 共 {rows.length} 条</div> : null}
        </div>
      ) : null}
    </Dialog>
  );
}

function AccountFileImportDialog({
  open,
  onOpenChange,
  envId,
  onImported,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  envId: string;
  onImported: () => void;
}) {
  const toast = useToastStore();
  const [path, setPath] = React.useState('');
  const [passphrase, setPassphrase] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="导入加密账号包"
      description="账号包由「导出账号」生成，使用口令解密后写入当前环境。"
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button
            variant="primary"
            loading={busy}
            onClick={async () => {
              if (!path) {
                toast.warning('请选择账号包');
                return;
              }
              setBusy(true);
              try {
                const count = await ipc.accountImportFile(envId, path, passphrase);
                toast.success('账号包导入完成', `${count} 条新记录`);
                onImported();
                onOpenChange(false);
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            导入
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="flex gap-2">
          <Input value={path} readOnly placeholder="选择 .finenv 账号包" />
          <Button
            variant="secondary"
            onClick={async () => {
              try {
                const files = await pickFile(accountPackageFilter());
                if (files?.[0]) setPath(files[0]);
              } catch (error) {
                toastError(error);
              }
            }}
          >
            选择
          </Button>
        </div>
        <Field label="账号包口令">
          <Input type="password" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} />
        </Field>
      </div>
    </Dialog>
  );
}

function AccountExportDialog({
  open,
  onOpenChange,
  env,
  count,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  env: EnvironmentView;
  count: number;
}) {
  const toast = useToastStore();
  const [passphrase, setPassphrase] = React.useState('');
  const [ack, setAck] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="导出账号台账"
      description={`导出「${env.name}」的 ${count} 条账号记录，密码将随包一起使用口令加密。`}
      width="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button
            variant="primary"
            loading={busy}
            disabled={!ack || passphrase.length < 6}
            onClick={async () => {
              const path = await saveFile(`accounts-${env.name}.finenv`, accountPackageFilter());
              if (!path) return;
              setBusy(true);
              try {
                const exported = await ipc.accountExport([env.id], path, passphrase, true);
                toast.success('账号已导出', `${exported} 条 · ${path}`);
                onOpenChange(false);
                setPassphrase('');
                setAck(false);
              } catch (error) {
                toastError(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            <Download className="h-4 w-4" /> 导出
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="加密口令" required hint="至少 6 个字符">
          <Input type="password" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} />
        </Field>
        <label className="flex items-start gap-2 text-xs leading-5 text-content-weak">
          <input
            type="checkbox"
            className={cn('mt-0.5 accent-[var(--color-primary)]')}
            checked={ack}
            onChange={(event) => setAck(event.target.checked)}
          />
          我了解该文件包含账号明文密码（整包加密），属于高危导出操作。
        </label>
      </div>
    </Dialog>
  );
}
