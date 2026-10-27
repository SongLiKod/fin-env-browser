import { create } from 'zustand';
import { ipc } from '@/ipc/client';
import type { PermissionKey, Role, SessionInfo } from '@/ipc/types';

interface AuthStore {
  session: SessionInfo | null;
  loading: boolean;
  error: string | null;
  initialize: () => Promise<void>;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  refresh: () => Promise<void>;
  can: (permission: PermissionKey) => boolean;
  hasRole: (...roles: Role[]) => boolean;
}

const emptySession: SessionInfo = {
  authenticated: false,
  user: null,
  permissions: [],
};

export const useAuthStore = create<AuthStore>((set, get) => ({
  session: null,
  loading: true,
  error: null,
  initialize: async () => {
    set({ loading: true });
    try {
      const session = await ipc.authStatus();
      set({ session, loading: false, error: null });
    } catch (error) {
      set({
        session: emptySession,
        loading: false,
        error: error instanceof Error ? error.message : '无法读取登录状态',
      });
    }
  },
  login: async (username, password) => {
    const session = await ipc.authLogin(username, password);
    set({ session, error: null });
  },
  logout: async () => {
    try {
      await ipc.authLogout();
    } finally {
      set({ session: emptySession });
    }
  },
  refresh: async () => {
    const session = await ipc.authStatus();
    set({ session });
  },
  can: (permission) => {
    const session = get().session;
    return !!session?.authenticated && session.permissions.includes(permission);
  },
  hasRole: (...roles) => {
    const role = get().session?.user?.role;
    return !!role && roles.includes(role);
  },
}));
