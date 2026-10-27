import { create } from 'zustand';
import { ipc } from '@/ipc/client';
import type { ThemeConfig, ThemeDensity, ThemeMode } from '@/ipc/types';
import {
  applyTheme,
  DEFAULT_THEME,
  loadCachedTheme,
  nextTheme,
  resolveThemeMode,
} from '@/theme/theme';

interface ThemeStore {
  theme: ThemeConfig;
  hydrated: boolean;
  saving: boolean;
  lastError: string | null;
  initialize: () => Promise<void>;
  setMode: (mode: ThemeMode) => void;
  setPrimaryColor: (color: string) => void;
  setDensity: (density: ThemeDensity) => void;
  persist: () => Promise<void>;
  reset: () => void;
  exportTheme: (path: string) => Promise<void>;
  importTheme: (path: string) => Promise<ThemeConfig>;
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleSave(get: () => ThemeStore) {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    void get().persist();
  }, 450);
}

export const useThemeStore = create<ThemeStore>((set, get) => {
  const cached = loadCachedTheme();
  applyTheme(cached);

  return {
    theme: cached,
    hydrated: false,
    saving: false,
    lastError: null,
    initialize: async () => {
      applyTheme(get().theme);
      try {
        const theme = await ipc.themeGet();
        applyTheme(theme);
        set({ theme, hydrated: true, lastError: null });
      } catch {
        set({ hydrated: true });
      }
    },
    setMode: (mode) => {
      const theme = nextTheme(get().theme, { mode });
      applyTheme(theme);
      set({ theme });
      scheduleSave(get);
    },
    setPrimaryColor: (color) => {
      const theme = nextTheme(get().theme, { primaryColor: color });
      applyTheme(theme);
      set({ theme });
      scheduleSave(get);
    },
    setDensity: (density) => {
      const theme = nextTheme(get().theme, { density });
      applyTheme(theme);
      set({ theme });
      scheduleSave(get);
    },
    persist: async () => {
      const theme = get().theme;
      set({ saving: true });
      try {
        const saved = await ipc.themeSave({
          ...theme,
          updatedAt: new Date().toISOString(),
        });
        applyTheme(saved);
        set({ theme: saved, saving: false, lastError: null, hydrated: true });
      } catch (error) {
        set({
          saving: false,
          lastError: error instanceof Error ? error.message : '主题保存失败',
        });
      }
    },
    reset: () => {
      const theme = { ...DEFAULT_THEME, updatedAt: new Date().toISOString() };
      applyTheme(theme);
      set({ theme });
      scheduleSave(get);
    },
    exportTheme: async (path) => {
      await ipc.themeExport(path);
    },
    importTheme: async (path) => {
      const theme = await ipc.themeImport(path);
      applyTheme(theme);
      set({ theme, hydrated: true });
      return theme;
    },
  };
});

export function currentResolvedMode(): 'light' | 'dark' {
  return resolveThemeMode(useThemeStore.getState().theme.mode);
}
