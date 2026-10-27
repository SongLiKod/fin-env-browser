import { create } from 'zustand';
import { ipc } from '@/ipc/client';
import type {
  BrowserRuntime,
  EnvironmentFilter,
  EnvironmentView,
  Group,
} from '@/ipc/types';

interface EnvStore {
  envs: EnvironmentView[];
  groups: Group[];
  projects: string[];
  tags: string[];
  filter: EnvironmentFilter;
  loading: boolean;
  initialized: boolean;
  error: string | null;
  refresh: (withFacets?: boolean) => Promise<void>;
  syncRuntimes: () => Promise<void>;
  setFilter: (filter: Partial<EnvironmentFilter>) => void;
  resetFilter: () => void;
  get: (id: string) => EnvironmentView | undefined;
  replace: (env: EnvironmentView) => void;
  remove: (id: string) => void;
  applyRuntime: (runtime: BrowserRuntime) => void;
  clearRuntime: (envId: string) => void;
}

export const useEnvStore = create<EnvStore>((set, get) => ({
  envs: [],
  groups: [],
  projects: [],
  tags: [],
  filter: {},
  loading: false,
  initialized: false,
  error: null,
  refresh: async (withFacets = true) => {
    set({ loading: true });
    try {
      const envs = await ipc.envList(get().filter);
      set({ envs, loading: false, initialized: true, error: null });
      if (withFacets) {
        const [groups, projects, tags] = await Promise.all([
          ipc.groupList(),
          ipc.envProjects(),
          ipc.envTags(),
        ]);
        set({ groups, projects, tags });
      }
    } catch (error) {
      set({
        loading: false,
        initialized: true,
        error: error instanceof Error ? error.message : '环境列表加载失败',
      });
      throw error;
    }
  },
  syncRuntimes: async () => {
    try {
      const snapshot = await ipc.resourceSnapshot();
      const map = new Map(snapshot.instances.map((item) => [item.envId, item]));
      set((state) => ({
        envs: state.envs.map((env) => ({
          ...env,
          runtime: map.get(env.id) ?? null,
        })),
      }));
    } catch {
      // Runtime polling is best effort; the event stream is the primary source.
    }
  },
  setFilter: (filter) => set((state) => ({ filter: { ...state.filter, ...filter } })),
  resetFilter: () => set({ filter: {} }),
  get: (id) => get().envs.find((env) => env.id === id),
  replace: (env) =>
    set((state) => ({
      envs: state.envs.some((item) => item.id === env.id)
        ? state.envs.map((item) => (item.id === env.id ? env : item))
        : [env, ...state.envs],
    })),
  remove: (id) => set((state) => ({ envs: state.envs.filter((env) => env.id !== id) })),
  applyRuntime: (runtime) =>
    set((state) => ({
      envs: state.envs.map((env) =>
        env.id === runtime.envId ? { ...env, runtime } : env,
      ),
    })),
  clearRuntime: (envId) =>
    set((state) => ({
      envs: state.envs.map((env) => (env.id === envId ? { ...env, runtime: null } : env)),
    })),
}));
