import * as React from 'react';
import { listen } from '@tauri-apps/api/event';
import { AlertTriangle } from 'lucide-react';

import { LoginPage } from '@/pages/Login';
import { AppShell } from '@/components/layout/AppShell';
import { ToastViewport } from '@/components/ui/toast';
import { Button } from '@/components/ui/button';
import { useAuthStore } from '@/store/auth';
import { useThemeStore } from '@/store/theme';
import { useEnvStore } from '@/store/env';
import { useToastStore } from '@/store/toast';
import { applyTheme } from '@/theme/theme';
import {
  ipc,
  onApiError,
  onBrowserExit,
  onBrowserRuntime,
  onCaptcha,
  onResourceAlert,
} from '@/ipc/client';
import { EnvironmentsPage } from '@/pages/Environments';
import { EnvironmentDetailPage } from '@/pages/EnvironmentDetail';
import { ProxiesPage } from '@/pages/Proxies';
import { AuditPage } from '@/pages/Audit';
import { CaptchaPage } from '@/pages/Captcha';
import { ThemePage } from '@/pages/Theme';
import { SettingsPage } from '@/pages/Settings';
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom';

function Splash() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 bg-surface-secondary">
      <img src="/logo.png" alt="FinEnv" className="h-14 w-14 animate-pulse rounded-2xl" />
      <div className="text-center">
        <p className="text-sm font-semibold text-content-main">FinEnv</p>
        <p className="mt-1 text-[11px] text-content-hint">正在加载隔离环境与主题配置…</p>
      </div>
    </div>
  );
}

class ErrorBoundary extends React.Component<
  { children: React.ReactNode },
  { error: Error | null }
> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-full items-center justify-center bg-surface-secondary p-6">
          <div className="card max-w-lg p-6 text-center">
            <AlertTriangle className="mx-auto h-8 w-8 text-danger" />
            <h2 className="mt-3 text-base font-semibold text-content-main">界面渲染失败</h2>
            <p className="mt-2 break-words text-xs leading-5 text-content-weak">{this.state.error.message}</p>
            <Button className="mt-4" variant="primary" onClick={() => window.location.reload()}>
              重新加载
            </Button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

export default function App() {
  return (
    <ErrorBoundary>
      <AppContent />
    </ErrorBoundary>
  );
}

function AppContent() {
  const session = useAuthStore((state) => state.session);
  const authLoading = useAuthStore((state) => state.loading);
  const initializeAuth = useAuthStore((state) => state.initialize);
  const initializeTheme = useThemeStore((state) => state.initialize);
  const [booted, setBooted] = React.useState(false);
  const startupHandled = React.useRef(false);

  React.useEffect(() => {
    let active = true;
    void (async () => {
      await initializeTheme();
      await initializeAuth();
      if (active) setBooted(true);
    })();
    return () => {
      active = false;
    };
  }, [initializeAuth, initializeTheme]);

  React.useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => {
      const theme = useThemeStore.getState().theme;
      if (theme.mode === 'auto') applyTheme(theme);
    };
    media.addEventListener('change', onChange);
    return () => media.removeEventListener('change', onChange);
  }, []);

  const authenticated = !!session?.authenticated;

  React.useEffect(() => {
    if (!authenticated) return;
    void useEnvStore
      .getState()
      .refresh(true)
      .catch(() => undefined);
  }, [authenticated]);

  React.useEffect(() => {
    if (!authenticated || startupHandled.current) return;
    startupHandled.current = true;
    void (async () => {
      try {
        const envId = await ipc.startupEnv();
        if (!envId) return;
        window.location.hash = `#/envs/${envId}`;
        await ipc.browserOpen(envId, {});
        useToastStore.getState().success('已从桌面快捷方式启动环境');
      } catch (error) {
        useToastStore.getState().error(
          '快捷方式启动失败',
          error instanceof Error ? error.message : undefined,
        );
      }
    })();
  }, [authenticated]);

  React.useEffect(() => {
    if (!authenticated) return;
    let unlisten: (() => void) | undefined;
    listen<string>('shortcut://env', (event) => {
      const envId = event.payload;
      window.location.hash = `#/envs/${envId}`;
      void ipc
        .browserOpen(envId, {})
        .then(() => useToastStore.getState().success('已从桌面快捷方式启动环境'))
        .catch((error) =>
          useToastStore
            .getState()
            .error('快捷方式启动失败', error instanceof Error ? error.message : undefined),
        );
    })
      .then((dispose) => {
        unlisten = dispose;
      })
      .catch(() => undefined);
    return () => {
      unlisten?.();
    };
  }, [authenticated]);

  React.useEffect(() => {
    if (!authenticated) return;
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const register = (promise: Promise<() => void>) => {
      promise
        .then((unlisten) => {
          if (disposed) unlisten();
          else unlisteners.push(unlisten);
        })
        .catch(() => undefined);
    };
    register(
      onBrowserRuntime((runtime) => {
        useEnvStore.getState().applyRuntime(runtime);
      }),
    );
    register(
      onBrowserExit((event) => {
        useEnvStore.getState().clearRuntime(event.envId);
        if (event.reason === 'crash') {
          useToastStore.getState().warning('浏览器进程异常退出', `环境 ${event.envId} 已退出，可在列表中重新打开`);
        }
      }),
    );
    register(
      onCaptcha((event) => {
        useToastStore
          .getState()
          .info('检测到验证码', `${event.envName} · ${event.url || '当前页面'}`);
      }),
    );
    register(
      onResourceAlert((event) => {
        useToastStore.getState().warning('资源告警', event.message);
      }),
    );
    register(
      onApiError((message) => {
        useToastStore.getState().error('API 服务启动失败', message);
      }),
    );
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [authenticated]);

  if (!booted || (authLoading && !session)) {
    return (
      <>
        <Splash />
        <ToastViewport />
      </>
    );
  }

  if (!authenticated) {
    return (
      <>
        <LoginPage />
        <ToastViewport />
      </>
    );
  }

  return (
    <>
      <HashRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route index element={<EnvironmentsPage />} />
            <Route path="envs/:id" element={<EnvironmentDetailPage />} />
            <Route path="proxies" element={<ProxiesPage />} />
            <Route path="captcha" element={<CaptchaPage />} />
            <Route path="audit" element={<AuditPage />} />
            <Route path="theme" element={<ThemePage />} />
            <Route path="settings" element={<SettingsPage />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Route>
        </Routes>
      </HashRouter>
      <ToastViewport />
    </>
  );
}
