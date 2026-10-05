import { useEffect, useRef, useState, type FormEvent } from 'react';
import { createPortal } from 'react-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useWorkspace } from '@/providers/workspace-provider';
import { useAppSettings } from '@/providers/settings-provider';
import { useAppUpdate, type AppUpdateContextValue } from '@/providers/app-update-provider';
import { AppUpdatePanel } from '@/components/app-update-panel';
import { Icon } from '@/components/ui/icon';
import {
  AI_AUTO_ORGANIZE_POINTER,
  AI_PROVIDER_PRESETS,
  AI_TEST_COST_NOTE,
  aiBaseUrlHelp,
  aiKeyHelpText,
  aiKeySaveStatusText,
  describeCurrentAiService,
  getAiConfigMessage,
  getEmbeddingConfigMessage,
  isSavedAiApiKeyPlaceholder,
  shouldFlushAiApiKey,
  shouldFlushEmbeddingApiKey,
  toBackendAiRequestConfig,
  toBackendEmbeddingRequestConfig,
  type AIProviderPreset,
  type AIProviderPresetOption,
} from '@/lib/ai-config';
import {
  autoSyncDescription,
  buildReadinessRows,
  CLEAR_LOCAL_DATA_DONE,
  CLEAR_LOCAL_DATA_FAILED,
  CLEAR_LOCAL_DATA_WARNING,
  CONNECT_SAVING_KEYCHAIN,
  CONNECT_SLOW,
  CONNECT_VERIFYING,
  formatMinute,
  formatSyncComplete,
  GIST_BACKUP_LIBRARY,
  GIST_BACKUP_MARKS,
  GIST_IMPORT_WARNING,
  GIST_NEEDS_SCOPE,
  GIST_SECTION_TITLE,
  gistImportHint,
  GITHUB_TOKEN_EXPLAINER,
  GITHUB_TOKEN_SAVED_NOTE,
  GITHUB_TOKEN_SCOPES,
  RUNTIME_CHECK_COST_NOTE,
  RUNTIME_CHECK_TITLES,
  SYNC_INTERVAL_HINT,
  WELCOME_STARTUP_HINT,
} from '@/lib/automation-view';
import { compactNumber } from '@/lib/format';
import { COLOR_PRESETS } from '@/types-settings';
import type { AiStreamEvent, AppIdentity, DashboardStats, RuntimeReadinessCheckItem, RuntimeReadinessCheckResult } from '@/types';
import type {
  AISettings as AISettingsValue,
  EmbeddingSettings as EmbeddingSettingsValue,
  RuntimeSelfCheckRecord,
  ThemeSettings as ThemeSettingsValue,
} from '@/types-settings';

type SettingsTab = 'github' | 'ai' | 'embedding' | 'general' | 'backup' | 'about';

type AiRequestDiagnostics = {
  provider: string;
  wireProtocol: string;
  baseUrl: string | null;
  endpoint: string | null;
  model: string;
  apiKeyState: string;
  connectTimeoutSeconds: number;
  requestTimeoutSeconds: number;
  modelListTimeoutSeconds: number;
  retryAttempts: number;
  retryBaseDelayMs: number;
  readmePromptCharLimit: number;
  tagNetworkBatchSize: number;
  summaryPromptVersion: string;
  tagNetworkPromptVersion: string;
  status: 'ready' | 'invalid' | string;
  failureReason: string | null;
};

const FEATURED_AI_PROVIDER_IDS: AIProviderPreset[] = [
  'openai',
  'anthropic',
  'deepseek',
  'qwen',
  'ollama',
  'custom-openai-compatible',
];
const FEATURED_AI_PROVIDER_PRESETS = AI_PROVIDER_PRESETS.filter((preset) => FEATURED_AI_PROVIDER_IDS.includes(preset.id));
const MORE_AI_PROVIDER_PRESETS = AI_PROVIDER_PRESETS.filter(
  (preset) => preset.id !== 'none' && !FEATURED_AI_PROVIDER_IDS.includes(preset.id),
);
const DISABLED_AI_PROVIDER_PRESET = AI_PROVIDER_PRESETS.find((preset) => preset.id === 'none')!;
const PROJECT_REPOSITORY_URL = 'https://github.com/xiexiuyu-blip/GitHub-Stars-AI-Tools/tree/fox-product-lab';
const PROJECT_ISSUES_URL = 'https://github.com/xiexiuyu-blip/GitHub-Stars-AI-Tools/issues';
const PROJECT_LICENSE_URL = 'https://github.com/xiexiuyu-blip/GitHub-Stars-AI-Tools/blob/fox-product-lab/LICENSE';
const PROJECT_ACKNOWLEDGEMENTS_URL = 'https://github.com/xiexiuyu-blip/GitHub-Stars-AI-Tools/tree/fox-product-lab#%E8%87%B4%E8%B0%A2';

type AiConnectionTestResult = {
  summaryZh: string;
  model: string;
  promptVersion: string;
  inputTokens: number;
  outputTokens: number;
};

type AiModelOption = {
  id: string;
  displayName?: string | null;
  ownedBy?: string | null;
};

type AiTestDialogState = {
  isOpen: boolean;
  status: 'preparing' | 'running' | 'success' | 'error';
  stage: string;
  providerLabel: string;
  endpoint: string;
  model: string;
  startedAt: number;
  firstTokenLatencyMs: number | null;
  totalLatencyMs: number | null;
  outputTokens: number | null;
  tokensPerSecond: number | null;
  summary: string | null;
  error: string | null;
};

type EmbeddingConnectionTestResult = {
  model: string;
  dimensions: number;
};

type VectorIndexStatus = {
  enabled: boolean;
  model: string | null;
  dimensions: number | null;
  sqliteCount: number;
  zvecCount: number;
  ready: boolean;
  message: string;
};

type EmbeddingRuntimeStatus = {
  accountId: string;
  enabled: boolean;
  provider: string;
  state: 'disabled' | 'missing' | 'downloading' | 'verifying' | 'loading' | 'indexing' | 'ready' | 'partial' | 'error';
  modelReady: boolean;
  model: string;
  revision: string | null;
  profileId: string;
  dimensions: number;
  cacheBytes: number;
  indexedCount: number;
  totalCount: number;
  failedCount: number;
  message: string;
  canRetry: boolean;
};

type EmbeddingPanelMessage = {
  type: 'success' | 'error' | 'info';
  text: string;
};

const EMBEDDING_SETUP_STAGES = [
  { state: 'downloading', label: '下载模型' },
  { state: 'verifying', label: '校验文件' },
  { state: 'loading', label: '加载模型' },
  { state: 'indexing', label: '构建索引' },
] as const;

type VectorIndexBuildSummary = {
  totalCount: number;
  indexedCount: number;
  restoredCount: number;
  skippedCount: number;
  failedCount: number;
  failures: string[];
};

