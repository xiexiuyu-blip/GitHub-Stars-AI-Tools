import { describe, expect, it } from 'vitest';
import {
  MAINTENANCE_ACTIVE_DAYS,
  MAINTENANCE_FILTER_HINTS,
  MAINTENANCE_HINTS,
  MAINTENANCE_LABELS,
  MAINTENANCE_STALE_DAYS,
  MAINTENANCE_TIERS,
  maintenanceOf,
} from './maintenance';

const NOW = '2026-10-22T12:00:00Z';
const NOW_MS = Date.parse(NOW);

const BOUNDARY: Array<{ pushed: string; tier: 'active' | 'slow' | 'stale' }> = [
  { pushed: '2026-10-22T12:00:00Z', tier: 'active' },
  { pushed: '2026-10-23T12:00:00Z', tier: 'active' },
  { pushed: '2026-07-24T12:00:00Z', tier: 'active' },
  { pushed: '2026-07-24T11:59:59Z', tier: 'slow' },
  { pushed: '2025-10-22T12:00:00Z', tier: 'slow' },
  { pushed: '2025-10-22T11:59:59Z', tier: 'stale' },
  { pushed: '2021-10-22T12:00:00Z', tier: 'stale' },
];

describe('维护状态门槛', () => {
  it('和后端同一张边界表：90 天、365 天，空日期没有状态，三档文案固定', () => {
    expect(MAINTENANCE_ACTIVE_DAYS).toBe(90);
    expect(MAINTENANCE_STALE_DAYS).toBe(365);
    expect(MAINTENANCE_TIERS).toEqual(['active', 'slow', 'stale', 'archived']);
    expect(MAINTENANCE_LABELS).toEqual({ active: '活跃', slow: '放缓', stale: '疑似停更', archived: '已归档' });
    expect(MAINTENANCE_HINTS).toEqual({
      active: '最近 3 个月内有更新',
      slow: '3 到 12 个月没有更新',
      stale: '超过 1 年没有更新，可能已经不维护了',
      archived: '作者已在 GitHub 上把它归档：只读，不会再更新',
    });

    for (const row of BOUNDARY) {
      const pushedMs = Date.parse(row.pushed);
      expect(maintenanceOf(row.pushed, NOW), row.pushed).toBe(row.tier);
      expect(maintenanceOf(row.pushed, NOW_MS), `ms now ${row.pushed}`).toBe(row.tier);
      expect(maintenanceOf(row.pushed, new Date(NOW_MS)), `date now ${row.pushed}`).toBe(row.tier);
      expect(maintenanceOf(new Date(pushedMs).toISOString(), NOW_MS), `ms pushed ${row.pushed}`).toBe(row.tier);
    }

    for (const pushed of [null, undefined, '', '   ', '\n\t', 'not-a-date']) {
      expect(maintenanceOf(pushed, NOW), String(pushed)).toBeNull();
      expect(maintenanceOf(pushed, NOW_MS), `ms ${String(pushed)}`).toBeNull();
      expect(maintenanceOf(pushed, new Date(NOW_MS)), `date ${String(pushed)}`).toBeNull();
    }
  });

  it('已归档压过推送时间，false 或不传时边界表不变，两种悬停文案固定', () => {
    const yesterday = '2026-10-21T12:00:00Z';
    expect(maintenanceOf(null, NOW, true)).toBe('archived');
    expect(maintenanceOf('not-a-date', NOW, true)).toBe('archived');
    expect(maintenanceOf(yesterday, NOW, true)).toBe('archived');
    expect(maintenanceOf(yesterday, NOW_MS, true)).toBe('archived');
    expect(maintenanceOf(yesterday, new Date(NOW_MS), true)).toBe('archived');

    for (const row of BOUNDARY) {
      expect(maintenanceOf(row.pushed, NOW, false), `false ${row.pushed}`).toBe(row.tier);
      expect(maintenanceOf(row.pushed, NOW), `omit ${row.pushed}`).toBe(row.tier);
      expect(maintenanceOf(row.pushed, NOW_MS, false), `ms false ${row.pushed}`).toBe(row.tier);
      expect(maintenanceOf(row.pushed, new Date(NOW_MS)), `date omit ${row.pushed}`).toBe(row.tier);
    }
    for (const pushed of [null, undefined, '', '   ', '\n\t', 'not-a-date']) {
      expect(maintenanceOf(pushed, NOW, false), `false ${String(pushed)}`).toBeNull();
      expect(maintenanceOf(pushed, NOW), `omit ${String(pushed)}`).toBeNull();
    }

    expect(MAINTENANCE_TIERS).toEqual(['active', 'slow', 'stale', 'archived']);
    expect(MAINTENANCE_TIERS.at(-1)).toBe('archived');
    expect(MAINTENANCE_LABELS.archived).toBe('已归档');
    expect(MAINTENANCE_HINTS.archived).toBe('作者已在 GitHub 上把它归档：只读，不会再更新');
    expect(MAINTENANCE_FILTER_HINTS).toEqual({
      archived: '作者已在 GitHub 上把它归档：只读，不会再更新。旧数据点一次「同步」后才会标出来',
    });
  });
});
