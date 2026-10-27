import { Outlet } from 'react-router-dom';
import { BroadcastBar } from './BroadcastBar';
import { RunningEnvironments } from './RunningEnvironments';
import { Sidebar } from './Sidebar';
import { Topbar } from './Topbar';

export function AppShell() {
  return (
    <div className="flex h-full overflow-hidden bg-surface-secondary text-content-main">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar />
        <RunningEnvironments />
        <BroadcastBar />
        <main className="min-h-0 flex-1 overflow-y-auto">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
