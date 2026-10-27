import * as React from 'react';
import { Link } from 'react-router-dom';
import {
  CheckCircle2,
  Download,
  FileCheck2,
  FilterX,
  RefreshCw,
  ScrollText,
  Search,
  ShieldAlert,
  ShieldCheck,
} from 'lucide-react';
import type { AuditLog, AuditVerifyResult } from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { useEnvStore } from '@/store/env';
import { toastError, useToastStore } from '@/store/toast';
import {
  auditActionLabels,
  cn,
  formatDate,
} from '@/utils';
import { csvFilter, jsonFilter, saveFile } from '@/utils/dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, EmptyState, Spinner } from '@/components/ui/card';
import { CopyButton } from '@/components/ui/misc';
import { Input, Select } from '@/components/ui/input';
import { Tooltip } from '@/components/ui/tooltip';

const defaultActions = Object.keys(auditActionLabels);

export function AuditPage() {
  const toast = useToastStore();
  const envs = useEnvStore((state) => state.envs);
  const canExport = useAuthStore((state) => state.can('audit_view'));
  const [logs, setLogs] = React.useState<AuditLog[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [search, setSearch] = React.useState('');
  const [action, setAction] = React.useState('');
  const [envId, setEnvId] = React.useState('');
  const [from, setFrom] = React.useState('');
  const [to, setTo] = React.useState('');
  const [verify, setVerify] = React.useState<AuditVerifyResult | null>(null);
  const [verifying, setVerifying] = React.useState(false);
  const [exporting, setExporting] = React.useState(false);

  const filter = React.useMemo(
    () => ({
      search: search.trim() || null,
      action: action || null,
      envId: envId || null,
      from: from ? new Date(`${from}T00:00:00`).toISOString() : null,
      to: to ? new Date(`${to}T23:59:59.999`).toISOString() : null,
      limit: 500,
      offset: 0,
    }),
    [search, action, envId, from, to],
  );

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      setLogs(await ipc.auditList(filter));
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, [filter]);

  React.useEffect(() => {
    const timer = window.setTimeout(() => void load(), search ? 300 : 0);
    return () => window.clearTimeout(timer);
  }, [load, search]);

  React.useEffect(() => {
    if (useEnvStore.getState().envs.length === 0) void useEnvStore.getState().refresh(false).catch(() => undefined);
  }, []);

  const actions = React.useMemo(() => {
    const set = new Set(defaultActions);
    logs.forEach((log) => set.add(log.action));
    return Array.from(set).sort();
  }, [logs]);

  const envName = (id: string | null) => envs.find((env) => env.id === id)?.name ?? id?.slice(0, 8) ?? '—';

  const verifyChain = async () => {
    setVerifying(true);
    try {
      const result = await ipc.auditVerify();
      setVerify(result);
      if (result.valid) toast.success('审计哈希链完整', `已校验 ${result.checked} 条记录`);
      else toast.error('审计链校验失败', result.message);
    } catch (error) {
      toastError(error);
    } finally {
      setVerifying(false);
    }
  };

  const exportLogs = async (format: 'json' | 'csv') => {
    if (!canExport) return;
    const path = await saveFile(`finenv-audit-${new Date().toISOString().slice(0, 10)}.${format}`, format === 'csv' ? csvFilter() : jsonFilter());
    if (!path) return;
    setExporting(true);
    try {
      const count = await ipc.auditExport(path, format, filter);
      toast.success('审计日志已导出', `${count} 条 · ${path}`);
    } catch (error) {
      toastError(error);
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="page-pad space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-[10px] font-semibold uppercase tracking-widest text-content-hint">Security / Audit Trail</p>
          <h1 className="mt-1 text-xl font-semibold tracking-tight text-content-main">审计日志</h1>
          <p className="mt-1 max-w-2xl text-xs leading-5 text-content-weak">
            所有子进程操作、高危接口与敏感字段访问都会写入追加式日志；哈希链 + 数据库触发器保证不可篡改。
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="secondary" onClick={() => void verifyChain()} loading={verifying}>
            <ShieldCheck className="h-4 w-4" /> 校验哈希链
          </Button>
          <Button variant="secondary" onClick={() => void exportLogs('json')} loading={exporting} disabled={!canExport}>
            <Download className="h-4 w-4" /> 导出 JSON
          </Button>
          <Button variant="secondary" onClick={() => void exportLogs('csv')} loading={exporting} disabled={!canExport}>
            <Download className="h-4 w-4" /> 导出 CSV
          </Button>
          <Button variant="secondary" onClick={() => void load()}>
            <RefreshCw className="h-4 w-4" /> 刷新
          </Button>
        </div>
      </div>

      {verify ? (
        <div
          className={cn(
            'flex items-center gap-2 rounded-md border px-3 py-2 text-xs',
            verify.valid
              ? 'border-[color-mix(in_srgb,var(--success)_30%,transparent)] bg-[color-mix(in_srgb,var(--success)_8%,transparent)] text-[var(--success)]'
              : 'border-[color-mix(in_srgb,var(--danger)_30%,transparent)] bg-[color-mix(in_srgb,var(--danger)_8%,transparent)] text-danger',
          )}
        >
          {verify.valid ? <CheckCircle2 className="h-4 w-4" /> : <ShieldAlert className="h-4 w-4" />}
          <span className="font-medium">{verify.message}</span>
          {!verify.valid && verify.brokenAt ? <span className="font-mono">断点：{verify.brokenAt}</span> : null}
        </div>
      ) : null}

      <Card className="overflow-hidden">
        <div className="space-y-2 border-b border-border p-4">
          <div className="flex flex-wrap items-center gap-2">
            <div className="relative min-w-[220px] flex-1">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-content-hint" />
              <Input className="pl-9" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索操作人、动作、详情、账号、用例…" />
            </div>
            <Select wrapperClassName="w-[150px]" value={action} placeholder="全部动作" onChange={(event) => setAction(event.target.value)}>
              {actions.map((key) => (
                <option key={key} value={key}>
                  {auditActionLabels[key] ?? key}
                </option>
              ))}
            </Select>
            <Select wrapperClassName="w-[170px]" value={envId} placeholder="全部环境" onChange={(event) => setEnvId(event.target.value)}>
              {envs.map((env) => (
                <option key={env.id} value={env.id}>
                  {env.name}
                </option>
              ))}
            </Select>
            <Input className="w-[145px]" type="date" value={from} onChange={(event) => setFrom(event.target.value)} title="开始日期" />
            <Input className="w-[145px]" type="date" value={to} onChange={(event) => setTo(event.target.value)} title="结束日期" />
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                setSearch('');
                setAction('');
                setEnvId('');
                setFrom('');
                setTo('');
              }}
            >
              <FilterX className="h-3.5 w-3.5" /> 清除
            </Button>
          </div>
          <p className="text-[10px] text-content-hint">
            展示最近 500 条 · 日志一旦写入不可更新或删除 · 哈希链覆盖操作人/动作/环境/时间/详情
          </p>
        </div>

        {loading ? (
          <div className="flex justify-center py-16"><Spinner /></div>
        ) : logs.length === 0 ? (
          <EmptyState icon={<ScrollText className="h-5 w-5" />} title="没有审计记录" description="执行环境操作、敏感导出或系统设置变更后，日志会出现在这里。" />
        ) : (
          <div className="table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>时间</th>
                  <th>操作人</th>
                  <th>动作</th>
                  <th>环境</th>
                  <th>账号 / 项目 / 用例</th>
                  <th>详情</th>
                  <th>哈希</th>
                </tr>
              </thead>
              <tbody>
                {logs.map((log) => (
                  <tr key={log.id}>
                    <td>
                      <p className="whitespace-nowrap text-xs text-content-main">{formatDate(log.createdAt)}</p>
                      <p className="font-mono text-[10px] text-content-hint">{log.id.slice(0, 8)}</p>
                    </td>
                    <td className="text-xs text-content-main">{log.actor}</td>
                    <td>
                      <Badge tone="outline">{auditActionLabels[log.action] ?? log.action}</Badge>
                    </td>
                    <td>
                      {log.envId ? (
                        <Link className="text-xs text-primary hover:underline" to={`/envs/${log.envId}`}>
                          {envName(log.envId)}
                        </Link>
                      ) : (
                        <span className="text-xs text-content-hint">—</span>
                      )}
                    </td>
                    <td>
                      <div className="max-w-[220px] space-y-0.5 text-[11px]">
                        {log.account ? <p className="font-mono text-content-main">{log.account}</p> : null}
                        <p className="truncate text-content-hint">
                          {log.project || '—'} {log.caseId ? `· ${log.caseId}` : ''}
                        </p>
                      </div>
                    </td>
                    <td>
                      <p className="max-w-[360px] whitespace-pre-wrap break-words text-[11px] leading-5 text-content-weak">{log.detail}</p>
                    </td>
                    <td>
                      <div className="flex items-center gap-1">
                        <Tooltip content={`${log.prevHash.slice(0, 16)} → ${log.hash.slice(0, 16)}`}>
                          <span className="font-mono text-[10px] text-content-hint">{log.hash.slice(0, 10)}…</span>
                        </Tooltip>
                        <CopyButton value={log.hash} />
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <div className="flex flex-wrap items-center gap-3">
        <Card className="flex flex-1 items-start gap-2 p-3 text-[11px] leading-5 text-content-weak">
          <FileCheck2 className="mt-0.5 h-4 w-4 shrink-0 text-primary" />
          哈希链采用 SHA-256(prevHash + 规范化字段)，数据库触发器禁止 UPDATE / DELETE；导出文件保留完整链信息，可在离线环境复核。
        </Card>
      </div>
    </div>
  );
}