export function SettingsPage() {
  const workspace = useWorkspace();
  const settingsHook = useAppSettings();
  const appUpdate = useAppUpdate();
  const [activeTab, setActiveTab] = useState<SettingsTab>('github');

  // 页签：GitHub / AI 设置 / 通用 / GitHub 备份 / 关于（审读第 42 条）。
  // 「向量」页签先藏起来（审读第 15 条）：新界面里没有任何页面用到向量检索，点「下载并启用」却要下载约 490 MB。
  // 面板（EmbeddingSettingsPanel）和后台命令都留着；要恢复，在这里加回一项 key 为 'embedding' 的页签。
  const tabs: { key: SettingsTab; icon: string; label: string; shortLabel: string }[] = [
    { key: 'github', icon: 'code', label: 'GitHub', shortLabel: 'GitHub' },
    { key: 'ai', icon: 'smart_toy', label: 'AI 设置', shortLabel: 'AI 设置' },
    { key: 'general', icon: 'tune', label: '通用', shortLabel: '通用' },
    { key: 'backup', icon: 'backup', label: 'GitHub 备份', shortLabel: 'GitHub 备份' },
    { key: 'about', icon: 'info', label: '关于', shortLabel: '关于' },
  ];
  const activeTabLabel = tabs.find((tab) => tab.key === activeTab)?.label ?? '设置';

  return (
    <div className="settings-page flex h-full min-h-0 flex-col overflow-y-auto p-4 sm:p-5 lg:p-margin-page">
      <header className="mx-auto mb-6 flex w-full max-w-5xl flex-col items-start gap-4">
        <div>
          <h2 className="font-headline-md text-headline-md text-on-surface">设置</h2>
          <p className="mt-1 font-body-md text-sm text-on-surface-variant">{activeTabLabel}</p>
        </div>
        <nav className="grid w-full grid-cols-3 gap-1 rounded-lg border border-card-border bg-surface-container-lowest p-1 sm:grid-cols-5" aria-label="设置分类">
          {tabs.map((tab) => (
            <button
              key={tab.key}
              type="button"
              onClick={() => setActiveTab(tab.key)}
              title={tab.label}
              aria-label={tab.label}
              aria-current={activeTab === tab.key ? 'page' : undefined}
              className={`flex min-h-11 min-w-0 items-center justify-center gap-2 rounded-md px-2 py-2 text-center transition-colors focus:outline-none focus:ring-2 focus:ring-primary/35 focus:ring-offset-1 ${
                activeTab === tab.key
                  ? 'bg-primary/10 text-primary'
                  : 'text-on-surface-variant hover:bg-surface-container-low hover:text-on-surface'
              }`}
            >
              <Icon name={tab.icon} size={19} />
              <span className="truncate font-body-md text-xs font-medium">{tab.shortLabel}</span>
            </button>
          ))}
        </nav>
      </header>

      {/* 设置内容 */}
      <div className="mx-auto w-full max-w-5xl min-w-0 flex-1 space-y-6">
        {settingsHook.settingsError && (
          <div className="rounded-lg border border-error/20 bg-error/10 px-4 py-3 font-body-md text-sm text-error">
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
        {activeTab === 'ai' && <AISettings settingsHook={settingsHook} />}
        {activeTab === 'embedding' && (
          <EmbeddingSettingsPanel
            accountId={workspace.authState.user ? String(workspace.authState.user.id) : null}
            settingsHook={settingsHook}
          />
        )}
        {activeTab === 'general' && <GeneralSettings settingsHook={settingsHook} workspace={workspace} />}
        {activeTab === 'backup' && <BackupSettings workspace={workspace} />}
        {activeTab === 'about' && <AboutSettings appUpdate={appUpdate} />}
      </div>
    </div>
  );
}

/* === GitHub 设置 === */
function GitHubSettings({
  workspace,
  settingsHook,
  onOpenAiSettings,
}: {
  workspace: ReturnType<typeof useWorkspace>;
  settingsHook: ReturnType<typeof useAppSettings>;
  onOpenAiSettings: () => void;
}) {
  const isConnected = workspace.authState.hasToken && Boolean(workspace.authState.user);
  const user = workspace.authState.user;
  const [connectStatus, setConnectStatus] = useState<string | null>(null);
  const [readinessStats, setReadinessStats] = useState<DashboardStats | null>(null);
  const [isLoadingReadiness, setIsLoadingReadiness] = useState(false);
  const [readinessError, setReadinessError] = useState<string | null>(null);
  const [readinessWarning, setReadinessWarning] = useState<string | null>(null);
  const [runtimeCheck, setRuntimeCheck] = useState<RuntimeReadinessCheckResult | null>(null);
  const [runtimeCheckCompletedAt, setRuntimeCheckCompletedAt] = useState<string | null>(null);
  const [isCheckingRuntime, setIsCheckingRuntime] = useState(false);
  const autoSummaryConfigMessage = settingsHook.settings.ai.enableAutoSummary
    ? getAiConfigMessage(settingsHook.settings.ai)
    : null;

  useEffect(() => {
    let cancelled = false;
    const accountId = user ? String(user.id) : null;
    if (!accountId) {
      setReadinessStats(null);
      setIsLoadingReadiness(false);
      setReadinessError(null);
      setReadinessWarning(null);
      return;
    }

    setIsLoadingReadiness(true);
    setReadinessError(null);
    invoke<DashboardStats>('get_dashboard_stats', { request: { accountId } })
      .then((stats) => {
        if (!cancelled) {
          setReadinessStats(stats);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setReadinessStats(null);
          setReadinessError(toErrorMessage(error));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoadingReadiness(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [
    user?.id,
    workspace.repositoryPage?.totalCount,
    workspace.syncSummary,
    workspace.readmeSummary,
    workspace.batchAiSummary,
  ]);

  useEffect(() => {
    setRuntimeCheck(null);
    setRuntimeCheckCompletedAt(null);
  }, [
    user?.id,
    settingsHook.settings.ai.provider,
    settingsHook.settings.ai.baseUrl,
    settingsHook.settings.ai.model,
    settingsHook.settings.ai.apiKey,
    workspace.repositoryPage?.totalCount,
  ]);

  async function handleGitHubConnectSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (workspace.isSavingToken || !workspace.token.trim()) {
      return;
    }

    const statusTimers: number[] = [];
    setConnectStatus(CONNECT_VERIFYING);
    try {
      statusTimers.push(window.setTimeout(() => {
        setConnectStatus(CONNECT_SAVING_KEYCHAIN);
      }, 1200));
      statusTimers.push(window.setTimeout(() => {
        setConnectStatus(`${CONNECT_SLOW}。`);
      }, 8000));
      await workspace.connectWithToken(workspace.token);
      setConnectStatus(null);
    } catch {
      setConnectStatus(null);
    } finally {
      statusTimers.forEach(window.clearTimeout);
    }
  }

  async function handleRuntimeReadinessCheck() {
    setIsCheckingRuntime(true);
    setReadinessError(null);
    setReadinessWarning(null);
    try {
      if (shouldFlushAiApiKey(settingsHook.settings.ai)) {
        await settingsHook.flushAIKey(settingsHook.settings.ai.apiKey);
      }
      const result = await invoke<RuntimeReadinessCheckResult>('check_runtime_readiness', {
        request: {
          aiConfig: toBackendAiRequestConfig(settingsHook.settings.ai),
        },
      });
      setRuntimeCheck(result);
      const record = buildRuntimeSelfCheckRecord(result);
      setRuntimeCheckCompletedAt(record.completedAt);
      try {
        await settingsHook.updateRuntime({ lastSelfCheckRecord: record });
      } catch {
        setReadinessWarning('检查已经做完了，但这次的结果没能记下来。你的资料都还在。');
      }
    } catch (error) {
      setReadinessError(toErrorMessage(error));
    } finally {
      setIsCheckingRuntime(false);
    }
  }

  return (
    <>
      {/* 连接状态卡片 */}
      <section className="glass-panel rounded-xl p-6 relative overflow-hidden">
        <div className="absolute top-0 right-0 p-6 pointer-events-none opacity-10">
          <Icon name="cloud_sync" size={96} />
        </div>
        <div className="relative z-10 flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="min-w-0">
            <h3 className="font-headline-md text-[20px] font-semibold text-on-surface mb-1">GitHub 连接状态</h3>
            <p className="font-body-md text-on-surface-variant mb-6">
              {GITHUB_TOKEN_EXPLAINER}{GITHUB_TOKEN_SCOPES}
            </p>
            <div
              className={`flex w-fit max-w-full flex-wrap items-center gap-3 rounded-lg border px-4 py-2.5 ${
                isConnected ? 'bg-success/10 border-success/20' : 'bg-error/10 border-error/20'
              }`}
            >
              <Icon
                name={isConnected ? 'check_circle' : 'error'}
                size={20}
                className={`shrink-0 ${isConnected ? 'text-success' : 'text-error'}`}
              />
              <span className={`font-body-md font-medium ${isConnected ? 'text-success' : 'text-error'}`}>
                {isConnected ? '已连接（令牌有效）' : '未连接'}
              </span>
            </div>
          </div>
          {isConnected && user && (
            <div className="flex max-w-full shrink-0 items-center gap-3 rounded-lg border border-card-border bg-surface-container-low px-4 py-2">
              {user.avatarUrl ? (
                <img src={user.avatarUrl} alt={user.login} className="h-6 w-6 shrink-0 rounded-full" />
              ) : (
                <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary-container/20">
                  <Icon name="person" size={16} className="text-primary" />
                </div>
              )}
              <span className="min-w-0 truncate font-label-sm text-label-sm text-on-surface" title={`@${user.login}`}>
                @{user.login}
              </span>
            </div>
          )}
        </div>
      </section>

      <RuntimeReadinessPanel
        isConnected={isConnected}
        userLogin={user?.login ?? null}
        stats={readinessStats}
        isLoading={isLoadingReadiness}
        errorMessage={readinessError}
        warningMessage={readinessWarning}
        aiConfigMessage={getAiConfigMessage(settingsHook.settings.ai)}
        aiModel={settingsHook.settings.ai.model}
        isSyncingStars={workspace.isSyncingStars}
        isFetchingReadmes={workspace.isFetchingReadmes}
        isBatchGeneratingAiDocuments={workspace.isBatchGeneratingAiDocuments}
        runtimeCheck={runtimeCheck}
        runtimeCheckCompletedAt={runtimeCheckCompletedAt}
        lastSelfCheckRecord={settingsHook.settings.runtime.lastSelfCheckRecord}
        isCheckingRuntime={isCheckingRuntime}
        onSyncStars={() => void workspace.handleSyncStars()}
        onFetchReadmes={() => {
          void workspace.handleFetchReadmes({
            aiConfig: settingsHook.settings.ai,
            autoGenerateAi: false,
            onlyMissing: true,
          });
        }}
        onGenerateAi={() => {
          void (async () => {
            if (shouldFlushAiApiKey(settingsHook.settings.ai)) {
              await settingsHook.flushAIKey(settingsHook.settings.ai.apiKey);
            }
            await workspace.handleFetchReadmes({
              aiConfig: settingsHook.settings.ai,
              autoGenerateAi: true,
              onlyMissing: true,
            });
          })().catch(() => undefined);
        }}
        onOpenAiSettings={onOpenAiSettings}
        onCheckRuntime={() => void handleRuntimeReadinessCheck()}
      />

      {/* 配置表单 */}
      <section className="glass-panel rounded-xl p-0 overflow-hidden">
        <div className="p-6 border-b border-card-border bg-surface/50">
          <h3 className="font-headline-md text-[20px] font-semibold text-on-surface">访问配置</h3>
        </div>
        <div className="p-6 space-y-6">
          {/* 令牌输入 */}
          {isConnected ? (
            <div className="space-y-2">
              <label className="font-body-md text-on-surface font-medium flex items-center gap-2">
                GitHub 令牌
                <Icon name="info" size={16} className="text-on-surface-variant cursor-help" />
              </label>
              <div className="flex gap-3">
                <div className="flex-1 rounded-lg border border-outline-variant bg-surface-container-low px-4 py-2">
                  <p className="font-label-sm text-on-surface">{GITHUB_TOKEN_SAVED_NOTE}</p>
                </div>
                <button
                  onClick={() => void workspace.handleClearToken()}
                  disabled={workspace.isClearingToken}
                  className="px-4 py-2 bg-surface-container-high hover:bg-surface-container-highest text-on-surface rounded-lg border border-card-border font-body-md transition-all"
                >
                  断开连接
                </button>
              </div>
            </div>
          ) : (
            <form
              onSubmit={(event) => void handleGitHubConnectSubmit(event)}
              className="space-y-2"
            >
              <label className="font-body-md text-on-surface font-medium flex items-center gap-2">
                GitHub 令牌
                <Icon name="info" size={16} className="text-on-surface-variant cursor-help" />
              </label>
              <div className="flex gap-3">
                <input
                  type="password"
                  autoComplete="off"
                  autoCapitalize="none"
                  autoCorrect="off"
                  spellCheck={false}
                  inputMode="text"
                  value={workspace.token}
                  onChange={(e) => workspace.setToken(e.target.value)}
                  placeholder="ghp_xxxxxxxxxxxx"
                  className="flex-1 bg-surface-container-low border border-outline-variant rounded-lg px-4 py-2 font-label-sm text-on-surface focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                />
                <button
                  type="submit"
                  disabled={workspace.isSavingToken || !workspace.token.trim()}
                  className="px-4 py-2 bg-primary text-white rounded-lg font-body-md hover:brightness-110 transition-all shadow-sm disabled:opacity-60"
                >
                  {workspace.isSavingToken ? '连接中…' : '连接 GitHub'}
                </button>
              </div>
            </form>
          )}

          {workspace.authMessage && (
            <p className="font-body-md text-[13px] text-success flex items-center gap-1">
              <Icon name="check_circle" size={16} /> {workspace.authMessage}
            </p>
          )}
          {connectStatus && (
            <p className="font-body-md text-[13px] text-primary flex items-center gap-1 rounded-lg border border-primary/20 bg-primary/10 px-3 py-2">
              <Icon name="progress_activity" size={16} className="animate-spin" /> {connectStatus}
            </p>
          )}
          {workspace.error && (
            <p className="font-body-md text-[13px] text-error flex items-center gap-1 rounded-lg border border-error/20 bg-error/10 px-3 py-2">
              <Icon name="error" size={16} /> {workspace.error}
            </p>
          )}

          <hr className="border-card-border" />

          <div className="flex items-center justify-between py-4">
            <div className="flex-1 pr-6">
              <h4 className="font-body-lg font-medium text-on-surface flex items-center gap-2">
                项目说明（README）
                <span className="bg-primary-fixed text-on-primary-fixed text-[10px] px-2 py-0.5 rounded-full font-bold uppercase tracking-wider">
                  AI 要读的资料
                </span>
              </h4>
              <p className="font-body-md text-on-surface-variant mt-1">
                项目说明是 AI 写中文说明时要读的资料；只下载项目说明，不会直接生成中文说明。
              </p>
            </div>
          </div>
        </div>
      </section>

      {/* 同步操作 */}
      <section className="glass-panel rounded-lg p-6">
        <h3 className="font-headline-md text-[20px] font-semibold text-on-surface mb-4">同步</h3>
        <div className="flex flex-wrap gap-3">
          <button
            onClick={() => void workspace.handleSyncStars()}
            disabled={workspace.isSyncingStars || !isConnected}
            className="px-4 py-2 bg-primary text-white rounded-lg font-body-md flex items-center gap-2 hover:brightness-110 transition-all shadow-sm disabled:opacity-60"
          >
            <Icon name="sync" size={18} className={workspace.isSyncingStars ? 'animate-spin' : ''} />
            {workspace.isSyncingStars ? '同步中…' : '同步我的收藏'}
          </button>
          <button
            onClick={() => {
              void (async () => {
                if (settingsHook.settings.ai.enableAutoSummary && shouldFlushAiApiKey(settingsHook.settings.ai)) {
                  await settingsHook.flushAIKey(settingsHook.settings.ai.apiKey);
                }
                const readmeSummary = await workspace.handleFetchReadmes({
                  aiConfig: settingsHook.settings.ai,
                  autoGenerateAi: settingsHook.settings.ai.enableAutoSummary,
                  onlyMissing: true,
                });
                if (!readmeSummary) {
                  return;
                }
              })().catch(() => undefined);
            }}
            disabled={
              workspace.isFetchingReadmes ||
              workspace.isBatchGeneratingAiDocuments ||
              !isConnected ||
              Boolean(autoSummaryConfigMessage)
            }
            title={autoSummaryConfigMessage ?? undefined}
            className="px-4 py-2 bg-surface-container-high hover:bg-surface-container-highest text-on-surface rounded-lg border border-card-border font-body-md flex items-center gap-2 transition-all disabled:opacity-60"
          >
            <Icon name="description" size={18} className={workspace.isFetchingReadmes || workspace.isBatchGeneratingAiDocuments ? 'animate-spin' : ''} />
            {workspace.isFetchingReadmes
              ? '下载中…'
              : workspace.isBatchGeneratingAiDocuments
                ? 'AI 整理中…'
                : settingsHook.settings.ai.enableAutoSummary
                  ? '下载项目说明并整理（会花钱）'
                  : '下载项目说明'}
          </button>
        </div>
        {autoSummaryConfigMessage && (
          <div className="mt-4 rounded-lg border border-error/20 bg-error/10 px-3 py-2 font-body-md text-sm text-error">
            {autoSummaryConfigMessage}
          </div>
        )}
        {workspace.syncSummary && (
          <div className="mt-4 p-3 rounded-lg bg-success/10 border border-success/20">
            <p className="font-body-md text-sm text-success">
              {formatSyncComplete(workspace.syncSummary)}
            </p>
          </div>
        )}
      </section>
    </>
  );
}

function RuntimeReadinessPanel(props: {
  isConnected: boolean;
  userLogin: string | null;
  stats: DashboardStats | null;
  isLoading: boolean;
  errorMessage: string | null;
  warningMessage: string | null;
  aiConfigMessage: string | null;
  aiModel: string;
  isSyncingStars: boolean;
  isFetchingReadmes: boolean;
  isBatchGeneratingAiDocuments: boolean;
  runtimeCheck: RuntimeReadinessCheckResult | null;
  runtimeCheckCompletedAt: string | null;
  lastSelfCheckRecord: RuntimeSelfCheckRecord | null;
  isCheckingRuntime: boolean;
  onSyncStars: () => void;
  onFetchReadmes: () => void;
  onGenerateAi: () => void;
  onOpenAiSettings: () => void;
  onCheckRuntime: () => void;
}) {
  const runtimeCheckSummary = props.runtimeCheck ? summarizeRuntimeCheck(props.runtimeCheck) : null;
  const selfCheckRecord = runtimeCheckSummary
    ? {
        completedAt: props.runtimeCheckCompletedAt ?? '刚刚完成',
        ...runtimeCheckSummary,
      }
    : props.lastSelfCheckRecord;
  // 清单每一行读真实数据（资料库里有几个收藏、AI 设的是哪个模型），逻辑在 lib/automation-view.ts 的 buildReadinessRows
  const readinessRows = buildReadinessRows({
    connected: props.isConnected,
    login: props.userLogin,
    totalRepos: props.stats?.totalRepos ?? 0,
    totalReadmes: props.stats?.totalReadmes ?? 0,
    totalAiSummaries: props.stats?.totalAiSummaries ?? 0,
    aiConfigMessage: props.aiConfigMessage,
    aiModel: props.aiModel,
    syncing: props.isSyncingStars,
    fetchingReadmes: props.isFetchingReadmes,
    writingCards: props.isBatchGeneratingAiDocuments,
  });
  const rowActions = {
    sync: props.onSyncStars,
    readme: props.onFetchReadmes,
    writeCards: props.onGenerateAi,
    openAiSettings: props.onOpenAiSettings,
  } as const;

  return (
    <section className="glass-panel rounded-xl p-6">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h3 className="font-headline-md text-[20px] font-semibold text-on-surface">运行检查</h3>
          <p className="mt-1 font-body-md text-sm text-on-surface-variant">
            看看 GitHub 连接、同步、AI 是不是都正常。
          </p>
          <p className="mt-1 font-body-md text-xs text-on-surface-variant">{RUNTIME_CHECK_COST_NOTE}</p>
        </div>
        {props.isLoading && (
          <span className="inline-flex items-center gap-1.5 rounded-lg border border-primary/20 bg-primary/10 px-3 py-1.5 text-xs text-primary">
            <Icon name="progress_activity" size={14} className="animate-spin" />
            正在读取
          </span>
        )}
        <button
          type="button"
          onClick={props.onCheckRuntime}
          disabled={props.isCheckingRuntime}
          className="inline-flex items-center gap-1.5 rounded-lg border border-primary/25 bg-primary/10 px-3 py-1.5 font-body-md text-xs font-medium text-primary transition-colors hover:bg-primary/15 disabled:opacity-60"
        >
          <Icon name="verified" size={14} className={props.isCheckingRuntime ? 'animate-spin' : ''} />
          {props.isCheckingRuntime ? '正在检查…' : '开始检查'}
        </button>
      </div>
      {props.errorMessage && (
        <p className="mt-4 rounded-lg border border-error/20 bg-error/10 px-3 py-2 font-body-md text-sm text-error">
          没能读出现在的状态。你的资料都还在。再点一次「开始检查」；还不行就重启应用，再不行把这条截图发给开发者。
        </p>
      )}
      {props.warningMessage && (
        <p className="mt-4 rounded-lg border border-warning/25 bg-warning/10 px-3 py-2 font-body-md text-sm text-warning">
          {props.warningMessage}
        </p>
      )}
      <div className="mt-5 grid gap-3 sm:grid-cols-2">
        {readinessRows.map((row) => (
          <div
            key={row.key}
            className={`rounded-lg border px-4 py-3 ${
              row.done
                ? 'border-success/20 bg-success/10'
                : 'border-card-border bg-surface-container-low'
            }`}
          >
            <div className="flex items-center gap-2">
              <Icon
                name={row.done ? 'check_circle' : 'radio_button_unchecked'}
                size={18}
                className={row.done ? 'text-success' : 'text-on-surface-variant'}
              />
              <h4 className="font-body-md font-medium text-on-surface">{row.title}</h4>
            </div>
            <p className="mt-1 pl-6 font-body-md text-xs leading-relaxed text-on-surface-variant">
              {row.detail}
            </p>
            {row.action && (
              <button
                type="button"
                onClick={rowActions[row.action]}
                disabled={row.actionDisabled}
                className="mt-3 ml-6 inline-flex items-center gap-1.5 rounded-lg border border-card-border bg-surface px-3 py-1.5 font-body-md text-xs text-on-surface transition-colors hover:bg-surface-container-high disabled:opacity-60"
              >
                {row.busy && (
                  <Icon name="progress_activity" size={14} className="animate-spin" />
                )}
                {row.actionLabel ?? '继续'}
              </button>
            )}
          </div>
        ))}
      </div>
      {!props.runtimeCheck && selfCheckRecord && (
        <div className="mt-5 rounded-lg border border-card-border bg-surface-container-low p-4">
          <div className="flex min-w-0 items-start gap-2">
            <Icon name="fact_check" size={18} className="mt-0.5 shrink-0 text-primary" />
            <div className="min-w-0 flex-1">
              <h4 className="font-body-md font-medium text-on-surface">上次检查结果</h4>
              <p className="mt-1 font-body-md text-xs leading-relaxed text-on-surface-variant">
                上次检查的结果已记下，再点「开始检查」可以更新。
              </p>
              <SelfCheckRecordCard record={selfCheckRecord} className="mt-3" />
            </div>
          </div>
        </div>
      )}
      {props.runtimeCheck && (
        <div className="mt-5 rounded-lg border border-card-border bg-surface-container-low p-4">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
            <div className="flex min-w-0 items-center gap-2">
              <Icon name="fact_check" size={18} className="shrink-0 text-primary" />
              <h4 className="font-body-md font-medium text-on-surface">检查结果</h4>
            </div>
            {selfCheckRecord && (
              <SelfCheckRecordCard record={selfCheckRecord} />
            )}
          </div>
          <div className="mt-3 grid gap-2">
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.storage} item={props.runtimeCheck.storage} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.settings} item={props.runtimeCheck.settings} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.github} item={props.runtimeCheck.github} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.stars} item={props.runtimeCheck.stars} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.readme} item={props.runtimeCheck.readme} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.ai} item={props.runtimeCheck.ai} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.tagNetwork} item={props.runtimeCheck.tagNetwork} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.recommendation} item={props.runtimeCheck.recommendation} />
          </div>
        </div>
      )}
    </section>
  );
}

