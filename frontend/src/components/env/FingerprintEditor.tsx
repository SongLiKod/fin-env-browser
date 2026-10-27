import * as React from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  Dices,
  Fingerprint as FingerprintIcon,
  RotateCcw,
  Save,
  Sparkles,
} from 'lucide-react';
import type {
  BrandVersion,
  EnvironmentView,
  Fingerprint,
  FingerprintValidation,
  PluginInfo,
} from '@/ipc/types';
import { ipc } from '@/ipc/client';
import { toastError, useToastStore } from '@/store/toast';
import { useAuthStore } from '@/store/auth';
import { cn } from '@/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardHeader } from '@/components/ui/card';
import { Field, Input, Select, Textarea } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';

const timezones = [
  'Asia/Shanghai',
  'Asia/Hong_Kong',
  'Asia/Taipei',
  'Asia/Tokyo',
  'Asia/Seoul',
  'Asia/Singapore',
  'Asia/Kolkata',
  'Europe/London',
  'Europe/Berlin',
  'Europe/Paris',
  'Europe/Amsterdam',
  'Europe/Moscow',
  'America/New_York',
  'America/Chicago',
  'America/Los_Angeles',
  'America/Toronto',
  'America/Sao_Paulo',
  'Australia/Sydney',
  'UTC',
];

function randomSeed(): number {
  return Math.floor(Math.random() * 0xffffffff);
}

