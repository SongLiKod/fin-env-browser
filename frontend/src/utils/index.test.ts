import { describe, expect, it } from 'vitest';
import {
  auditActionLabels,
  formatBytes,
  formatDuration,
  parseCsvRows,
  parseTags,
  permissionLabels,
  roleLabels,
  truncate,
} from './index';

describe('formatting helpers', () => {
  it('formats byte sizes', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1024)).toBe('1.0 KB');
    expect(formatBytes(10 * 1024)).toBe('10 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
  });

  it('formats durations', () => {
    expect(formatDuration(0)).toBe('0s');
    expect(formatDuration(12_000)).toBe('12s');
    expect(formatDuration(65_000)).toBe('1m 5s');
    expect(formatDuration(3_600_000)).toBe('1h 0m');
  });

  it('truncates long strings', () => {
    expect(truncate('abcdef', 3)).toBe('abc…');
    expect(truncate('abc', 3)).toBe('abc');
  });
});

describe('input parsing', () => {
  it('parses and deduplicates tags', () => {
    expect(parseTags('支付, 回归  支付\nP0')).toEqual(['支付', '回归', 'P0']);
  });

  it('parses quoted CSV with newlines and commas', () => {
    const rows = parseCsvRows('username,remark\n"alice","hello, world"\n"bob","line1\nline2"');
    expect(rows).toEqual([
      ['username', 'remark'],
      ['alice', 'hello, world'],
      ['bob', 'line1\nline2'],
    ]);
  });

  it('keeps empty fields in CSV rows', () => {
    expect(parseCsvRows('a,,c\n,,d\n')).toEqual([
      ['a', '', 'c'],
      ['', '', 'd'],
    ]);
  });
});

describe('labels', () => {
  it('has labels for every role and permission', () => {
    expect(Object.keys(roleLabels)).toHaveLength(4);
    expect(Object.values(roleLabels).every(Boolean)).toBe(true);
    expect(Object.keys(permissionLabels)).toHaveLength(12);
  });

  it('covers core audit actions', () => {
    expect(auditActionLabels['env.create']).toBe('创建环境');
    expect(auditActionLabels['browser.open']).toBe('打开浏览器');
    expect(auditActionLabels['account.reveal']).toBe('查看密码');
  });
});
