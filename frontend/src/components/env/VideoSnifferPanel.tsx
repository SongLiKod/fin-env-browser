import * as React from 'react';
import { Download, Film, RefreshCw, Trash2 } from 'lucide-react';
import type { EnvironmentView, SniffedVideo } from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { toastError, useToastStore } from '@/store/toast';
import { formatBytes } from '@/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardHeader, EmptyState, Spinner } from '@/components/ui/card';

const kindLabels: Record<string, string> = {
  video: '视频',
  audio: '音频',
  hls: 'HLS 流',
  dash: 'DASH 流',
};

export function VideoSnifferPanel({ env }: { env: EnvironmentView }) {
  const toast = useToastStore();
  const [enabled, setEnabled] = React.useState(false);
  const [videos, setVideos] = React.useState<SniffedVideo[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [downloading, setDownloading] = React.useState<string | null>(null);

  const load = React.useCallback(
    async (quiet = false) => {
      if (!quiet) setLoading(true);
      try {
        const [plugins, items] = await Promise.all([ipc.pluginList(), ipc.videoList(env.id)]);
        setEnabled(plugins.some((plugin) => plugin.id === 'video-sniffer' && plugin.enabled));
        setVideos(items);
      } catch (error) {
        if (!quiet) toastError(error);
      } finally {
        if (!quiet) setLoading(false);
      }
    },
    [env.id],
  );

  React.useEffect(() => {
    void load();
  }, [load]);

  React.useEffect(() => {
    if (!enabled) return;
    const timer = window.setInterval(() => void load(true), 2500);
    return () => window.clearInterval(timer);
  }, [enabled, load]);

  const download = async (video: SniffedVideo) => {
    setDownloading(video.id);
    try {
      const path = await ipc.videoDownload(env.id, video.id);
      toast.success('视频下载完成', path);
    } catch (error) {
      toast.error(
        video.downloadable ? '视频下载失败' : '合并下载失败',
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setDownloading(null);
    }
  };

  const clear = async () => {
    try {
      await ipc.videoClear(env.id);
      setVideos([]);
      toast.success('已清空视频列表');
    } catch (error) {
      toastError(error);
    }
  };

  return (
    <Card>
      <CardHeader
        title="视频嗅探"
        description="捕获当前受控标签页的视频/音频响应；B 站 DASH 分片会自动解析并合并为 MP4，其他 HLS/DASH 可复制流地址。"
        actions={
          <div className="flex items-center gap-1.5">
            <Button variant="ghost" size="sm" onClick={() => void load()}>
              <RefreshCw className="h-3.5 w-3.5" /> 刷新
            </Button>
            {videos.length > 0 ? (
              <Button variant="ghost" size="sm" onClick={() => void clear()}>
                <Trash2 className="h-3.5 w-3.5" /> 清空
              </Button>
            ) : null}
          </div>
        }
      />
      <CardBody>
        <div className="mb-3 rounded-md border border-border bg-surface-secondary px-3 py-2 text-[10px] leading-5 text-content-hint">
          只检测当前环境受控标签页。当前页面：
          <span className="ml-1 break-all font-mono text-content-weak">
            {env.runtime?.currentUrl || '环境未运行'}
          </span>
          <br />
          切换页面后旧页面视频会自动清空；如果在新标签页打开视频，FinEnv 当前不会自动接管该标签页。
        </div>
        {!enabled ? (
          <EmptyState
            icon={<Film className="h-5 w-5" />}
            title="视频嗅探未启用"
            description="请到「设置 → 插件」启用视频嗅探。启用后会对当前环境正在加载的媒体请求进行采集。"
          />
        ) : loading ? (
          <div className="flex justify-center py-10">
            <Spinner />
          </div>
        ) : videos.length === 0 ? (
          <EmptyState
            icon={<Film className="h-5 w-5" />}
            title="暂时没有发现视频"
            description="在受控标签页中播放视频，或刷新页面后再回来查看；视频列表每 2.5 秒自动更新。"
          />
        ) : (
          <div className="divide-y divide-border">
            {videos.map((video) => (
              <div key={video.id} className="flex flex-wrap items-center gap-3 py-3 first:pt-0 last:pb-0">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary-light text-primary">
                  <Film className="h-4 w-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="max-w-full truncate text-sm font-medium text-content-main" title={video.title}>
                      {video.title}
                    </span>
                    <Badge tone={video.downloadable ? 'success' : 'warning'}>
                      {kindLabels[video.kind] ?? video.kind}
                    </Badge>
                    {video.size ? (
                      <span className="text-[10px] text-content-hint">{formatBytes(video.size)}</span>
                    ) : null}
                  </div>
                  <p className="mt-1 truncate font-mono text-[10px] text-content-hint" title={video.url}>
                    {video.url}
                  </p>
                </div>
                {video.downloadable ? (
                  <Button
                    variant="secondary"
                    size="sm"
                    loading={downloading === video.id}
                    onClick={() => void download(video)}
                  >
                    <Download className="h-3.5 w-3.5" /> 下载
                  </Button>
                ) : (
                  <div className="flex items-center gap-1.5">
                    <Button
                      variant="secondary"
                      size="sm"
                      loading={downloading === video.id}
                      onClick={() => void download(video)}
                    >
                      <Download className="h-3.5 w-3.5" /> 合并下载
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={async () => {
                        try {
                          await navigator.clipboard.writeText(video.url);
                          toast.success('已复制流地址');
                        } catch {
                          toast.info('流地址', video.url);
                        }
                      }}
                    >
                      复制
                    </Button>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </CardBody>
    </Card>
  );
}
