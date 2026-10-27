import * as React from 'react';
import { NavLink, useLocation } from 'react-router-dom';
import {
  Activity,
  Fingerprint,
  KeyRound,
  LogOut,
  Palette,
  ScrollText,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Users,
} from 'lucide-react';
import { cn, roleLabels } from '@/utils';
import { useAuthStore } from '@/store/auth';
import { useEnvStore } from '@/store/env';
import { useToastStore } from '@/store/toast';
import { Button } from '@/components/ui/button';
import { Tooltip } from '@/components/ui/tooltip';
import { PasswordDialog } from './PasswordDialog';

const navigation = [
  { to: '/', label: '环境管理', icon: Fingerprint, permission: 'view_list' as const },
  { to: '/proxies', label: '代理池', icon: SlidersHorizontal, permission: 'view_list' as const },
  { to: '/captcha', label: '验证码聚合', icon: ShieldCheck, permission: 'view_list' as const },
  { to: '/audit', label: '审计日志', icon: ScrollText, permission: 'audit_view' as const },
  { to: '/theme', label: '主题外观', icon: Palette, permission: null },
  { to: '/settings', label: '系统设置', icon: Settings2, permission: 'settings_manage' as const },
];

export function Sidebar({ onNavigate }: { onNavigate?: () => void }) {
  const location = useLocation();
  const user = useAuthStore((state) => state.session?.user);
  const can = useAuthStore((state) => state.can);
  const logout = useAuthStore((state) => state.logout);
  const envs = useEnvStore((state) => state.envs);
  const running = envs.filter((env) => env.runtime).length;
  const toast = useToastStore();
  const [passwordOpen, setPasswordOpen] = React.useState(false);

  const items = navigation.filter((item) => !item.permission || can(item.permission));

  return (
    <aside
      className="flex h-full shrink-0 flex-col border-r border-border bg-surface-base"
      style={{ width: 'var(--sidebar-width)' }}
    >
      <div className="flex h-16 items-center gap-2.5 border-b border-border px-4">
        <img src="/logo.png" alt="FinEnv" className="h-8 w-8 rounded-md" />
        <div className="min-w-0">
          <p className="truncate text-sm font-semibold tracking-tight text-content-main">FinEnv</p>
          <p className="text-[10px] uppercase tracking-widest text-content-hint">Fingerprint Env</p>
        </div>
      </div>

      <div className="flex items-center justify-between px-4 pb-2 pt-4">
        <span className="text-[10px] font-semibold uppercase tracking-widest text-content-hint">工作区</span>
        <span className="chip">
          <span className="h-1.5 w-1.5 rounded-full bg-[var(--success)]" />
          {running} 运行
        </span>
      </div>

      <nav className="space-y-0.5 px-2">
        {items.map((item) => {
          const Icon = item.icon;
          const active =
            item.to === '/'
              ? location.pathname === '/' || location.pathname.startsWith('/envs/')
              : location.pathname === item.to;
          return (
            <NavLink
              key={item.to}
              to={item.to}
              onClick={onNavigate}
              className={cn('nav-item', active && 'active')}
            >
              <Icon className="h-4 w-4" />
              <span className="flex-1">{item.label}</span>
              {item.to === '/' && envs.length > 0 ? (
                <span className="text-[10px] tabular-nums text-content-hint">{envs.length}</span>
              ) : null}
            </NavLink>
          );
        })}
      </nav>

      <div className="mx-3 mt-4 rounded-lg border border-border bg-surface-secondary p-3">
        <div className="flex items-center gap-2 text-xs font-medium text-content-main">
          <Activity className="h-3.5 w-3.5 text-primary" />
          资源管控
        </div>
        <p className="mt-1 text-[11px] leading-5 text-content-weak">
          独立进程 · 独立 profile · 随机调试端口
        </p>
        <div className="mt-2 flex items-center gap-1.5 text-[10px] text-content-hint">
          <ShieldCheck className="h-3 w-3" /> AES-256-GCM 本地加密
        </div>
      </div>

      <div className="mt-auto border-t border-border p-3">
        <div className="flex items-center gap-2.5 rounded-md px-1 py-1.5">
          <div className="flex h-8 w-8 items-center justify-center rounded-full bg-primary-light text-xs font-semibold text-primary">
            {user?.displayName?.slice(0, 1).toUpperCase() ?? 'F'}
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-xs font-medium text-content-main">{user?.displayName ?? '未登录'}</p>
            <p className="truncate text-[10px] text-content-hint">
              {user ? roleLabels[user.role] : '—'}
            </p>
          </div>
          <Tooltip content="修改密码">
            <Button variant="ghost" size="icon-sm" onClick={() => setPasswordOpen(true)}>
              <KeyRound className="h-3.5 w-3.5" />
            </Button>
          </Tooltip>
          <Tooltip content="退出登录">
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={async () => {
                try {
                  await logout();
                  toast.info('已退出登录');
                } catch (error) {
                  toast.error('退出失败', error instanceof Error ? error.message : undefined);
                }
              }}
            >
              <LogOut className="h-3.5 w-3.5" />
            </Button>
          </Tooltip>
        </div>
        <div className="mt-2 flex items-center justify-between px-1 text-[10px] text-content-hint">
          <span>FinEnv V1.0</span>
          <span className="inline-flex items-center gap-1">
            <Users className="h-3 w-3" /> 团队版
          </span>
        </div>
      </div>
      <PasswordDialog open={passwordOpen} onOpenChange={setPasswordOpen} />
    </aside>
  );
}
