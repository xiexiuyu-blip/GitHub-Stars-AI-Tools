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

/** 主导航只有这 5 项，顺序固定。「按类型看」（原来叫地图）、参考包、工具箱不在这里。 */
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

/** 二级切换。key 是导航高亮，值是这一组里的页面。 */
export const SUB_TABS: Record<'library' | 'solve' | 'settings', SubTab[]> = {
  library: [
    { key: 'library', label: '资料库' },
    { key: 'map', label: '按类型看' },
  ],
  // 参考包已经在「找方案」页右边了；旧的单独「参考包」页不再放进标签条（只有一项就不显示标签条）
  solve: [{ key: 'solve', label: '找方案' }],
  settings: [
    { key: 'settings', label: '设置' },
    { key: 'toolbox', label: '工具箱' },
  ],
};

const PAGE_NAV: Record<AppPage, NavKey | null> = {
  today: 'today',
  library: 'library',
  map: 'library',
  solve: 'solve',
  packs: 'solve',
  capture: 'capture',
  discover: 'discover',
  settings: 'settings',
  toolbox: 'settings',
  dashboard: null,
  repositories: null,
  rankings: null,
  'tag-network': null,
  'ai-search': null,
  profile: null,
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
