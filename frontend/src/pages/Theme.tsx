import * as React from 'react';
import {
  Check,
  Download,
  Monitor,
  Moon,
  Palette,
  RotateCcw,
  Save,
  Sun,
  Upload,
} from 'lucide-react';
import { useThemeStore } from '@/store/theme';
import { toastError, useToastStore } from '@/store/toast';
import { cn } from '@/utils';
import { jsonFilter, pickFile, saveFile } from '@/utils/dialog';
import { normalizeHex, readableOn } from '@/theme/color';
import {
  densities,
  primaryDarkFor,
  primaryLightFor,
  resolveThemeMode,
  themeModes,
} from '@/theme/theme';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardHeader } from '@/components/ui/card';
import { Field, Input } from '@/components/ui/input';

const presets = ['#165DFF', '#0E9F6E', '#7C3AED', '#F53F3F', '#FF7D00', '#0EA5E9', '#DB2777', '#1D2129'];

export function ThemePage() {
  const toast = useToastStore();
  const theme = useThemeStore((state) => state.theme);
  const saving = useThemeStore((state) => state.saving);
  const lastError = useThemeStore((state) => state.lastError);
  const setMode = useThemeStore((state) => state.setMode);
  const setDensity = useThemeStore((state) => state.setDensity);
  const setPrimaryColor = useThemeStore((state) => state.setPrimaryColor);
  const persist = useThemeStore((state) => state.persist);
  const reset = useThemeStore((state) => state.reset);
  const exportTheme = useThemeStore((state) => state.exportTheme);
  const importTheme = useThemeStore((state) => state.importTheme);
  const [hex, setHex] = React.useState(theme.primaryColor);
  const resolved = resolveThemeMode(theme.mode);

  React.useEffect(() => {
    setHex(theme.primaryColor);
  }, [theme.primaryColor]);

  const applyHex = (value: string) => {
    const normalized = normalizeHex(value);
    setHex(normalized);
    setPrimaryColor(normalized);
  };

  return (
    <div className="page-pad space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-[10px] font-semibold uppercase tracking-widest text-content-hint">Appearance / Theme Tokens</p>
          <h1 className="mt-1 text-xl font-semibold tracking-tight text-content-main">主题外观</h1>
          <p className="mt-1 max-w-2xl text-xs leading-5 text-content-weak">
            亮色 / 暗色 / 跟随系统、自定义主色与布局密度。配置持久化到本地数据库，刷新不会丢失。
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="secondary" onClick={async () => {
            try {
              const path = await saveFile('finenv-theme.json', jsonFilter());
              if (!path) return;
              await exportTheme(path);
              toast.success('主题已导出', path);
            } catch (error) {
              toastError(error);
            }
          }}>
            <Download className="h-4 w-4" /> 导出主题
          </Button>
          <Button variant="secondary" onClick={async () => {
            try {
              const files = await pickFile(jsonFilter());
              if (!files?.[0]) return;
              const imported = await importTheme(files[0]);
              toast.success('主题已导入', `${imported.mode} · ${imported.primaryColor}`);
            } catch (error) {
              toastError(error);
            }
          }}>
            <Upload className="h-4 w-4" /> 导入主题
          </Button>
          <Button variant="primary" loading={saving} onClick={() => void persist()}>
            <Save className="h-4 w-4" /> 保存
          </Button>
        </div>
      </div>

      {lastError ? (
        <div className="rounded-md border border-[color-mix(in_srgb,var(--danger)_30%,transparent)] bg-[color-mix(in_srgb,var(--danger)_8%,transparent)] px-3 py-2 text-xs text-danger">
          {lastError}
        </div>
      ) : null}

      <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_380px]">
        <div className="space-y-4">
          <Card>
            <CardHeader title="显示模式" description="仅作用于 FinEnv 管理面板，不会注入被测页面。" />
            <CardBody>
              <div className="grid gap-2 sm:grid-cols-3">
                {themeModes.map((mode) => {
                  const Icon = mode.value === 'light' ? Sun : mode.value === 'dark' ? Moon : Monitor;
                  const active = theme.mode === mode.value;
                  return (
                    <button
                      key={mode.value}
                      type="button"
                      onClick={() => setMode(mode.value)}
                      className={cn(
                        'rounded-lg border p-3 text-left transition-colors',
                        active ? 'border-primary bg-primary-light' : 'border-border hover:bg-surface-secondary',
                      )}
                    >
                      <div className="flex items-center justify-between">
                        <Icon className={cn('h-5 w-5', active ? 'text-primary' : 'text-content-weak')} />
                        {active ? <Check className="h-4 w-4 text-primary" /> : null}
                      </div>
                      <p className={cn('mt-2 text-sm font-medium', active ? 'text-primary' : 'text-content-main')}>{mode.label}</p>
                      <p className="mt-0.5 text-[10px] text-content-hint">{mode.hint}</p>
                    </button>
                  );
                })}
              </div>
            </CardBody>
          </Card>

          <Card>
            <CardHeader title="主色" description="修改 --color-primary，并自动派生 light / dark 变体。" />
            <CardBody className="space-y-4">
              <div className="flex flex-wrap items-end gap-3">
                <Field label="取色器">
                  <input
                    type="color"
                    value={theme.primaryColor}
                    onChange={(event) => applyHex(event.target.value)}
                    className="h-10 w-16 cursor-pointer rounded-md border border-border bg-surface-base"
                  />
                </Field>
                <Field label="HEX" hint="支持 #RRGGBB">
                  <Input
                    className="w-36 font-mono uppercase"
                    value={hex}
                    onChange={(event) => setHex(event.target.value)}
                    onBlur={() => applyHex(hex)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') applyHex(hex);
                    }}
                    maxLength={7}
                  />
                </Field>
                <div className="flex flex-wrap gap-1.5 pb-1.5">
                  {presets.map((preset) => (
                    <button
                      key={preset}
                      type="button"
                      title={preset}
                      onClick={() => applyHex(preset)}
                      className={cn(
                        'h-8 w-8 rounded-md border-2 transition-transform hover:scale-105',
                        theme.primaryColor.toUpperCase() === preset ? 'border-content-main' : 'border-transparent',
                      )}
                      style={{ background: preset }}
                    />
                  ))}
                </div>
              </div>
              <div className="grid grid-cols-3 gap-2">
                {[
                  { label: 'primary', value: theme.primaryColor },
                  { label: 'primary-light', value: primaryLightFor(theme.primaryColor, resolved) },
                  { label: 'primary-dark', value: primaryDarkFor(theme.primaryColor, resolved) },
                ].map((token) => (
                  <div key={token.label} className="overflow-hidden rounded-md border border-border">
                    <div className="flex h-14 items-center justify-center text-[10px] font-medium" style={{ background: token.value, color: readableOn(token.value) }}>
                      {token.value}
                    </div>
                    <p className="border-t border-border px-2 py-1 font-mono text-[9px] text-content-hint">--{token.label}</p>
                  </div>
                ))}
              </div>
            </CardBody>
          </Card>

          <Card>
            <CardHeader title="布局密度" description="控制面板间距、控件高度与表格行高，不改变字体大小。" />
            <CardBody>
              <div className="grid gap-2 sm:grid-cols-2">
                {densities.map((density) => {
                  const active = theme.density === density.value;
                  return (
                    <button
                      key={density.value}
                      type="button"
                      onClick={() => setDensity(density.value)}
                      className={cn(
                        'rounded-lg border p-3 text-left transition-colors',
                        active ? 'border-primary bg-primary-light' : 'border-border hover:bg-surface-secondary',
                      )}
                    >
                      <div className="flex items-center justify-between">
                        <span className={cn('text-sm font-medium', active ? 'text-primary' : 'text-content-main')}>{density.label}</span>
                        {active ? <Badge tone="primary">当前</Badge> : null}
                      </div>
                      <p className="mt-0.5 text-[10px] text-content-hint">{density.hint}</p>
                      <div className={cn('mt-2 space-y-1', density.value === 'compact' ? 'space-y-0.5' : 'space-y-1.5')}>
                        {[0, 1, 2].map((row) => (
                          <div key={row} className={cn('rounded bg-surface-tertiary', density.value === 'compact' ? 'h-1.5' : 'h-2.5')} />
                        ))}
                      </div>
                    </button>
                  );
                })}
              </div>
            </CardBody>
          </Card>
        </div>

        <div className="space-y-3 xl:sticky xl:top-4">
          <Card>
            <CardHeader title="实时预览" description="以下组件使用当前 CSS Token 渲染" />
            <CardBody className="space-y-3">
              <div className="rounded-lg border border-border bg-surface-secondary p-3">
                <div className="flex items-center gap-2">
                  <div className="flex h-8 w-8 items-center justify-center rounded-lg" style={{ background: theme.primaryColor, color: readableOn(theme.primaryColor) }}>
                    <Palette className="h-4 w-4" />
                  </div>
                  <div>
                    <p className="text-xs font-semibold text-content-main">交易中台 · 回归测试</p>
                    <p className="text-[10px] text-content-hint">Windows 11 · Asia/Shanghai</p>
                  </div>
                  <Badge tone="success" className="ml-auto">运行中</Badge>
                </div>
                <div className="mt-3 flex flex-wrap gap-1.5">
                  <Badge tone="primary">支付</Badge>
                  <Badge tone="outline">P0</Badge>
                  <Badge tone="warning">待复核</Badge>
                </div>
                <div className="mt-3 flex items-center gap-2">
                  <button type="button" className="rounded-md px-2.5 py-1.5 text-[11px] font-medium" style={{ background: theme.primaryColor, color: readableOn(theme.primaryColor) }}>
                    打开环境
                  </button>
                  <button type="button" className="rounded-md border border-border bg-surface-base px-2.5 py-1.5 text-[11px] font-medium text-content-main">
                    查看详情
                  </button>
                  <span className="ml-auto text-[10px] text-content-hint">PID 12480</span>
                </div>
              </div>
              <div className="rounded-lg border border-border p-3">
                <div className="flex items-center justify-between text-[11px]">
                  <span className="text-content-weak">内存</span>
                  <span className="font-mono text-content-main">428 MB</span>
                </div>
                <div className="mt-1.5 h-2 overflow-hidden rounded-full bg-surface-tertiary">
                  <div className="h-full w-[62%] rounded-full" style={{ background: theme.primaryColor }} />
                </div>
                <div className="mt-2 flex items-center justify-between text-[10px] text-content-hint">
                  <span>全局上限 1500 MB</span>
                  <span>62%</span>
                </div>
              </div>
            </CardBody>
          </Card>

          <Card>
            <CardHeader title="主题配置" />
            <CardBody className="space-y-2 text-[11px]">
              <div className="flex items-center justify-between"><span className="text-content-weak">模式</span><span className="font-mono text-content-main">{theme.mode}</span></div>
              <div className="flex items-center justify-between"><span className="text-content-weak">主色</span><span className="font-mono text-content-main">{theme.primaryColor}</span></div>
              <div className="flex items-center justify-between"><span className="text-content-weak">密度</span><span className="font-mono text-content-main">{theme.density}</span></div>
              <div className="flex items-center justify-between"><span className="text-content-weak">保存状态</span><span className={saving ? 'text-[var(--warning)]' : 'text-[var(--success)]'}>{saving ? '保存中…' : '已保存'}</span></div>
              <div className="mt-2 grid grid-cols-2 gap-2">
                <Button variant="secondary" size="sm" onClick={() => void persist()}>
                  <Save className="h-3.5 w-3.5" /> 立即保存
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    reset();
                    toast.info('已恢复默认主题');
                  }}
                >
                  <RotateCcw className="h-3.5 w-3.5" /> 恢复默认
                </Button>
              </div>
            </CardBody>
          </Card>

          <Card>
            <CardBody className="text-[11px] leading-5 text-content-weak">
              主题 CSS 仅作用于管理面板自身；被测页面运行在独立 Chromium 子进程中，不会受到任何注入或样式影响。导出 JSON 可用于团队统一风格。
            </CardBody>
          </Card>
        </div>
      </div>
    </div>
  );
}
