import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { SubTabs } from '@/components/sub-tabs';
import type { AppPage } from '@/types';
// 源码原文：守住「页签名只在 nav.ts 里定义」（Vite 的 ?raw，不用 node 的类型）
import appSource from '../App.tsx?raw';
import { NAV_ITEMS, SETTINGS_ITEM, SUB_TABS, navKeyFor, subTabsFor } from './nav';

const ALL_PAGES: AppPage[] = [
  'today',
  'library',
  'map',
  'solve',
  'packs',
  'capture',
  'discover',
  'settings',
  'toolbox',
  'dashboard',
  'repositories',
  'rankings',
  'tag-network',
  'ai-search',
  'profile',
];

/** 渲染出来的页签文字（一个 <button> 一个页签）。 */
function renderedTabTexts(page: AppPage): string[] {
  const tabs = subTabsFor(page);
  if (!tabs) return [];
  const html = renderToStaticMarkup(createElement(SubTabs, { tabs, active: page, onSelect: () => {} }));
  return [...html.matchAll(/<button[^>]*>(.*?)<\/button>/g)].map((match) => match[1].replace(/<[^>]+>/g, '').trim());
}

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
  it('资料库右边的子页签叫「按类型看」，页面 key 仍是 map，用户看得见的字里没有「地图」', () => {
    expect(SUB_TABS.library).toEqual([
      { key: 'library', label: '资料库' },
      { key: 'map', label: '按类型看' },
    ]);
    expect(subTabsFor('map')?.map((tab) => tab.label)).toEqual(['资料库', '按类型看']);
    expect(navKeyFor('map')).toBe('library');
    const allLabels = [
      ...NAV_ITEMS.map((item) => item.label),
      ...Object.values(SUB_TABS).flatMap((tabs) => tabs.map((tab) => tab.label)),
    ];
    expect(allLabels.filter((label) => label.includes('地图'))).toEqual([]);
  });

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

  it('设置旁边的子页签叫「自动整理与备份」，页面 key 仍是 toolbox', () => {
    expect(SUB_TABS.settings).toEqual([
      { key: 'settings', label: '设置' },
      { key: 'toolbox', label: '自动整理与备份' },
    ]);
    expect(subTabsFor('toolbox')?.map((tab) => tab.label)).toEqual(['设置', '自动整理与备份']);
    expect(subTabsFor('settings')?.map((tab) => tab.key)).toEqual(['settings', 'toolbox']);
    expect(navKeyFor('toolbox')).toBe('settings');
  });
});

describe('渲染出来的页签文字', () => {
  it('资料库那一组是「资料库」「按类型看」，设置那一组是「设置」「自动整理与备份」', () => {
    expect(renderedTabTexts('library')).toEqual(['资料库', '按类型看']);
    expect(renderedTabTexts('map')).toEqual(['资料库', '按类型看']);
    expect(renderedTabTexts('settings')).toEqual(['设置', '自动整理与备份']);
    expect(renderedTabTexts('toolbox')).toEqual(['设置', '自动整理与备份']);
  });

  it('所有页面渲染出来的页签文字里，都没有「工具箱」「地图」（左边主导航的文字也一样）', () => {
    const texts = [
      ...ALL_PAGES.flatMap((page) => renderedTabTexts(page)),
      ...NAV_ITEMS.map((item) => item.label),
      SETTINGS_ITEM.label,
    ];
    expect(texts.length).toBeGreaterThan(8);
    for (const text of texts) {
      expect(text, text).not.toContain('工具箱');
      expect(text, text).not.toContain('地图');
    }
  });

  it('App 里直接用 nav.ts 的页签，不再自己覆盖页签名（名字只在 nav.ts 里定义一次）', () => {
    expect(appSource).toContain('const tabs = subTabsFor(currentPage);');
    expect(appSource).not.toContain("label: '自动整理与备份'");
    expect(appSource).not.toContain("label: '按类型看'");
    expect(appSource).not.toMatch(/tab\.key === '(toolbox|map)'/);
  });
});
