import * as React from 'react';
import {
  Camera,
  CircleStop,
  Code2,
  Cookie,
  Download,
  ExternalLink,
  Film,
  FolderDown,
  FolderOpen,
  Globe2,
  HardDriveDownload,
  Play,
  Radar,
  RefreshCw,
  Terminal,
  Video,
} from 'lucide-react';
import type {
  CdpInfo,
  CookieRecord,
  EnvironmentView,
  RecordingResult,
  ScreenshotResult,
} from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { toastError, useToastStore } from '@/store/toast';
import { copyText, formatDate, formatDuration } from '@/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardHeader, Spinner } from '@/components/ui/card';
import { CopyButton } from '@/components/ui/misc';
import { Dialog } from '@/components/ui/dialog';
import { Field, Input, Textarea } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';

export function DebugPanel({
  env,
  onRefresh,
}: {
  env: EnvironmentView;
  onRefresh: () => Promise<void>;
}) {
  const toast = useToastStore();
  const canOpen = useAuthStore((state) => state.can('open_close'));
  const canSensitive = useAuthStore((state) => state.can('export_sensitive'));
  const [url, setUrl] = React.useState('https://');
  const [preScript, setPreScript] = React.useState('');
  const [headless, setHeadless] = React.useState(false);
  const [busy, setBusy] = React.useState<string | null>(null);
  const [cdp, setCdp] = React.useState<CdpInfo | null>(null);
  const [cdpError, setCdpError] = React.useState<string | null>(null);
  const [screenshot, setScreenshot] = React.useState<ScreenshotResult | null>(null);
  const [recordingResult, setRecordingResult] = React.useState<RecordingResult | null>(null);
  const [recordingInterval, setRecordingInterval] = React.useState('1000');
  const [harPath, setHarPath] = React.useState<string | null>(null);
  const [script, setScript] = React.useState('document.title');
  const [scriptResult, setScriptResult] = React.useState<unknown>(null);
  const [cookies, setCookies] = React.useState<CookieRecord[] | null>(null);
  const [cookieEditor, setCookieEditor] = React.useState(false);
  const [cookieText, setCookieText] = React.useState('[]');
  const [navigateUrl, setNavigateUrl] = React.useState('');
  const running = !!env.runtime;

  const loadCdp = React.useCallback(async () => {
    if (!running) {
      setCdp(null);
      return;
    }
    try {
      setCdp(await ipc.browserCdpInfo(env.id));
      setCdpError(null);
    } catch (error) {
      setCdp(null);
      setCdpError(error instanceof Error ? error.message : 'CDP 信息不可用');
    }
  }, [env.id, running]);

  React.useEffect(() => {
    void loadCdp();
  }, [loadCdp]);

  const open = async () => {
    setBusy('open');
    try {
      const runtime = await ipc.browserOpen(env.id, {
        url: url.trim() && url.trim() !== 'https://' ? url.trim() : null,
        preScript: preScript.trim() || null,
        headless,
      });
      toast.success('环境已启动', `PID ${runtime.pid} · 调试端口 ${runtime.debugPort}`);
      if (runtime.warnings.length > 0) toast.warning('启动告警', runtime.warnings.join('；'));
      await onRefresh();
      await loadCdp();
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(null);
    }
  };

  const navigate = async () => {
    if (!navigateUrl.trim()) return;
    setBusy('navigate');
    try {
      await ipc.browserNavigate(env.id, navigateUrl.trim());
      toast.success('页面已跳转');
      await onRefresh();
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
      <div className="space-y-4">
        <Card>
          <CardHeader
            title={running ? '启动参数与导航' : '打开环境'}
            description={running ? '环境已运行，可继续导航或执行脚本。' : '打开时自动访问测试地址并可执行前置脚本。'}
            actions={running ? <Badge tone="success">运行中</Badge> : <Badge tone="neutral">未运行</Badge>}
          />
          <CardBody className="space-y-3">
            {!running ? (
              <>
                <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
                  <Field label="启动后访问地址" hint="留空使用 FinEnv 内置环境信息启动页">
                    <Input value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://test.example.com/login" />
                  </Field>
                  <div className="flex items-end pb-1">
                    <Switch checked={headless} onCheckedChange={setHeadless} label="无头模式" />
                  </div>
                </div>
                <Field label="前置脚本（可选）" hint="在页面加载完成后通过 CDP Runtime.evaluate 执行">
                  <Textarea
                    className="min-h-[110px] font-mono text-[11px]"
                    value={preScript}
                    onChange={(event) => setPreScript(event.target.value)}
                    placeholder="// 例如：localStorage.setItem('token','test-token')"
                  />
                </Field>
                <Button variant="primary" loading={busy === 'open'} disabled={!canOpen} onClick={() => void open()}>
                  <Play className="h-4 w-4" /> 启动独立浏览器
                </Button>
              </>
            ) : (
              <>
                <div className="flex gap-2">
                  <Input value={navigateUrl} onChange={(event) => setNavigateUrl(event.target.value)} placeholder="https://test.example.com/orders" />
                  <Button variant="secondary" loading={busy === 'navigate'} onClick={() => void navigate()}>
                    <ExternalLink className="h-4 w-4" /> 跳转
                  </Button>
                  <Button variant="secondary" onClick={() => window.open(env.runtime?.currentUrl || 'about:blank')} disabled={!env.runtime?.currentUrl || env.runtime.currentUrl === 'about:blank'}>
                    <Globe2 className="h-4 w-4" /> 当前页
                  </Button>
                </div>
                <div className="grid grid-cols-2 gap-2 text-xs md:grid-cols-4">
                  <div className="rounded-md border border-border bg-surface-secondary p-2">
                    <p className="text-[10px] text-content-hint">PID</p>
                    <p className="font-mono text-content-main">{env.runtime?.pid}</p>
                  </div>
                  <div className="rounded-md border border-border bg-surface-secondary p-2">
                    <p className="text-[10px] text-content-hint">调试端口</p>
                    <p className="font-mono text-content-main">{env.runtime?.debugPort}</p>
                  </div>
                  <div className="rounded-md border border-border bg-surface-secondary p-2">
                    <p className="text-[10px] text-content-hint">内存</p>
                    <p className="font-mono text-content-main">{env.runtime?.memoryMb} MB</p>
                  </div>
                  <div className="rounded-md border border-border bg-surface-secondary p-2">
                    <p className="text-[10px] text-content-hint">启动时间</p>
                    <p className="text-[10px] text-content-main">{formatDate(env.runtime?.startedAt)}</p>
                  </div>
                </div>
              </>
            )}
          </CardBody>
        </Card>

        <Card>
          <CardHeader
            title="脚本执行"
            description="通过页面 target 的 Runtime.evaluate 执行，返回 JSON 值"
            actions={
              <Button variant="secondary" size="sm" disabled={!running || !canOpen} onClick={async () => {
                setBusy('execute');
                try {
                  setScriptResult(await ipc.browserExecute(env.id, script));
                  toast.success('脚本已执行');
                } catch (error) {
                  toastError(error);
                } finally {
                  setBusy(null);
                }
              }}>
                <Terminal className="h-3.5 w-3.5" /> 运行
              </Button>
            }
          />
          <CardBody className="space-y-3">
            <Textarea className="min-h-[100px] font-mono text-[11px]" value={script} onChange={(event) => setScript(event.target.value)} />
            <pre className="code-block max-h-56 min-h-[60px] whitespace-pre-wrap break-all">
              {scriptResult === null ? '// 执行结果将显示在这里' : JSON.stringify(scriptResult, null, 2)}
            </pre>
          </CardBody>
        </Card>

        <Card>
          <CardHeader title="截图与录屏" description="截图保存为 PNG；录屏通过定时采集页面帧保存，可后续合成视频。" />
          <CardBody className="space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <Button
                variant="secondary"
                loading={busy === 'screenshot'}
                disabled={!running || !canOpen}
                onClick={async () => {
                  setBusy('screenshot');
                  try {
                    const result = await ipc.browserScreenshot(env.id);
                    setScreenshot(result);
                    toast.success('截图完成', `${result.width}×${result.height}`);
                  } catch (error) {
                    toastError(error);
                  } finally {
                    setBusy(null);
                  }
                }}
              >
                <Camera className="h-4 w-4" /> 截图
              </Button>
              <Input
                className="w-28"
                type="number"
                min={200}
                max={10000}
                value={recordingInterval}
                onChange={(event) => setRecordingInterval(event.target.value)}
                title="帧间隔 ms"
              />
              <Button
                variant={env.runtime?.recording ? 'danger' : 'secondary'}
                loading={busy === 'recording'}
                disabled={!running || !canOpen}
                onClick={async () => {
                  setBusy('recording');
                  try {
                    if (env.runtime?.recording) {
                      const result = await ipc.browserRecordingStop(env.id);
                      setRecordingResult(result);
                      toast.success('录屏已结束', `${result.frameCount} 帧`);
                    } else {
                      await ipc.browserRecordingStart(env.id, Number(recordingInterval) || 1000);
                      toast.success('开始录屏', '再次点击结束并生成 manifest');
                    }
                    await onRefresh();
                  } catch (error) {
                    toastError(error);
                  } finally {
                    setBusy(null);
                  }
                }}
              >
                {env.runtime?.recording ? <CircleStop className="h-4 w-4" /> : <Video className="h-4 w-4" />}
                {env.runtime?.recording ? '停止录屏' : '开始录屏'}
              </Button>
              <Button
                variant="secondary"
                loading={busy === 'har'}
                disabled={!running || !canOpen}
                onClick={async () => {
                  setBusy('har');
                  try {
                    if (env.runtime?.harRecording) {
                      const path = await ipc.browserHarStop(env.id);
                      setHarPath(path);
                      toast.success('HAR 已导出', path);
                    } else {
                      await ipc.browserHarStart(env.id);
                      toast.info('开始 HAR 抓包', '浏览页面后再次点击导出');
                    }
                    await onRefresh();
                  } catch (error) {
                    toastError(error);
                  } finally {
                    setBusy(null);
                  }
                }}
              >
                <FolderDown className="h-4 w-4" />
                {env.runtime?.harRecording ? '导出 HAR' : '开始抓包'}
              </Button>
              <Button
                variant="secondary"
                loading={busy === 'captcha'}
                disabled={!running || !canOpen}
                onClick={async () => {
                  setBusy('captcha');
                  try {
                    const path = await ipc.captchaCapture(env.id);
                    toast.success('验证码截图已采集', path ?? '已记录事件');
                  } catch (error) {
                    toastError(error);
                  } finally {
                    setBusy(null);
                  }
                }}
              >
                <Radar className="h-4 w-4" /> 采集验证码
              </Button>
              <Button
                variant="secondary"
                size="sm"
                onClick={() =>
                  void ipc.openArtifactDir('screenshots').catch(toastError)
                }
                title="打开截图保存目录"
              >
                <FolderOpen className="h-4 w-4" /> 图片目录
              </Button>
              <Button
                variant="secondary"
                size="sm"
                onClick={() =>
                  void ipc.openArtifactDir('recordings').catch(toastError)
                }
                title="打开录屏保存目录"
              >
                <FolderOpen className="h-4 w-4" /> 视频目录
              </Button>
            </div>
            {screenshot ? (
              <div className="rounded-md border border-border p-2">
                <img src={screenshot.dataUrl} alt="截图" className="max-h-[360px] w-full rounded object-contain" />
                <div className="mt-2 flex items-center justify-between gap-2 text-[10px] text-content-hint">
                  <span className="truncate">{screenshot.path}</span>
                  <div className="flex shrink-0 items-center gap-2">
                    <button
                      type="button"
                      className="inline-flex items-center gap-1 text-primary"
                      onClick={() => void ipc.openArtifactDir('screenshots').catch(toastError)}
                    >
                      <FolderOpen className="h-3 w-3" /> 目录
                    </button>
                    <a className="inline-flex items-center gap-1 text-primary" href={screenshot.dataUrl} download={`finenv-${env.id}.png`}>
                      <Download className="h-3 w-3" /> 下载
                    </a>
                  </div>
                </div>
              </div>
            ) : null}
            {recordingResult ? (
              <div className="rounded-md border border-border bg-surface-secondary p-2 text-[11px]">
                <p className="font-medium text-content-main">录屏完成 · {recordingResult.frameCount} 帧 · {formatDuration(recordingResult.durationMs)}</p>
                <div className="mt-0.5 flex items-center gap-2">
                  <p className="min-w-0 flex-1 break-all text-content-hint">{recordingResult.manifestPath || recordingResult.directory}</p>
                  <button
                    type="button"
                    className="inline-flex shrink-0 items-center gap-1 text-primary"
                    onClick={() => void ipc.openArtifactDir('recordings').catch(toastError)}
                  >
                    <FolderOpen className="h-3 w-3" /> 打开目录
                  </button>
                </div>
              </div>
            ) : null}
            {harPath ? (
              <div className="flex items-center gap-2 rounded-md border border-border bg-surface-secondary p-2 text-[11px]">
                <HardDriveDownload className="h-3.5 w-3.5 text-primary" />
                <span className="min-w-0 flex-1 break-all text-content-main">{harPath}</span>
                <button
                  type="button"
                  className="inline-flex shrink-0 items-center gap-1 text-[10px] text-primary"
                  onClick={() => void ipc.openArtifactDir('har').catch(toastError)}
                >
                  <FolderOpen className="h-3 w-3" /> 目录
                </button>
                <CopyButton value={harPath} />
              </div>
            ) : null}
          </CardBody>
        </Card>

        <Card>
          <CardHeader title="Cookie 读写" description="读取结果会同步加密缓存到本地数据库；仅导出权限可用。" />
          <CardBody className="space-y-2">
            <div className="flex flex-wrap gap-2">
              <Button
                variant="secondary"
                loading={busy === 'cookies-get'}
                disabled={!canSensitive}
                onClick={async () => {
                  setBusy('cookies-get');
                  try {
                    const records = await ipc.browserCookiesGet(env.id);
                    setCookies(records);
                    toast.success('Cookie 已读取', `${records.length} 条`);
                  } catch (error) {
                    toastError(error);
                  } finally {
                    setBusy(null);
                  }
                }}
              >
                <Cookie className="h-4 w-4" /> 读取 Cookie
              </Button>
              <Button
                variant="secondary"
                disabled={!canSensitive || !running}
                onClick={async () => {
                  try {
                    const records = await ipc.browserCookiesGet(env.id);
                    setCookieText(JSON.stringify(records, null, 2));
                    setCookieEditor(true);
                  } catch (error) {
                    toastError(error);
                  }
                }}
              >
                <Code2 className="h-4 w-4" /> 编辑并写入
              </Button>
            </div>
            <p className="text-[10px] text-content-hint">
              浏览器停止时将自动把 Cookie 以 AES-256-GCM 密文缓存到数据库；数据库文件无法直接读取明文。
            </p>
          </CardBody>
        </Card>
      </div>

      <div className="space-y-3 xl:sticky xl:top-4">
        <Card>
          <CardHeader
            title="CDP 调试端点"
            description="可直接连接 Playwright / Selenium / 自研 CDP 客户端"
            actions={<Button variant="ghost" size="icon-sm" onClick={() => void loadCdp()}><RefreshCw className="h-3.5 w-3.5" /></Button>}
          />
          <CardBody className="space-y-2">
            {running ? (
              cdp ? (
                <>
                  <div className="rounded-md border border-border bg-surface-secondary p-2">
                    <p className="text-[10px] text-content-hint">HTTP</p>
                    <div className="flex items-center gap-1">
                      <code className="min-w-0 flex-1 truncate font-mono text-[10px]">{cdp.httpEndpoint}</code>
                      <CopyButton value={cdp.httpEndpoint} />
                    </div>
                  </div>
                  <div className="rounded-md border border-border bg-surface-secondary p-2">
                    <p className="text-[10px] text-content-hint">WebSocket</p>
                    <div className="flex items-center gap-1">
                      <code className="min-w-0 flex-1 truncate font-mono text-[10px]">{cdp.wsEndpoint}</code>
                      <CopyButton value={cdp.wsEndpoint} />
                    </div>
                  </div>
                  <div>
                    <p className="mb-1 text-[10px] text-content-hint">Playwright 连接示例</p>
                    <pre className="code-block whitespace-pre-wrap break-all text-[10px]">{cdp.playwrightExample}</pre>
                    <Button
                      variant="secondary"
                      size="sm"
                      className="mt-2 w-full"
                      onClick={async () => {
                        if (await copyText(cdp.playwrightExample)) toast.success('连接示例已复制');
                      }}
                    >
                      <Download className="h-3.5 w-3.5" /> 复制示例
                    </Button>
                  </div>
                </>
              ) : (
                <p className="text-xs text-content-hint">{cdpError ?? '读取 CDP 信息…'}</p>
              )
            ) : (
              <div className="py-6 text-center text-xs text-content-hint">
                <Spinner className="mb-2" />
                启动环境后显示调试端口
              </div>
            )}
          </CardBody>
        </Card>

        <Card>
          <CardHeader title="当前页面" />
          <CardBody>
            {running ? (
              <>
                <p className="break-all font-mono text-[11px] text-content-main">{env.runtime?.currentUrl || 'about:blank'}</p>
                <div className="mt-2 flex items-center gap-1">
                  <CopyButton value={env.runtime?.currentUrl ?? ''} label="复制地址" size="sm" />
                  <Badge tone="outline">PID {env.runtime?.pid}</Badge>
                </div>
                {env.runtime?.warnings?.length ? (
                  <div className="mt-2 rounded-md bg-[color-mix(in_srgb,var(--warning)_10%,transparent)] p-2 text-[10px] leading-4 text-[var(--warning)]">
                    {env.runtime.warnings.join('；')}
                  </div>
                ) : null}
              </>
            ) : (
              <p className="text-xs text-content-hint">环境未运行</p>
            )}
          </CardBody>
        </Card>

        <Card>
          <CardBody className="flex items-start gap-2 text-[11px] leading-5 text-content-weak">
            <Film className="mt-0.5 h-4 w-4 shrink-0 text-primary" />
            录屏采用逐帧 PNG 保存，目录包含 manifest.json。可在 CI 中下载帧序列并用 ffmpeg 合成视频。
          </CardBody>
        </Card>
      </div>

      <Dialog
        open={!!screenshot}
        onOpenChange={(value) => !value && setScreenshot(null)}
        title="环境截图"
        width="max-w-4xl"
        footer={<Button variant="secondary" onClick={() => setScreenshot(null)}>关闭</Button>}
      >
        {screenshot ? <img src={screenshot.dataUrl} alt="截图" className="max-h-[70vh] w-full object-contain" /> : null}
      </Dialog>

      <Dialog
        open={!!cookies}
        onOpenChange={(value) => !value && setCookies(null)}
        title="Cookie 列表"
        description={`${cookies?.length ?? 0} 条 · 包含敏感登录凭据，请勿外传`}
        width="max-w-3xl"
        footer={
          <>
            <Button
              variant="secondary"
              onClick={async () => {
                if (cookies && (await copyText(JSON.stringify(cookies, null, 2)))) toast.success('已复制 Cookie JSON');
              }}
            >
              复制 JSON
            </Button>
            <Button variant="secondary" onClick={() => setCookies(null)}>关闭</Button>
          </>
        }
      >
        <pre className="code-block max-h-[55vh]">{JSON.stringify(cookies, null, 2)}</pre>
      </Dialog>

      <Dialog
        open={cookieEditor}
        onOpenChange={setCookieEditor}
        title="写入 Cookie"
        description="粘贴 CookieRecord 数组，通过 CDP Network.setCookies 写入当前 profile"
        width="max-w-3xl"
        footer={
          <>
            <Button variant="secondary" onClick={() => setCookieEditor(false)}>取消</Button>
            <Button
              variant="primary"
              onClick={async () => {
                try {
                  const parsed = JSON.parse(cookieText) as CookieRecord[];
                  if (!Array.isArray(parsed)) throw new Error('必须是 CookieRecord 数组');
                  const count = await ipc.browserCookiesSet(env.id, parsed);
                  toast.success('Cookie 已写入', `${count} 条`);
                  setCookieEditor(false);
                } catch (error) {
                  toastError(error);
                }
              }}
            >
              写入
            </Button>
          </>
        }
      >
        <Textarea className="min-h-[360px] font-mono text-[11px]" value={cookieText} onChange={(event) => setCookieText(event.target.value)} />
      </Dialog>
    </div>
  );
}
