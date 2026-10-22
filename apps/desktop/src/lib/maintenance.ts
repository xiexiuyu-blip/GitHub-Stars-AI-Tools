import type { MaintenanceTier } from '@/lib/contracts';

/** 与 `library_view.rs` 的同名常量一致。改一边，另一边的边界测试会失败。 */
export const MAINTENANCE_ACTIVE_DAYS = 90;
export const MAINTENANCE_STALE_DAYS = 365;

const DAY_MS = 86_400_000;

/** 左栏和标签的固定顺序：活跃、放缓、疑似停更、已归档。 */
export const MAINTENANCE_TIERS: readonly MaintenanceTier[] = ['active', 'slow', 'stale', 'archived'];

export const MAINTENANCE_LABELS: Record<MaintenanceTier, string> = {
  active: '活跃',
  slow: '放缓',
  stale: '疑似停更',
  archived: '已归档',
};

export const MAINTENANCE_HINTS: Record<MaintenanceTier, string> = {
  active: '最近 3 个月内有更新',
  slow: '3 到 12 个月没有更新',
  stale: '超过 1 年没有更新，可能已经不维护了',
  archived: '作者已在 GitHub 上把它归档：只读，不会再更新',
};

/** 只覆盖左栏筛选项要改写的悬停。没有的档用 `MAINTENANCE_HINTS`。 */
export const MAINTENANCE_FILTER_HINTS: Partial<Record<MaintenanceTier, string>> = {
  archived: '作者已在 GitHub 上把它归档：只读，不会再更新。旧数据点一次「同步」后才会标出来',
};

/**
 * 已归档（第三个参数为 true）直接返回 `archived`，不看推送时间。
 * 否则距 `now` 多久：≤ 90 天活跃，> 90 天且 ≤ 365 天放缓，更久疑似停更。
 * 空、空白、解析不了 → null。比现在还晚（时钟误差）→ 活跃。用毫秒差，不用小数天。
 */
export function maintenanceOf(
  pushedAt: string | null | undefined,
  now: Date | number | string,
  archived?: boolean,
): MaintenanceTier | null {
  if (archived === true) return 'archived';
  const pushed = parseInstant(pushedAt);
  const current = parseInstant(now);
  if (pushed == null || current == null) return null;
  const age = current - pushed;
  if (age <= MAINTENANCE_ACTIVE_DAYS * DAY_MS) return 'active';
  if (age <= MAINTENANCE_STALE_DAYS * DAY_MS) return 'slow';
  return 'stale';
}

function parseInstant(value: Date | number | string | null | undefined): number | null {
  if (value == null) return null;
  if (typeof value === 'number') return Number.isFinite(value) ? value : null;
  if (value instanceof Date) {
    const ms = value.getTime();
    return Number.isFinite(ms) ? ms : null;
  }
  const text = value.trim();
  if (!text) return null;
  const ms = Date.parse(text);
  return Number.isFinite(ms) ? ms : null;
}
