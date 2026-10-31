import * as React from 'react';
import { Film, Puzzle, RefreshCw } from 'lucide-react';
import type { PluginInfo } from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { toastError, useToastStore } from '@/store/toast';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardHeader, Spinner } from '@/components/ui/card';
import { Switch } from '@/components/ui/switch';

const icons: Record<string, React.ReactNode> = {
  'video-sniffer': <Film className="h-4 w-4" />,
};

export function PluginsPanel() {
  const toast = useToastStore();
  const [plugins, setPlugins] = React.useState<PluginInfo[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [busy, setBusy] = React.useState<string | null>(null);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      setPlugins(await ipc.pluginList());
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    void load();
  }, [load]);

  const toggle = async (plugin: PluginInfo, enabled: boolean) => {
    setBusy(plugin.id);
    try {
      setPlugins(await ipc.pluginSetEnabled(plugin.id, enabled));
      toast.success(
        enabled ? `${plugin.name} 已启用` : `${plugin.name} 已暂停`,
        enabled ? '对之后打开或正在运行的受控标签页生效' : '已停止收集新数据',
      );
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader
          title="内置插件"
          description="插件随 FinEnv 本地运行，不加载远程代码；全局开关对所有环境生效。"
          actions={
            <Button variant="ghost" size="sm" onClick={() => void load()} disabled={loading}>
              <RefreshCw className="h-3.5 w-3.5" /> 刷新
            </Button>
          }
        />
        <CardBody className="space-y-3">
          {loading ? (
            <div className="flex justify-center py-8">
              <Spinner />
            </div>
          ) : (
            plugins.map((plugin) => (
              <div
                key={plugin.id}
                className="flex flex-wrap items-center gap-3 rounded-lg border border-border bg-surface-secondary p-4"
              >
                <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary-light text-primary">
                  {icons[plugin.id] ?? <Puzzle className="h-4 w-4" />}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-semibold text-content-main">{plugin.name}</span>
                    <Badge tone="outline">内置</Badge>
                    <Badge tone={plugin.enabled ? 'success' : 'neutral'}>
                      {plugin.enabled ? '运行中' : '已暂停'}
                    </Badge>
                    <span className="font-mono text-[10px] text-content-hint">v{plugin.version}</span>
                  </div>
                  <p className="mt-1 text-xs leading-5 text-content-weak">{plugin.description}</p>
                </div>
                <Switch
                  checked={plugin.enabled}
                  disabled={busy === plugin.id}
                  onCheckedChange={(value) => void toggle(plugin, value)}
                  label={plugin.enabled ? '已启用' : '已暂停'}
                />
              </div>
            ))
          )}
          <div className="rounded-lg border border-dashed border-border px-3.5 py-3 text-[11px] leading-5 text-content-hint">
            当前插件为编译内置，只有启用后才会接收网络事件。视频嗅探捕获的是 FinEnv 受控标签页的媒体响应；手动新开的标签页不在当前受控范围内。
          </div>
        </CardBody>
      </Card>
    </div>
  );
}
