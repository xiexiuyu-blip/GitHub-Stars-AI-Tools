import { describe, expect, it } from 'vitest';
import type { AppPage } from '@/types';
import { NAV_ITEMS, SETTINGS_ITEM, SUB_TABS, navKeyFor, subTabsFor } from './nav';

describe('导航数据', () => {
  it('NAV_ITEMS 恰好 5 项，顺序和标签固定', () => {
    expect(NAV_ITEMS).toHaveLength(5);
    expect(NAV_ITEMS.map((item) => item.key)).toEqual([
      'today',
      'library',
      'solve',
      'capture',
      'discover',
    ]);
    expect(NAV_ITEMS.map((item) => item.label)).toEqual([
      '今日',
      '资料库',
      '找方案',
      '收集',
      '发现',
    ]);
    expect(NAV_ITEMS.every((item) => item.icon.length > 0)).toBe(true);
    expect(SETTINGS_ITEM).toEqual({ key: 'settings', label: '设置', icon: 'settings' });
  });

  it('navKeyFor 对 15 个页面返回正确高亮', () => {
    const cases: [AppPage, string | null][] = [
      ['today', 'today'],
      ['library', 'library'],
      ['map', 'library'],
      ['solve', 'solve'],
      ['packs', 'solve'],
      ['capture', 'capture'],
      ['discover', 'discover'],
      ['settings', 'settings'],
      ['toolbox', 'settings'],
      ['dashboard', null],
      ['repositories', null],
      ['rankings', null],
      ['tag-network', null],
      ['ai-search', null],
      ['profile', null],
    ];

    expect(cases.length).toBeGreaterThanOrEqual(12);
    for (const [page, key] of cases) {
      expect(navKeyFor(page), page).toBe(key);
    }
  });

  it('SUB_TABS 里每个页面的 navKeyFor 等于它所在的组', () => {
    const groups = Object.keys(SUB_TABS) as (keyof typeof SUB_TABS)[];
    expect(groups).toEqual(['library', 'solve', 'settings']);
    for (const group of groups) {
      expect(SUB_TABS[group].length).toBeGreaterThan(0);
      for (const tab of SUB_TABS[group]) {
        expect(navKeyFor(tab.key), `${group}:${tab.key}`).toBe(group);
      }
    }
  });

  it('NAV_ITEMS 里没有地图、参考包、工具箱', () => {
    const keys = NAV_ITEMS.map((item) => item.key);
    expect(keys).not.toContain('map');
    expect(keys).not.toContain('packs');
    expect(keys).not.toContain('toolbox');
    const labels = NAV_ITEMS.map((item) => item.label);
    expect(labels).not.toContain('地图');
    expect(labels).not.toContain('参考包');
    expect(labels).not.toContain('工具箱');
  });
});

describe('subTabsFor', () => {
  it('资料库和设置有两个标签，找方案只有一个就不显示标签条', () => {
    expect(subTabsFor('library')?.map((tab) => tab.key)).toEqual(['library', 'map']);
    expect(subTabsFor('settings')?.map((tab) => tab.key)).toEqual(['settings', 'toolbox']);
    expect(subTabsFor('solve')).toBeNull();
    expect(subTabsFor('packs')).toBeNull();
  });

  it('不在这三组里的页面没有标签条', () => {
    expect(subTabsFor('today')).toBeNull();
    expect(subTabsFor('capture')).toBeNull();
    expect(subTabsFor('discover')).toBeNull();
  });
});
