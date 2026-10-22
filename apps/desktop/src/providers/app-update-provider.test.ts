import { describe, expect, it } from 'vitest';
import SOURCE from '@/providers/app-update-provider.tsx?raw';

function checkForUpdateBody(source: string): string {
  const start = source.indexOf('async function checkForUpdate');
  if (start < 0) {
    return '';
  }
  const next = source.indexOf('\n  async function ', start + 'async function checkForUpdate'.length);
  return source.slice(start, next < 0 ? source.length : next);
}

describe('应用更新：这个版本不自己去问上游', () => {
  it('源码里没有启动时的静默检查', () => {
    expect(SOURCE).not.toContain('silent: true');
  });

  it('checkForUpdate 在调用 runUpdateCheck 之前先看开关', () => {
    const body = checkForUpdateBody(SOURCE);
    const headerEnd = body.indexOf(') {');
    expect(headerEnd).toBeGreaterThanOrEqual(0);
    const head = body.slice(headerEnd + ') {'.length).trimStart();
    const guard = head.indexOf('IN_APP_UPDATES_ENABLED');
    const call = head.indexOf('runUpdateCheck(');
    expect(head.startsWith('if (!IN_APP_UPDATES_ENABLED)')).toBe(true);
    expect(guard).toBeGreaterThanOrEqual(0);
    expect(call).toBeGreaterThan(guard);
  });

  it('仍然读取当前版本', () => {
    expect(SOURCE).toContain('getVersion()');
  });
});
