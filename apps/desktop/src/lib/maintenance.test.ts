import { describe, expect, it } from 'vitest';
import {
  MAINTENANCE_ACTIVE_DAYS,
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
    expect(MAINTENANCE_TIERS).toEqual(['active', 'slow', 'stale']);
    expect(MAINTENANCE_LABELS).toEqual({ active: '活跃', slow: '放缓', stale: '疑似停更' });
    expect(MAINTENANCE_HINTS).toEqual({
      active: '最近 3 个月内有更新',
      slow: '3 到 12 个月没有更新',
      stale: '超过 1 年没有更新，可能已经不维护了',
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
});
