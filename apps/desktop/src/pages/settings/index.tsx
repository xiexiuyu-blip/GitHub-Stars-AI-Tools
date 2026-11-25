import { useEffect, useState } from 'react';
import { useWorkspace } from '@/providers/workspace-provider';
import { useAppSettings } from '@/providers/settings-provider';
import { useAppUpdate } from '@/providers/app-update-provider';
import { Icon } from '@/components/ui/icon';
import { GitHubSettings } from './github-section';
import { AISettings } from './ai-section';
import { OrganizeSection } from './organize-section';
import { BackupSection } from './backup-section';
import { ConnectSection } from './connect-section';
import { GeneralAboutSection } from './general-about-section';

export type SettingsTab = 'github' | 'ai' | 'organize' | 'backup' | 'connect' | 'general';

const TABS: Array<{ key: SettingsTab; icon: string; label: string }> = [
  { key: 'github', icon: 'code', label: '账号与同步' },
  { key: 'ai', icon: 'smart_toy', label: 'AI 设置' },
  { key: 'organize', icon: 'auto_awesome', label: '自动整理' },
  { key: 'backup', icon: 'backup', label: '备份与导出' },
  { key: 'connect', icon: 'hub', label: '外部连接' },
  { key: 'general', icon: 'tune', label: '外观与关于' },
];

export function SettingsPage({ initialTab = 'github' }: { initialTab?: SettingsTab } = {}) {
  const workspace = useWorkspace();
  const settingsHook = useAppSettings();
  const appUpdate = useAppUpdate();
  const [activeTab, setActiveTab] = useState<SettingsTab>(initialTab);
  useEffect(() => setActiveTab(initialTab), [initialTab]);

  const activeTabLabel = TABS.find((tab) => tab.key === activeTab)?.label ?? '设置';

  return (
    <div className="settings-page flex h-full min-h-0 flex-col overflow-y-auto p-4 sm:p-5 lg:p-margin-page">
      <header className="mx-auto mb-5 flex w-full max-w-6xl items-baseline justify-between gap-4">
        <div>
          <h2 className="font-headline-md text-headline-md text-on-surface">设置</h2>
          <p className="font-body-md mt-1 text-sm text-on-surface-variant">{activeTabLabel}</p>
        </div>
      </header>

      <div className="mx-auto grid w-full max-w-6xl min-w-0 flex-1 gap-6 lg:grid-cols-[200px_minmax(0,1fr)] lg:items-start">
        <nav
          className="grid grid-cols-3 gap-1 rounded-xl border border-card-border bg-surface-container-lowest p-1.5 sm:grid-cols-6 lg:sticky lg:top-0 lg:grid-cols-1"
          aria-label="设置分类"
        >
          {TABS.map((tab) => (
            <button
              key={tab.key}
              type="button"
              onClick={() => setActiveTab(tab.key)}
              title={tab.label}
              aria-label={tab.label}
              aria-current={activeTab === tab.key ? 'page' : undefined}
              className={`flex min-h-10 min-w-0 items-center justify-center gap-2.5 rounded-lg px-3 py-2 text-left transition-colors focus:ring-2 focus:ring-primary/35 focus:outline-none lg:justify-start ${
                activeTab === tab.key
                  ? 'bg-primary/10 text-primary'
                  : 'text-on-surface-variant hover:bg-surface-container-low hover:text-on-surface'
              }`}
            >
              <Icon name={tab.icon} size={18} className="shrink-0" />
              <span className="font-body-md truncate text-xs font-medium lg:text-sm">{tab.label}</span>
            </button>
          ))}
        </nav>

        <div className="min-w-0 flex-1 space-y-6">
          {settingsHook.settingsError && (
            <div className="font-body-md rounded-lg border border-error/20 bg-error/10 px-4 py-3 text-sm text-error">
              {settingsHook.settingsError}
            </div>
          )}
          {activeTab === 'github' && (
            <GitHubSettings
              workspace={workspace}
              settingsHook={settingsHook}
              onOpenAiSettings={() => setActiveTab('ai')}
            />
          )}
          {activeTab === 'ai' && (
            <AISettings
              settingsHook={settingsHook}
              onOpenOrganize={() => setActiveTab('organize')}
            />
          )}
          {activeTab === 'organize' && (
            <OrganizeSection onOpenAiSettings={() => setActiveTab('ai')} />
          )}
          {activeTab === 'backup' && <BackupSection workspace={workspace} />}
          {activeTab === 'connect' && <ConnectSection />}
          {activeTab === 'general' && (
            <GeneralAboutSection
              settingsHook={settingsHook}
              workspace={workspace}
              appUpdate={appUpdate}
              onOpenBackup={() => setActiveTab('backup')}
            />
          )}
        </div>
      </div>
    </div>
  );
}
