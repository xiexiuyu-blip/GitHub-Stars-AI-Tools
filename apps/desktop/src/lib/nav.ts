import type { AppPage } from '@/types';

export type NavKey = 'today' | 'library' | 'solve' | 'capture' | 'discover' | 'settings';

export type NavItem = {
  key: NavKey;
  label: string;
  icon: string;
};

export type SubTab = {
  key: AppPage;
  label: string;
};

/** 主导航只有这 5 项，顺序固定。「按类型看」、参考包不在这里；设置已收成单层分类。 */
export const NAV_ITEMS: NavItem[] = [
  { key: 'today', label: '今日', icon: 'home' },
  { key: 'library', label: '资料库', icon: 'folder_special' },
  { key: 'solve', label: '找方案', icon: 'psychology' },
  { key: 'capture', label: '收集', icon: 'add_link' },
  { key: 'discover', label: '发现', icon: 'explore' },
];

export const SETTINGS_ITEM: NavItem = {
  key: 'settings',
  label: '设置',
  icon: 'settings',
};

/** 二级切换。现在全应用都收成单层导航，只留一项时不显示二级标签条。 */
export const SUB_TABS: Record<'library' | 'solve' | 'settings', SubTab[]> = {
  library: [{ key: 'library', label: '资料库' }],
  solve: [{ key: 'solve', label: '找方案' }],
  settings: [{ key: 'settings', label: '设置' }],
};

const PAGE_NAV: Record<AppPage, NavKey | null> = {
  today: 'today',
  library: 'library',
  solve: 'solve',
  capture: 'capture',
  discover: 'discover',
  settings: 'settings',
};

export function navKeyFor(page: AppPage): NavKey | null {
  return PAGE_NAV[page];
}

export function subTabsFor(page: AppPage): SubTab[] | null {
  const group = navKeyFor(page);
  if (group === 'library' || group === 'solve' || group === 'settings') {
    return SUB_TABS[group].length > 1 ? SUB_TABS[group] : null;
  }
  return null;
}
