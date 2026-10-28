import * as React from 'react';
import { Outlet } from 'react-router-dom';
import { BroadcastBar } from './BroadcastBar';
import { RunningEnvironments } from './RunningEnvironments';
import { Sidebar } from './Sidebar';
import { Topbar } from './Topbar';

const SIDEBAR_COLLAPSED_KEY = 'finenv.sidebar.collapsed';

export function AppShell() {
  const [sidebarCollapsed, setSidebarCollapsed] = React.useState(() => {
    try {
      return window.localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === '1';
    } catch {
      return false;
    }
  });

  const toggleSidebar = () => {
    setSidebarCollapsed((current) => {
      const next = !current;
      try {
        window.localStorage.setItem(SIDEBAR_COLLAPSED_KEY, next ? '1' : '0');
      } catch {
        // localStorage can be unavailable in hardened webviews.
      }
      return next;
    });
  };

  return (
    <div className="flex h-full overflow-hidden bg-surface-secondary text-content-main">
      <Sidebar collapsed={sidebarCollapsed} />
      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar sidebarCollapsed={sidebarCollapsed} onToggleSidebar={toggleSidebar} />
        <RunningEnvironments />
        <BroadcastBar />
        <main className="min-h-0 flex-1 overflow-y-auto">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
