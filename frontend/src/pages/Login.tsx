import * as React from 'react';
import { Eye, EyeOff, Fingerprint, KeyRound, LockKeyhole, Moon, ShieldCheck, Sun, Terminal } from 'lucide-react';
import { ipc } from '@/ipc/client';
import { useAuthStore } from '@/store/auth';
import { useThemeStore } from '@/store/theme';
import { Button } from '@/components/ui/button';
import { Field, Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import { toastError } from '@/store/toast';

const onPrimary = 'text-[var(--text-on-primary)]';
const onPrimaryTint = (amount: number) => `color-mix(in srgb, var(--text-on-primary) ${amount}%, transparent)`;

export function LoginPage() {
  const login = useAuthStore((state) => state.login);
  const mode = useThemeStore((state) => state.theme.mode);
  const setMode = useThemeStore((state) => state.setMode);
  const [username, setUsername] = React.useState('admin');
  const [password, setPassword] = React.useState('');
  const [showPassword, setShowPassword] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [version, setVersion] = React.useState<string>('1.0.0');

  React.useEffect(() => {
    ipc
      .appInfo()
      .then((info) => setVersion(info.version))
      .catch(() => undefined);
  }, []);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!username.trim() || !password) {
      setError('请输入用户名和密码');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await login(username.trim(), password);
      await useThemeStore.getState().initialize();
    } catch (err) {
      setError(err instanceof Error ? err.message : '登录失败');
      toastError(err);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="relative flex min-h-full overflow-hidden bg-surface-secondary">
      <div className="absolute right-4 top-4 z-10">
        <Button variant="secondary" size="icon" onClick={() => setMode(mode === 'dark' ? 'light' : 'dark')}>
          {mode === 'dark' ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
        </Button>
      </div>

      <div
        className={`relative hidden w-[52%] flex-col justify-between overflow-hidden bg-[var(--color-primary)] p-12 lg:flex ${onPrimary}`}
      >        <div
          className="absolute -right-24 -top-24 h-96 w-96 rounded-full border"
          style={{ borderColor: onPrimaryTint(20) }}
        />
        <div
          className="absolute -bottom-32 -left-24 h-[28rem] w-[28rem] rounded-full border"
          style={{ borderColor: onPrimaryTint(15) }}
        />
        <div
          className="absolute bottom-20 right-14 h-40 w-40 rounded-full border"
          style={{ borderColor: onPrimaryTint(10) }}
        />
        <div className="relative">
          <div className="flex items-center gap-3">
            <div
              className="flex h-10 w-10 items-center justify-center rounded-xl backdrop-blur"
              style={{ background: onPrimaryTint(15) }}
            >
              <Fingerprint className="h-5 w-5" />
            </div>
            <div>
              <p className="text-lg font-semibold tracking-tight">FinEnv</p>
              <p className="text-[10px] uppercase tracking-[0.28em]" style={{ color: onPrimaryTint(60) }}>
                Fingerprint Environment
              </p>
            </div>
          </div>
          <h1 className="mt-16 max-w-md text-4xl font-semibold leading-tight tracking-tight">
            每个测试账号，
            <br />
            一套独立浏览器环境。
          </h1>
          <p className="mt-5 max-w-md text-sm leading-7" style={{ color: onPrimaryTint(70) }}>
            隔离 Cookie、缓存与指纹，保留登录态；为 Playwright、Selenium 与 CI 流水线提供可控的 CDP 环境。
          </p>
        </div>

        <div className="relative grid max-w-md grid-cols-2 gap-3">
          {[
            { icon: ShieldCheck, title: '进程级隔离', text: '独立 profile / 调试端口' },
            { icon: Fingerprint, title: '指纹一致性', text: 'UA / Canvas / WebGL / Audio' },
            { icon: Terminal, title: '自动化友好', text: 'REST API + JS / Python SDK' },
            { icon: LockKeyhole, title: '本地加密', text: 'AES-256-GCM 落盘' },
          ].map((item) => {
            const Icon = item.icon;
            return (
              <div
                key={item.title}
                className="rounded-lg border p-3 backdrop-blur-sm"
                style={{ borderColor: onPrimaryTint(15), background: onPrimaryTint(10) }}
              >
                <Icon className="h-4 w-4" style={{ color: onPrimaryTint(80) }} />
                <p className="mt-2 text-xs font-medium">{item.title}</p>
                <p className="mt-0.5 text-[10px]" style={{ color: onPrimaryTint(60) }}>
                  {item.text}
                </p>
              </div>
            );
          })}
        </div>
        <p className="relative text-[10px]" style={{ color: onPrimaryTint(45) }}>
          FinEnv V{version} · 管理面板不加载任何被测页面
        </p>
      </div>

      <div className="flex flex-1 items-center justify-center px-6 py-10">
        <div className="w-full max-w-[380px]">
          <div className="mb-8 lg:hidden">
            <div className="flex items-center gap-2.5">
              <img src="/logo.png" alt="FinEnv" className="h-9 w-9 rounded-lg" />
              <div>
                <p className="text-base font-semibold">FinEnv</p>
                <p className="text-[10px] uppercase tracking-widest text-content-hint">Fingerprint Env</p>
              </div>
            </div>
          </div>
          <div>
            <p className="text-[10px] font-semibold uppercase tracking-widest text-content-hint">团队工作台</p>
            <h2 className="mt-2 text-2xl font-semibold tracking-tight text-content-main">登录管理面板</h2>
            <p className="mt-1.5 text-xs leading-5 text-content-weak">
              敏感账号与 Cookie 仅以密文保存在本机，默认不上云。
            </p>
          </div>

          <Card className="mt-6 p-5">
            <form className="space-y-4" onSubmit={submit}>
              <Field label="用户名" required>
                <div className="relative">
                  <KeyRound className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-content-hint" />
                  <Input
                    className="pl-9"
                    value={username}
                    autoComplete="username"
                    onChange={(event) => setUsername(event.target.value)}
                    placeholder="admin"
                  />
                </div>
              </Field>
              <Field label="密码" required>
                <div className="relative">
                  <LockKeyhole className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-content-hint" />
                  <Input
                    className="pl-9 pr-10"
                    type={showPassword ? 'text' : 'password'}
                    value={password}
                    autoComplete="current-password"
                    onChange={(event) => setPassword(event.target.value)}
                    placeholder="输入登录密码"
                  />
                  <button
                    type="button"
                    className="absolute right-2.5 top-1/2 -translate-y-1/2 text-content-hint hover:text-content-main"
                    onClick={() => setShowPassword((value) => !value)}
                    aria-label={showPassword ? '隐藏密码' : '显示密码'}
                  >
                    {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                  </button>
                </div>
              </Field>
              {error ? (
                <div className="rounded-md border border-[color-mix(in_srgb,var(--danger)_30%,transparent)] bg-[color-mix(in_srgb,var(--danger)_8%,transparent)] px-3 py-2 text-xs text-danger">
                  {error}
                </div>
              ) : null}
              <Button type="submit" variant="primary" className="w-full" loading={busy}>
                登录 FinEnv
              </Button>
            </form>
            <div className="mt-4 border-t border-border pt-3 text-[11px] leading-5 text-content-hint">
              首次运行默认账号 <span className="font-mono text-content-weak">admin</span> /{' '}
              <span className="font-mono text-content-weak">FinEnv@2026</span>，登录后请在系统设置中立即修改密码。
            </div>
          </Card>

          <p className="mt-4 text-center text-[10px] text-content-hint">
            IPC 最小权限 · 高危操作二次确认 · 审计日志不可篡改
          </p>
        </div>
      </div>
    </div>
  );
}
