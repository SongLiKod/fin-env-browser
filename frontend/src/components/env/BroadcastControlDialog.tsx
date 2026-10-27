import * as React from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  MousePointerClick,
  Pause,
  Play,
  Radio,
  Square,
} from 'lucide-react';
import { ipc } from '@/ipc/client';
import type { BroadcastStatus, EnvironmentView } from '@/ipc/types';
import { useEnvStore } from '@/store/env';
import { toastError, useToastStore } from '@/store/toast';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input, Select } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';

export function BroadcastControlDialog({
  open,
  onOpenChange,
  envs,
  onChanged,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  envs: EnvironmentView[];
  onChanged: () => void;
}) {
  const toast = useToastStore();
  const allEnvs = useEnvStore((state) => state.envs);
  const [status, setStatus] = React.useState<BroadcastStatus | null>(null);
  const [masterId, setMasterId] = React.useState('');
  const [url, setUrl] = React.useState('');
  const [openMissing, setOpenMissing] = React.useState(true);
  const [busy, setBusy] = React.useState(false);
  const [loading, setLoading] = React.useState(false);

  const master =
    allEnvs.find((env) => env.id === status?.masterEnvId) ??
    envs.find((env) => env.id === status?.masterEnvId) ??
    null;

  React.useEffect(() => {
    if (!open) return;
    setBusy(false);
    setLoading(true);
    setStatus(null);
    setUrl('');
    setOpenMissing(true);
    void (async () => {
      try {
        const next = await ipc.broadcastStatus();
        setStatus(next.active ? next : null);
      } catch (error) {
        toastError(error);
      } finally {
        setLoading(false);
      }
    })();
  }, [open]);

  React.useEffect(() => {
    if (!masterId || !envs.some((env) => env.id === masterId)) {
      setMasterId(envs[0]?.id ?? '');
    }
  }, [envs, masterId]);

  React.useEffect(() => {
    if (!open || !status?.active) return;
    const timer = window.setInterval(() => {
      void ipc
        .broadcastStatus()
        .then((next) => setStatus(next.active ? next : null))
        .catch(() => undefined);
    }, 1500);
    return () => window.clearInterval(timer);
  }, [open, status?.active]);

  const start = async () => {
    if (envs.length < 2) {
      toast.warning('至少选择 2 个环境');
      return;
    }
    if (!masterId) {
      toast.warning('请选择主控环境');
      return;
    }
    if (url.trim() && !/^https?:\/\//i.test(url.trim())) {
      toast.warning('请输入 http:// 或 https:// 地址，或留空保持当前页面');
      return;
    }
    setBusy(true);
    try {
      const next = await ipc.broadcastStart({
        envIds: envs.map((env) => env.id),
        masterEnvId: masterId,
        url: url.trim() || null,
        openMissing,
      });
      setStatus(next);
      onChanged();
      if (next.errors.length > 0) {
        toast.warning(
          '同步控制已开启，但有环境未加入',
          next.errors.slice(0, 2).join('；'),
        );
      } else {
        toast.success(
          '同步控制已开启',
          `主控窗口：${allEnvs.find((env) => env.id === next.masterEnvId)?.name ?? next.masterEnvId} · ${next.envIds.length} 个窗口`,
        );
      }
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  const togglePause = async () => {
    if (!status) return;
    setBusy(true);
    try {
      const next = await ipc.broadcastPause(!status.paused);
      setStatus(next.active ? next : null);
      toast.success(next.paused ? '同步控制已暂停' : '同步控制已恢复');
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    try {
      await ipc.broadcastStop('控制面板停止');
      setStatus(null);
      onChanged();
      toast.success('同步控制已停止', '浏览器窗口保持打开');
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
      title={status?.active ? '同步控制进行中' : '开启同步控制'}
      description={
        status?.active
          ? '在一个主控窗口中操作，鼠标点击、滚动和键盘输入会实时同步到其他参与窗口。'
          : '选择参与环境并指定主控窗口；开启后请直接在 master 浏览器窗口中操作。'
      }
      width="max-w-2xl"
      footer={
        status?.active ? (
          <>
            <span className="mr-auto text-[11px] text-content-hint">
              关闭此面板不会停止同步，顶部状态栏可随时暂停或停止。
            </span>
            <Button variant="secondary" onClick={() => onOpenChange(false)}>
              关闭
            </Button>
            <Button variant="danger" loading={busy} onClick={() => void stop()}>
              <Square className="h-4 w-4" /> 停止同步
            </Button>
          </>
        ) : (
          <>
            <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
              取消
            </Button>
            <Button
              variant="primary"
              loading={busy}
              disabled={loading || envs.length < 2 || !masterId}
              onClick={() => void start()}
            >
              <Radio className="h-4 w-4" /> 开启同步控制
            </Button>
          </>
        )
      }
    >
      {loading ? (
        <p className="py-10 text-center text-xs text-content-hint">正在读取同步状态…</p>
      ) : status?.active ? (
        <div className="space-y-4">
          <div className="rounded-lg border border-primary/30 bg-[color-mix(in_srgb,var(--color-primary)_8%,var(--bg-base))] p-4">
            <div className="flex flex-wrap items-center gap-2">
              <Badge tone={status.paused ? 'warning' : 'success'}>
                {status.paused ? '已暂停' : '同步中'}
              </Badge>
              <span className="text-sm font-semibold text-content-main">
                主控：{master?.name ?? status.masterEnvId}
              </span>
              <span className="text-xs text-content-weak">
                {status.envIds.length} 个窗口参与 · 已转发 {status.forwarded} 次输入
              </span>
            </div>
            <p className="mt-2 flex items-start gap-1.5 text-xs leading-5 text-content-weak">
              <MousePointerClick className="mt-0.5 h-3.5 w-3.5 shrink-0 text-primary" />
              请在「{master?.name ?? '主控'}」窗口中点击输入框、按钮或滚动页面；其他环境会执行相同的鼠标与键盘事件。
              面板可以最小化，浏览器窗口需要保持打开。
            </p>
          </div>

          <div className="flex flex-wrap gap-1.5">
            {status.envIds.map((id) => {
              const env = allEnvs.find((item) => item.id === id) ?? envs.find((item) => item.id === id);
              return (
                <Badge key={id} tone={id === status.masterEnvId ? 'primary' : 'neutral'}>
                  {id === status.masterEnvId ? '主控 · ' : ''}
                  {env?.name ?? id.slice(0, 8)}
                </Badge>
              );
            })}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button variant="secondary" loading={busy} onClick={() => void togglePause()}>
              {status.paused ? <Play className="h-4 w-4" /> : <Pause className="h-4 w-4" />}
              {status.paused ? '恢复同步' : '暂停同步'}
            </Button>
            <span className="text-[11px] text-content-hint">
              暂停后主控窗口仍可正常操作，但不会向其他窗口转发输入。
            </span>
          </div>

          {status.errors.length > 0 ? (
            <div className="rounded-lg border border-[color-mix(in_srgb,var(--warning)_35%,transparent)] bg-[color-mix(in_srgb,var(--warning)_8%,transparent)] p-3">
              <p className="flex items-center gap-1.5 text-xs font-medium text-content-main">
                <AlertTriangle className="h-3.5 w-3.5 text-[var(--warning)]" />
                {status.errors.length} 个环境未加入同步
              </p>
              <ul className="mt-1 space-y-0.5 text-[11px] text-content-weak">
                {status.errors.map((error, index) => (
                  <li key={`${index}-${error}`}>{error}</li>
                ))}
              </ul>
            </div>
          ) : (
            <p className="flex items-center gap-1.5 text-[11px] text-[var(--success)]">
              <CheckCircle2 className="h-3.5 w-3.5" /> 所有选中环境均已加入同步控制
            </p>
          )}
        </div>
      ) : (
        <div className="space-y-4">
          <div className="rounded-lg border border-border bg-surface-secondary/60 p-3">
            <p className="text-xs font-medium text-content-main">
              已选择 {envs.length} 个环境
            </p>
            <div className="mt-1.5 flex flex-wrap gap-1.5">
              {envs.slice(0, 12).map((env) => (
                <Badge key={env.id} tone={env.runtime ? 'success' : 'neutral'}>
                  {env.name}
                </Badge>
              ))}
              {envs.length > 12 ? (
                <span className="self-center text-[11px] text-content-hint">+{envs.length - 12} 个</span>
              ) : null}
            </div>
            {envs.length < 2 ? (
              <p className="mt-2 text-[11px] text-danger">请至少选择 2 个环境后再开启同步控制。</p>
            ) : null}
          </div>

          <div className="grid grid-cols-2 gap-3">
            <Field label="主控窗口" hint="你只在这个窗口中操作，其余窗口接收同步事件">
              <Select value={masterId} onChange={(event) => setMasterId(event.target.value)}>
                {envs.map((env) => (
                  <option key={env.id} value={env.id}>
                    {env.name}
                    {env.runtime ? ' · 运行中' : ' · 未运行'}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label="统一打开地址" hint="可选；留空则保持各窗口当前页面">
              <Input
                value={url}
                onChange={(event) => setUrl(event.target.value)}
                placeholder="https://test.example.com"
              />
            </Field>
          </div>

          <Switch
            checked={openMissing}
            onCheckedChange={setOpenMissing}
            label="自动打开未运行的环境"
            description="开启后会把选中的窗口全部启动；若填写了统一地址，会先导航到该地址再开始同步。"
          />

          <div className="rounded-lg border border-border bg-surface-secondary/60 p-3 text-[11px] leading-5 text-content-hint">
            <p className="font-medium text-content-weak">同步范围</p>
            <p>鼠标移动、左/中/右键点击、滚轮、键盘输入、粘贴与中文输入法上屏文字。</p>
            <p className="mt-1">浏览器地址栏、前进/后退按钮和标签栏操作不会同步；页面内点击链接、表单输入与滚动会同步。</p>
          </div>
        </div>
      )}
    </Dialog>
  );
}
