import { describe, expect, it, vi } from 'vitest';
import type { ReactElement, ReactNode } from 'react';
import SOURCE from '@/pages/settings.tsx?raw';
import APP_SOURCE from '@/App.tsx?raw';

vi.mock('@/providers/workspace-provider', () => ({ useWorkspace: () => ({}) }));
vi.mock('@/providers/settings-provider', () => ({ useAppSettings: () => ({ settingsError: null }) }));
vi.mock('@/providers/app-update-provider', () => ({ useAppUpdate: () => ({}) }));
vi.mock('react', async () => ({
  ...await vi.importActual('react'),
  useState: (initial: unknown) => [initial, vi.fn()],
  useEffect: vi.fn(),
}));
import { SettingsPage } from './settings';

function selectedTab(node: ReactNode): string | undefined {
  if (Array.isArray(node)) return node.map(selectedTab).find(Boolean);
  if (!node || typeof node !== 'object' || !('props' in node)) return undefined;
  const props = (node as ReactElement<Record<string, unknown>>).props;
  if (props['aria-current'] === 'page') return props['aria-label'] as string;
  return selectedTab(props.children as ReactNode);
}

describe('设置入口', () => {
  it('普通入口默认 GitHub；AI 入口首次就选中 AI 配置，不需要再找页签', () => {
    expect(selectedTab(SettingsPage())).toBe('GitHub');
    expect(selectedTab(SettingsPage({ initialTab: 'ai' }))).toBe('AI 设置');
  });

  it('App 将 AI 入口接到 settings 和 AI 页签；自动整理仍用独立入口', () => {
    const handler = APP_SOURCE.slice(APP_SOURCE.indexOf('function handleOpenAiSettings()'), APP_SOURCE.indexOf('function handleLegacyRepositorySearch'));
    expect(handler).toContain("setSettingsTab('ai')");
    expect(handler).toContain("setCurrentPage('settings')");
    expect(APP_SOURCE).toContain('<SettingsPage initialTab={settingsTab} />');
    expect(APP_SOURCE.match(/onOpenAiSettings={handleOpenAiSettings}/g)).toHaveLength(3);
    expect(APP_SOURCE).toContain("onOpenToolbox={() => setCurrentPage('toolbox')}");
  });
});

describe('设置页运行检查源码', () => {
  it('不再读取已停止检查的运行结果字段', () => {
    expect(SOURCE).not.toContain('RUNTIME_CHECK_TITLES.tagNetwork');
    expect(SOURCE).not.toContain('props.runtimeCheck.tagNetwork');
    expect(SOURCE).not.toContain('result.tagNetwork');
    expect(SOURCE).not.toContain('result.recommendation');
    expect(SOURCE).not.toContain('runtimeCheck.recommendation');
    expect(SOURCE).not.toContain('已停止检查');
  });

  it('保留六项运行检查行，标签网络设置项仍可保留', () => {
    for (const key of ['storage', 'settings', 'github', 'stars', 'readme', 'ai']) {
      expect(SOURCE).toContain(`RUNTIME_CHECK_TITLES.${key}`);
    }
    expect(SOURCE).toContain('tagNetworkBatchSize');
    expect(SOURCE).toContain('tagNetworkPromptVersion');
  });
});
