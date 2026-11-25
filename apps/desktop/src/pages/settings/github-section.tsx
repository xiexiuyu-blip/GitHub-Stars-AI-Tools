import { useEffect, useState, type FormEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { useAppSettings } from '@/providers/settings-provider';
import type { useWorkspace } from '@/providers/workspace-provider';
import { Icon } from '@/components/ui/icon';
import { getAiConfigMessage, shouldFlushAiApiKey } from '@/lib/ai-config';
import {
  autoSyncDescription,
  buildReadinessRows,
  CONNECT_SAVING_KEYCHAIN,
  CONNECT_SLOW,
  CONNECT_VERIFYING,
  formatMinute,
  formatSyncComplete,
  GITHUB_TOKEN_EXPLAINER,
  GITHUB_TOKEN_SAVED_NOTE,
  GITHUB_TOKEN_SCOPES,
  RUNTIME_CHECK_COST_NOTE,
  RUNTIME_CHECK_TITLES,
  SYNC_INTERVAL_HINT,
} from '@/lib/automation-view';
import { toBackendAiRequestConfig } from '@/lib/ai-config';
import type { DashboardStats, RuntimeReadinessCheckItem, RuntimeReadinessCheckResult } from '@/types';
import type { RuntimeSelfCheckRecord } from '@/types-settings';
import { normalizeSyncIntervalInput, ToggleSwitch, toErrorMessage } from './shared';

export function GitHubSettings({
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
        if (!cancelled) setReadinessStats(stats);
      })
      .catch((error) => {
        if (!cancelled) {
          setReadinessStats(null);
          setReadinessError(toErrorMessage(error));
        }
      })
      .finally(() => {
        if (!cancelled) setIsLoadingReadiness(false);
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
    if (workspace.isSavingToken || !workspace.token.trim()) return;

    const statusTimers: number[] = [];
    setConnectStatus(CONNECT_VERIFYING);
    try {
      statusTimers.push(window.setTimeout(() => setConnectStatus(CONNECT_SAVING_KEYCHAIN), 1200));
      statusTimers.push(window.setTimeout(() => setConnectStatus(`${CONNECT_SLOW}。`), 8000));
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
    <div className="space-y-6">
      {/* 连接状态卡片 */}
      <section className="glass-panel relative overflow-hidden rounded-xl p-6">
        <div className="pointer-events-none absolute top-0 right-0 p-6 opacity-10">
          <Icon name="cloud_sync" size={96} />
        </div>
        <div className="relative z-10 flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="min-w-0">
            <h3 className="font-headline-md mb-1 text-[20px] font-semibold text-on-surface">GitHub 连接状态</h3>
            <p className="font-body-md mb-6 text-on-surface-variant">
              {GITHUB_TOKEN_EXPLAINER}{GITHUB_TOKEN_SCOPES}
            </p>
            <div
              className={`flex w-fit max-w-full flex-wrap items-center gap-3 rounded-lg border px-4 py-2.5 ${
                isConnected ? 'border-success/20 bg-success/10' : 'border-error/20 bg-error/10'
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
              <span className="font-label-sm text-label-sm min-w-0 truncate text-on-surface" title={`@${user.login}`}>
                @{user.login}
              </span>
            </div>
          )}
        </div>
      </section>

      {/* 访问配置 */}
      <section className="glass-panel overflow-hidden rounded-xl p-0">
        <div className="border-b border-card-border bg-surface/50 p-6">
          <h3 className="font-headline-md text-[20px] font-semibold text-on-surface">访问配置</h3>
        </div>
        <div className="space-y-6 p-6">
          {isConnected ? (
            <div className="space-y-2">
              <label className="font-body-md flex items-center gap-2 font-medium text-on-surface">
                GitHub 令牌
                <Icon name="info" size={16} className="cursor-help text-on-surface-variant" />
              </label>
              <div className="flex gap-3">
                <div className="flex-1 rounded-lg border border-outline-variant bg-surface-container-low px-4 py-2">
                  <p className="font-label-sm text-on-surface">{GITHUB_TOKEN_SAVED_NOTE}</p>
                </div>
                <button
                  type="button"
                  onClick={() => void workspace.handleClearToken()}
                  disabled={workspace.isClearingToken}
                  className="font-body-md rounded-lg border border-card-border bg-surface-container-high px-4 py-2 text-on-surface transition-all hover:bg-surface-container-highest"
                >
                  断开连接
                </button>
              </div>
            </div>
          ) : (
            <form onSubmit={(event) => void handleGitHubConnectSubmit(event)} className="space-y-2">
              <label className="font-body-md flex items-center gap-2 font-medium text-on-surface">
                GitHub 令牌
                <Icon name="info" size={16} className="cursor-help text-on-surface-variant" />
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
                  className="font-label-sm flex-1 rounded-lg border border-outline-variant bg-surface-container-low px-4 py-2 text-on-surface focus:border-primary focus:ring-1 focus:ring-primary focus:outline-none"
                />
                <button
                  type="submit"
                  disabled={workspace.isSavingToken || !workspace.token.trim()}
                  className="font-body-md rounded-lg bg-primary px-4 py-2 text-white shadow-sm transition-all hover:brightness-110 disabled:opacity-60"
                >
                  {workspace.isSavingToken ? '连接中…' : '连接 GitHub'}
                </button>
              </div>
            </form>
          )}

          {workspace.authMessage && (
            <p className="font-body-md flex items-center gap-1 text-[13px] text-success">
              <Icon name="check_circle" size={16} /> {workspace.authMessage}
            </p>
          )}
          {connectStatus && (
            <p className="font-body-md flex items-center gap-1 rounded-lg border border-primary/20 bg-primary/10 px-3 py-2 text-[13px] text-primary">
              <Icon name="progress_activity" size={16} className="animate-spin" /> {connectStatus}
            </p>
          )}
          {workspace.error && (
            <p className="font-body-md flex items-center gap-1 rounded-lg border border-error/20 bg-error/10 px-3 py-2 text-[13px] text-error">
              <Icon name="error" size={16} /> {workspace.error}
            </p>
          )}
        </div>
      </section>

      {/* 同步与项目说明 */}
      <section className="glass-panel rounded-lg p-6">
        <h3 className="font-headline-md mb-1 text-[20px] font-semibold text-on-surface">同步收藏</h3>
        <p className="font-body-md mb-4 text-sm text-on-surface-variant">
          只从 GitHub 读取你收藏的项目和项目说明（README），不会自动调用付费 AI。
        </p>
        <div className="flex flex-wrap gap-3">
          <button
            type="button"
            onClick={() => void workspace.handleSyncStars()}
            disabled={workspace.isSyncingStars || !isConnected}
            className="font-body-md flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-white shadow-sm transition-all hover:brightness-110 disabled:opacity-60"
          >
            <Icon name="sync" size={18} className={workspace.isSyncingStars ? 'animate-spin' : ''} />
            {workspace.isSyncingStars ? '同步中…' : '同步我的收藏'}
          </button>
          <button
            type="button"
            onClick={() => {
              void workspace.handleFetchReadmes({
                aiConfig: settingsHook.settings.ai,
                autoGenerateAi: false,
                onlyMissing: true,
              }).catch(() => undefined);
            }}
            disabled={workspace.isFetchingReadmes || !isConnected}
            className="font-body-md flex items-center gap-2 rounded-lg border border-card-border bg-surface-container-high px-4 py-2 text-on-surface transition-all hover:bg-surface-container-highest disabled:opacity-60"
          >
            <Icon name="description" size={18} className={workspace.isFetchingReadmes ? 'animate-spin' : ''} />
            {workspace.isFetchingReadmes ? '下载中…' : '下载项目说明'}
          </button>
        </div>

        <div className="mt-5 space-y-4 border-t border-card-border pt-4">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0 flex-1">
              <h4 className="font-body-lg font-medium text-on-surface">自动同步</h4>
              <p className="font-body-md mt-1 text-on-surface-variant">
                {autoSyncDescription(settingsHook.settings.sync.autoSyncInterval)}
              </p>
            </div>
            <ToggleSwitch
              checked={settingsHook.settings.sync.enableAutoSync}
              onChange={(checked) => void settingsHook.updateSync({ enableAutoSync: checked })}
            />
          </div>
          {settingsHook.settings.sync.enableAutoSync && (
            <div className="flex flex-col gap-3 border-t border-card-border pt-4 sm:flex-row sm:items-center sm:justify-between">
              <div className="min-w-0 flex-1 sm:pr-6">
                <h4 className="font-body-lg font-medium text-on-surface">同步间隔</h4>
                <p className="font-body-md mt-1 text-on-surface-variant">{SYNC_INTERVAL_HINT}</p>
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
                  className="w-24 rounded-lg border border-outline-variant bg-surface-container-low px-3 py-1.5 text-sm text-on-surface focus:border-primary focus:ring-1 focus:ring-primary focus:outline-none"
                />
                <span className="text-sm text-on-surface-variant">分钟</span>
              </div>
            </div>
          )}
        </div>

        {workspace.syncSummary && (
          <div className="mt-4 rounded-lg border border-success/20 bg-success/10 p-3">
            <p className="font-body-md text-sm text-success">
              {formatSyncComplete(workspace.syncSummary)}
            </p>
          </div>
        )}
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
    </div>
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
    ? { completedAt: props.runtimeCheckCompletedAt ?? '刚刚完成', ...runtimeCheckSummary }
    : props.lastSelfCheckRecord;
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
          <p className="font-body-md mt-1 text-sm text-on-surface-variant">
            看看 GitHub 连接、同步、AI 是不是都正常。
          </p>
          <p className="font-body-md mt-1 text-xs text-on-surface-variant">{RUNTIME_CHECK_COST_NOTE}</p>
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
          className="font-body-md inline-flex items-center gap-1.5 rounded-lg border border-primary/25 bg-primary/10 px-3 py-1.5 text-xs font-medium text-primary transition-colors hover:bg-primary/15 disabled:opacity-60"
        >
          <Icon name="verified" size={14} className={props.isCheckingRuntime ? 'animate-spin' : ''} />
          {props.isCheckingRuntime ? '正在检查…' : '开始检查'}
        </button>
      </div>
      {props.errorMessage && (
        <p className="font-body-md mt-4 rounded-lg border border-error/20 bg-error/10 px-3 py-2 text-sm text-error">
          没能读出现在的状态。你的资料都还在。再点一次「开始检查」；还不行就重启应用，再不行把这条截图发给开发者。
        </p>
      )}
      {props.warningMessage && (
        <p className="font-body-md mt-4 rounded-lg border border-warning/25 bg-warning/10 px-3 py-2 text-sm text-warning">
          {props.warningMessage}
        </p>
      )}
      <div className="mt-5 grid gap-3 sm:grid-cols-2">
        {readinessRows.map((row) => (
          <div
            key={row.key}
            className={`rounded-lg border px-4 py-3 ${
              row.done ? 'border-success/20 bg-success/10' : 'border-card-border bg-surface-container-low'
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
            <p className="font-body-md mt-1 pl-6 text-xs leading-relaxed text-on-surface-variant">
              {row.detail}
            </p>
            {row.action && (
              <button
                type="button"
                onClick={rowActions[row.action]}
                disabled={row.actionDisabled}
                className="font-body-md mt-3 ml-6 inline-flex items-center gap-1.5 rounded-lg border border-card-border bg-surface px-3 py-1.5 text-xs text-on-surface transition-colors hover:bg-surface-container-high disabled:opacity-60"
              >
                {row.busy && <Icon name="progress_activity" size={14} className="animate-spin" />}
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
              <p className="font-body-md mt-1 text-xs leading-relaxed text-on-surface-variant">
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
            {selfCheckRecord && <SelfCheckRecordCard record={selfCheckRecord} />}
          </div>
          <div className="mt-3 grid gap-2">
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.storage} item={props.runtimeCheck.storage} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.settings} item={props.runtimeCheck.settings} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.github} item={props.runtimeCheck.github} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.stars} item={props.runtimeCheck.stars} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.readme} item={props.runtimeCheck.readme} />
            <RuntimeCheckRow title={RUNTIME_CHECK_TITLES.ai} item={props.runtimeCheck.ai} />
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
      <p className="font-body-md mt-1">检查时间：{props.record.completedAt}</p>
      <p className="font-body-md mt-1">
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
  const items = [result.storage, result.settings, result.github, result.stars, result.readme, result.ai];
  return items.reduce(
    (summary, item) => {
      if (item.status === 'passed') summary.passed += 1;
      else if (item.status === 'failed') summary.failed += 1;
      else summary.skipped += 1;
      return summary;
    },
    { passed: 0, failed: 0, skipped: 0 },
  );
}

function RuntimeCheckRow({ title, item }: { title: string; item: RuntimeReadinessCheckItem }) {
  const tone = item.status === 'passed' ? 'text-success' : item.status === 'failed' ? 'text-error' : 'text-on-surface-variant';
  const icon = item.status === 'passed' ? 'check_circle' : item.status === 'failed' ? 'error' : 'radio_button_unchecked';
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
          <p className="font-body-md mt-0.5 text-xs leading-relaxed text-on-surface-variant">{item.message}</p>
          {item.detail && (
            <p className="font-body-md mt-1 break-words text-[11px] leading-relaxed text-on-surface-variant/80">
              {item.detail}
            </p>
          )}
          {item.action && (
            <p className="font-body-md mt-2 rounded-md border border-outline-variant/25 bg-surface-container-low px-2 py-1.5 text-[11px] leading-relaxed text-on-surface">
              <span className="font-medium text-on-surface">建议处理：</span>
              {item.action}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
