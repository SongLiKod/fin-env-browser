export function normalizeHex(input: string): string {
  let value = String(input ?? '').trim().replace(/^#/, '');
  if (/^[0-9a-fA-F]{3}$/.test(value)) {
    value = value
      .split('')
      .map((char) => char + char)
      .join('');
  }
  if (!/^[0-9a-fA-F]{6}$/.test(value)) {
    return '#165DFF';
  }
  return `#${value.toUpperCase()}`;
}

export function hexToRgb(hex: string): [number, number, number] {
  const value = normalizeHex(hex).slice(1);
  return [
    parseInt(value.slice(0, 2), 16),
    parseInt(value.slice(2, 4), 16),
    parseInt(value.slice(4, 6), 16),
  ];
}

export function rgbToHex(rgb: [number, number, number]): string {
  return `#${rgb
    .map((channel) => Math.max(0, Math.min(255, Math.round(channel))).toString(16).padStart(2, '0'))
    .join('')
    .toUpperCase()}`;
}

export function mixHex(hex: string, target: [number, number, number], amount: number): string {
  const source = hexToRgb(hex);
  const ratio = Math.max(0, Math.min(1, amount));
  return rgbToHex([
    source[0] + (target[0] - source[0]) * ratio,
    source[1] + (target[1] - source[1]) * ratio,
    source[2] + (target[2] - source[2]) * ratio,
  ]);
}

export function lighten(hex: string, amount: number): string {
  return mixHex(hex, [255, 255, 255], amount);
}

export function darken(hex: string, amount: number): string {
  return mixHex(hex, [0, 0, 0], amount);
}

export function readableOn(hex: string): '#ffffff' | '#1d2129' {
  const [r, g, b] = hexToRgb(hex);
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return luminance > 0.62 ? '#1d2129' : '#ffffff';
}
