import * as React from 'react';
import {
  CheckCircle2,
  ExternalLink,
  Info,
  Radar,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  XCircle,
} from 'lucide-react';
import type { EnvironmentView, FingerprintReport } from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { toastError, useToastStore } from '@/store/toast';
import { formatDate, formatDateShort } from '@/utils';
import { envHasAccess } from '@/utils/environment';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardHeader, EmptyState, Spinner } from '@/components/ui/card';
import { CopyButton } from '@/components/ui/misc';

export function DetectionPanel({ env }: { env: EnvironmentView }) {
  const toast = useToastStore();
  const canOpen =
    useAuthStore((state) => state.can('open_close')) && envHasAccess(env, 'operate');
  const [reports, setReports] = React.useState<FingerprintReport[]>([]);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [detecting, setDetecting] = React.useState(false);
  const [preview, setPreview] = React.useState<string | null>(null);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const data = await ipc.fingerprintReports(env.id);
      setReports(data);
      setSelectedId((current) => current ?? data[0]?.id ?? null);
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, [env.id]);

  React.useEffect(() => {
    void load();
  }, [load]);

  const selected = reports.find((report) => report.id === selectedId) ?? reports[0] ?? null;
  const failures = selected?.checks.filter((check) => !check.passed) ?? [];
  const score = selected?.score ?? 0;
  const scoreTone = score >= 90 ? 'var(--success)' : score >= 70 ? 'var(--warning)' : 'var(--danger)';

  React.useEffect(() => {
    const path = selected?.screenshotPath;
    if (!path) {
      setPreview(null);
      return;
    }
    let active = true;
    ipc
      .readArtifact(path)
      .then((data) => {
        if (active) setPreview(data);
      })
      .catch(() => {
        if (active) setPreview(null);
      });
    return () => {
      active = false;
    };
  }, [selected?.id, selected?.screenshotPath]);

  const detect = async () => {
    setDetecting(true);
    try {
      const report = await ipc.fingerprintDetect(env.id);
      toast.success('指纹检测完成', `一致性评分 ${report.score}/100`);
      await load();
      setSelectedId(report.id);
    } catch (error) {
      toastError(error);
    } finally {
      setDetecting(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold text-content-main">指纹检测</h3>
          <p className="mt-0.5 text-xs text-content-weak">
            新开独立检测页访问配置的检测服务，通过 CDP 探针比对实际值，不会干扰当前业务页面。
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" size="sm" onClick={() => void load()}>
            <RefreshCw className="h-3.5 w-3.5" /> 刷新报告
          </Button>
          <Button
            variant="primary"
            size="sm"
            loading={detecting}
            disabled={!canOpen}
            onClick={() => void detect()}
          >
            <Radar className="h-3.5 w-3.5" /> 一键检测
          </Button>
        </div>
      </div>

      {loading ? (
        <div className="flex justify-center py-16"><Spinner /></div>
      ) : !selected ? (
        <Card>
          <EmptyState
            icon={<Radar className="h-5 w-5" />}
            title="暂无检测报告"
            description="启动环境后执行一键检测，FinEnv 会生成实际浏览器值与配置值的逐项对比报告。"
            action={
              <Button variant="primary" size="sm" loading={detecting} disabled={!canOpen} onClick={() => void detect()}>
                开始首次检测
              </Button>
            }
          />
        </Card>
      ) : (
        <div className="grid items-start gap-4 xl:grid-cols-[280px_minmax(0,1fr)]">
          <div className="space-y-2">
            <Card className="p-4">
              <div className="flex items-center gap-4">
                <div
                  className="flex h-20 w-20 shrink-0 items-center justify-center rounded-full"
                  style={{
                    background: `conic-gradient(${scoreTone} ${score * 3.6}deg, var(--bg-tertiary) 0deg)`,
                  }}
                >
                  <div className="flex h-16 w-16 flex-col items-center justify-center rounded-full bg-surface-base">
                    <span className="text-xl font-semibold tabular-nums text-content-main">{score}</span>
                    <span className="text-[9px] text-content-hint">/ 100</span>
                  </div>
                </div>
                <div className="min-w-0">
                  <p className="text-sm font-semibold text-content-main">
                    {score >= 90 ? '一致性优秀' : score >= 70 ? '存在偏差' : '高风险'}
                  </p>
                  <p className="mt-0.5 text-xs text-content-weak">
                    {selected.checks.filter((c) => c.passed).length}/{selected.checks.length} 项通过
                  </p>
                  <p className="mt-1 text-[10px] text-content-hint">{formatDate(selected.createdAt)}</p>
                </div>
              </div>
              <div className="mt-3 flex items-center gap-2">
                {selected.score >= 90 ? (
                  <Badge tone="success"><ShieldCheck className="h-3 w-3" /> 指纹自洽</Badge>
                ) : (
                  <Badge tone="warning"><ShieldAlert className="h-3 w-3" /> 建议复核</Badge>
                )}
                <Badge tone="outline">{env.fingerprint.clientHints.platform}</Badge>
              </div>
              {selected.externalUrl ? (
                <div className="mt-3 flex items-center gap-1 text-[10px] text-content-hint">
                  <ExternalLink className="h-3 w-3 shrink-0" />
                  <span className="truncate" title={selected.externalUrl}>{selected.externalUrl}</span>
                  <CopyButton value={selected.externalUrl} />
                </div>
              ) : null}
              {selected.screenshotPath ? (
                <p className="mt-2 break-all text-[10px] text-content-hint">截图：{selected.screenshotPath}</p>
              ) : null}
              {preview ? (
                <img src={preview} alt="检测页截图" className="mt-2 max-h-44 w-full rounded-md border border-border object-cover" />
              ) : null}
            </Card>

            <div className="max-h-[420px] space-y-1.5 overflow-y-auto pr-1">
              {reports.map((report) => (
                <button
                  key={report.id}
                  type="button"
                  onClick={() => setSelectedId(report.id)}
                  className={`w-full rounded-md border px-3 py-2 text-left transition-colors ${
                    report.id === selected.id
                      ? 'border-primary bg-primary-light'
                      : 'border-border bg-surface-base hover:bg-surface-secondary'
                  }`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs font-medium text-content-main">评分 {report.score}</span>
                    <span className="text-[10px] text-content-hint">{formatDateShort(report.createdAt)}</span>
                  </div>
                  <p className="mt-0.5 text-[10px] text-content-hint">
                    {report.checks.filter((c) => c.passed).length}/{report.checks.length} 项通过
                  </p>
                </button>
              ))}
            </div>
          </div>

          <div className="space-y-4">
            <Card>
              <CardHeader
                title="逐项对比"
                description="权重越高，对自动化与风控识别影响越大"
                actions={<Badge tone={failures.length === 0 ? 'success' : 'warning'}>{failures.length} 项未通过</Badge>}
              />
              <div className="table-wrap">
                <table className="data-table">
                  <thead>
                    <tr>
                      <th className="w-8"></th>
                      <th>项目</th>
                      <th>配置值</th>
                      <th>实际值</th>
                      <th className="text-center">权重</th>
                    </tr>
                  </thead>
                  <tbody>
                    {selected.checks.map((check, index) => (
                      <tr key={`${check.name}-${index}`}>
                        <td>
                          {check.passed ? (
                            <CheckCircle2 className="h-4 w-4 text-[var(--success)]" />
                          ) : (
                            <XCircle className="h-4 w-4 text-danger" />
                          )}
                        </td>
                        <td>
                          <p className="text-xs font-medium text-content-main">{check.name}</p>
                          {!check.passed ? <p className="mt-0.5 text-[10px] text-content-hint">{check.hint}</p> : null}
                        </td>
                        <td><p className="max-w-[240px] truncate font-mono text-[10px] text-content-weak" title={check.expected}>{check.expected}</p></td>
                        <td><p className="max-w-[240px] truncate font-mono text-[10px] text-content-main" title={check.actual}>{check.actual}</p></td>
                        <td className="text-center text-[11px] tabular-nums text-content-hint">{check.weight}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </Card>

            <Card>
              <CardHeader
                title="原始探针数据"
                description="由页面上下文实际读取，可用于排查注入脚本被覆盖的原因"
                actions={<CopyButton value={JSON.stringify(selected.observed, null, 2)} label="复制 JSON" size="sm" />}
              />
              <CardBody>
                <pre className="code-block max-h-72 whitespace-pre-wrap break-all">{JSON.stringify(selected.observed, null, 2)}</pre>
              </CardBody>
            </Card>

            {failures.length > 0 ? (
              <Card>
                <CardHeader title="修复建议" description="按检测项逐条排查" />
                <CardBody className="space-y-2">
                  {failures.map((check, index) => (
                    <div key={index} className="flex items-start gap-2 rounded-md border border-border p-2 text-xs">
                      <Info className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--warning)]" />
                      <div>
                        <p className="font-medium text-content-main">{check.name}</p>
                        <p className="mt-0.5 text-content-weak">{check.hint}</p>
                      </div>
                    </div>
                  ))}
                </CardBody>
              </Card>
            ) : null}
          </div>
        </div>
      )}
    </div>
  );
}
