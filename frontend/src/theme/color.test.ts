import { describe, expect, it } from 'vitest';
import {
  darken,
  hexToRgb,
  lighten,
  mixHex,
  normalizeHex,
  readableOn,
  rgbToHex,
} from './color';

describe('theme color utilities', () => {
  it('normalizes short and invalid hex values', () => {
    expect(normalizeHex('#fff')).toBe('#FFFFFF');
    expect(normalizeHex('165dff')).toBe('#165DFF');
    expect(normalizeHex('not-a-color')).toBe('#165DFF');
  });

  it('converts rgb and hex consistently', () => {
    expect(hexToRgb('#165DFF')).toEqual([22, 93, 255]);
    expect(rgbToHex([22, 93, 255])).toBe('#165DFF');
  });

  it('mixes toward white and black', () => {
    expect(lighten('#000000', 0.5)).toBe('#808080');
    expect(darken('#FFFFFF', 0.5)).toBe('#808080');
    expect(mixHex('#000000', [255, 255, 255], 1)).toBe('#FFFFFF');
    expect(mixHex('#000000', [255, 255, 255], 2)).toBe('#FFFFFF');
  });

  it('chooses readable foreground', () => {
    expect(readableOn('#FFFFFF')).toBe('#1d2129');
    expect(readableOn('#000000')).toBe('#ffffff');
  });
});