function SelfCheckRecordCard(props: { record: RuntimeSelfCheckRecord; className?: string }) {
  return (
    <div className={`rounded-lg border border-outline-variant/30 bg-surface px-3 py-2 text-xs text-on-surface-variant ${props.className ?? ''}`}>
      <p className="font-body-md font-medium text-on-surface">检查结果小结</p>
      <p className="mt-1 font-body-md">
        检查时间：{props.record.completedAt}
      </p>
      <p className="mt-1 font-body-md">
        通过 {props.record.passed} 项，没通过 {props.record.failed} 项，没检查 {props.record.skipped} 项
      </p>
    </div>
  );
}

function buildRuntimeSelfCheckRecord(result: RuntimeReadinessCheckResult): RuntimeSelfCheckRecord {
  return {
    completedAt: formatMinute(new Date().toISOString()),
    ...summarizeRuntimeCheck(result),
  };
}

function summarizeRuntimeCheck(result: RuntimeReadinessCheckResult) {
  const items = [
    result.storage,
    result.settings,
    result.github,
    result.stars,
    result.readme,
    result.ai,
    result.tagNetwork,
    result.recommendation,
  ];
  return items.reduce(
    (summary, item) => {
      if (item.status === 'passed') {
        summary.passed += 1;
      } else if (item.status === 'failed') {
        summary.failed += 1;
      } else {
        summary.skipped += 1;
      }
      return summary;
    },
    { passed: 0, failed: 0, skipped: 0 },
  );
}

