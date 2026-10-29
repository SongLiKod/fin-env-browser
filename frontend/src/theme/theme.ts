import type { ThemeConfig, ThemeDensity, ThemeMode } from '@/ipc/types';
import { darken, lighten, normalizeHex, readableOn } from './color';

export const THEME_CACHE_KEY = 'finenv-theme';
export const DEFAULT_THEME: ThemeConfig = {
  mode: 'auto',
  primaryColor: '#165DFF',
  density: 'comfortable',
  config: {},
  updatedAt: new Date(0).toISOString(),
};

export function systemPrefersDark(): boolean {
  return typeof window !== 'undefined' && !!window.matchMedia?.('(prefers-color-scheme: dark)').matches;
}

export function resolveThemeMode(mode: ThemeMode): 'light' | 'dark' {
  if (mode === 'auto') return systemPrefersDark() ? 'dark' : 'light';
  return mode;
}

export function loadCachedTheme(): ThemeConfig {
  if (typeof localStorage === 'undefined') return { ...DEFAULT_THEME };
  try {
    const raw = localStorage.getItem(THEME_CACHE_KEY);
    if (!raw) return { ...DEFAULT_THEME };
    const parsed = JSON.parse(raw) as Partial<ThemeConfig>;
    return {
      mode: parsed.mode === 'light' || parsed.mode === 'dark' || parsed.mode === 'auto' ? parsed.mode : 'auto',
      primaryColor: normalizeHex(parsed.primaryColor ?? DEFAULT_THEME.primaryColor),
      density: parsed.density === 'compact' ? 'compact' : 'comfortable',
      config: parsed.config ?? {},
      updatedAt: parsed.updatedAt ?? DEFAULT_THEME.updatedAt,
    };
  } catch {
    return { ...DEFAULT_THEME };
  }
}

export function primaryLightFor(primary: string, resolved: 'light' | 'dark'): string {
  return resolved === 'dark' ? darken(primary, 0.72) : lighten(primary, 0.9);
}

export function primaryDarkFor(primary: string, resolved: 'light' | 'dark'): string {
  return resolved === 'dark' ? lighten(primary, 0.12) : darken(primary, 0.16);
}

export function applyTheme(theme: ThemeConfig): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  const resolved = resolveThemeMode(theme.mode);
  const primary = normalizeHex(theme.primaryColor);
  root.setAttribute('data-theme', resolved);
  root.setAttribute('data-density', theme.density);
  root.style.setProperty('--color-primary', primary);
  root.style.setProperty('--color-primary-light', primaryLightFor(primary, resolved));
  root.style.setProperty('--color-primary-dark', primaryDarkFor(primary, resolved));
  root.style.setProperty('--text-on-primary', readableOn(primary));
  root.style.colorScheme = resolved;
  try {
    localStorage.setItem(
      THEME_CACHE_KEY,
      JSON.stringify({
        mode: theme.mode,
        primaryColor: primary,
        density: theme.density,
        config: theme.config,
        updatedAt: theme.updatedAt,
      }),
    );
  } catch {
    // localStorage may be unavailable; the backend remains the source of truth.
  }
}

export function nextTheme(
  current: ThemeConfig,
  patch: Partial<Pick<ThemeConfig, 'mode' | 'primaryColor' | 'density'>>,
): ThemeConfig {
  return {
    ...current,
    ...patch,
    primaryColor: normalizeHex(patch.primaryColor ?? current.primaryColor),
    updatedAt: new Date().toISOString(),
  };
}

export const themeModes: Array<{ value: ThemeMode; label: string; hint: string }> = [
  { value: 'light', label: '亮色', hint: '始终使用浅色面板' },
  { value: 'dark', label: '暗色', hint: '始终使用深色面板' },
  { value: 'auto', label: '跟随系统', hint: '跟随操作系统明暗设置' },
];

export const densities: Array<{ value: ThemeDensity; label: string; hint: string }> = [
  { value: 'comfortable', label: '宽松', hint: '更大的行高与留白' },
  { value: 'compact', label: '紧凑', hint: '高密度信息展示' },
];
