import { describe, expect, it } from 'vitest';
import { envBorderColor, envBorderWidth, envIpSummary, hexToRgba, isValidHexColor } from './environment';

describe('environment identity helpers', () => {
  it('uses a custom border color and rejects invalid values', () => {
    expect(envBorderColor({ id: 'abc', borderColor: '#16a34a' })).toBe('#16A34A');
    expect(isValidHexColor('#16A34A')).toBe(true);
    expect(isValidHexColor('green')).toBe(false);
  });

  it('derives a stable fallback color from the environment id', () => {
    const first = envBorderColor({ id: 'env-123', borderColor: '' });
    const second = envBorderColor({ id: 'env-123', borderColor: '' });
    expect(first).toMatch(/^#[0-9A-F]{6}$/);
    expect(first).toBe(second);
  });

  it('clamps the accent width to a usable range', () => {
    expect(envBorderWidth({ borderWidth: 6 })).toBe(6);
    expect(envBorderWidth({ borderWidth: 0 })).toBe(1);
    expect(envBorderWidth({ borderWidth: 99 })).toBe(12);
    expect(envBorderWidth({})).toBe(3);
  });

  it('formats the IP summary and rgba color', () => {
    expect(envIpSummary({ ipAddress: '203.0.113.10', ipCity: 'Tokyo' })).toBe('203.0.113.10 · Tokyo');
    expect(envIpSummary({ ipAddress: null })).toBe('未检测');
    expect(hexToRgba('#2563EB', 0.1)).toBe('rgba(37, 99, 235, 0.1)');
  });
});