function RuntimeCheckRow({ title, item }: { title: string; item: RuntimeReadinessCheckItem }) {
  const tone = item.status === 'passed'
    ? 'text-success'
    : item.status === 'failed'
      ? 'text-error'
      : 'text-on-surface-variant';
  const icon = item.status === 'passed'
    ? 'check_circle'
    : item.status === 'failed'
      ? 'error'
      : 'radio_button_unchecked';
  const label = item.status === 'passed' ? '通过' : item.status === 'failed' ? '没通过' : '没检查';

  return (
    <div className="rounded-md border border-outline-variant/30 bg-surface px-3 py-2">
      <div className="flex items-start gap-2">
        <Icon name={icon} size={16} className={`mt-0.5 ${tone}`} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <p className="font-body-md text-xs font-medium text-on-surface">{title}</p>
            <span className={`text-[11px] font-medium ${tone}`}>{label}</span>
          </div>
          <p className="mt-0.5 font-body-md text-xs leading-relaxed text-on-surface-variant">
            {item.message}
          </p>
          {item.detail && (
            <p className="mt-1 break-words font-body-md text-[11px] leading-relaxed text-on-surface-variant/80">
              {item.detail}
            </p>
          )}
          {item.action && (
            <p className="mt-2 rounded-md border border-outline-variant/25 bg-surface-container-low px-2 py-1.5 font-body-md text-[11px] leading-relaxed text-on-surface">
              <span className="font-medium text-on-surface">建议处理：</span>
              {item.action}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

/* === AI 设置 === */
function AISettings({ settingsHook }: { settingsHook: ReturnType<typeof useAppSettings> }) {
  const ai = settingsHook.settings.ai;
  const needsRemoteConfig = ai.provider !== 'none';
  const hasSavedApiKey = isSavedAiApiKeyPlaceholder(ai.apiKey);
  const [apiKeyDraft, setApiKeyDraft] = useState(hasSavedApiKey ? '' : ai.apiKey);
  const [isTestingAi, setIsTestingAi] = useState(false);
  const [isLoadingModels, setIsLoadingModels] = useState(false);
  const [isLoadingDiagnostics, setIsLoadingDiagnostics] = useState(false);
  const [aiDiagnostics, setAiDiagnostics] = useState<AiRequestDiagnostics | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [availableModels, setAvailableModels] = useState<AiModelOption[]>([]);
  const [testMessage, setTestMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [modelListMessage, setModelListMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [testDialog, setTestDialog] = useState<AiTestDialogState | null>(null);
  const [moreProvidersOpen, setMoreProvidersOpen] = useState(
    MORE_AI_PROVIDER_PRESETS.some((preset) => preset.id === ai.providerPreset),
  );
  // 「高级」（请求地址和诊断信息）默认收起；自定义服务一定要填请求地址，这时自动展开
  const needsBaseUrlNow = ai.provider === 'openai-compatible' && !ai.baseUrl.trim();
  const [advancedOpen, setAdvancedOpen] = useState(needsBaseUrlNow);
  const activeTestRequestRef = useRef<{ requestId: string; startedAt: number; firstTokenSeen: boolean } | null>(null);
  const currentPreset = getAiProviderPreset(ai.providerPreset);
  const endpointPlaceholder = currentPreset.baseUrl || getAiEndpointPlaceholder(ai.provider);
  const apiKeyPlaceholder = currentPreset.apiKeyLabel;
  const modelPlaceholder = currentPreset.modelPlaceholder;
  const effectiveApiKey = apiKeyDraft.trim() ? apiKeyDraft : ai.apiKey;
  // 人就在「AI 设置」这一页：提示指这一页上面的输入框，不再说「去设置 → AI 设置」
  const currentConfigMessage = getAiConfigMessage({ ...ai, apiKey: effectiveApiKey }, { onAiSettingsPage: true });
  const canTestAi = needsRemoteConfig && !currentConfigMessage;
  const aiKeySaveMessage = getAiKeySaveMessage(settingsHook.aiKeySaveStatus);
  const modelOptions = mergeAiModelOptions(currentPreset, availableModels, ai.model);

  useEffect(() => {
    setApiKeyDraft(isSavedAiApiKeyPlaceholder(ai.apiKey) ? '' : ai.apiKey);
  }, [ai.apiKey]);

  useEffect(() => {
    if (needsBaseUrlNow) setAdvancedOpen(true);
  }, [needsBaseUrlNow]);

  useEffect(() => {
    setAvailableModels([]);
    setModelListMessage(null);
  }, [ai.providerPreset, ai.provider, ai.baseUrl]);

  useEffect(() => {
    void refreshAiDiagnostics();
  }, [ai.provider, ai.baseUrl, ai.model, ai.apiKey]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void listen<AiStreamEvent>('ai-stream', (event) => {
      const payload = event.payload;
      const active = activeTestRequestRef.current;
      if (!active || payload.taskType !== 'ai-config-test' || payload.requestId !== active.requestId) {
        return;
      }

      const now = performance.now();
      if (payload.status === 'delta') {
        const firstTokenLatencyMs = active.firstTokenSeen ? null : now - active.startedAt;
        if (!active.firstTokenSeen) {
          activeTestRequestRef.current = { ...active, firstTokenSeen: true };
        }
        setTestDialog((current) => current
          ? {
              ...current,
              status: 'running',
              stage: '正在接收 AI 的回复',
              firstTokenLatencyMs: current.firstTokenLatencyMs ?? firstTokenLatencyMs,
              summary: payload.text ? payload.text.slice(0, 120) : current.summary,
            }
          : current);
        return;
      }

      if (payload.status === 'started' || payload.status === 'fallback') {
        setTestDialog((current) => current
          ? {
              ...current,
              status: 'running',
              stage: payload.message ?? current.stage,
            }
          : current);
      }
    }).then((nextUnlisten) => {
      unlisten = nextUnlisten;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  async function handleProviderPresetChange(providerPreset: AIProviderPreset) {
    const preset = getAiProviderPreset(providerPreset);
    setApiKeyDraft('');
    setTestMessage({
      type: 'success',
      text: preset.provider === 'none'
        ? 'AI 已关闭，已保存的密钥还留在 Mac 的钥匙串里。'
        : `已切换到 ${preset.label}，请填写密钥和模型名。`,
    });
    await settingsHook.updateAI({
      provider: preset.provider,
      providerPreset: preset.id,
      baseUrl: preset.baseUrl,
      model: '',
      enableAutoSummary: preset.provider === 'none' ? false : ai.enableAutoSummary,
    });
  }

  async function handleTestAiConnection() {
    const nextAi = { ...ai, apiKey: effectiveApiKey };
    const configMessage = getAiConfigMessage(nextAi, { onAiSettingsPage: true });
    const startedAt = performance.now();
    const requestId = `ai-config-test-${Date.now()}`;
    const providerLabel = currentPreset.label;
    const endpoint = nextAi.baseUrl.trim() || currentPreset.baseUrl || '默认服务地址';
    const model = nextAi.model.trim() || '还没填模型名';

    setTestDialog({
      isOpen: true,
      status: configMessage ? 'error' : 'preparing',
      stage: configMessage ? '设置还没填完整' : '正在准备',
      providerLabel,
      endpoint,
      model,
      startedAt,
      firstTokenLatencyMs: null,
      totalLatencyMs: null,
      outputTokens: null,
      tokensPerSecond: null,
      summary: null,
      error: configMessage,
    });
    activeTestRequestRef.current = configMessage ? null : { requestId, startedAt, firstTokenSeen: false };

    if (configMessage) {
      setTestMessage({ type: 'error', text: configMessage });
      return;
    }

    setIsTestingAi(true);
    setTestMessage(null);
    try {
      if (shouldFlushAiApiKey(nextAi)) {
        await settingsHook.flushAIKey(nextAi.apiKey);
      }
      setTestDialog((current) => current
        ? { ...current, status: 'running', stage: '正在请求 AI 服务' }
        : current);
      const result = await invoke<AiConnectionTestResult>('test_ai_connection', {
        request: {
          aiConfig: toBackendAiRequestConfig(nextAi),
          requestId,
        },
      });
      const completedAt = performance.now();
      const totalLatencyMs = completedAt - startedAt;
      const outputTokens = Math.max(0, result.outputTokens || estimateTextTokens(result.summaryZh));
      const tokensPerSecond = totalLatencyMs > 0 ? outputTokens / (totalLatencyMs / 1000) : null;
      const summary = result.summaryZh.slice(0, 120);
      setTestMessage({
        type: 'success',
        text: `AI 设置可用，${result.model} 回复了一段测试内容：${summary}`,
      });
      setTestDialog((current) => current
        ? {
            ...current,
            status: 'success',
            stage: '测试完成',
            firstTokenLatencyMs: current.firstTokenLatencyMs ?? totalLatencyMs,
            totalLatencyMs,
            outputTokens,
            tokensPerSecond,
            summary,
            error: null,
          }
        : current);
    } catch (error) {
      const message = toErrorMessage(error);
      const completedAt = performance.now();
      setTestMessage({ type: 'error', text: message });
      setTestDialog((current) => current
        ? {
            ...current,
            status: 'error',
            stage: '测试失败',
            firstTokenLatencyMs: null,
            totalLatencyMs: completedAt - startedAt,
            outputTokens: null,
            tokensPerSecond: null,
            summary: null,
            error: message,
          }
        : current);
      await refreshAiDiagnostics({ skipLoading: true });
    } finally {
      setIsTestingAi(false);
      activeTestRequestRef.current = null;
    }
  }

  async function refreshAiDiagnostics(options?: { flushKey?: boolean; skipLoading?: boolean }) {
    const nextAi = { ...ai, apiKey: effectiveApiKey };
    if (!options?.skipLoading) {
      setIsLoadingDiagnostics(true);
    }
    setDiagnosticsError(null);
    try {
      if (options?.flushKey && shouldFlushAiApiKey(nextAi)) {
        await settingsHook.flushAIKey(nextAi.apiKey);
      }
      const diagnostics = await invoke<AiRequestDiagnostics>('diagnose_ai_request', {
        request: {
          aiConfig: toBackendAiRequestConfig(nextAi),
        },
      });
      setAiDiagnostics(diagnostics);
    } catch (error) {
      setAiDiagnostics(null);
      setDiagnosticsError(toErrorMessage(error));
    } finally {
      if (!options?.skipLoading) {
        setIsLoadingDiagnostics(false);
      }
    }
  }

  async function handleLoadAiModels() {
    if (!needsRemoteConfig) {
      setModelListMessage({ type: 'error', text: '请先选择 AI 服务。' });
      return;
    }

    const nextAi = { ...ai, apiKey: effectiveApiKey };
    const baseUrl = nextAi.baseUrl.trim();
    if (nextAi.provider === 'openai-compatible' && !baseUrl) {
      setModelListMessage({ type: 'error', text: '请先到下面的「高级」里填写请求地址。' });
      return;
    }
    if (!nextAi.apiKey.trim() && nextAi.provider !== 'openai-compatible') {
      setModelListMessage({ type: 'error', text: '请先填写密钥，再获取模型列表。' });
      return;
    }

    setIsLoadingModels(true);
    setModelListMessage(null);
    try {
      if (shouldFlushAiApiKey(nextAi)) {
        await settingsHook.flushAIKey(nextAi.apiKey);
      }
      const models = await invoke<AiModelOption[]>('list_ai_models', {
        request: {
          aiConfig: toBackendAiRequestConfig({
            ...nextAi,
            model: nextAi.model.trim() || currentPreset.modelSuggestions[0] || 'model-list',
          }),
        },
      });
      const normalizedModels = normalizeAiModelOptions(models);
      setAvailableModels(normalizedModels);
      setModelListMessage({
        type: 'success',
        text: normalizedModels.length > 0
          ? `已获取 ${normalizedModels.length} 个可用模型。`
          : '服务已响应，但没有返回可显示的模型。',
      });
    } catch (error) {
      setModelListMessage({ type: 'error', text: toErrorMessage(error) });
    } finally {
      setIsLoadingModels(false);
    }
  }

  async function handleClearSavedAiKey() {
    setApiKeyDraft('');
    setTestMessage({ type: 'success', text: '已清除保存的密钥。' });
    await settingsHook.updateAI({ apiKey: '' });
  }

  return (
    <>
      <section className="glass-panel rounded-lg p-6">
        <div className="mb-4">
          <h3 className="font-headline-md text-[20px] font-semibold text-on-surface">AI 设置</h3>
        </div>
        <div className="space-y-6">
        <div className="space-y-4">
          <div>
            <h4 className="font-body-md text-sm font-semibold text-on-surface">常用服务</h4>
            <p className="mt-1 font-body-md text-xs text-on-surface-variant">
              {describeCurrentAiService(currentPreset.label, currentPreset.id === DISABLED_AI_PROVIDER_PRESET.id)}
            </p>
          </div>
          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
            {FEATURED_AI_PROVIDER_PRESETS.map((preset) => (
              <AIProviderOption
                key={preset.id}
                preset={preset}
                selected={currentPreset.id === preset.id}
                onSelect={() => void handleProviderPresetChange(preset.id)}
              />
            ))}
          </div>

          <details
            className="rounded-lg border border-card-border bg-surface-container-lowest"
            open={moreProvidersOpen}
            onToggle={(event) => setMoreProvidersOpen(event.currentTarget.open)}
          >
            <summary className="flex cursor-pointer list-none items-center justify-between gap-3 px-4 py-3 font-body-md text-sm font-medium text-on-surface">
              <span className="flex items-center gap-2">
                <Icon name="apps" size={17} className="text-on-surface-variant" />
                更多服务
              </span>
              <Icon name="expand_more" size={18} className="text-on-surface-variant" />
            </summary>
            <div className="grid gap-2 border-t border-card-border p-3 sm:grid-cols-2 xl:grid-cols-3">
              {MORE_AI_PROVIDER_PRESETS.map((preset) => (
                <AIProviderOption
                  key={preset.id}
                  preset={preset}
                  selected={currentPreset.id === preset.id}
                  onSelect={() => void handleProviderPresetChange(preset.id)}
                />
              ))}
            </div>
          </details>

          <button
            type="button"
            onClick={() => void handleProviderPresetChange(DISABLED_AI_PROVIDER_PRESET.id)}
            aria-pressed={currentPreset.id === DISABLED_AI_PROVIDER_PRESET.id}
            className={`flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors focus:outline-none focus:ring-2 focus:ring-primary/30 ${
              currentPreset.id === DISABLED_AI_PROVIDER_PRESET.id
                ? 'border-warning/35 bg-warning/10 text-on-surface'
                : 'border-card-border bg-transparent text-on-surface-variant hover:bg-surface-container-low'
            }`}
          >
            <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-surface-container-high">
              <Icon name={DISABLED_AI_PROVIDER_PRESET.icon} size={17} />
            </span>
            <span className="min-w-0 flex-1">
              <span className="block font-body-md text-sm font-medium text-on-surface">关闭 AI</span>
              <span className="block font-body-md text-xs text-on-surface-variant">保留已保存的密钥，不再调用 AI；已有的中文说明和搜索照常能用</span>
            </span>
            {currentPreset.id === DISABLED_AI_PROVIDER_PRESET.id && <Icon name="check_circle" size={18} className="shrink-0 text-warning" />}
          </button>
        </div>
        {needsRemoteConfig && (
          <>
            <div className="grid gap-2">
              <label className="font-body-md text-on-surface font-medium">
                {currentPreset.apiKeyLabel}
              </label>
              <input
                type="password"
                autoComplete="off"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                inputMode="text"
                value={apiKeyDraft}
                onChange={(e) => {
                  const nextApiKey = e.target.value;
                  setApiKeyDraft(nextApiKey);
                  void settingsHook.updateAIKeyDraft(nextApiKey);
                }}
                onBlur={() => {
                  const nextAi = { ...ai, apiKey: apiKeyDraft };
                  if ((apiKeyDraft.trim() || !hasSavedApiKey) && shouldFlushAiApiKey(nextAi)) {
                    void settingsHook.flushAIKey(apiKeyDraft).catch(() => undefined);
                  }
                }}
                placeholder={apiKeyPlaceholder}
                className="w-full bg-surface-container-low border border-outline-variant rounded-lg px-4 py-2 font-body-md text-on-surface focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
              />
              <p className="font-body-md text-[12px] text-on-surface-variant">
                {aiKeyHelpText({ hasSavedKey: hasSavedApiKey, provider: ai.provider })}
              </p>
              {hasSavedApiKey && !apiKeyDraft.trim() && (
                <button
                  type="button"
                  onClick={() => void handleClearSavedAiKey()}
                  className="w-fit rounded-lg border border-card-border bg-surface-container-high px-3 py-1.5 font-body-md text-xs text-on-surface transition-colors hover:bg-surface-container-highest"
                >
                  清除已保存的密钥
                </button>
              )}
              {aiKeySaveMessage && (
                <p
                  className={`font-body-md text-[12px] ${
                    settingsHook.aiKeySaveStatus === 'error' ? 'text-error' : 'text-on-surface-variant'
                  }`}
                >
                  {aiKeySaveMessage}
                </p>
              )}
            </div>
          </>
        )}
        <div className="border-t border-card-border py-4">
          <h4 className="font-body-lg font-medium text-on-surface">自动整理</h4>
          <p className="font-body-md text-on-surface-variant mt-1">{AI_AUTO_ORGANIZE_POINTER}。</p>
        </div>
        <div className="grid gap-4 border-t border-card-border py-4 lg:grid-cols-[minmax(0,1fr)_minmax(320px,420px)] lg:items-start">
          <div>
            <h4 className="font-body-lg font-medium text-on-surface">模型</h4>
            <p className="font-body-md text-on-surface-variant mt-1">
              可以从当前服务获取模型列表，也可以直接填服务商给你的模型名。
            </p>
          </div>
          <div className="min-w-0 space-y-3">
            <div className="flex gap-2">
              <select
                value={modelOptions.some((model) => model.id === ai.model) ? ai.model : ''}
                onChange={(event) => {
                  if (event.target.value) {
                    void settingsHook.updateAI({ model: event.target.value });
                  }
                }}
                disabled={!needsRemoteConfig || modelOptions.length === 0}
                className="min-w-0 flex-1 rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 font-body-md text-sm text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary disabled:opacity-60"
              >
                <option value="">选择模型</option>
                {modelOptions.map((model) => (
                  <option key={model.id} value={model.id}>
                    {formatAiModelOptionLabel(model)}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={() => void handleLoadAiModels()}
                disabled={!needsRemoteConfig || isLoadingModels}
                className="gsat-primary-button inline-flex shrink-0 items-center gap-2 rounded-lg bg-primary px-3 py-2 font-body-md text-sm font-medium text-white transition-all hover:bg-primary-container disabled:opacity-60"
              >
                <Icon name="refresh" size={16} />
                {isLoadingModels ? '获取中' : '获取模型'}
              </button>
            </div>
            <div className="flex flex-wrap gap-2">
              {currentPreset.modelSuggestions.map((model) => (
                <button
                  key={model}
                  type="button"
                  disabled={!needsRemoteConfig}
                  onClick={() => void settingsHook.updateAI({ model })}
                  className={`rounded-md border px-2.5 py-1.5 font-body-md text-xs transition-colors disabled:opacity-60 ${
                    ai.model === model
                      ? 'border-primary bg-primary text-white'
                      : 'border-card-border bg-surface-container-high text-on-surface hover:bg-surface-container-highest'
                  }`}
                >
                  {model}
                </button>
              ))}
            </div>
            <input
              type="text"
              autoComplete="off"
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              inputMode="text"
              value={ai.model}
              onChange={(e) => void settingsHook.updateAI({ model: e.target.value })}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.currentTarget.blur();
                }
              }}
              placeholder={modelPlaceholder}
              disabled={!needsRemoteConfig}
              className="w-full rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 font-body-md text-sm text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary disabled:opacity-60"
            />
            {modelListMessage && (
              <p
                className={`rounded-lg border px-3 py-2 font-body-md text-xs ${
                  modelListMessage.type === 'success'
                    ? 'border-success/20 bg-success/10 text-success'
                    : 'border-error/20 bg-error/10 text-error'
                }`}
              >
                {modelListMessage.text}
              </p>
            )}
          </div>
        </div>
        {needsRemoteConfig && (
          <details
            className="rounded-lg border border-card-border bg-surface-container-lowest"
            open={advancedOpen}
            onToggle={(event) => setAdvancedOpen(event.currentTarget.open)}
          >
            <summary className="flex cursor-pointer list-none items-center justify-between gap-3 px-4 py-3 font-body-md text-sm font-medium text-on-surface">
              <span className="flex items-center gap-2">
                <Icon name="settings" size={17} className="text-on-surface-variant" />
                高级（请求地址和诊断信息）
              </span>
              <Icon name="expand_more" size={18} className="text-on-surface-variant" />
            </summary>
            <div className="space-y-4 border-t border-card-border p-4">
                <div className="grid gap-2">
                  <label className="font-body-md text-on-surface font-medium">请求地址</label>
                  <input
                    type="url"
                    autoComplete="off"
                    autoCapitalize="none"
                    autoCorrect="off"
                    spellCheck={false}
                    inputMode="url"
                    value={ai.baseUrl}
                    onChange={(e) => void settingsHook.updateAI({ baseUrl: e.target.value })}
                    placeholder={endpointPlaceholder}
                    className="w-full bg-surface-container-low border border-outline-variant rounded-lg px-4 py-2 font-body-md text-on-surface focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
                  />
                  <p className="font-body-md text-[12px] text-on-surface-variant">
                    {aiBaseUrlHelp(ai.provider)}
                  </p>
                </div>
              <AiRequestDiagnosticsPanel
                diagnostics={aiDiagnostics}
                errorMessage={diagnosticsError}
                isLoading={isLoadingDiagnostics}
                testMessage={testMessage}
                onRefresh={() => void refreshAiDiagnostics({ flushKey: true })}
              />
            </div>
          </details>
        )}
        <div className="flex flex-col items-stretch justify-between gap-4 border-t border-card-border py-4 sm:flex-row sm:items-start">
          <div className="flex-1">
            <h4 className="font-body-lg font-medium text-on-surface">测试连接</h4>
            <p className="font-body-md text-on-surface-variant mt-1">
              {AI_TEST_COST_NOTE}
            </p>
            <p
              className={`mt-3 rounded-lg border px-3 py-2 font-body-md text-sm ${
                canTestAi
                  ? 'border-success/20 bg-success/10 text-success'
                  : 'border-error/20 bg-error/10 text-error'
              }`}
            >
              {canTestAi ? '设置已填完整，可以点「测试连接」。' : currentConfigMessage}
            </p>
            {testMessage && (
              <p
                className={`mt-3 rounded-lg border px-3 py-2 font-body-md text-sm ${
                  testMessage.type === 'success'
                    ? 'border-success/20 bg-success/10 text-success'
                    : 'border-error/20 bg-error/10 text-error'
                }`}
              >
                {testMessage.text}
              </p>
            )}
          </div>
          <button
            type="button"
            onClick={() => void handleTestAiConnection()}
            disabled={isTestingAi || !canTestAi}
            title={canTestAi ? '用现在的设置真实调用一次 AI，会产生很少的费用' : (currentConfigMessage ?? '请先把设置填完整')}
            className="shrink-0 rounded-lg border border-card-border bg-surface-container-high px-4 py-2 font-body-md text-sm text-on-surface transition-all hover:bg-surface-container-highest disabled:opacity-60"
          >
            {isTestingAi ? '测试中…' : '测试连接'}
          </button>
        </div>
        </div>
      </section>
      {testDialog?.isOpen && (
        <AiTestDialog
          state={testDialog}
          isTesting={isTestingAi}
          onClose={() => setTestDialog((current) => current ? { ...current, isOpen: false } : current)}
        />
      )}
    </>
  );
}

function AIProviderOption({
  preset,
  selected,
  onSelect,
}: {
  preset: AIProviderPresetOption;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      className={`flex min-h-[74px] min-w-0 items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors focus:outline-none focus:ring-2 focus:ring-primary/30 ${
        selected
          ? 'border-primary bg-primary/10 text-on-surface'
          : 'border-card-border bg-surface-container-lowest text-on-surface hover:border-primary/35 hover:bg-surface-container-low'
      }`}
    >
      <span className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md ${selected ? 'bg-primary text-white' : 'bg-surface-container-high text-on-surface-variant'}`}>
        <Icon name={preset.icon} size={18} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-2">
          <span className="truncate font-body-md text-sm font-semibold">{preset.label}</span>
          {selected && <Icon name="check_circle" size={15} className="shrink-0 text-primary" />}
        </span>
        <span className="mt-0.5 line-clamp-2 block font-body-md text-xs leading-relaxed text-on-surface-variant">
          {preset.description}
        </span>
      </span>
    </button>
  );
}

function EmbeddingSettingsPanel({
  accountId,
  settingsHook,
}: {
  accountId: string | null;
  settingsHook: ReturnType<typeof useAppSettings>;
}) {
  const embedding = settingsHook.settings.embedding;
  const hasSavedApiKey = isSavedAiApiKeyPlaceholder(embedding.apiKey);
  const [apiKeyDraft, setApiKeyDraft] = useState(hasSavedApiKey ? '' : embedding.apiKey);
  const [isTesting, setIsTesting] = useState(false);
  const [isRebuilding, setIsRebuilding] = useState(false);
  const [isLoadingStatus, setIsLoadingStatus] = useState(false);
  const [status, setStatus] = useState<EmbeddingRuntimeStatus | null>(null);
  const [message, setMessage] = useState<EmbeddingPanelMessage | null>(null);
  const [isPreparing, setIsPreparing] = useState(false);
  const [pendingLocalEmbedding, setPendingLocalEmbedding] = useState<EmbeddingSettingsValue | null>(null);
  const effectiveApiKey = apiKeyDraft.trim() ? apiKeyDraft : embedding.apiKey;
  const effectiveEmbedding = { ...embedding, apiKey: effectiveApiKey };
  const configMessage = getEmbeddingConfigMessage(effectiveEmbedding);
  const canUseEmbedding = Boolean(accountId && embedding.enabled && !configMessage);
  const keySaveMessage = getAiKeySaveMessage(settingsHook.embeddingKeySaveStatus);

  useEffect(() => {
    setApiKeyDraft(isSavedAiApiKeyPlaceholder(embedding.apiKey) ? '' : embedding.apiKey);
  }, [embedding.apiKey]);

  useEffect(() => {
    setStatus(null);
    setMessage(null);
    setPendingLocalEmbedding(null);
  }, [accountId, embedding.enabled, embedding.provider, embedding.downloadSource, embedding.baseUrl, embedding.model, embedding.dimensions]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void listen<EmbeddingRuntimeStatus>('embedding-runtime-status', (event) => {
      if (!accountId || event.payload.accountId !== accountId) {
        return;
      }
      setStatus(event.payload);
      setIsPreparing(['downloading', 'verifying', 'loading', 'indexing'].includes(event.payload.state));
    }).then((dispose) => {
      unlisten = dispose;
    }).catch(() => undefined);
    return () => {
      unlisten?.();
    };
  }, [accountId]);

  useEffect(() => {
    if (!accountId) {
      setIsLoadingStatus(false);
      return;
    }
    let cancelled = false;
    setIsLoadingStatus(true);
    void invoke<EmbeddingRuntimeStatus>('get_embedding_runtime_status', {
      request: {
        accountId,
        embeddingConfig: toBackendEmbeddingRequestConfig(embedding),
      },
    })
      .then((result) => {
        if (!cancelled) {
          setStatus(result);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setMessage({ type: 'error', text: toErrorMessage(error) });
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoadingStatus(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [
    accountId,
    embedding.enabled,
    embedding.provider,
    embedding.baseUrl,
    embedding.model,
    embedding.dimensions,
    embedding.minScore,
    embedding.maxResults,
  ]);

  async function persistEmbeddingKey() {
    if (shouldFlushEmbeddingApiKey(effectiveEmbedding)) {
      await settingsHook.flushEmbeddingKey(effectiveApiKey);
    }
  }

  async function loadStatus(preserveMessage = false) {
    if (!accountId) {
      setMessage({ type: 'error', text: '请先连接 GitHub 账号并同步 Stars。' });
      return;
    }
    setIsLoadingStatus(true);
    try {
      await persistEmbeddingKey();
      const result = await invoke<EmbeddingRuntimeStatus>('get_embedding_runtime_status', {
        request: {
          accountId,
          embeddingConfig: toBackendEmbeddingRequestConfig(effectiveEmbedding),
        },
      });
      setStatus(result);
      if (!preserveMessage) {
        setMessage({ type: 'info', text: `状态已更新：${result.message}` });
      }
    } catch (error) {
      setMessage({ type: 'error', text: toErrorMessage(error) });
    } finally {
      setIsLoadingStatus(false);
    }
  }

  async function handleTestConnection() {
    if (configMessage) {
      setMessage({ type: 'error', text: configMessage });
      return;
    }
    setIsTesting(true);
    setMessage(null);
    try {
      await persistEmbeddingKey();
      const result = await invoke<EmbeddingConnectionTestResult>('test_embedding_connection', {
        request: { embeddingConfig: toBackendEmbeddingRequestConfig(effectiveEmbedding) },
      });
      setMessage({
        type: 'success',
        text: `Embedding 服务可用：${result.model} 返回 ${result.dimensions} 维向量。`,
      });
    } catch (error) {
      setMessage({ type: 'error', text: toErrorMessage(error) });
    } finally {
      setIsTesting(false);
    }
  }

  async function handleToggle(checked: boolean) {
    if (!checked) {
      setMessage(null);
      try {
        await settingsHook.updateEmbedding({ enabled: false });
        if (accountId && embedding.provider === 'local') {
          await invoke('disable_embedding_runtime', { request: { accountId, embeddingConfig: null } });
        }
        setMessage({ type: 'success', text: '向量检索已关闭，当前使用关键词检索。' });
      } catch (error) {
        setMessage({ type: 'error', text: toErrorMessage(error) });
      }
      return;
    }
    if (!accountId) {
      setMessage({ type: 'error', text: '请先连接 GitHub 账号并同步 Stars。' });
      return;
    }
    const nextEmbedding = {
      ...effectiveEmbedding,
      enabled: true,
      provider: embedding.provider === 'none' ? 'local' : embedding.provider,
    } as EmbeddingSettingsValue;
    setMessage(nextEmbedding.provider === 'local'
      ? { type: 'info', text: '正在检查本地模型…' }
      : null);
    setIsLoadingStatus(true);
    try {
      if (nextEmbedding.provider === 'local') {
        const runtime = await invoke<EmbeddingRuntimeStatus>('get_embedding_runtime_status', {
          request: { accountId, embeddingConfig: toBackendEmbeddingRequestConfig(nextEmbedding) },
        });
        setStatus(runtime);
        if (!runtime.modelReady) {
          setPendingLocalEmbedding(nextEmbedding);
          setMessage(null);
          return;
        }
        await startLocalEmbedding(nextEmbedding);
      } else {
        await settingsHook.updateEmbedding({ enabled: true, provider: nextEmbedding.provider });
        setMessage({ type: 'success', text: '远程 Embedding 已启用，请在高级设置中检查连接。' });
      }
    } catch (error) {
      setMessage({ type: 'error', text: toErrorMessage(error) });
    } finally {
      setIsLoadingStatus(false);
      setIsPreparing(false);
    }
  }

  async function startLocalEmbedding(nextEmbedding: EmbeddingSettingsValue) {
    await settingsHook.updateEmbedding({ enabled: true, provider: 'local' });
    setIsPreparing(true);
    const result = await invoke<EmbeddingRuntimeStatus>('enable_local_embedding', {
      request: { accountId, embeddingConfig: toBackendEmbeddingRequestConfig(nextEmbedding) },
    });
    setStatus(result);
    setMessage(result.state === 'partial'
      ? { type: 'error', text: result.message }
      : { type: 'success', text: '本地向量检索已就绪。' });
  }

  async function handleConfirmLocalEmbeddingDownload() {
    const nextEmbedding = pendingLocalEmbedding;
    if (!nextEmbedding || !accountId) {
      return;
    }
    setPendingLocalEmbedding(null);
    setMessage(null);
    try {
      await startLocalEmbedding(nextEmbedding);
    } catch (error) {
      setMessage({ type: 'error', text: toErrorMessage(error) });
    } finally {
      setIsPreparing(false);
      await loadStatus(true).catch(() => undefined);
    }
  }

  async function handleRebuildIndex() {
    if (!accountId) {
      setMessage({ type: 'error', text: '请先连接 GitHub 账号并同步 Stars。' });
      return;
    }
    if (configMessage) {
      setMessage({ type: 'error', text: configMessage });
      return;
    }
    setIsRebuilding(true);
    setMessage(null);
    try {
      await persistEmbeddingKey();
      const result = await invoke<VectorIndexBuildSummary>('rebuild_vector_index', {
        request: {
          accountId,
          embeddingConfig: toBackendEmbeddingRequestConfig(effectiveEmbedding),
        },
      });
      const succeeded = result.indexedCount + result.restoredCount;
      setMessage({
        type: result.failedCount > 0 ? 'error' : 'success',
        text: result.failedCount > 0
          ? `向量索引已处理 ${result.totalCount} 个仓库，成功 ${succeeded} 个，失败 ${result.failedCount} 个。${result.failures[0] ? ` 首个错误：${result.failures[0]}` : ''}`
          : `向量索引已就绪：新生成 ${result.indexedCount} 个，从 SQLite 恢复 ${result.restoredCount} 个。`,
      });
      await loadStatus(true);
    } catch (error) {
      setMessage({ type: 'error', text: toErrorMessage(error) });
    } finally {
      setIsRebuilding(false);
    }
  }

  async function handleRetry() {
    if (!accountId) return;
    setIsPreparing(true);
    setMessage(null);
    try {
      const result = await invoke<EmbeddingRuntimeStatus>('retry_embedding_setup', {
        request: { accountId, embeddingConfig: toBackendEmbeddingRequestConfig(effectiveEmbedding) },
      });
      setStatus(result);
      setMessage(result.state === 'partial'
        ? { type: 'error', text: result.message }
        : { type: 'success', text: '本地 Embedding 已完成重试。' });
    } catch (error) {
      setMessage({ type: 'error', text: toErrorMessage(error) });
    } finally {
      setIsPreparing(false);
      await loadStatus(true).catch(() => undefined);
    }
  }

  async function handleDeleteModel() {
    if (!accountId) return;
    try {
      await invoke<EmbeddingRuntimeStatus>('delete_local_embedding_model', {
        request: { accountId, embeddingConfig: null },
      });
      await loadStatus();
      setMessage({ type: 'success', text: '本地模型已删除。' });
    } catch (error) {
      setMessage({ type: 'error', text: toErrorMessage(error) });
    }
  }

  async function handleClearSavedKey() {
    setApiKeyDraft('');
    await settingsHook.updateEmbedding({ apiKey: '' });
    setMessage({ type: 'success', text: '已清除保存的 Embedding API Key。' });
  }

  return (
    <section className="glass-panel rounded-lg p-6">
      <div className="mb-5 flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0">
          <h3 className="font-headline-md text-[20px] font-semibold text-on-surface">向量检索</h3>
          <p className="mt-1 max-w-3xl font-body-md text-sm text-on-surface-variant">
            默认使用本地模型生成中英双语向量，由本机 zvec 完成语义召回；远程服务仅在高级设置中启用。
          </p>
        </div>
        <ToggleSwitch
          checked={embedding.enabled}
          onChange={(checked) => void handleToggle(checked)}
          disabled={isPreparing}
        />
      </div>

      <div className="space-y-5 border-t border-card-border pt-5">
        <div className="grid gap-4 sm:grid-cols-3">
          <div className="rounded-lg border border-card-border bg-surface-container-low px-3 py-3">
            <p className="font-body-md text-xs text-on-surface-variant">当前模式</p>
            <p className="mt-1 font-body-md text-sm font-medium text-on-surface">{embedding.provider === 'local' ? '本地模型' : '远程服务'}</p>
          </div>
          <div className="rounded-lg border border-card-border bg-surface-container-low px-3 py-3">
            <p className="font-body-md text-xs text-on-surface-variant">模型</p>
            <p className="mt-1 break-all font-body-md text-sm font-medium text-on-surface">{embedding.provider === 'local' ? 'multilingual-e5-small' : embedding.model || '未配置'}</p>
          </div>
          <div className="rounded-lg border border-card-border bg-surface-container-low px-3 py-3">
            <p className="font-body-md text-xs text-on-surface-variant">向量维度</p>
            <p className="mt-1 font-body-md text-sm font-medium text-on-surface">{embedding.provider === 'local' ? 384 : embedding.dimensions}</p>
          </div>
        </div>

        {embedding.provider === 'local' && (
          <div className="border-y border-outline-variant/30 py-4">
            <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
              <div className="min-w-0">
                <p className="font-body-md text-sm font-medium text-on-surface">模型下载源</p>
                <p className="mt-1 max-w-2xl font-body-md text-xs leading-relaxed text-on-surface-variant">
                  中国大陆网络建议使用国内源。若下载提示 Connection refused，请切换下载源后重试，或检查代理的 TUN 模式。
                </p>
              </div>
              <div
                className="grid shrink-0 grid-cols-2 rounded-lg border border-outline-variant/40 bg-surface-container-low p-1"
                role="radiogroup"
                aria-label="本地模型下载源"
              >
                {([
                  { value: 'modelscope', label: '国内源', detail: 'ModelScope' },
                  { value: 'huggingface', label: '官方源', detail: 'Hugging Face' },
                ] as const).map((source) => {
                  const selected = embedding.downloadSource === source.value;
                  return (
                    <button
                      key={source.value}
                      type="button"
                      role="radio"
                      aria-checked={selected}
                      disabled={isPreparing}
                      onClick={() => {
                        void settingsHook.updateEmbedding({ downloadSource: source.value });
                        setMessage({
                          type: 'info',
                          text: status?.modelReady
                            ? `已切换为${source.label}，下次重新下载模型时生效。`
                            : `已切换为${source.label}，重试时将从 ${source.detail} 下载。`,
                        });
                      }}
                      className={`min-w-28 rounded-md px-3 py-2 text-left transition-colors disabled:opacity-60 ${
                        selected
                          ? 'bg-surface text-on-surface shadow-sm'
                          : 'text-on-surface-variant hover:bg-surface/70 hover:text-on-surface'
                      }`}
                    >
                      <span className="block font-body-md text-xs font-medium">{source.label}</span>
                      <span className="mt-0.5 block font-body-md text-[11px]">{source.detail}</span>
                    </button>
                  );
                })}
              </div>
            </div>
            <p className="mt-3 flex items-start gap-1.5 font-body-md text-xs leading-relaxed text-on-surface-variant">
              <Icon name="verified_user" size={15} className="mt-0.5 shrink-0 text-primary" />
              两个源使用同一模型工件；应用仍会校验固定字节数和 SHA-256，校验通过后才会加载。
            </p>
          </div>
        )}

        <details className="rounded-lg border border-card-border bg-surface-container-low px-4 py-3" open={embedding.provider !== 'local' && embedding.provider !== 'none'}>
            <summary className="cursor-pointer font-body-md text-sm font-medium text-on-surface">高级远程 Embedding 设置</summary>
            <div className="mt-4 space-y-4">
              <div className="grid gap-4 lg:grid-cols-2">
                <label className="grid gap-2 font-body-md text-sm font-medium text-on-surface">
                  Embedding 服务
                  <select
                    value={embedding.provider}
                    onChange={(event) => {
                      const provider = event.target.value as EmbeddingSettingsValue['provider'];
                      void settingsHook.updateEmbedding({
                        provider,
                        enabled: false,
                        baseUrl: provider === 'openai' ? 'https://api.openai.com/v1' : embedding.baseUrl,
                      });
                    }}
                    className="rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 font-body-md text-sm text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                  >
                    <option value="local">本地模型</option>
                    <option value="openai">OpenAI</option>
                    <option value="openai-compatible">OpenAI 兼容接口</option>
                  </select>
                </label>
                <label className="grid gap-2 font-body-md text-sm font-medium text-on-surface">
                  请求地址
                  <input type="url" autoComplete="off" autoCapitalize="none" autoCorrect="off" spellCheck={false} value={embedding.baseUrl} onChange={(event) => void settingsHook.updateEmbedding({ baseUrl: event.target.value })} disabled={!embedding.enabled || embedding.provider === 'local'} placeholder="https://api.openai.com/v1" className="rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 font-body-md text-sm text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary disabled:opacity-60" />
                </label>
              </div>
              <div className="grid gap-4 lg:grid-cols-2">
                <label className="grid gap-2 font-body-md text-sm font-medium text-on-surface">
                  API Key
                  <input type="password" autoComplete="off" autoCapitalize="none" autoCorrect="off" spellCheck={false} value={apiKeyDraft} onChange={(event) => setApiKeyDraft(event.target.value)} onBlur={() => void persistEmbeddingKey().catch(() => undefined)} disabled={!embedding.enabled || embedding.provider === 'local'} placeholder={hasSavedApiKey ? '已保存，输入新 Key 可替换' : '输入 Embedding API Key'} className="rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 font-body-md text-sm text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary disabled:opacity-60" />
                  <span className="font-body-md text-xs font-normal text-on-surface-variant">只发送工具的公开资料文本，不发送个人笔记；密钥只保存在 Mac 的钥匙串里。</span>
                  {hasSavedApiKey && !apiKeyDraft.trim() && <button type="button" onClick={() => void handleClearSavedKey()} className="w-fit rounded-lg border border-card-border bg-surface-container-high px-3 py-1.5 font-body-md text-xs text-on-surface">清除已保存 Key</button>}
                  {keySaveMessage && <span className="font-body-md text-xs text-on-surface-variant">{keySaveMessage}</span>}
                </label>
                <label className="grid gap-2 font-body-md text-sm font-medium text-on-surface">
                  模型 ID
                  <input type="text" autoComplete="off" autoCapitalize="none" autoCorrect="off" spellCheck={false} value={embedding.model} onChange={(event) => void settingsHook.updateEmbedding({ model: event.target.value })} disabled={!embedding.enabled || embedding.provider === 'local'} placeholder="text-embedding-3-small" className="rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 font-body-md text-sm text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary disabled:opacity-60" />
                </label>
              </div>
            </div>
          </details>

        <div className="grid gap-4 sm:grid-cols-3">
          <label className="grid gap-2 font-body-md text-sm font-medium text-on-surface">
            向量维度
            <input
              type="number"
              min={1}
              max={8192}
              step={1}
              value={embedding.provider === 'local' ? 384 : embedding.dimensions}
              onChange={(event) => void settingsHook.updateEmbedding({ dimensions: Number(event.target.value) })}
              disabled={!embedding.enabled || embedding.provider === 'local'}
              className="rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 font-body-md text-sm text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary disabled:opacity-60"
            />
          </label>
          <label className="grid gap-2 font-body-md text-sm font-medium text-on-surface">
            AI 不可用时最低相似度
            <input
              type="number"
              min={0}
              max={1}
              step={0.01}
              value={embedding.minScore}
              onChange={(event) => void settingsHook.updateEmbedding({ minScore: Number(event.target.value) })}
              disabled={!embedding.enabled}
              className="rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 font-body-md text-sm text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary disabled:opacity-60"
            />
          </label>
          <label className="grid gap-2 font-body-md text-sm font-medium text-on-surface">
            AI 不可用时结果数
            <input
              type="number"
              min={1}
              max={10}
              step={1}
              value={embedding.maxResults}
              onChange={(event) => void settingsHook.updateEmbedding({ maxResults: Number(event.target.value) })}
              disabled={!embedding.enabled}
              className="rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 font-body-md text-sm text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary disabled:opacity-60"
            />
          </label>
        </div>

        <div className="rounded-lg border border-card-border bg-surface-container-low px-4 py-3">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <p className="font-body-md text-sm font-medium text-on-surface">
                {status?.message ?? (embedding.enabled ? '可检查本机索引状态。' : '启用后可生成并使用语义向量。')}
              </p>
              {embedding.enabled && status && (
                <p className="mt-1 font-body-md text-xs text-on-surface-variant">
                  已索引 {status.indexedCount} / {status.totalCount} 个仓库 · {status.model} / {status.dimensions} 维 · 缓存 {(status.cacheBytes / 1024 / 1024).toFixed(0)} MB
                </p>
              )}
            </div>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => void handleTestConnection()}
                disabled={!canUseEmbedding || embedding.provider === 'local' || isTesting || isRebuilding || isPreparing}
                className="rounded-lg border border-card-border bg-surface-container-high px-3 py-2 font-body-md text-sm text-on-surface transition-colors hover:bg-surface-container-highest disabled:opacity-60"
              >
                {isTesting ? '测试中…' : '测试连接'}
              </button>
              <button
                type="button"
                onClick={() => void loadStatus()}
                disabled={!accountId || isLoadingStatus || isRebuilding || isPreparing}
                className="rounded-lg border border-card-border bg-surface-container-high px-3 py-2 font-body-md text-sm text-on-surface transition-colors hover:bg-surface-container-highest disabled:opacity-60"
              >
                {isLoadingStatus ? '检查中…' : '检查状态'}
              </button>
              <button
                type="button"
                onClick={() => void handleRebuildIndex()}
                disabled={!canUseEmbedding || isTesting || isRebuilding || isPreparing}
                className="gsat-primary-button rounded-lg bg-primary px-3 py-2 font-body-md text-sm font-medium text-white transition-colors hover:bg-primary-container disabled:opacity-60"
              >
                {isRebuilding ? '重建中…' : '重建索引'}
              </button>
              {embedding.provider === 'local' && !embedding.enabled && (
                <button
                  type="button"
                  onClick={() => void handleToggle(true)}
                  disabled={!accountId || isLoadingStatus || isPreparing}
                  className="gsat-primary-button rounded-lg bg-primary px-3 py-2 font-body-md text-sm font-medium text-white transition-colors hover:bg-primary-container disabled:opacity-60"
                >
                  下载并启用
                </button>
              )}
              {embedding.provider === 'local' && embedding.enabled && status?.canRetry && (
                <button type="button" onClick={() => void handleRetry()} disabled={isPreparing} className="rounded-lg border border-card-border bg-surface-container-high px-3 py-2 font-body-md text-sm text-on-surface disabled:opacity-60">重试准备</button>
              )}
              {embedding.provider === 'local' && !embedding.enabled && Boolean(status?.cacheBytes) && (
                <button type="button" onClick={() => void handleDeleteModel()} className="rounded-lg border border-card-border bg-surface-container-high px-3 py-2 font-body-md text-sm text-on-surface">删除本地模型</button>
              )}
            </div>
          </div>
          {status && <EmbeddingSetupProgress state={status.state} />}
          {!canUseEmbedding && (
            <p className="mt-3 font-body-md text-xs text-on-surface-variant">
              {accountId ? configMessage : '请先连接 GitHub 账号并同步 Stars。'}
            </p>
          )}
          {message && (
            <p className={`mt-3 rounded-lg border px-3 py-2 font-body-md text-sm ${
              message.type === 'success'
                ? 'border-success/20 bg-success/10 text-success'
                : message.type === 'error'
                  ? 'border-error/20 bg-error/10 text-error'
                  : 'border-primary/20 bg-primary/10 text-primary'
            }`}>
              {message.text}
            </p>
          )}
        </div>
      </div>
      {pendingLocalEmbedding && (
        <LocalEmbeddingDownloadDialog
          downloadSource={pendingLocalEmbedding.downloadSource}
          onCancel={() => {
            setPendingLocalEmbedding(null);
            setMessage({ type: 'info', text: '已取消启用，未下载本地模型。' });
          }}
          onConfirm={() => void handleConfirmLocalEmbeddingDownload()}
        />
      )}
    </section>
  );
}

function EmbeddingSetupProgress({ state }: { state: EmbeddingRuntimeStatus['state'] }) {
  const activeIndex = EMBEDDING_SETUP_STAGES.findIndex((stage) => stage.state === state);
  if (activeIndex < 0) {
    return null;
  }

  const activeStage = EMBEDDING_SETUP_STAGES[activeIndex];
  return (
    <div className="mt-3 rounded-lg border border-primary/20 bg-primary/5 px-3 py-3" role="status">
      <div className="flex items-center justify-between gap-3 text-xs">
        <span className="font-medium text-on-surface">{activeStage.label}中</span>
        <span className="text-on-surface-variant">阶段 {activeIndex + 1} / {EMBEDDING_SETUP_STAGES.length}</span>
      </div>
      <div
        className="mt-2 grid grid-cols-4 gap-1"
        role="progressbar"
        aria-label="本地向量模型准备进度"
        aria-valuetext={`${activeStage.label}，第 ${activeIndex + 1} 阶段，共 ${EMBEDDING_SETUP_STAGES.length} 阶段`}
      >
        {EMBEDDING_SETUP_STAGES.map((stage, index) => (
          <div key={stage.state} className="h-1.5 overflow-hidden rounded-full bg-surface-container-high">
            {index < activeIndex && <div className="h-full w-full rounded-full bg-success" />}
            {index === activeIndex && <div className="task-progress-indeterminate h-full w-1/3 rounded-full bg-primary" />}
          </div>
        ))}
      </div>
      <div className="mt-2 grid grid-cols-4 gap-1 text-center text-[11px] text-on-surface-variant">
        {EMBEDDING_SETUP_STAGES.map((stage, index) => (
          <span key={stage.state} className={index === activeIndex ? 'font-medium text-primary' : ''}>
            {stage.label}
          </span>
        ))}
      </div>
      {state === 'downloading' && (
        <p className="mt-2 text-xs text-on-surface-variant">模型文件较大，请保持应用开启。下载完成后会自动校验并继续建库。</p>
      )}
    </div>
  );
}

function LocalEmbeddingDownloadDialog({
  downloadSource,
  onCancel,
  onConfirm,
}: {
  downloadSource: EmbeddingSettingsValue['downloadSource'];
  onCancel: () => void;
  onConfirm: () => void;
}) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onCancel();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onCancel]);

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 px-4 py-6 backdrop-blur-md">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="local-embedding-download-title"
        className="w-full max-w-md rounded-lg border border-card-border bg-surface p-5 shadow-xl shadow-black/20"
      >
        <div className="flex items-start gap-3">
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10">
            <Icon name="cloud_download" size={20} className="text-primary" />
          </span>
          <div className="min-w-0">
            <h4 id="local-embedding-download-title" className="font-body-lg font-semibold text-on-surface">
              下载本地向量模型
            </h4>
            <p className="mt-2 text-sm leading-relaxed text-on-surface-variant">
              首次启用需要下载约 490 MB 的中英双语模型。模型只保存在本机，下载完成后可断网检索。
            </p>
          </div>
        </div>
        <dl className="mt-4 grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 border-y border-outline-variant/30 py-3 text-sm">
          <dt className="text-on-surface-variant">模型</dt>
          <dd className="break-all font-medium text-on-surface">multilingual-e5-small</dd>
          <dt className="text-on-surface-variant">存储位置</dt>
          <dd className="font-medium text-on-surface">本机应用缓存</dd>
          <dt className="text-on-surface-variant">下载源</dt>
          <dd className="font-medium text-on-surface">
            {downloadSource === 'modelscope' ? 'ModelScope 国内源' : 'Hugging Face 官方源'}
          </dd>
          <dt className="text-on-surface-variant">数据传输</dt>
          <dd className="font-medium text-on-surface">仓库内容不会发送到远程服务</dd>
        </dl>
        <div className="mt-5 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <button
            type="button"
            onClick={onCancel}
            className="inline-flex items-center justify-center rounded-lg border border-outline-variant/40 bg-surface-container-low px-4 py-2 text-sm font-medium text-on-surface transition-colors hover:bg-surface-container-high"
          >
            暂不启用
          </button>
          <button
            type="button"
            onClick={onConfirm}
            autoFocus
            className="gsat-primary-button inline-flex items-center justify-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-primary-container"
          >
            <Icon name="cloud_download" size={17} />
            下载并启用
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

function AiRequestDiagnosticsPanel(props: {
  diagnostics: AiRequestDiagnostics | null;
  errorMessage: string | null;
  isLoading: boolean;
  testMessage: { type: 'success' | 'error'; text: string } | null;
  onRefresh: () => void;
}) {
  const diagnostics = props.diagnostics;
  const testFailure = props.testMessage?.type === 'error' ? props.testMessage.text : null;
  const failureReason = testFailure ?? diagnostics?.failureReason ?? props.errorMessage;
  const successMessage = props.testMessage?.type === 'success' ? props.testMessage.text : null;
  const statusClass = diagnostics?.status === 'ready'
    ? 'border-success/20 bg-success/10 text-success'
    : 'border-error/20 bg-error/10 text-error';

  return (
    <div className="border-t border-card-border py-4">
      <div className="mb-3 flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h4 className="font-body-lg font-medium text-on-surface">AI 请求详情</h4>
          <p className="font-body-md mt-1 text-sm text-on-surface-variant">
            显示这次请求最终用的参数，方便查模型、地址、超时和失败原因。
          </p>
        </div>
        <button
          type="button"
          onClick={props.onRefresh}
          disabled={props.isLoading}
          className="inline-flex shrink-0 items-center justify-center gap-2 rounded-lg border border-card-border bg-surface-container-high px-3 py-2 font-body-md text-sm text-on-surface transition-colors hover:bg-surface-container-highest disabled:opacity-60"
        >
          <Icon name={props.isLoading ? 'progress_activity' : 'refresh'} size={16} className={props.isLoading ? 'animate-spin' : ''} />
          {props.isLoading ? '读取中' : '重新读取'}
        </button>
      </div>

      {diagnostics ? (
        <>
          <div className={`mb-3 rounded-lg border px-3 py-2 font-body-md text-sm ${statusClass}`}>
            {diagnostics.status === 'ready'
              ? '设置已填完整，可以测试连接。'
              : diagnostics.failureReason ?? '设置还没填完整。'}
          </div>
          <dl className="grid gap-x-6 gap-y-0 overflow-hidden rounded-lg border border-card-border md:grid-cols-2">
            <AiDiagnosticsRow label="服务类型" value={diagnostics.provider} />
            <AiDiagnosticsRow label="请求协议" value={formatAiWireProtocol(diagnostics.wireProtocol)} />
            <AiDiagnosticsRow label="请求地址" value={diagnostics.baseUrl ?? '没填，用默认地址'} />
            <AiDiagnosticsRow label="实际请求的地址" value={diagnostics.endpoint ?? '还没生成'} />
            <AiDiagnosticsRow label="模型" value={diagnostics.model || '还没填'} />
            <AiDiagnosticsRow label="密钥" value={formatAiApiKeyState(diagnostics.apiKeyState)} />
            <AiDiagnosticsRow label="连接超时" value={`${diagnostics.connectTimeoutSeconds} 秒`} />
            <AiDiagnosticsRow label="请求超时" value={`${diagnostics.requestTimeoutSeconds} 秒`} />
            <AiDiagnosticsRow label="获取模型超时" value={`${diagnostics.modelListTimeoutSeconds} 秒`} />
            <AiDiagnosticsRow label="失败重试" value={`${diagnostics.retryAttempts} 次，间隔 ${diagnostics.retryBaseDelayMs}ms 起`} />
            <AiDiagnosticsRow label="项目说明读取上限" value={`${compactNumber(diagnostics.readmePromptCharLimit)} 字符`} />
            <AiDiagnosticsRow label="说明提示词版本" value={diagnostics.summaryPromptVersion} />
          </dl>
        </>
      ) : (
        <div className="rounded-lg border border-outline-variant/25 bg-surface-container-low px-3 py-3 font-body-md text-sm text-on-surface-variant">
          {props.errorMessage ?? '正在读取…'}
        </div>
      )}

      {failureReason && diagnostics?.status === 'ready' ? (
        <p className="mt-3 rounded-lg border border-error/20 bg-error/10 px-3 py-2 font-body-md text-sm text-error">
          最近失败原因：{failureReason}
        </p>
      ) : null}
      {successMessage ? (
        <p className="mt-3 rounded-lg border border-success/20 bg-success/10 px-3 py-2 font-body-md text-sm text-success">
          最近测试：{successMessage}
        </p>
      ) : null}
    </div>
  );
}

function AiDiagnosticsRow(props: { label: string; value: string }) {
  return (
    <div className="grid min-w-0 grid-cols-[112px_minmax(0,1fr)] gap-3 border-b border-card-border px-3 py-2.5 last:border-b-0 md:[&:nth-last-child(-n+2)]:border-b-0">
      <dt className="font-body-md text-xs font-medium text-on-surface-variant">{props.label}</dt>
      <dd className="min-w-0 break-words font-body-md text-xs text-on-surface" title={props.value}>
        {props.value}
      </dd>
    </div>
  );
}

function formatAiWireProtocol(value: string) {
  switch (value) {
    case 'openai':
      return 'OpenAI Chat Completions';
    case 'anthropic':
      return 'Anthropic Messages';
    default:
      return value || '未知';
  }
}

function formatAiApiKeyState(value: string) {
  switch (value) {
    case 'present':
      return '已读取';
    case 'missing':
      return '没有';
    case 'optional_for_local':
      return '本机兼容服务可留空';
    case 'not_required':
      return '不需要';
    default:
      return value || '未知';
  }
}

function getAiEndpointPlaceholder(provider: AISettingsValue['provider']) {
  switch (provider) {
    case 'anthropic':
      return 'https://api.anthropic.com/v1';
    case 'openai-compatible':
      return 'https://你的兼容服务/v1 或 http://127.0.0.1:11434/v1';
    case 'openai':
    default:
      return 'https://api.openai.com/v1';
  }
}

function AiTestDialog(props: {
  state: AiTestDialogState;
  isTesting: boolean;
  onClose: () => void;
}) {
  const state = props.state;
  const toneClass = state.status === 'success'
    ? 'border-success/20 bg-success/10 text-success'
    : state.status === 'error'
      ? 'border-error/20 bg-error/10 text-error'
      : 'border-primary/20 bg-primary/10 text-primary';
  const statusLabel = state.status === 'success'
    ? '测试通过'
    : state.status === 'error'
      ? '测试失败'
      : '测试中';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-[560px] rounded-xl border border-card-border bg-surface p-5 shadow-2xl">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h3 className="font-headline-md text-[20px] font-semibold text-on-surface">AI 连接测试</h3>
            <p className="mt-1 font-body-md text-sm text-on-surface-variant">
              正在用现在的设置真实调用一次 AI，会产生很少的费用。结果会显示用时和大概的输出速度。
            </p>
          </div>
          <button
            type="button"
            onClick={props.onClose}
            className="rounded-lg border border-card-border bg-surface-container-high px-3 py-1.5 font-body-md text-sm text-on-surface transition-colors hover:bg-surface-container-highest"
          >
            {props.isTesting ? '继续在后台等' : '关闭'}
          </button>
        </div>

        <div className={`mt-4 rounded-lg border px-3 py-2 font-body-md text-sm ${toneClass}`}>
          <div className="flex items-center gap-2 font-medium">
            {state.status === 'running' || state.status === 'preparing' ? (
              <Icon name="progress_activity" size={16} className="animate-spin" />
            ) : (
              <Icon name={state.status === 'success' ? 'check_circle' : 'error'} size={16} />
            )}
            {statusLabel}：{state.stage}
          </div>
        </div>

        <dl className="mt-4 grid gap-3 rounded-lg border border-outline-variant/30 bg-surface-container-low p-3 font-body-md text-sm sm:grid-cols-2">
          <AiTestMetric label="服务" value={state.providerLabel} />
          <AiTestMetric label="模型" value={state.model} />
          <AiTestMetric label="请求地址" value={state.endpoint} wide />
          <AiTestMetric label="开始回复用时" value={formatLatency(state.firstTokenLatencyMs)} />
          <AiTestMetric label="总用时" value={formatLatency(state.totalLatencyMs)} />
          <AiTestMetric label="输出量（token）" value={state.outputTokens === null ? '等待结果' : String(state.outputTokens)} />
          <AiTestMetric label="输出速度" value={formatTokensPerSecond(state.tokensPerSecond)} />
          <AiTestMetric label="最长等待" value="约 120 秒" />
        </dl>

        {state.summary && (
          <div className="mt-4 rounded-lg border border-success/20 bg-success/10 px-3 py-2">
            <p className="font-body-md text-xs font-medium text-success">AI 回复的测试内容</p>
            <p className="mt-1 font-body-md text-sm leading-relaxed text-on-surface">{state.summary}</p>
          </div>
        )}

        {state.error && (
          <div className="mt-4 rounded-lg border border-error/20 bg-error/10 px-3 py-2">
            <p className="font-body-md text-xs font-medium text-error">失败原因</p>
            <p className="mt-1 font-body-md text-sm leading-relaxed text-error">{state.error}</p>
          </div>
        )}
      </div>
    </div>
  );
}

function AiTestMetric(props: { label: string; value: string; wide?: boolean }) {
  return (
    <div className={props.wide ? 'min-w-0 sm:col-span-2' : 'min-w-0'}>
      <dt className="font-body-md text-xs text-on-surface-variant">{props.label}</dt>
      <dd className="mt-1 truncate font-body-md text-sm font-medium text-on-surface" title={props.value}>
        {props.value}
      </dd>
    </div>
  );
}

function formatLatency(value: number | null) {
  if (value === null) {
    return '等待结果';
  }
  if (value < 1000) {
    return `${Math.round(value)} ms`;
  }
  return `${(value / 1000).toFixed(2)} 秒`;
}

function formatTokensPerSecond(value: number | null) {
  if (value === null || !Number.isFinite(value)) {
    return '等待结果';
  }
  return `${value.toFixed(1)} token/s`;
}

function estimateTextTokens(value: string) {
  return Math.max(1, Math.ceil(Array.from(value).length / 4));
}

function getAiProviderPreset(providerPreset: AIProviderPreset): AIProviderPresetOption {
  return AI_PROVIDER_PRESETS.find((preset) => preset.id === providerPreset)
    ?? AI_PROVIDER_PRESETS.find((preset) => preset.id === 'custom-openai-compatible')
    ?? AI_PROVIDER_PRESETS[0];
}

function mergeAiModelOptions(
  preset: AIProviderPresetOption,
  availableModels: AiModelOption[],
  currentModel: string,
) {
  const seen = new Set<string>();
  const options: AiModelOption[] = [];

  for (const id of preset.modelSuggestions) {
    if (!seen.has(id)) {
      seen.add(id);
      options.push({ id });
    }
  }

  for (const model of normalizeAiModelOptions(availableModels)) {
    if (!seen.has(model.id)) {
      seen.add(model.id);
      options.push(model);
    }
  }

  const normalizedCurrentModel = currentModel.trim();
  if (normalizedCurrentModel && !seen.has(normalizedCurrentModel)) {
    options.unshift({ id: normalizedCurrentModel });
  }

  return options;
}

function normalizeAiModelOptions(models: AiModelOption[]) {
  const seen = new Set<string>();
  const normalizedModels: AiModelOption[] = [];

  for (const model of models) {
    const id = model.id.trim();
    if (!id || seen.has(id)) {
      continue;
    }
    seen.add(id);
    normalizedModels.push({
      id,
      displayName: model.displayName?.trim() || null,
      ownedBy: model.ownedBy?.trim() || null,
    });
  }

  return normalizedModels.sort((left, right) => left.id.localeCompare(right.id));
}

function formatAiModelOptionLabel(model: AiModelOption) {
  const meta = model.displayName || model.ownedBy;
  return meta && meta !== model.id ? `${model.id} · ${meta}` : model.id;
}

function getAiApiKeyPlaceholder(provider: AISettingsValue['provider']) {
  switch (provider) {
    case 'anthropic':
      return 'sk-ant-...';
    case 'openai-compatible':
      return '本机服务可留空，云端服务填写密钥';
    case 'openai':
    default:
      return 'sk-...';
  }
}

function getAiModelPlaceholder(provider: AISettingsValue['provider']) {
  switch (provider) {
    case 'anthropic':
      return 'claude-sonnet-5';
    case 'openai-compatible':
      return '服务商给你的模型名';
    case 'openai':
    default:
      return 'gpt-5.5';
  }
}

function getAiKeySaveMessage(status: ReturnType<typeof useAppSettings>['aiKeySaveStatus']) {
  return aiKeySaveStatusText(status);
}

/* === 通用设置 === */
function GeneralSettings({
  settingsHook,
  workspace,
}: {
  settingsHook: ReturnType<typeof useAppSettings>;
  workspace: ReturnType<typeof useWorkspace>;
}) {
  const [isClearingAllLocalData, setIsClearingAllLocalData] = useState(false);
  const [isClearDataConfirmOpen, setIsClearDataConfirmOpen] = useState(false);
  const [localDataMessage, setLocalDataMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  async function handleClearAllLocalData() {
    setIsClearingAllLocalData(true);
    setLocalDataMessage(null);
    try {
      await settingsHook.resetSettings();
      await workspace.handleClearLocalData();
      setIsClearDataConfirmOpen(false);
      setLocalDataMessage({ type: 'success', text: CLEAR_LOCAL_DATA_DONE });
    } catch {
      setLocalDataMessage({ type: 'error', text: CLEAR_LOCAL_DATA_FAILED });
    } finally {
      setIsClearingAllLocalData(false);
    }
  }

  return (
    <section className="glass-panel rounded-xl p-6">
      <h3 className="font-headline-md text-[20px] font-semibold text-on-surface mb-6">通用</h3>
      <div className="space-y-6">
        <div className="flex flex-col gap-3 py-4 md:flex-row md:items-center md:justify-between">
          <div className="min-w-0 flex-1 md:pr-6">
            <h4 className="font-body-lg font-medium text-on-surface">界面主题</h4>
            <p className="font-body-md text-on-surface-variant mt-1">选择浅色、深色，或跟随系统外观自动切换。</p>
          </div>
          <div className="flex w-full rounded-lg border border-outline-variant/30 bg-surface-container-low p-1 md:w-auto">
            {[
              { value: 'system', label: '系统' },
              { value: 'light', label: '浅色' },
              { value: 'dark', label: '深色' },
            ].map((item) => (
              <button
                key={item.value}
                type="button"
                onClick={() => void settingsHook.updateTheme({ mode: item.value as ThemeSettingsValue['mode'] })}
                className={`flex-1 rounded-md px-3 py-1.5 text-sm transition-colors md:flex-none ${
                  settingsHook.settings.theme.mode === item.value
                    ? 'bg-primary text-white'
                    : 'text-on-surface-variant hover:text-on-surface'
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>
        <div>
          <label className="font-body-md text-on-surface font-medium mb-2 block">品牌色</label>
          <div className="flex gap-2">
            {COLOR_PRESETS.map((preset) => (
              <button
                key={preset.name}
                type="button"
                title={preset.label}
                onClick={() => void settingsHook.updateTheme({ brandColor: preset.color, colorPreset: preset.name })}
                className={`w-10 h-10 rounded-full border-2 transition-transform hover:scale-110 ${
                  settingsHook.settings.theme.brandColor === preset.color
                    ? 'border-on-surface scale-110'
                    : 'border-transparent'
                }`}
                style={{ backgroundColor: preset.color }}
              />
            ))}
          </div>
        </div>
        <div className="flex flex-col gap-3 border-t border-card-border py-4 md:flex-row md:items-center md:justify-between">
          <div className="min-w-0 flex-1 md:pr-6">
            <h4 className="font-body-lg font-medium text-on-surface">界面字号</h4>
            <p className="font-body-md text-on-surface-variant mt-1">调整主要文字、标题和标签的显示尺寸。</p>
          </div>
          <div className="flex w-full rounded-lg border border-outline-variant/30 bg-surface-container-low p-1 md:w-auto">
            {[
              { value: 'small', label: '小' },
              { value: 'medium', label: '中' },
              { value: 'large', label: '大' },
            ].map((item) => (
              <button
                key={item.value}
                type="button"
                onClick={() => void settingsHook.updateTheme({ fontSize: item.value as ThemeSettingsValue['fontSize'] })}
                className={`flex-1 rounded-md px-3 py-1.5 text-sm transition-colors md:flex-none ${
                  settingsHook.settings.theme.fontSize === item.value
                    ? 'bg-primary text-white'
                    : 'text-on-surface-variant hover:text-on-surface'
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>
        <div className="flex items-start justify-between gap-4 border-t border-card-border py-4">
          <div className="min-w-0 flex-1">
            <h4 className="font-body-lg font-medium text-on-surface">启动时显示欢迎引导</h4>
            <p className="font-body-md text-on-surface-variant mt-1">{WELCOME_STARTUP_HINT}</p>
          </div>
          <ToggleSwitch
            checked={settingsHook.settings.general.showWelcomeOnStartup}
            onChange={(checked) => void settingsHook.updateGeneral({ showWelcomeOnStartup: checked })}
          />
        </div>
        <div className="flex items-start justify-between gap-4 border-t border-card-border py-4">
          <div className="min-w-0 flex-1">
            <h4 className="font-body-lg font-medium text-on-surface">自动同步</h4>
            <p className="font-body-md text-on-surface-variant mt-1">
              {autoSyncDescription(settingsHook.settings.sync.autoSyncInterval)}
            </p>
          </div>
          <ToggleSwitch
            checked={settingsHook.settings.sync.enableAutoSync}
            onChange={(checked) => void settingsHook.updateSync({ enableAutoSync: checked })}
          />
        </div>
        {settingsHook.settings.sync.enableAutoSync && (
          <div className="flex flex-col gap-3 border-t border-card-border py-4 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0 flex-1 sm:pr-6">
              <h4 className="font-body-lg font-medium text-on-surface">同步间隔</h4>
              <p className="font-body-md text-on-surface-variant mt-1">{SYNC_INTERVAL_HINT}</p>
            </div>
            <div className="flex items-center gap-2">
              <input
                type="number"
                min={5}
                max={1440}
                step={5}
                value={settingsHook.settings.sync.autoSyncInterval}
                onChange={(event) =>
                  void settingsHook.updateSync({ autoSyncInterval: normalizeSyncIntervalInput(event.target.value) })
                }
                className="w-24 rounded-lg border border-outline-variant bg-surface-container-low px-3 py-1.5 text-sm text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
              />
              <span className="text-sm text-on-surface-variant">分钟</span>
            </div>
          </div>
        )}
        <div className="border-t border-card-border pt-5">
          <div className="mb-5 rounded-xl border border-error/25 bg-error/10 p-4">
            <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <Icon name="delete" size={20} className="text-error" />
                  <h4 className="font-body-lg font-semibold text-on-surface">清空本机数据</h4>
                </div>
                <p className="mt-2 max-w-2xl font-body-md text-sm leading-relaxed text-on-surface-variant">
                  {CLEAR_LOCAL_DATA_WARNING}
                </p>
                {localDataMessage && (
                  <p
                    className={`mt-3 text-sm ${
                      localDataMessage.type === 'success' ? 'text-success' : 'text-error'
                    }`}
                  >
                    {localDataMessage.text}
                  </p>
                )}
              </div>
              <button
                type="button"
                onClick={() => {
                  setLocalDataMessage(null);
                  setIsClearDataConfirmOpen(true);
                }}
                disabled={isClearingAllLocalData || workspace.isClearingLocalData}
                className="inline-flex items-center justify-center gap-2 rounded-lg border border-error/30 bg-error px-4 py-2 text-sm font-semibold text-on-error transition-colors hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
              >
                <Icon name="delete" size={17} />
                {isClearingAllLocalData || workspace.isClearingLocalData ? '正在清空' : '清空本机数据'}
              </button>
            </div>
          </div>
          {isClearDataConfirmOpen && (
            <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 px-4 py-6 backdrop-blur-sm">
              <div
                role="dialog"
                aria-modal="true"
                aria-labelledby="clear-local-data-title"
                className="w-full max-w-md rounded-lg border border-error/25 bg-surface p-5 shadow-xl shadow-black/20"
              >
                <div className="flex items-start gap-3">
                  <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-error/10">
                    <Icon name="delete" size={20} className="text-error" />
                  </span>
                  <div className="min-w-0">
                    <h4 id="clear-local-data-title" className="font-body-lg font-semibold text-on-surface">
                      确认清空本机数据
                    </h4>
                    <p className="mt-2 text-sm leading-relaxed text-on-surface-variant">
                      {CLEAR_LOCAL_DATA_WARNING}
                    </p>
                  </div>
                </div>
                <div className="mt-5 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
                  <button
                    type="button"
                    onClick={() => setIsClearDataConfirmOpen(false)}
                    disabled={isClearingAllLocalData || workspace.isClearingLocalData}
                    className="inline-flex items-center justify-center rounded-lg border border-outline-variant/40 bg-surface-container-low px-4 py-2 text-sm font-medium text-on-surface transition-colors hover:bg-surface-container-high disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    取消
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleClearAllLocalData()}
                    disabled={isClearingAllLocalData || workspace.isClearingLocalData}
                    className="inline-flex items-center justify-center gap-2 rounded-lg bg-error px-4 py-2 text-sm font-semibold text-on-error transition-colors hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    <Icon name="delete" size={17} />
                    {isClearingAllLocalData || workspace.isClearingLocalData ? '正在清空' : '确认清空'}
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function AppIdentityDetails({ identity, errorMessage }: { identity: AppIdentity | null; errorMessage: string | null }) {
  const identityRows = identity ? [
    { label: '显示名称', value: identity.displayName },
    { label: 'Bundle ID', value: identity.bundleIdentifier },
    { label: 'App Bundle', value: identity.appBundleName },
    { label: '安装包命名', value: identity.dmgFilePattern },
    { label: '配置文件夹', value: identity.configDir },
    { label: '资料库文件', value: identity.databasePath },
    { label: '设置文件', value: identity.settingsPath },
    { label: '钥匙串服务名', value: identity.credentialService },
    { label: 'GitHub 备份文件名', value: identity.annotationGistFile },
    { label: '更新地址', value: identity.updateEndpoint },
    { label: '项目的 GitHub 页面', value: identity.repositoryUrl },
  ] : [];

  return (
    <div className="mt-4 border-t border-outline-variant/25 pt-4">
      <div className="mb-3 flex items-center gap-2">
        <Icon name="fingerprint" size={19} className="text-primary" />
        <h5 className="font-body-md font-semibold text-on-surface">个人版身份与本机位置</h5>
      </div>
      {errorMessage ? (
        <p className="rounded-lg border border-error/20 bg-error/10 px-3 py-2 text-sm text-error">
          没读出来。你的资料都还在。重启应用再试；还不行就把这条截图发给开发者。
        </p>
      ) : null}
      {!identity && !errorMessage ? (
        <p className="rounded-lg border border-outline-variant/25 bg-surface px-3 py-2 text-sm text-on-surface-variant">
          正在读取…
        </p>
      ) : null}
      {identity ? (
        <dl className="grid gap-x-4 gap-y-3 text-sm md:grid-cols-2">
          {identityRows.map((row) => (
            <IdentityField key={row.label} label={row.label} value={row.value} />
          ))}
        </dl>
      ) : null}
    </div>
  );
}

function IdentityField({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 border-b border-outline-variant/20 pb-2">
      <dt className="text-xs font-medium text-on-surface-variant">{label}</dt>
      <dd className="mt-1 break-words font-mono text-[12px] leading-5 text-on-surface [overflow-wrap:anywhere]" title={value}>
        {value}
      </dd>
    </div>
  );
}

function AboutSettings({ appUpdate }: { appUpdate: AppUpdateContextValue }) {
  const [appIdentity, setAppIdentity] = useState<AppIdentity | null>(null);
  const [appIdentityError, setAppIdentityError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<AppIdentity>('get_app_identity')
      .then((identity) => {
        if (!cancelled) {
          setAppIdentity(identity);
          setAppIdentityError(null);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setAppIdentity(null);
          setAppIdentityError(toErrorMessage(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <section className="glass-panel rounded-xl p-6">
      <h3 className="mb-6 font-headline-md text-[20px] font-semibold text-on-surface">关于</h3>
      <div className="space-y-5">
        <div className="rounded-xl border border-outline-variant/30 bg-surface-container-low p-4">
          <div className="flex items-center gap-2">
            <Icon name="stars" size={20} className="shrink-0 text-primary" />
            <h4 className="min-w-0 font-body-lg font-semibold text-on-surface">Fox Stars Lab</h4>
          </div>
          <dl className="mt-4 grid gap-x-4 gap-y-3 text-sm md:grid-cols-2">
            <IdentityField label="版本号" value={appUpdate.currentVersion || '读取中'} />
            <IdentityField
              label="数据放在哪个文件夹"
              value={appIdentity?.dataDir ?? (appIdentityError ? '没读出来。你的资料都还在。' : '正在读取…')}
            />
            <div className="min-w-0 border-b border-outline-variant/20 pb-2 md:col-span-2">
              <dt className="text-xs font-medium text-on-surface-variant">许可证</dt>
              <dd className="mt-1 text-sm leading-relaxed text-on-surface">
                非商用许可：个人学习、研究和非营利用途可以使用；商业使用、集成或再分发需要另行获得授权。
                <a
                  href={PROJECT_LICENSE_URL}
                  target="_blank"
                  rel="noreferrer"
                  className="ml-2 text-primary hover:underline"
                >
                  查看许可证全文
                </a>
              </dd>
            </div>
          </dl>
        </div>
        <AppUpdatePanel appUpdate={appUpdate} />
        <details className="rounded-xl border border-outline-variant/30 bg-surface-container-low px-4 py-3">
          <summary className="cursor-pointer font-body-md text-sm font-medium text-on-surface">开发者信息</summary>
          <p className="mt-3 max-w-2xl font-body-md text-sm leading-relaxed text-on-surface-variant">
            这是 Fox 长期使用的个人定制版，应用身份、数据文件夹和钥匙串条目已与原版隔离。许可证全称：PolyForm Noncommercial 1.0.0。
          </p>
          <div className="mt-3 flex flex-wrap gap-2 text-sm">
            <a
              href={PROJECT_REPOSITORY_URL}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1.5 rounded-lg border border-outline-variant/35 bg-surface px-3 py-1.5 text-on-surface transition-colors hover:bg-surface-container-high"
            >
              <Icon name="code" size={16} />
              项目的 GitHub 页面
              <Icon name="open_in_new" size={14} />
            </a>
            <a
              href={PROJECT_ISSUES_URL}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1.5 rounded-lg border border-outline-variant/35 bg-surface px-3 py-1.5 text-on-surface transition-colors hover:bg-surface-container-high"
            >
              <Icon name="bug_report" size={16} />
              问题反馈
              <Icon name="open_in_new" size={14} />
            </a>
            <a
              href={PROJECT_ACKNOWLEDGEMENTS_URL}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1.5 rounded-lg border border-outline-variant/35 bg-surface px-3 py-1.5 text-on-surface transition-colors hover:bg-surface-container-high"
            >
              <Icon name="diversity_3" size={16} />
              开源组件鸣谢
              <Icon name="open_in_new" size={14} />
            </a>
          </div>
          <AppIdentityDetails identity={appIdentity} errorMessage={appIdentityError} />
        </details>
      </div>
    </section>
  );
}

function normalizeSyncIntervalInput(value: string) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return 30;
  }

  return Math.min(Math.max(parsed, 5), 1440);
}

/* === GitHub 备份设置 === */
function BackupSettings({ workspace }: { workspace: ReturnType<typeof useWorkspace> }) {
  const isConnected = workspace.authState.hasToken && Boolean(workspace.authState.user);
  const isAnyBackupActionRunning =
    workspace.isExportingAnnotations ||
    workspace.isImportingAnnotations ||
    workspace.isExportingRepositoryLibrary ||
    workspace.isImportingRepositoryLibrary;
  // 覆盖标记的警告一直显示在导入区上面；这里只说「为什么现在点不了」
  const importHint = gistImportHint(isConnected, Boolean(workspace.gistIdDraft.trim()));

  return (
    <section className="glass-panel rounded-xl p-6">
      <h3 className="font-headline-md text-[20px] font-semibold text-on-surface mb-6">{GIST_SECTION_TITLE}</h3>
      <div className="space-y-6">
        {!isConnected && (
          <p className="rounded-lg border border-error/20 bg-error/10 px-3 py-2 font-body-md text-sm text-error">
            {GIST_NEEDS_SCOPE}
          </p>
        )}
        {workspace.authMessage && (
          <p className="rounded-lg border border-success/20 bg-success/10 px-3 py-2 font-body-md text-sm text-success">
            {workspace.authMessage}
          </p>
        )}
        {workspace.error && (
          <p className="rounded-lg border border-error/20 bg-error/10 px-3 py-2 font-body-md text-sm text-error">
            {workspace.error}
          </p>
        )}
        <div className="p-4 rounded-lg bg-surface-container-low border border-card-border">
          <h4 className="font-body-lg font-medium text-on-surface mb-2 flex items-center gap-2">
            <Icon name="cloud_upload" size={20} className="text-primary" />
            {GIST_BACKUP_MARKS.title}
          </h4>
          <p className="font-body-md text-sm text-on-surface-variant mb-3">
            {GIST_BACKUP_MARKS.body}
          </p>
          <button
            onClick={() => void workspace.handleExportAnnotations()}
            disabled={isAnyBackupActionRunning || !isConnected}
            title={isConnected ? '把标记、标签和笔记上传到你的私密 Gist' : '请先连接 GitHub 账号'}
            className="px-4 py-2 bg-primary text-white rounded-lg font-body-md flex items-center gap-2 hover:brightness-110 transition-all shadow-sm disabled:opacity-60"
          >
            <Icon name="upload" size={18} />
            {workspace.isExportingAnnotations ? '上传中…' : GIST_BACKUP_MARKS.title}
          </button>
        </div>
        <div className="p-4 rounded-lg bg-surface-container-low border border-card-border">
          <h4 className="font-body-lg font-medium text-on-surface mb-2 flex items-center gap-2">
            <Icon name="inventory_2" size={20} className="text-primary" />
            {GIST_BACKUP_LIBRARY.title}
          </h4>
          <p className="font-body-md text-sm text-on-surface-variant mb-3">
            {GIST_BACKUP_LIBRARY.body}
          </p>
          <button
            onClick={() => void workspace.handleExportRepositoryLibrary()}
            disabled={isAnyBackupActionRunning || !isConnected}
            title={isConnected ? '把整个清单（含标记、标签和笔记）上传到你的私密 Gist' : '请先连接 GitHub 账号'}
            className="px-4 py-2 bg-primary text-white rounded-lg font-body-md flex items-center gap-2 hover:brightness-110 transition-all shadow-sm disabled:opacity-60"
          >
            <Icon name="upload" size={18} />
            {workspace.isExportingRepositoryLibrary ? '上传中…' : GIST_BACKUP_LIBRARY.title}
          </button>
        </div>
        <div className="p-4 rounded-lg bg-surface-container-low border border-card-border">
          <h4 className="font-body-lg font-medium text-on-surface mb-2 flex items-center gap-2">
            <Icon name="cloud_download" size={20} className="text-primary" />
            从 GitHub 导入备份
          </h4>
          <p className="font-body-md text-sm text-on-surface-variant mb-3">
            填入上传备份时得到的 Gist ID（备份的编号），再选要导入什么。导入整个清单时，这台电脑上还没有的收藏会补上。
          </p>
          <p className="mb-3 rounded-lg border border-warning/25 bg-warning/10 px-3 py-2 font-body-md text-xs text-warning">
            {GIST_IMPORT_WARNING}
          </p>
          <form
            onSubmit={(event) => void workspace.handleImportAnnotations(event)}
            className="space-y-3"
          >
            <input
              type="text"
              value={workspace.gistIdDraft}
              onChange={(e) => workspace.setGistIdDraft(e.target.value)}
              placeholder="粘贴 Gist ID"
              className="w-full bg-surface border border-outline-variant rounded-lg px-4 py-2 font-body-md text-on-surface focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
            />
            <div className="flex flex-col gap-2 sm:flex-row sm:gap-3">
              <button
                type="submit"
                disabled={isAnyBackupActionRunning || !workspace.gistIdDraft.trim() || !isConnected}
                title={isConnected ? '从 Gist 恢复这台电脑上已有工具的标记、标签和笔记' : '请先连接 GitHub 账号'}
                className="px-4 py-2 bg-surface-container-high hover:bg-surface-container-highest text-on-surface rounded-lg border border-card-border font-body-md transition-all disabled:opacity-60"
              >
                {workspace.isImportingAnnotations ? '导入中…' : '导入我的标记'}
              </button>
              <button
                type="button"
                onClick={(event) => void workspace.handleImportRepositoryLibrary(event)}
                disabled={isAnyBackupActionRunning || !workspace.gistIdDraft.trim() || !isConnected}
                title={isConnected ? '从 Gist 恢复整个清单（含标记、标签和笔记）' : '请先连接 GitHub 账号'}
                className="px-4 py-2 bg-surface-container-high hover:bg-surface-container-highest text-on-surface rounded-lg border border-card-border font-body-md transition-all disabled:opacity-60"
              >
                {workspace.isImportingRepositoryLibrary ? '导入中…' : '导入整个清单'}
              </button>
            </div>
          </form>
          {importHint && (
            <p className={`mt-2 font-body-md text-xs ${isConnected ? 'text-on-surface-variant' : 'text-error'}`}>
              {importHint}
            </p>
          )}
        </div>
      </div>
    </section>
  );
}

/* === Toggle Switch 组件 === */
function ToggleSwitch({
  checked,
  disabled = false,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  onChange?: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => {
        if (disabled) return;
        const next = !checked;
        onChange?.(next);
      }}
      className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 shrink-0 ${
        checked ? 'bg-primary' : 'bg-outline-variant'
      } disabled:cursor-not-allowed disabled:opacity-50`}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
          checked ? 'translate-x-6' : 'translate-x-1'
        }`}
      />
    </button>
  );
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
