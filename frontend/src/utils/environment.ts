export const ENV_COLOR_PALETTE = [
  '#2563EB',
  '#7C3AED',
  '#DB2777',
  '#DC2626',
  '#EA580C',
  '#CA8A04',
  '#16A34A',
  '#0D9488',
  '#0891B2',
  '#475569',
];

const HEX_COLOR = /^#[0-9A-Fa-f]{6}$/;

export function isValidHexColor(value: string | null | undefined): boolean {
  return !!value && HEX_COLOR.test(value);
}

/** Stable fallback color so every environment is distinguishable even before
 *  the user picks a custom accent. */
export function envBorderColor(env: { id: string; borderColor?: string | null }): string {
  if (env.borderColor && isValidHexColor(env.borderColor)) return env.borderColor.toUpperCase();
  let hash = 0;
  for (const char of env.id) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return ENV_COLOR_PALETTE[hash % ENV_COLOR_PALETTE.length];
}

export function hexToRgba(hex: string, alpha: number): string {
  const value = hex.replace('#', '');
  if (value.length !== 6) return `rgba(37, 99, 235, ${alpha})`;
  const r = Number.parseInt(value.slice(0, 2), 16);
  const g = Number.parseInt(value.slice(2, 4), 16);
  const b = Number.parseInt(value.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function envIpSummary(env: {
  ipAddress?: string | null;
  ipCountry?: string | null;
  ipCountryCode?: string | null;
  ipCity?: string | null;
}): string {
  if (!env.ipAddress) return '未检测';
  const location = env.ipCity || env.ipCountry || env.ipCountryCode;
  return location ? `${env.ipAddress} · ${location}` : env.ipAddress;
}
