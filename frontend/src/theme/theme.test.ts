import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  applyTheme,
  DEFAULT_THEME,
  densities,
  loadCachedTheme,
  nextTheme,
  resolveThemeMode,
  themeModes,
  THEME_CACHE_KEY,
} from './theme';

function mockPrefersDark(dark: boolean) {
  const listeners: Array<() => void> = [];
  const media = {
    matches: dark,
    addEventListener: (_event: string, handler: () => void) => listeners.push(handler),
    removeEventListener: vi.fn(),
  };
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: vi.fn(() => media),
  });
}

describe('theme system', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');
    document.documentElement.removeAttribute('data-density');
    document.documentElement.style.cssText = '';
  });

  it('resolves auto mode from the system preference', () => {
    mockPrefersDark(true);
    expect(resolveThemeMode('auto')).toBe('dark');
    mockPrefersDark(false);
    expect(resolveThemeMode('auto')).toBe('light');
    expect(resolveThemeMode('light')).toBe('light');
    expect(resolveThemeMode('dark')).toBe('dark');
  });

  it('applies CSS tokens and persists the cache', () => {
    mockPrefersDark(true);
    applyTheme({ ...DEFAULT_THEME, mode: 'auto', primaryColor: '#0e9f6e', density: 'compact' });
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.documentElement.dataset.density).toBe('compact');
    expect(document.documentElement.style.getPropertyValue('--color-primary')).toBe('#0E9F6E');
    expect(document.documentElement.style.getPropertyValue('--color-primary-light')).not.toBe('');
    const cached = JSON.parse(localStorage.getItem(THEME_CACHE_KEY) ?? '{}');
    expect(cached.primaryColor).toBe('#0E9F6E');
    expect(cached.density).toBe('compact');
  });

  it('loads the cached theme and falls back safely', () => {
    localStorage.setItem(THEME_CACHE_KEY, JSON.stringify({ mode: 'dark', primaryColor: '#fff', density: 'nope' }));
    const loaded = loadCachedTheme();
    expect(loaded.mode).toBe('dark');
    expect(loaded.primaryColor).toBe('#FFFFFF');
    expect(loaded.density).toBe('comfortable');
    localStorage.setItem(THEME_CACHE_KEY, '{bad json');
    expect(loadCachedTheme().mode).toBe('auto');
  });

  it('updates timestamps and normalizes primary color', () => {
    const updated = nextTheme(DEFAULT_THEME, { primaryColor: 'abc' });
    expect(updated.primaryColor).toBe('#AABBCC');
    expect(updated.updatedAt).not.toBe(DEFAULT_THEME.updatedAt);
    expect(themeModes.map((item) => item.value)).toEqual(['light', 'dark', 'auto']);
    expect(densities.map((item) => item.value)).toEqual(['comfortable', 'compact']);
  });
});
