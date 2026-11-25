import { describe, expect, it, vi } from 'vitest';
import type { ReactElement, ReactNode } from 'react';
import GITHUB_SOURCE from '@/pages/settings/github-section.tsx?raw';
import AI_SOURCE from '@/pages/settings/ai-section.tsx?raw';
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
  it('普通入口默认账号与同步；AI 与自动整理入口直接选中对应页签', () => {
    expect(selectedTab(SettingsPage())).toBe('账号与同步');
    expect(selectedTab(SettingsPage({ initialTab: 'ai' }))).toBe('AI 设置');
    expect(selectedTab(SettingsPage({ initialTab: 'organize' }))).toBe('自动整理');
  });

  it('App 将 AI 和自动整理入口接到单层设置页的对应页签', () => {
    const aiHandler = APP_SOURCE.slice(APP_SOURCE.indexOf('function handleOpenAiSettings()'), APP_SOURCE.indexOf('function handleOpenOrganize()'));
    expect(aiHandler).toContain("setSettingsTab('ai')");
    expect(aiHandler).toContain("setCurrentPage('settings')");
    const orgHandler = APP_SOURCE.slice(APP_SOURCE.indexOf('function handleOpenOrganize()'), APP_SOURCE.indexOf('const failedTaskRetry'));
    expect(orgHandler).toContain("setSettingsTab('organize')");
    expect(orgHandler).toContain("setCurrentPage('settings')");
    expect(APP_SOURCE).toContain('<SettingsPage initialTab={settingsTab} />');
    expect(APP_SOURCE.match(/onOpenAiSettings={handleOpenAiSettings}/g)).toHaveLength(3);
    expect(APP_SOURCE.match(/onOpenToolbox={handleOpenOrganize}/g)).toHaveLength(4);
  });
});

describe('设置页运行检查源码', () => {
  it('不再读取已停止检查的运行结果字段，也不留旧版「下载项目说明并整理（会花钱）」', () => {
    expect(GITHUB_SOURCE).not.toContain('RUNTIME_CHECK_TITLES.tagNetwork');
    expect(GITHUB_SOURCE).not.toContain('props.runtimeCheck.tagNetwork');
    expect(GITHUB_SOURCE).not.toContain('result.tagNetwork');
    expect(GITHUB_SOURCE).not.toContain('result.recommendation');
    expect(GITHUB_SOURCE).not.toContain('runtimeCheck.recommendation');
    expect(GITHUB_SOURCE).not.toContain('已停止检查');
    expect(GITHUB_SOURCE).not.toContain('下载项目说明并整理（会花钱）');
  });

  it('保留六项运行检查行，且重试间隔显示为秒而不是毫秒', () => {
    for (const key of ['storage', 'settings', 'github', 'stars', 'readme', 'ai']) {
      expect(GITHUB_SOURCE).toContain(`RUNTIME_CHECK_TITLES.${key}`);
    }
    expect(AI_SOURCE).toContain('tagNetworkBatchSize');
    expect(AI_SOURCE).toContain('tagNetworkPromptVersion');
    expect(AI_SOURCE).not.toContain('retryBaseDelayMs}ms');
  });
});