function BrandEditor({
  title,
  brands,
  onChange,
}: {
  title: string;
  brands: BrandVersion[];
  onChange: (brands: BrandVersion[]) => void;
}) {
  return (
    <div>
      <div className="mb-1.5 flex items-center justify-between">
        <span className="text-xs font-medium text-content-weak">{title}</span>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onChange([...brands, { brand: '', version: '' }])}
        >
          + 添加
        </Button>
      </div>
      <div className="space-y-1.5">
        {brands.map((item, index) => (
          <div key={index} className="flex items-center gap-1.5">
            <Input
              className="h-8 flex-1 text-xs"
              value={item.brand}
              placeholder="brand"
              onChange={(event) => {
                const next = brands.slice();
                next[index] = { ...item, brand: event.target.value };
                onChange(next);
              }}
            />
            <Input
              className="h-8 w-28 text-xs"
              value={item.version}
              placeholder="version"
              onChange={(event) => {
                const next = brands.slice();
                next[index] = { ...item, version: event.target.value };
                onChange(next);
              }}
            />
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => onChange(brands.filter((_, i) => i !== index))}
            >
              ×
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}

export function FingerprintEditor({
  env,
  onSaved,
}: {
  env: EnvironmentView;
  onSaved: (env: EnvironmentView) => void;
}) {
  const toast = useToastStore();
  const can = useAuthStore((state) => state.can);
  const [fp, setFp] = React.useState<Fingerprint>(env.fingerprint);
  const [validation, setValidation] = React.useState<FingerprintValidation | null>(null);
  const [validating, setValidating] = React.useState(false);
  const [saving, setSaving] = React.useState(false);
  const [advanced, setAdvanced] = React.useState(false);
  const [advancedText, setAdvancedText] = React.useState('');
  const [advancedError, setAdvancedError] = React.useState<string | null>(null);
  const [pluginsText, setPluginsText] = React.useState(JSON.stringify(env.fingerprint.plugins, null, 2));
  const [pluginsError, setPluginsError] = React.useState<string | null>(null);

  React.useEffect(() => {
    setFp(env.fingerprint);
    setPluginsText(JSON.stringify(env.fingerprint.plugins, null, 2));
    setAdvancedText(JSON.stringify(env.fingerprint, null, 2));
    setPluginsError(null);
    setAdvancedError(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [env.id]);

  const patch = (partial: Partial<Fingerprint>) => {
    setFp((current) => ({ ...current, ...partial }));
  };

  const patchScreen = (partial: Partial<Fingerprint['screen']>) =>
    setFp((current) => ({ ...current, screen: { ...current.screen, ...partial } }));

  const patchHints = (partial: Partial<Fingerprint['clientHints']>) =>
    setFp((current) => ({
      ...current,
      clientHints: { ...current.clientHints, ...partial },
    }));

  const patchWebgl = (partial: Partial<Fingerprint['webgl']>) =>
    setFp((current) => ({ ...current, webgl: { ...current.webgl, ...partial } }));

  React.useEffect(() => {
    if (advanced) {
      setAdvancedText(JSON.stringify(fp, null, 2));
    }
  }, [fp, advanced]);

  React.useEffect(() => {
    const timer = window.setTimeout(async () => {
      setValidating(true);
      try {
        const result = await ipc.fingerprintValidate(fp);
        setValidation(result);
      } catch {
        setValidation(null);
      } finally {
        setValidating(false);
      }
    }, 320);
    return () => window.clearTimeout(timer);
  }, [fp]);

  const applyTemplate = async (os: string) => {
    try {
      const template = await ipc.fingerprintTemplate(os);
      setFp(template);
      setPluginsText(JSON.stringify(template.plugins, null, 2));
      setAdvancedText(JSON.stringify(template, null, 2));
      setPluginsError(null);
      setAdvancedError(null);
      toast.info('已生成指纹模板', '保存后生效；建议先执行一致性校验');
    } catch (error) {
      toastError(error);
    }
  };

  const save = async () => {
    let candidate = fp;
    if (advanced) {
      try {
        const parsed = JSON.parse(advancedText) as Fingerprint;
        setFp(parsed);
        setPluginsText(JSON.stringify(parsed.plugins, null, 2));
        setPluginsError(null);
        candidate = parsed;
      } catch (error) {
        toast.error('指纹 JSON 无效', error instanceof Error ? error.message : undefined);
        return;
      }
    }
    if (pluginsError) {
      toast.error('插件列表 JSON 无效', pluginsError);
      return;
    }
    setSaving(true);
    try {
      const validate = await ipc.fingerprintValidate(candidate);
      setValidation(validate);
      if (!validate.valid) {
        toast.error('指纹存在冲突', validate.errors.map((issue) => issue.message).join('；'));
        return;
      }
      const updated = await ipc.fingerprintUpdate(env.id, candidate);
      onSaved(updated);
      toast.success('指纹已保存', validate.warnings.length ? `存在 ${validate.warnings.length} 条提示` : '一致性校验通过');
    } catch (error) {
      toastError(error);
    } finally {
      setSaving(false);
    }
  };

  const updatePlugins = (text: string) => {
    setPluginsText(text);
    try {
      const parsed = JSON.parse(text) as PluginInfo[];
      if (!Array.isArray(parsed)) throw new Error('必须是数组');
      setFp((current) => ({ ...current, plugins: parsed }));
      setPluginsError(null);
    } catch (error) {
      setPluginsError(error instanceof Error ? error.message : 'JSON 格式错误');
    }
  };

  return (
    <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_300px]">
      <div className="space-y-4">
        <Card>
          <CardHeader
            title="基础标识"
            description="UA、平台、语言与时区由 CDP Emulation + JS Hook 双通道注入"
            actions={<Badge tone="primary">Chromium V1</Badge>}
          />
          <CardBody className="space-y-3">
            <Field label="User-Agent" required>
              <Textarea
                className="min-h-[68px] font-mono text-xs"
                value={fp.userAgent}
                onChange={(event) => patch({ userAgent: event.target.value })}
              />
            </Field>
            <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
              <Field label="navigator.platform" required>
                <Input value={fp.platform} onChange={(event) => patch({ platform: event.target.value })} />
              </Field>
              <Field label="时区" required>
                <Input
                  list="finenv-timezones"
                  value={fp.timezone}
                  onChange={(event) => patch({ timezone: event.target.value })}
                />
                <datalist id="finenv-timezones">
                  {timezones.map((tz) => (
                    <option key={tz} value={tz} />
                  ))}
                </datalist>
              </Field>
              <Field label="语言" hint="逗号分隔，首个为主语言">
                <Input
                  value={fp.languages.join(',')}
                  onChange={(event) =>
                    patch({ languages: event.target.value.split(',').map((v) => v.trim()).filter(Boolean) })
                  }
                />
              </Field>
              <Field label="配色偏好">
                <Select value={fp.colorScheme} onChange={(event) => patch({ colorScheme: event.target.value as Fingerprint['colorScheme'] })}>
                  <option value="light">light</option>
                  <option value="dark">dark</option>
                  <option value="no-preference">no-preference</option>
                </Select>
              </Field>
              <Field label="CPU 核心数">
                <Input
                  type="number"
                  min={2}
                  max={32}
                  value={fp.hardwareConcurrency}
                  onChange={(event) => patch({ hardwareConcurrency: Number(event.target.value) })}
                />
              </Field>
              <Field label="设备内存 GB">
                <Select value={fp.deviceMemory} onChange={(event) => patch({ deviceMemory: Number(event.target.value) })}>
                  {[2, 4, 8, 16, 32].map((value) => (
                    <option key={value} value={value}>
                      {value}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label="触摸点">
                <Input
                  type="number"
                  min={0}
                  max={10}
                  value={fp.touchPoints}
                  onChange={(event) => patch({ touchPoints: Number(event.target.value) })}
                />
              </Field>
              <div className="flex items-end pb-2">
                <Switch
                  checked={fp.doNotTrack}
                  onCheckedChange={(value) => patch({ doNotTrack: value })}
                  label="Do Not Track"
                />
              </div>
            </div>
          </CardBody>
        </Card>

        <Card>
          <CardHeader title="屏幕与设备" description="navigator / screen JS Hook 覆盖，保持可用尺寸比例合理" />
          <CardBody>
            <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
              <Field label="屏幕宽">
                <Input type="number" value={fp.screen.width} onChange={(e) => patchScreen({ width: Number(e.target.value) })} />
              </Field>
              <Field label="屏幕高">
                <Input type="number" value={fp.screen.height} onChange={(e) => patchScreen({ height: Number(e.target.value) })} />
              </Field>
              <Field label="可用宽">
                <Input type="number" value={fp.screen.availWidth} onChange={(e) => patchScreen({ availWidth: Number(e.target.value) })} />
              </Field>
              <Field label="可用高">
                <Input type="number" value={fp.screen.availHeight} onChange={(e) => patchScreen({ availHeight: Number(e.target.value) })} />
              </Field>
              <Field label="色深">
                <Select value={fp.screen.colorDepth} onChange={(e) => patchScreen({ colorDepth: Number(e.target.value) })}>
                  <option value={24}>24</option>
                  <option value={30}>30</option>
                  <option value={32}>32</option>
                </Select>
              </Field>
              <Field label="DPR">
                <Input type="number" step="0.25" min={0.5} max={4} value={fp.screen.pixelRatio} onChange={(e) => patchScreen({ pixelRatio: Number(e.target.value) })} />
              </Field>
            </div>
          </CardBody>
        </Card>

        <Card>
          <CardHeader title="Canvas / WebGL / Audio" description="V1 使用 CDP Runtime.evaluate 注入确定性噪声 Hook" />
          <CardBody className="space-y-4">
            <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
              <Field label="Canvas 模式">
                <Select value={fp.canvas.mode} onChange={(e) => patch({ canvas: { ...fp.canvas, mode: e.target.value as 'noise' | 'off' } })}>
                  <option value="noise">noise</option>
                  <option value="off">原生</option>
                </Select>
              </Field>
              <Field label="Canvas 种子">
                <div className="flex gap-1">
                  <Input type="number" value={fp.canvas.seed} onChange={(e) => patch({ canvas: { ...fp.canvas, seed: Number(e.target.value) } })} />
                  <Button variant="secondary" size="icon" onClick={() => patch({ canvas: { ...fp.canvas, seed: randomSeed() } })}>
                    <Dices className="h-4 w-4" />
                  </Button>
                </div>
              </Field>
              <Field label="Audio 模式">
                <Select value={fp.audio.mode} onChange={(e) => patch({ audio: { ...fp.audio, mode: e.target.value as 'noise' | 'off' } })}>
                  <option value="noise">noise</option>
                  <option value="off">原生</option>
                </Select>
              </Field>
              <Field label="Audio 种子">
                <div className="flex gap-1">
                  <Input type="number" value={fp.audio.seed} onChange={(e) => patch({ audio: { ...fp.audio, seed: Number(e.target.value) } })} />
                  <Button variant="secondary" size="icon" onClick={() => patch({ audio: { ...fp.audio, seed: randomSeed() } })}>
                    <Dices className="h-4 w-4" />
                  </Button>
                </div>
              </Field>
            </div>
            <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
              <Field label="WebGL 厂商" className="md:col-span-2">
                <Input value={fp.webgl.vendor} onChange={(e) => patchWebgl({ vendor: e.target.value })} />
              </Field>
              <Field label="WebGL 种子">
                <Input type="number" value={fp.webgl.seed} onChange={(e) => patchWebgl({ seed: Number(e.target.value) })} />
              </Field>
              <div className="flex items-end">
                <Button variant="secondary" className="w-full" onClick={() => patchWebgl({ seed: randomSeed() })}>
                  <Dices className="h-4 w-4" /> 随机种子
                </Button>
              </div>
              <Field label="WebGL 渲染器" className="md:col-span-4">
                <Input value={fp.webgl.renderer} onChange={(e) => patchWebgl({ renderer: e.target.value })} />
              </Field>
            </div>
          </CardBody>
        </Card>

        <Card>
          <CardHeader title="Client Hints" description="navigator.userAgentData 主版本必须与 UA 的 Chrome 主版本一致" />
          <CardBody className="space-y-4">
            <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
              <Field label="platform">
                <Select value={fp.clientHints.platform} onChange={(e) => patchHints({ platform: e.target.value })}>
                  <option value="Windows">Windows</option>
                  <option value="macOS">macOS</option>
                  <option value="Linux">Linux</option>
                </Select>
              </Field>
              <Field label="platformVersion">
                <Input value={fp.clientHints.platformVersion} onChange={(e) => patchHints({ platformVersion: e.target.value })} />
              </Field>
              <Field label="architecture">
                <Select value={fp.clientHints.architecture} onChange={(e) => patchHints({ architecture: e.target.value })}>
                  <option value="x86">x86</option>
                  <option value="arm">arm</option>
                </Select>
              </Field>
              <Field label="bitness">
                <Select value={fp.clientHints.bitness} onChange={(e) => patchHints({ bitness: e.target.value })}>
                  <option value="64">64</option>
                  <option value="32">32</option>
                </Select>
              </Field>
            </div>
            <div className="grid gap-4 md:grid-cols-2">
              <BrandEditor title="brands" brands={fp.clientHints.brands} onChange={(brands) => patchHints({ brands })} />
              <BrandEditor title="fullVersionList" brands={fp.clientHints.fullVersionList} onChange={(brands) => patchHints({ fullVersionList: brands })} />
            </div>
            <Switch
              checked={fp.clientHints.mobile}
              onCheckedChange={(value) => patchHints({ mobile: value })}
              label="移动端 Client Hints"
              description="桌面 UA 应保持关闭；开启时会与 UA 的 Mobile 标识共同校验。"
            />
          </CardBody>
        </Card>

        <Card>
          <CardHeader title="字体 / WebRTC / 插件" description="字体列表参与字体指纹检测；WebRTC 防止真实 IP 泄漏" />
          <CardBody className="space-y-4">
            <Field label="字体列表" hint="逗号分隔，至少 3 个；建议包含目标操作系统常见字体">
              <Textarea
                className="min-h-[60px] font-mono text-xs"
                value={fp.fonts.join(', ')}
                onChange={(event) =>
                  patch({ fonts: event.target.value.split(',').map((v) => v.trim()).filter(Boolean) })
                }
              />
            </Field>
            <div className="grid grid-cols-2 gap-3">
              <Field label="WebRTC 模式">
                <Select value={fp.webrtc.mode} onChange={(e) => patch({ webrtc: { ...fp.webrtc, mode: e.target.value as Fingerprint['webrtc']['mode'] } })}>
                  <option value="disable">disable · 禁止非代理 UDP</option>
                  <option value="proxy">proxy · 仅代理</option>
                  <option value="public">public · 指定公网 IP</option>
                </Select>
              </Field>
              <Field label="WebRTC 公网 IP" hint="public 模式必填">
                <Input
                  value={fp.webrtc.publicIp ?? ''}
                  onChange={(e) => patch({ webrtc: { ...fp.webrtc, publicIp: e.target.value || null } })}
                  placeholder="203.0.113.10"
                />
              </Field>
            </div>
            <Field label="插件列表 JSON" error={pluginsError} hint="现代 Chrome 至少应包含 PDF Viewer，禁止 Flash">
              <Textarea
                className="min-h-[140px] font-mono text-[11px]"
                value={pluginsText}
                onChange={(event) => updatePlugins(event.target.value)}
              />
            </Field>
          </CardBody>
        </Card>

        <Card>
          <CardHeader
            title="高级：完整指纹 JSON"
            description="用于批量调整或从其他环境粘贴配置，保存前仍会执行一致性校验"
            actions={
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  setAdvanced((value) => !value);
                  setAdvancedText(JSON.stringify(fp, null, 2));
                  setAdvancedError(null);
                }}
              >
                {advanced ? '收起' : '展开'}
              </Button>
            }
          />
          {advanced ? (
            <CardBody>
              <Textarea
                className="min-h-[360px] font-mono text-[11px]"
                value={advancedText}
                onChange={(event) => {
                  setAdvancedText(event.target.value);
                  try {
                    JSON.parse(event.target.value);
                    setAdvancedError(null);
                  } catch (error) {
                    setAdvancedError(error instanceof Error ? error.message : 'JSON 格式错误');
                  }
                }}
              />
              {advancedError ? <p className="mt-1 text-xs text-danger">{advancedError}</p> : null}
            </CardBody>
          ) : null}
        </Card>
      </div>

      <div className="space-y-3 xl:sticky xl:top-4">
        <Card>
          <CardHeader title="一致性校验" description={validating ? '校验中…' : '内置规则禁止参数冲突'} />
          <CardBody className="space-y-3">
            {validation ? (
              <>
                <div
                  className={cn(
                    'flex items-center gap-2 rounded-md px-3 py-2 text-xs',
                    validation.valid
                      ? 'bg-[color-mix(in_srgb,var(--success)_10%,transparent)] text-[var(--success)]'
                      : 'bg-[color-mix(in_srgb,var(--danger)_10%,transparent)] text-danger',
                  )}
                >
                  {validation.valid ? <CheckCircle2 className="h-4 w-4" /> : <AlertTriangle className="h-4 w-4" />}
                  {validation.valid ? '未发现冲突' : `${validation.errors.length} 个错误`}
                  {validation.warnings.length > 0 ? ` · ${validation.warnings.length} 个提示` : ''}
                </div>
                <div className="max-h-56 space-y-1.5 overflow-y-auto">
                  {validation.errors.map((issue, index) => (
                    <div key={`e-${index}`} className="rounded-md border border-[color-mix(in_srgb,var(--danger)_25%,transparent)] p-2 text-[11px]">
                      <p className="font-mono text-[10px] text-danger">{issue.field}</p>
                      <p className="mt-0.5 text-content-weak">{issue.message}</p>
                    </div>
                  ))}
                  {validation.warnings.map((issue, index) => (
                    <div key={`w-${index}`} className="rounded-md border border-border p-2 text-[11px]">
                      <p className="font-mono text-[10px] text-[var(--warning)]">{issue.field}</p>
                      <p className="mt-0.5 text-content-weak">{issue.message}</p>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <p className="text-xs text-content-hint">正在等待校验结果…</p>
            )}
            <div className="grid grid-cols-2 gap-2">
              <Button
                variant="secondary"
                onClick={async () => {
                  try {
                    setValidation(await ipc.fingerprintValidate(fp));
                    toast.success('校验完成');
                  } catch (error) {
                    toastError(error);
                  }
                }}
              >
                立即校验
              </Button>
              <Button
                variant="secondary"
                onClick={() => {
                  setFp(env.fingerprint);
                  setPluginsText(JSON.stringify(env.fingerprint.plugins, null, 2));
                  setAdvancedText(JSON.stringify(env.fingerprint, null, 2));
                  setPluginsError(null);
                  toast.info('已恢复到上次保存的指纹');
                }}
              >
                <RotateCcw className="h-4 w-4" /> 重置
              </Button>
            </div>
            {can('edit_fingerprint_proxy') ? (
              <Button variant="primary" className="w-full" loading={saving} onClick={() => void save()}>
                <Save className="h-4 w-4" /> 保存指纹配置
              </Button>
            ) : (
              <p className="text-xs text-content-hint">当前角色仅有查看权限。</p>
            )}
          </CardBody>
        </Card>

        <Card>
          <CardHeader title="指纹模板" description="自动生成自洽的 UA / 屏幕 / 时区 / 语言组合" />
          <CardBody className="space-y-2">
            {[
              { key: 'win11', label: 'Windows 11 · Chrome' },
              { key: 'win10', label: 'Windows 10 · Chrome' },
              { key: 'mac', label: 'macOS · Chrome' },
              { key: 'linux', label: 'Linux · Chrome' },
            ].map((item) => (
              <Button
                key={item.key}
                variant="secondary"
                className="w-full justify-start"
                onClick={() => void applyTemplate(item.key)}
                disabled={!can('edit_fingerprint_proxy')}
              >
                <Sparkles className="h-4 w-4 text-primary" /> {item.label}
              </Button>
            ))}
            <p className="pt-1 text-[10px] leading-4 text-content-hint">
              模板会覆盖当前表单，保存后才写入数据库；切换后请先执行一致性校验。
            </p>
          </CardBody>
        </Card>

        <Card>
          <CardBody className="flex items-start gap-2 text-[11px] leading-5 text-content-weak">
            <FingerprintIcon className="mt-0.5 h-4 w-4 shrink-0 text-primary" />
            <span>
              V1 通过 CDP 注入 JS Hook；高保真内核级指纹将在后续版本提供。主题与面板样式不会影响被测页面。
            </span>
          </CardBody>
        </Card>
      </div>
    </div>
  );
}
