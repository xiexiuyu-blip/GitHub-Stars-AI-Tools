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
  'solve',
  'capture',
  'discover',
  'settings',
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

  it('navKeyFor 对 6 个现行页面返回正确高亮', () => {
    const cases: [AppPage, string | null][] = [
      ['today', 'today'],
      ['library', 'library'],
      ['solve', 'solve'],
      ['capture', 'capture'],
      ['discover', 'discover'],
      ['settings', 'settings'],
    ];

    expect(cases).toHaveLength(ALL_PAGES.length);
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
    expect(keys).not.toContain('toolbox');
    const labels = NAV_ITEMS.map((item) => item.label);
    expect(labels).not.toContain('地图');
    expect(labels).not.toContain('参考包');
    expect(labels).not.toContain('工具箱');
  });
});

describe('subTabsFor', () => {
  it('所有页面都收成单层导航，不显示二级标签条', () => {
    expect(SUB_TABS.library).toEqual([{ key: 'library', label: '资料库' }]);
    expect(SUB_TABS.settings).toEqual([{ key: 'settings', label: '设置' }]);
    for (const page of ALL_PAGES) {
      expect(subTabsFor(page), page).toBeNull();
    }
  });
});

describe('渲染出来的页签文字', () => {
  it('没有二级页签渲染', () => {
    expect(renderedTabTexts('library')).toEqual([]);
    expect(renderedTabTexts('settings')).toEqual([]);
  });

  it('主导航文字里没有「工具箱」「地图」', () => {
    const texts = [
      ...ALL_PAGES.flatMap((page) => renderedTabTexts(page)),
      ...NAV_ITEMS.map((item) => item.label),
      SETTINGS_ITEM.label,
    ];
    expect(texts.length).toBeGreaterThan(5);
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
