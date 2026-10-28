import * as React from 'react';
import {
  Globe2,
  Network,
  Pencil,
  Plus,
  RefreshCw,
  RotateCw,
  ServerCog,
  Trash2,
  Wifi,
} from 'lucide-react';
import type {
  EnvironmentView,
  HostsEntry,
  HostsEntryInput,
  MockRule,
  MockRuleInput,
  Proxy,
  ProxyCheckResult,
} from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { toastError, useToastStore } from '@/store/toast';
import { formatDateShort } from '@/utils';
import { envHasAccess } from '@/utils/environment';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardHeader, EmptyState, Spinner } from '@/components/ui/card';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input, Select, Textarea } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';

export function NetworkingPanel({
  env,
  proxies,
  onUpdated,
}: {
  env: EnvironmentView;
  proxies: Proxy[];
  onUpdated: (env: EnvironmentView) => void;
}) {
  const toast = useToastStore();
  const canEdit =
    useAuthStore((state) => state.can('edit_fingerprint_proxy')) &&
    envHasAccess(env, 'manage');
  const [proxyId, setProxyId] = React.useState(env.proxyId ?? '');
  const [restart, setRestart] = React.useState(false);
  const [savingProxy, setSavingProxy] = React.useState(false);
  const [checking, setChecking] = React.useState(false);
  const [aligning, setAligning] = React.useState(false);
  const [checkResult, setCheckResult] = React.useState<ProxyCheckResult | null>(null);
  const selected = proxies.find((proxy) => proxy.id === proxyId) ?? null;

  const [hosts, setHosts] = React.useState<HostsEntry[]>([]);
  const [hostsLoading, setHostsLoading] = React.useState(true);
  const [savingHosts, setSavingHosts] = React.useState(false);

  const [mocks, setMocks] = React.useState<MockRule[]>([]);
  const [mocksLoading, setMocksLoading] = React.useState(true);
  const [savingMocks, setSavingMocks] = React.useState(false);
  const [mockEditor, setMockEditor] = React.useState<{ index: number; rule: MockRuleInput } | null>(null);

  React.useEffect(() => {
    setProxyId(env.proxyId ?? '');
  }, [env.id, env.proxyId]);

  const loadAux = React.useCallback(async () => {
    setHostsLoading(true);
    setMocksLoading(true);
    try {
      const [hostRows, mockRows] = await Promise.all([ipc.hostsList(env.id), ipc.mockList(env.id)]);
      setHosts(hostRows);
      setMocks(mockRows);
    } catch (error) {
      toastError(error);
    } finally {
      setHostsLoading(false);
      setMocksLoading(false);
    }
  }, [env.id]);

  React.useEffect(() => {
    void loadAux();
  }, [loadAux]);

  const applyProxy = async () => {
    setSavingProxy(true);
    try {
      const updated = await ipc.browserSwitchProxy(env.id, proxyId || null, restart);
      onUpdated(updated);
      toast.success('代理绑定已更新', restart && env.runtime ? '浏览器已重启生效' : env.runtime ? '运行中的环境需重启后生效' : undefined);
      setCheckResult(null);
    } catch (error) {
      toastError(error);
    } finally {
      setSavingProxy(false);
    }
  };

  const check = async () => {
    if (!proxyId) {
      toast.warning('请选择一个代理');
      return;
    }
    setChecking(true);
    try {
      const result = await ipc.proxyCheck(proxyId);
      setCheckResult(result);
      if (result.ok) toast.success('代理连通', `${result.exitIp ?? '未知出口'} · ${result.latencyMs ?? '—'}ms`);
      else toast.error('代理检测失败', result.message);
    } catch (error) {
      toastError(error);
    } finally {
      setChecking(false);
    }
  };

  const align = async () => {
    if (!env.proxyId) {
      toast.warning('请先保存代理绑定，再执行时区对齐');
      return;
    }
    setAligning(true);
    try {
      const result = await ipc.proxyAlign(env.id);
      setCheckResult(result.check);
      onUpdated(result.environment);
      toast.success('已按出口 IP 对齐', `${result.environment.fingerprint.timezone} · ${result.environment.fingerprint.languages.join(',')}`);
    } catch (error) {
      toastError(error);
    } finally {
      setAligning(false);
    }
  };

  const saveHosts = async () => {
    setSavingHosts(true);
    try {
      const entries: HostsEntryInput[] = hosts.map((row) => ({
        host: row.host,
        ip: row.ip,
        enabled: row.enabled,
      }));
      setHosts(await ipc.hostsSet(env.id, entries));
      toast.success('Hosts 已保存', env.runtime ? '当前环境运行中，需重启后生效' : undefined);
    } catch (error) {
      toastError(error);
    } finally {
      setSavingHosts(false);
    }
  };

  const saveMocks = async () => {
    setSavingMocks(true);
    try {
      const rules: MockRuleInput[] = mocks.map((rule) => ({
        pattern: rule.pattern,
        method: rule.method,
        statusCode: rule.statusCode,
        headers: rule.headers,
        body: rule.body,
        delayMs: rule.delayMs,
        enabled: rule.enabled,
      }));
      setMocks(await ipc.mockSet(env.id, rules));
      toast.success('Mock 规则已保存', env.runtime ? '运行中的环境立即生效' : undefined);
    } catch (error) {
      toastError(error);
    } finally {
      setSavingMocks(false);
    }
  };

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader
          title="代理与网络隔离"
          description="每个环境独立绑定 HTTP / HTTPS / SOCKS5 代理；WebRTC 与 DNS 防泄漏参数随进程启动注入。"
          actions={
            <Button variant="ghost" size="sm" onClick={() => void check()} loading={checking} disabled={!proxyId}>
              <Wifi className="h-3.5 w-3.5" /> 检测连通
            </Button>
          }
        />
        <CardBody className="space-y-3">
          <div className="grid items-end gap-3 md:grid-cols-[minmax(0,1fr)_auto_auto]">
            <Field label="绑定代理">
              <Select value={proxyId} onChange={(event) => setProxyId(event.target.value)} placeholder="直连（不使用代理）">
                {proxies.map((proxy) => (
                  <option key={proxy.id} value={proxy.id}>
                    {proxy.name} · {proxy.protocol}://{proxy.host}:{proxy.port}
                  </option>
                ))}
              </Select>
            </Field>
            <Button variant="secondary" onClick={() => void check()} loading={checking} disabled={!proxyId}>
              <RefreshCw className="h-4 w-4" /> 检测
            </Button>
            <Button variant="primary" onClick={() => void applyProxy()} loading={savingProxy} disabled={!canEdit}>
              保存绑定
            </Button>
          </div>
          {env.runtime ? (
            <div className="rounded-md border border-border bg-surface-secondary p-3">
              <Switch
                checked={restart}
                onCheckedChange={setRestart}
                label="保存后立即重启浏览器"
                description="代理在 Chromium 启动参数中生效，切换后必须重启进程。"
              />
            </div>
          ) : null}
          <div className="grid gap-3 md:grid-cols-2">
            <div className="rounded-md border border-border bg-surface-secondary p-3">
              <p className="text-[10px] uppercase tracking-widest text-content-hint">当前代理</p>
              {selected ? (
                <div className="mt-1.5 space-y-1 text-xs">
                  <p className="font-medium text-content-main">{selected.name}</p>
                  <p className="font-mono text-[11px] text-content-weak">
                    {selected.protocol}://{selected.username ? `${selected.username}@` : ''}
                    {selected.host}:{selected.port}
                  </p>
                  <p className="text-content-hint">
                    认证 {selected.username ? (selected.hasPassword ? '已配置' : '仅用户名') : '无'} ·{' '}
                    {selected.lastCheckAt ? `上次检测 ${formatDateShort(selected.lastCheckAt)}` : '未检测'}
                  </p>
                </div>
              ) : (
                <p className="mt-1.5 text-xs text-content-weak">直连（无代理）。生产测试建议绑定独立 IP。</p>
              )}
            </div>
            <div className="rounded-md border border-border bg-surface-secondary p-3">
              <p className="text-[10px] uppercase tracking-widest text-content-hint">出口检测</p>
              {checkResult ? (
                <div className="mt-1.5 space-y-1 text-xs">
                  <p className={checkResult.ok ? 'text-[var(--success)]' : 'text-danger'}>
                    {checkResult.ok ? '出口可用' : '出口不可用'} · {checkResult.latencyMs ?? '—'}ms
                  </p>
                  <p className="font-mono text-[11px] text-content-main">{checkResult.exitIp ?? '未解析到出口 IP'}</p>
                  <p className="text-content-hint">
                    {checkResult.country ?? '未知国家'} · {checkResult.timezone ?? '未知时区'} · {checkResult.city ?? '—'}
                  </p>
                  <p className="text-content-hint">{checkResult.message}</p>
                </div>
              ) : (
                <div className="mt-1.5">
                  <p className="text-xs text-content-weak">检测会通过代理请求出口 IP 服务，并返回国家与时区。</p>
                </div>
              )}
              <div className="mt-2">
                <Button variant="secondary" size="sm" loading={aligning} disabled={!env.proxyId} onClick={() => void align()}>
                  <RotateCw className="h-3.5 w-3.5" /> 按时区对齐指纹
                </Button>
              </div>
            </div>
          </div>
          <div className="flex items-start gap-2 rounded-md bg-[color-mix(in_srgb,var(--color-primary)_7%,transparent)] px-3 py-2 text-[11px] text-content-weak">
            <Network className="mt-0.5 h-3.5 w-3.5 shrink-0 text-primary" />
            使用代理时自动追加 <code className="font-mono">--disable-quic</code> 与{' '}
            <code className="font-mono">--webrtc-ip-handling-policy</code>，降低 DNS / WebRTC 泄漏风险。直连环境仍需依赖网络侧隔离。
          </div>
        </CardBody>
      </Card>

      <Card>
        <CardHeader
          title="独立 Hosts"
          description="通过 Chromium --host-resolver-rules 注入，仅作用于本环境；修改后需重启浏览器。"
          actions={
            <>
              <Badge tone="outline">{hosts.length} 条</Badge>
              {canEdit ? (
                <>
                  <Button variant="ghost" size="sm" onClick={() => setHosts((current) => [...current, { id: `new-${Date.now()}`, envId: env.id, host: '', ip: '', enabled: true }])}>
                    <Plus className="h-3.5 w-3.5" /> 添加
                  </Button>
                  <Button variant="primary" size="sm" loading={savingHosts} onClick={() => void saveHosts()}>
                    保存
                  </Button>
                </>
              ) : null}
            </>
          }
        />
        <CardBody>
          {hostsLoading ? (
            <div className="flex justify-center py-6"><Spinner /></div>
          ) : hosts.length === 0 ? (
            <EmptyState icon={<ServerCog className="h-5 w-5" />} title="未配置独立 Hosts" description="用于将测试域名固定解析到指定环境 IP，例如把 api.example.test 指向 10.0.0.12。" />
          ) : (
            <div className="space-y-2">
              {hosts.map((row, index) => (
                <div key={row.id} className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto_auto] items-center gap-2">
                  <Input
                    className="font-mono text-xs"
                    placeholder="api.example.test"
                    value={row.host}
                    disabled={!canEdit}
                    onChange={(event) =>
                      setHosts((current) => current.map((item, i) => (i === index ? { ...item, host: event.target.value } : item)))
                    }
                  />
                  <Input
                    className="font-mono text-xs"
                    placeholder="10.0.0.12"
                    value={row.ip}
                    disabled={!canEdit}
                    onChange={(event) =>
                      setHosts((current) => current.map((item, i) => (i === index ? { ...item, ip: event.target.value } : item)))
                    }
                  />
                  <label className="flex items-center gap-1.5 text-[11px] text-content-weak">
                    <input
                      type="checkbox"
                      className="accent-[var(--color-primary)]"
                      checked={row.enabled}
                      disabled={!canEdit}
                      onChange={(event) =>
                        setHosts((current) => current.map((item, i) => (i === index ? { ...item, enabled: event.target.checked } : item)))
                      }
                    />
                    启用
                  </label>
                  {canEdit ? (
                    <Button variant="ghost" size="icon-sm" onClick={() => setHosts((current) => current.filter((_, i) => i !== index))}>
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  ) : <span />}
                </div>
              ))}
            </div>
          )}
        </CardBody>
      </Card>

      <Card>
        <CardHeader
          title="独立 Mock 规则"
          description="基于 CDP Fetch 域拦截请求并本地响应；运行中的环境保存后立即生效。"
          actions={
            <>
              <Badge tone="outline">{mocks.length} 条</Badge>
              {canEdit ? (
                <>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() =>
                      setMockEditor({
                        index: -1,
                        rule: {
                          pattern: '*',
                          method: 'ANY',
                          statusCode: 200,
                          headers: { 'Content-Type': 'application/json' },
                          body: '{\n  "code": 0\n}',
                          delayMs: 0,
                          enabled: true,
                        },
                      })
                    }
                  >
                    <Plus className="h-3.5 w-3.5" /> 添加规则
                  </Button>
                  <Button variant="primary" size="sm" loading={savingMocks} onClick={() => void saveMocks()}>
                    保存
                  </Button>
                </>
              ) : null}
            </>
          }
        />
        <CardBody>
          {mocksLoading ? (
            <div className="flex justify-center py-6"><Spinner /></div>
          ) : mocks.length === 0 ? (
            <EmptyState
              icon={<Globe2 className="h-5 w-5" />}
              title="未配置 Mock 规则"
              description="支持通配符 URL 匹配，例如 *://api.example.com/orders*，为测试提供稳定可控的接口响应。"
            />
          ) : (
            <div className="table-wrap -mx-4 -mb-4">
              <table className="data-table">
                <thead>
                  <tr>
                    <th>匹配模式</th>
                    <th>方法</th>
                    <th>状态码</th>
                    <th>延迟</th>
                    <th>启用</th>
                    <th className="text-right">操作</th>
                  </tr>
                </thead>
                <tbody>
                  {mocks.map((rule, index) => (
                    <tr key={rule.id}>
                      <td>
                        <p className="max-w-[360px] truncate font-mono text-[11px] text-content-main" title={rule.pattern}>
                          {rule.pattern}
                        </p>
                        <p className="mt-0.5 max-w-[360px] truncate font-mono text-[10px] text-content-hint">{rule.body}</p>
                      </td>
                      <td><Badge tone="outline">{rule.method}</Badge></td>
                      <td className="font-mono text-xs">{rule.statusCode}</td>
                      <td className="text-xs text-content-weak">{rule.delayMs}ms</td>
                      <td>
                        <input
                          type="checkbox"
                          className="accent-[var(--color-primary)]"
                          checked={rule.enabled}
                          disabled={!canEdit}
                          onChange={(event) =>
                            setMocks((current) => current.map((item, i) => (i === index ? { ...item, enabled: event.target.checked } : item)))
                          }
                        />
                      </td>
                      <td>
                        <div className="flex items-center justify-end gap-1">
                          {canEdit ? (
                            <>
                              <Button
                                variant="ghost"
                                size="icon-sm"
                                onClick={() =>
                                  setMockEditor({
                                    index,
                                    rule: {
                                      pattern: rule.pattern,
                                      method: rule.method,
                                      statusCode: rule.statusCode,
                                      headers: rule.headers,
                                      body: rule.body,
                                      delayMs: rule.delayMs,
                                      enabled: rule.enabled,
                                    },
                                  })
                                }
                              >
                                <Pencil className="h-3.5 w-3.5" />
                              </Button>
                              <Button variant="ghost" size="icon-sm" className="text-danger" onClick={() => setMocks((current) => current.filter((_, i) => i !== index))}>
                                <Trash2 className="h-3.5 w-3.5" />
                              </Button>
                            </>
                          ) : null}
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardBody>
      </Card>

      <MockEditorDialog
        open={!!mockEditor}
        onOpenChange={(value) => !value && setMockEditor(null)}
        state={mockEditor}
        onSave={(rule) => {
          setMocks((current) => {
            if (!mockEditor) return current;
            if (mockEditor.index >= 0) {
              return current.map((item, index) =>
                index === mockEditor.index
                  ? { ...item, ...rule, headers: rule.headers ?? {}, body: rule.body ?? '', method: rule.method ?? 'ANY', statusCode: rule.statusCode ?? 200, delayMs: rule.delayMs ?? 0, enabled: rule.enabled ?? true }
                  : item,
              );
            }
            return [
              ...current,
              {
                id: `new-${Date.now()}`,
                envId: env.id,
                pattern: rule.pattern,
                method: rule.method ?? 'ANY',
                statusCode: rule.statusCode ?? 200,
                headers: rule.headers ?? {},
                body: rule.body ?? '',
                delayMs: rule.delayMs ?? 0,
                enabled: rule.enabled ?? true,
              },
            ];
          });
          setMockEditor(null);
        }}
      />
    </div>
  );
}

function MockEditorDialog({
  open,
  onOpenChange,
  state,
  onSave,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  state: { index: number; rule: MockRuleInput } | null;
  onSave: (rule: MockRuleInput) => void;
}) {
  const toast = useToastStore();
  const [form, setForm] = React.useState<MockRuleInput>(state?.rule ?? { pattern: '' });
  const [headersText, setHeadersText] = React.useState(JSON.stringify(state?.rule.headers ?? {}, null, 2));
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!state) return;
    setForm(state.rule);
    setHeadersText(JSON.stringify(state.rule.headers ?? {}, null, 2));
    setError(null);
  }, [state]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={state && state.index >= 0 ? '编辑 Mock 规则' : '新增 Mock 规则'}
      description="URL 使用 * 通配符；规则按顺序匹配，首个命中的规则生效。"
      width="max-w-2xl"
      footer={
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)}>取消</Button>
          <Button
            variant="primary"
            onClick={() => {
              if (!form.pattern.trim()) {
                toast.warning('匹配模式不能为空');
                return;
              }
              try {
                const headers = headersText.trim() ? (JSON.parse(headersText) as Record<string, unknown>) : {};
                onSave({ ...form, headers });
              } catch (parseError) {
                setError(parseError instanceof Error ? parseError.message : 'JSON 格式错误');
              }
            }}
          >
            确认
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="URL 匹配模式" required hint="示例：*://api.example.com/orders*">
          <Input
            className="font-mono text-xs"
            value={form.pattern}
            onChange={(event) => setForm((current) => ({ ...current, pattern: event.target.value }))}
            placeholder="*://api.example.com/*"
            autoFocus
          />
        </Field>
        <div className="grid grid-cols-3 gap-3">
          <Field label="HTTP 方法">
            <Select value={form.method ?? 'ANY'} onChange={(event) => setForm((current) => ({ ...current, method: event.target.value }))}>
              {['ANY', 'GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS'].map((method) => (
                <option key={method} value={method}>{method}</option>
              ))}
            </Select>
          </Field>
          <Field label="状态码">
            <Input type="number" min={100} max={599} value={form.statusCode ?? 200} onChange={(event) => setForm((current) => ({ ...current, statusCode: Number(event.target.value) }))} />
          </Field>
          <Field label="延迟 ms">
            <Input type="number" min={0} max={60000} value={form.delayMs ?? 0} onChange={(event) => setForm((current) => ({ ...current, delayMs: Number(event.target.value) }))} />
          </Field>
        </div>
        <Field label="响应头 JSON" error={error}>
          <Textarea className="min-h-[90px] font-mono text-[11px]" value={headersText} onChange={(event) => { setHeadersText(event.target.value); setError(null); }} />
        </Field>
        <Field label="响应体">
          <Textarea className="min-h-[140px] font-mono text-[11px]" value={form.body ?? ''} onChange={(event) => setForm((current) => ({ ...current, body: event.target.value }))} />
        </Field>
        <Switch checked={form.enabled ?? true} onCheckedChange={(checked) => setForm((current) => ({ ...current, enabled: checked }))} label="启用该规则" />
      </div>
    </Dialog>
  );
}
