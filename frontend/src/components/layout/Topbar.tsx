import { useLocation, useNavigate } from 'react-router-dom';
import {
  ChevronLeft,
  MemoryStick,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Sun,
  Zap,
} from 'lucide-react';
import { useThemeStore } from '@/store/theme';
import { useEnvStore } from '@/store/env';
import { cn } from '@/utils';
import { Button } from '@/components/ui/button';
import { Tooltip } from '@/components/ui/tooltip';

const titles: Record<string, string> = {
  '/': '环境管理',
  '/proxies': '代理池',
  '/captcha': '验证码聚合',
  '/audit': '审计日志',
  '/theme': '主题外观',
  '/settings': '系统设置',
};

export function Topbar({
  sidebarCollapsed,
  onToggleSidebar,
}: {
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
}) {
  const location = useLocation();
  const navigate = useNavigate();
  const mode = useThemeStore((state) => state.theme.mode);
  const setMode = useThemeStore((state) => state.setMode);
  const envs = useEnvStore((state) => state.envs);
  const running = envs.filter((env) => env.runtime);
  const memory = running.reduce((total, env) => total + (env.runtime?.memoryMb ?? 0), 0);
  const title = location.pathname.startsWith('/envs/')
    ? '环境详情'
    : titles[location.pathname] ?? 'FinEnv';

  return (
    <header className="flex h-16 shrink-0 items-center justify-between gap-3 border-b border-border bg-surface-base/90 px-5 backdrop-blur">
      <div className="flex min-w-0 items-center gap-2">
        <Tooltip content={sidebarCollapsed ? '展开侧栏' : '折叠侧栏'} side="bottom">
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={onToggleSidebar}
            aria-label={sidebarCollapsed ? '展开侧栏' : '折叠侧栏'}
            aria-expanded={!sidebarCollapsed}
          >
            {sidebarCollapsed ? (
              <PanelLeftOpen className="h-4 w-4" />
            ) : (
              <PanelLeftClose className="h-4 w-4" />
            )}
          </Button>
        </Tooltip>
        {location.pathname.startsWith('/envs/') ? (
          <Button variant="ghost" size="icon-sm" onClick={() => navigate('/')} title="返回环境列表">
            <ChevronLeft className="h-4 w-4" />
          </Button>
        ) : null}
        <div className="min-w-0">
          <h1 className="truncate text-[15px] font-semibold tracking-tight text-content-main">{title}</h1>
          <p className="truncate text-[10px] text-content-hint">
            {running.length > 0
              ? `${running.length} 个独立浏览器进程运行中`
              : '管理面板与被测页面进程完全隔离'}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <div className="hidden items-center gap-3 rounded-full border border-border bg-surface-secondary px-3 py-1.5 text-[11px] text-content-weak md:flex">
          <span className="inline-flex items-center gap-1.5">
            <Zap className="h-3 w-3 text-[var(--success)]" />
            <span className="tabular-nums">{running.length}</span> 运行
          </span>
          <span className="h-3 w-px bg-border" />
          <span className="inline-flex items-center gap-1.5">
            <MemoryStick className="h-3 w-3 text-primary" />
            <span className="tabular-nums">{memory} MB</span>
          </span>
        </div>
        <Tooltip content={mode === 'dark' ? '切换亮色' : '切换暗色'}>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setMode(mode === 'dark' ? 'light' : 'dark')}
            aria-label="切换主题"
            className={cn(mode === 'auto' && 'text-primary')}
          >
            {mode === 'dark' ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
          </Button>
        </Tooltip>
      </div>
    </header>
  );
}
