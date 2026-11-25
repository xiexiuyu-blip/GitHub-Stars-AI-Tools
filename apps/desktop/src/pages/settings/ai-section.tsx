import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { useAppSettings } from '@/providers/settings-provider';
import { Button } from '@/components/kit';
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
  isSavedAiApiKeyPlaceholder,
  shouldFlushAiApiKey,
  toBackendAiRequestConfig,
  type AIProviderPreset,
  type AIProviderPresetOption,
} from '@/lib/ai-config';
import { compactNumber } from '@/lib/format';
import type { AiStreamEvent } from '@/types';
import type { AISettings as AISettingsValue } from '@/types-settings';
import { toErrorMessage } from './shared';

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

export function AISettings({
  settingsHook,
  onOpenOrganize,
}: {
  settingsHook: ReturnType<typeof useAppSettings>;
  onOpenOrganize?: () => void;
}) {
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
  const needsBaseUrlNow = ai.provider === 'openai-compatible' && !ai.baseUrl.trim();
  const [advancedOpen, setAdvancedOpen] = useState(needsBaseUrlNow);
  const activeTestRequestRef = useRef<{ requestId: string; startedAt: number; firstTokenSeen: boolean } | null>(null);
  const currentPreset = getAiProviderPreset(ai.providerPreset);
  const endpointPlaceholder = currentPreset.baseUrl || getAiEndpointPlaceholder(ai.provider);
  const apiKeyPlaceholder = currentPreset.apiKeyLabel;
  const modelPlaceholder = currentPreset.modelPlaceholder;
  const effectiveApiKey = apiKeyDraft.trim() ? apiKeyDraft : ai.apiKey;
  const currentConfigMessage = getAiConfigMessage({ ...ai, apiKey: effectiveApiKey }, { onAiSettingsPage: true });
  const canTestAi = needsRemoteConfig && !currentConfigMessage;
  const aiKeySaveMessage = aiKeySaveStatusText(settingsHook.aiKeySaveStatus);
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
      if (!active || payload.taskType !== 'ai-config-test' || payload.requestId !== active.requestId) return;

      const now = performance.now();
      if (payload.status === 'delta') {
        const firstTokenLatencyMs = active.firstTokenSeen ? null : now - active.startedAt;
        if (!active.firstTokenSeen) activeTestRequestRef.current = { ...active, firstTokenSeen: true };
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
          ? { ...current, status: 'running', stage: payload.message ?? current.stage }
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
      if (shouldFlushAiApiKey(nextAi)) await settingsHook.flushAIKey(nextAi.apiKey);
      setTestDialog((current) => current ? { ...current, status: 'running', stage: '正在请求 AI 服务' } : current);
      const result = await invoke<AiConnectionTestResult>('test_ai_connection', {
        request: { aiConfig: toBackendAiRequestConfig(nextAi), requestId },
      });
      const completedAt = performance.now();
      const totalLatencyMs = completedAt - startedAt;
      const outputTokens = Math.max(0, result.outputTokens || estimateTextTokens(result.summaryZh));
      const tokensPerSecond = totalLatencyMs > 0 ? outputTokens / (totalLatencyMs / 1000) : null;
      const summary = result.summaryZh.slice(0, 120);
      setTestMessage({ type: 'success', text: `AI 设置可用，${result.model} 回复了一段测试内容：${summary}` });
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
    if (!options?.skipLoading) setIsLoadingDiagnostics(true);
    setDiagnosticsError(null);
    try {
      if (options?.flushKey && shouldFlushAiApiKey(nextAi)) await settingsHook.flushAIKey(nextAi.apiKey);
      const diagnostics = await invoke<AiRequestDiagnostics>('diagnose_ai_request', {
        request: { aiConfig: toBackendAiRequestConfig(nextAi) },
      });
      setAiDiagnostics(diagnostics);
    } catch (error) {
      setAiDiagnostics(null);
      setDiagnosticsError(toErrorMessage(error));
    } finally {
      if (!options?.skipLoading) setIsLoadingDiagnostics(false);
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
      if (shouldFlushAiApiKey(nextAi)) await settingsHook.flushAIKey(nextAi.apiKey);
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
              <p className="font-body-md mt-1 text-xs text-on-surface-variant">
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
              <summary className="font-body-md flex cursor-pointer list-none items-center justify-between gap-3 px-4 py-3 text-sm font-medium text-on-surface">
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
              className={`flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors focus:ring-2 focus:ring-primary/30 focus:outline-none ${
                currentPreset.id === DISABLED_AI_PROVIDER_PRESET.id
                  ? 'border-warning/35 bg-warning/10 text-on-surface'
                  : 'border-card-border bg-transparent text-on-surface-variant hover:bg-surface-container-low'
              }`}
            >
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-surface-container-high">
                <Icon name={DISABLED_AI_PROVIDER_PRESET.icon} size={17} />
              </span>
              <span className="min-w-0 flex-1">
                <span className="font-body-md block text-sm font-medium text-on-surface">关闭 AI</span>
                <span className="font-body-md block text-xs text-on-surface-variant">保留已保存的密钥，不再调用 AI；已有的中文说明和搜索照常能用</span>
              </span>
              {currentPreset.id === DISABLED_AI_PROVIDER_PRESET.id && <Icon name="check_circle" size={18} className="shrink-0 text-warning" />}
            </button>
          </div>

          {needsRemoteConfig && (
            <div className="grid gap-2">
              <label className="font-body-md font-medium text-on-surface">{currentPreset.apiKeyLabel}</label>
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
                className="font-body-md w-full rounded-lg border border-outline-variant bg-surface-container-low px-4 py-2 text-on-surface focus:border-primary focus:ring-1 focus:ring-primary focus:outline-none"
              />
              <p className="font-body-md text-[12px] text-on-surface-variant">
                {aiKeyHelpText({ hasSavedKey: hasSavedApiKey, provider: ai.provider })}
              </p>
              {hasSavedApiKey && !apiKeyDraft.trim() && (
                <button
                  type="button"
                  onClick={() => void handleClearSavedAiKey()}
                  className="font-body-md w-fit rounded-lg border border-card-border bg-surface-container-high px-3 py-1.5 text-xs text-on-surface transition-colors hover:bg-surface-container-highest"
                >
                  清除已保存的密钥
                </button>
              )}
              {aiKeySaveMessage && (
                <p className={`font-body-md text-[12px] ${settingsHook.aiKeySaveStatus === 'error' ? 'text-error' : 'text-on-surface-variant'}`}>
                  {aiKeySaveMessage}
                </p>
              )}
            </div>
          )}

          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-card-border py-4">
            <div className="min-w-0 flex-1">
              <h4 className="font-body-lg font-medium text-on-surface">自动整理与每天额度</h4>
              <p className="font-body-md mt-1 text-on-surface-variant">{AI_AUTO_ORGANIZE_POINTER}。</p>
            </div>
            {onOpenOrganize ? (
              <Button size="sm" onClick={onOpenOrganize}>去设置自动整理</Button>
            ) : null}
          </div>

          <div className="grid gap-4 border-t border-card-border py-4 lg:grid-cols-[minmax(0,1fr)_minmax(320px,420px)] lg:items-start">
            <div>
              <h4 className="font-body-lg font-medium text-on-surface">模型</h4>
              <p className="font-body-md mt-1 text-on-surface-variant">
                可以从当前服务获取模型列表，也可以直接填服务商给你的模型名。
              </p>
            </div>
            <div className="min-w-0 space-y-3">
              <div className="flex gap-2">
                <select
                  value={modelOptions.some((model) => model.id === ai.model) ? ai.model : ''}
                  onChange={(event) => {
                    if (event.target.value) void settingsHook.updateAI({ model: event.target.value });
                  }}
                  disabled={!needsRemoteConfig || modelOptions.length === 0}
                  className="font-body-md min-w-0 flex-1 rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 text-sm text-on-surface focus:border-primary focus:ring-1 focus:ring-primary focus:outline-none disabled:opacity-60"
                >
                  <option value="">选择模型</option>
                  {modelOptions.map((model) => (
                    <option key={model.id} value={model.id}>{formatAiModelOptionLabel(model)}</option>
                  ))}
                </select>
                <button
                  type="button"
                  onClick={() => void handleLoadAiModels()}
                  disabled={!needsRemoteConfig || isLoadingModels}
                  className="gsat-primary-button font-body-md inline-flex shrink-0 items-center gap-2 rounded-lg bg-primary px-3 py-2 text-sm font-medium text-white transition-all hover:bg-primary-container disabled:opacity-60"
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
                    className={`font-body-md rounded-md border px-2.5 py-1.5 text-xs transition-colors disabled:opacity-60 ${
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
                  if (event.key === 'Enter') event.currentTarget.blur();
                }}
                placeholder={modelPlaceholder}
                disabled={!needsRemoteConfig}
                className="font-body-md w-full rounded-lg border border-outline-variant bg-surface-container-low px-3 py-2 text-sm text-on-surface focus:border-primary focus:ring-1 focus:ring-primary focus:outline-none disabled:opacity-60"
              />
              {modelListMessage && (
                <p className={`font-body-md rounded-lg border px-3 py-2 text-xs ${modelListMessage.type === 'success' ? 'border-success/20 bg-success/10 text-success' : 'border-error/20 bg-error/10 text-error'}`}>
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
              <summary className="font-body-md flex cursor-pointer list-none items-center justify-between gap-3 px-4 py-3 text-sm font-medium text-on-surface">
                <span className="flex items-center gap-2">
                  <Icon name="settings" size={17} className="text-on-surface-variant" />
                  高级（请求地址和诊断信息）
                </span>
                <Icon name="expand_more" size={18} className="text-on-surface-variant" />
              </summary>
              <div className="space-y-4 border-t border-card-border p-4">
                <div className="grid gap-2">
                  <label className="font-body-md font-medium text-on-surface">请求地址</label>
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
                    className="font-body-md w-full rounded-lg border border-outline-variant bg-surface-container-low px-4 py-2 text-on-surface focus:border-primary focus:ring-1 focus:ring-primary focus:outline-none"
                  />
                  <p className="font-body-md text-[12px] text-on-surface-variant">{aiBaseUrlHelp(ai.provider)}</p>
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
              <p className="font-body-md mt-1 text-on-surface-variant">{AI_TEST_COST_NOTE}</p>
              <p className={`font-body-md mt-3 rounded-lg border px-3 py-2 text-sm ${canTestAi ? 'border-success/20 bg-success/10 text-success' : 'border-error/20 bg-error/10 text-error'}`}>
                {canTestAi ? '设置已填完整，可以点「测试连接」。' : currentConfigMessage}
              </p>
              {testMessage && (
                <p className={`font-body-md mt-3 rounded-lg border px-3 py-2 text-sm ${testMessage.type === 'success' ? 'border-success/20 bg-success/10 text-success' : 'border-error/20 bg-error/10 text-error'}`}>
                  {testMessage.text}
                </p>
              )}
            </div>
            <button
              type="button"
              onClick={() => void handleTestAiConnection()}
              disabled={isTestingAi || !canTestAi}
              title={canTestAi ? '用现在的设置真实调用一次 AI，会产生很少的费用' : (currentConfigMessage ?? '请先把设置填完整')}
              className="font-body-md shrink-0 rounded-lg border border-card-border bg-surface-container-high px-4 py-2 text-sm text-on-surface transition-all hover:bg-surface-container-highest disabled:opacity-60"
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
      className={`flex min-h-[74px] min-w-0 items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors focus:ring-2 focus:ring-primary/30 focus:outline-none ${
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
          <span className="font-body-md truncate text-sm font-semibold">{preset.label}</span>
          {selected && <Icon name="check_circle" size={15} className="shrink-0 text-primary" />}
        </span>
        <span className="font-body-md mt-0.5 line-clamp-2 block text-xs leading-relaxed text-on-surface-variant">
          {preset.description}
        </span>
      </span>
    </button>
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
  const statusClass = diagnostics?.status === 'ready' ? 'border-success/20 bg-success/10 text-success' : 'border-error/20 bg-error/10 text-error';

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
          className="font-body-md inline-flex shrink-0 items-center justify-center gap-2 rounded-lg border border-card-border bg-surface-container-high px-3 py-2 text-sm text-on-surface transition-colors hover:bg-surface-container-highest disabled:opacity-60"
        >
          <Icon name={props.isLoading ? 'progress_activity' : 'refresh'} size={16} className={props.isLoading ? 'animate-spin' : ''} />
          {props.isLoading ? '读取中' : '重新读取'}
        </button>
      </div>

      {diagnostics ? (
        <>
          <div className={`font-body-md mb-3 rounded-lg border px-3 py-2 text-sm ${statusClass}`}>
            {diagnostics.status === 'ready' ? '设置已填完整，可以测试连接。' : diagnostics.failureReason ?? '设置还没填完整。'}
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
            <AiDiagnosticsRow label="失败重试" value={`${diagnostics.retryAttempts} 次，间隔 ${formatRetrySeconds(diagnostics.retryBaseDelayMs)} 起`} />
            <AiDiagnosticsRow label="项目说明读取上限" value={`${compactNumber(diagnostics.readmePromptCharLimit)} 字符`} />
            <AiDiagnosticsRow label="说明提示词版本" value={diagnostics.summaryPromptVersion} />
          </dl>
        </>
      ) : (
        <div className="font-body-md rounded-lg border border-outline-variant/25 bg-surface-container-low px-3 py-3 text-sm text-on-surface-variant">
          {props.errorMessage ?? '正在读取…'}
        </div>
      )}

      {failureReason && diagnostics?.status === 'ready' ? (
        <p className="font-body-md mt-3 rounded-lg border border-error/20 bg-error/10 px-3 py-2 text-sm text-error">
          最近失败原因：{failureReason}
        </p>
      ) : null}
      {successMessage ? (
        <p className="font-body-md mt-3 rounded-lg border border-success/20 bg-success/10 px-3 py-2 text-sm text-success">
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
      <dd className="font-body-md min-w-0 break-words text-xs text-on-surface" title={props.value}>{props.value}</dd>
    </div>
  );
}

function formatRetrySeconds(ms: number): string {
  const seconds = ms / 1000;
  return Number.isInteger(seconds) ? `${seconds} 秒` : `${seconds.toFixed(1)} 秒`;
}

function formatAiWireProtocol(value: string) {
  switch (value) {
    case 'openai': return 'OpenAI Chat Completions';
    case 'anthropic': return 'Anthropic Messages';
    default: return value || '未知';
  }
}

function formatAiApiKeyState(value: string) {
  switch (value) {
    case 'present': return '已读取';
    case 'missing': return '没有';
    case 'optional_for_local': return '本机兼容服务可留空';
    case 'not_required': return '不需要';
    default: return value || '未知';
  }
}

function getAiEndpointPlaceholder(provider: AISettingsValue['provider']) {
  switch (provider) {
    case 'anthropic': return 'https://api.anthropic.com/v1';
    case 'openai-compatible': return 'https://你的兼容服务/v1 或 http://127.0.0.1:11434/v1';
    case 'openai':
    default: return 'https://api.openai.com/v1';
  }
}

function AiTestDialog(props: { state: AiTestDialogState; isTesting: boolean; onClose: () => void }) {
  const state = props.state;
  const toneClass = state.status === 'success'
    ? 'border-success/20 bg-success/10 text-success'
    : state.status === 'error'
      ? 'border-error/20 bg-error/10 text-error'
      : 'border-primary/20 bg-primary/10 text-primary';
  const statusLabel = state.status === 'success' ? '测试通过' : state.status === 'error' ? '测试失败' : '测试中';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-[560px] rounded-xl border border-card-border bg-surface p-5 shadow-2xl">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h3 className="font-headline-md text-[20px] font-semibold text-on-surface">AI 连接测试</h3>
            <p className="font-body-md mt-1 text-sm text-on-surface-variant">
              正在用现在的设置真实调用一次 AI，会产生很少的费用。结果会显示用时和大概的输出速度。
            </p>
          </div>
          <button
            type="button"
            onClick={props.onClose}
            className="font-body-md rounded-lg border border-card-border bg-surface-container-high px-3 py-1.5 text-sm text-on-surface transition-colors hover:bg-surface-container-highest"
          >
            {props.isTesting ? '继续在后台等' : '关闭'}
          </button>
        </div>

        <div className={`font-body-md mt-4 rounded-lg border px-3 py-2 text-sm ${toneClass}`}>
          <div className="flex items-center gap-2 font-medium">
            {state.status === 'running' || state.status === 'preparing' ? (
              <Icon name="progress_activity" size={16} className="animate-spin" />
            ) : (
              <Icon name={state.status === 'success' ? 'check_circle' : 'error'} size={16} />
            )}
            {statusLabel}：{state.stage}
          </div>
        </div>

        <dl className="font-body-md mt-4 grid gap-3 rounded-lg border border-outline-variant/30 bg-surface-container-low p-3 text-sm sm:grid-cols-2">
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
            <p className="font-body-md mt-1 text-sm leading-relaxed text-on-surface">{state.summary}</p>
          </div>
        )}
        {state.error && (
          <div className="mt-4 rounded-lg border border-error/20 bg-error/10 px-3 py-2">
            <p className="font-body-md text-xs font-medium text-error">失败原因</p>
            <p className="font-body-md mt-1 text-sm leading-relaxed text-error">{state.error}</p>
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
      <dd className="font-body-md mt-1 truncate text-sm font-medium text-on-surface" title={props.value}>{props.value}</dd>
    </div>
  );
}

function formatLatency(value: number | null) {
  if (value === null) return '等待结果';
  if (value < 1000) return `${Math.round(value)} 毫秒`;
  return `${(value / 1000).toFixed(2)} 秒`;
}

function formatTokensPerSecond(value: number | null) {
  if (value === null || !Number.isFinite(value)) return '等待结果';
  return `${value.toFixed(1)} token/秒`;
}

function estimateTextTokens(value: string) {
  return Math.max(1, Math.ceil(Array.from(value).length / 4));
}

function getAiProviderPreset(providerPreset: AIProviderPreset): AIProviderPresetOption {
  return AI_PROVIDER_PRESETS.find((preset) => preset.id === providerPreset)
    ?? AI_PROVIDER_PRESETS.find((preset) => preset.id === 'custom-openai-compatible')
    ?? AI_PROVIDER_PRESETS[0];
}

function mergeAiModelOptions(preset: AIProviderPresetOption, availableModels: AiModelOption[], currentModel: string) {
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
    if (!id || seen.has(id)) continue;
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
    case 'anthropic': return 'sk-ant-...';
    case 'openai-compatible': return '本机服务可留空，云端服务填写密钥';
    case 'openai':
    default: return 'sk-...';
  }
}

function getAiModelPlaceholder(provider: AISettingsValue['provider']) {
  switch (provider) {
    case 'anthropic': return 'claude-sonnet-5';
    case 'openai-compatible': return '服务商给你的模型名';
    case 'openai':
    default: return 'gpt-5.5';
  }
}
