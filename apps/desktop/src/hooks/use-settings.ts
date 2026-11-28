import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  isSavedAiApiKeyPlaceholder,
  keychainFailureText,
  normalizeEmbeddingSettings,
  SAVED_AI_API_KEY_PLACEHOLDER,
  shouldFlushAiApiKey,
  shouldFlushEmbeddingApiKey,
} from '@/lib/ai-config';
import type { AppSettings } from '@/types-settings';
import { DEFAULT_SETTINGS } from '@/types-settings';

const STALE_SETTINGS_CACHE_KEY = 'fox-stars-lab-settings';
const SETTINGS_NOT_READ = '设置没读出来，先用了默认设置。你的资料都还在。重启应用再试一次；还不行就把这条截图发给开发者。';
const SETTINGS_NOT_SAVED = '设置没保存上。你的资料都还在。再改一次试试；还不行就重启应用，再不行把这条截图发给开发者。';
type AiKeySaveStatus = 'idle' | 'saving' | 'saved' | 'error';
type AppSettingsPatch = {
  theme?: Partial<AppSettings['theme']>;
  sync?: Partial<AppSettings['sync']>;
  ai?: Partial<AppSettings['ai']>;
  embedding?: Partial<AppSettings['embedding']>;
  general?: Partial<AppSettings['general']>;
  runtime?: Partial<AppSettings['runtime']>;
};

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(true);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [aiKeySaveStatus, setAiKeySaveStatus] = useState<AiKeySaveStatus>('idle');
  const [embeddingKeySaveStatus, setEmbeddingKeySaveStatus] = useState<AiKeySaveStatus>('idle');
  const aiKeySaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingAiKeyRef = useRef<string | null>(null);
  const pendingAiKeyProviderRef = useRef<AppSettings['ai']['provider']>(DEFAULT_SETTINGS.ai.provider);
  const settingsRef = useRef<AppSettings>(settings);
  const persistChainRef = useRef<Promise<void>>(Promise.resolve());
  const persistEpochRef = useRef(0);
  const aiKeyEpochRef = useRef(0);
  const embeddingKeyEpochRef = useRef(0);
  settingsRef.current = settings;

  useEffect(() => {
    loadSettings();
    return () => {
      clearAiKeySaveTimer();
      const pendingAiKey = pendingAiKeyRef.current;
      if (pendingAiKey !== null) {
        pendingAiKeyRef.current = null;
        void persistAiApiKey(pendingAiKey, pendingAiKeyProviderRef.current).catch(() => undefined);
      }
    };
  }, []);

  async function loadSettings() {
    let nextSettings = DEFAULT_SETTINGS;
    let nextError: string | null = null;

    try {
      const stored = await loadPersistedSettingsSnapshot();
      if (stored) {
        const parsed = stored;
        nextSettings = {
          ...DEFAULT_SETTINGS,
          ...parsed,
          theme: normalizeThemeSettings({ ...DEFAULT_SETTINGS.theme, ...parsed.theme }),
          sync: normalizeSyncSettings({ ...DEFAULT_SETTINGS.sync, ...parsed.sync }),
          ai: normalizeAiSettings({ ...DEFAULT_SETTINGS.ai, ...parsed.ai, apiKey: '' }),
          embedding: normalizeEmbeddingSettings({ ...DEFAULT_SETTINGS.embedding, ...parsed.embedding, apiKey: '' }),
          general: {
            showWelcomeOnStartup:
              parsed.general?.showWelcomeOnStartup ?? DEFAULT_SETTINGS.general.showWelcomeOnStartup,
          },
          runtime: normalizeRuntimeSettings({ ...DEFAULT_SETTINGS.runtime, ...parsed.runtime }),
        };
      }
    } catch {
      nextError = SETTINGS_NOT_READ;
      nextSettings = DEFAULT_SETTINGS;
    }

    let apiKey = '';
    if (shouldReadSavedAiApiKey(nextSettings.ai)) {
      try {
        apiKey = await readSavedAiApiKeyPlaceholder(nextSettings.ai);
      } catch (error) {
        apiKey = '';
        nextError = keychainFailureText('read');
      }
    }

    let embeddingApiKey = '';
    if (shouldReadSavedEmbeddingApiKey(nextSettings.embedding)) {
      try {
        embeddingApiKey = await readSavedEmbeddingApiKeyPlaceholder(nextSettings.embedding);
      } catch (error) {
        embeddingApiKey = '';
        nextError = mergeSettingsError(
          nextError,
          keychainFailureText('read'),
        );
      }
    }

    const hydratedSettings = {
      ...nextSettings,
      ai: { ...nextSettings.ai, apiKey },
      embedding: { ...nextSettings.embedding, apiKey: embeddingApiKey },
    };
    settingsRef.current = hydratedSettings;
    setSettings(hydratedSettings);
    try {
      await persistSettingsSnapshot(hydratedSettings);
    } catch {
      nextError = mergeSettingsError(nextError, SETTINGS_NOT_SAVED);
    }
    setSettingsError(nextError);
    setIsLoading(false);
  }

  function enqueuePersist<T>(task: () => Promise<T>): Promise<T> {
    const run = persistChainRef.current.catch(() => undefined).then(task);
    persistChainRef.current = run.then(() => undefined, () => undefined);
    return run;
  }

  async function updateSettings(partial: AppSettingsPatch) {
    const epoch = persistEpochRef.current;
    const base = settingsRef.current;
    const partialAiIncludesApiKey = Boolean(
      partial.ai && Object.prototype.hasOwnProperty.call(partial.ai, 'apiKey'),
    );
    const partialEmbeddingIncludesApiKey = Boolean(
      partial.embedding && Object.prototype.hasOwnProperty.call(partial.embedding, 'apiKey'),
    );
    const partialAiChangesProvider = Boolean(
      partial.ai?.provider && partial.ai.provider !== base.ai.provider,
    );
    let nextAi = normalizeAiSettings(partial.ai ? { ...base.ai, ...partial.ai } : base.ai);
    let nextEmbedding = normalizeEmbeddingSettings(
      partial.embedding ? { ...base.embedding, ...partial.embedding } : base.embedding,
    );
    const partialEmbeddingChangesCredentialScope = Boolean(
      partial.embedding
      && (nextEmbedding.provider !== base.embedding.provider
        || nextEmbedding.baseUrl !== base.embedding.baseUrl.trim()),
    );
    const needsAiKeyLookup = Boolean(partial.ai && partialAiChangesProvider && !partialAiIncludesApiKey);
    if (needsAiKeyLookup) {
      nextAi = { ...nextAi, apiKey: '' };
    }
    if (partialAiIncludesApiKey || needsAiKeyLookup) {
      aiKeyEpochRef.current += 1;
    }
    const aiKeyEpoch = aiKeyEpochRef.current;

    const embeddingScopeChanged = Boolean(
      partial.embedding && partialEmbeddingChangesCredentialScope && !partialEmbeddingIncludesApiKey,
    );
    const needsEmbeddingKeyLookup = Boolean(
      embeddingScopeChanged
      && (nextEmbedding.provider === 'openai' || nextEmbedding.provider === 'openai-compatible'),
    );
    if (embeddingScopeChanged) {
      nextEmbedding = { ...nextEmbedding, apiKey: '' };
    }
    if (partialEmbeddingIncludesApiKey || embeddingScopeChanged) {
      embeddingKeyEpochRef.current += 1;
    }
    const embeddingKeyEpoch = embeddingKeyEpochRef.current;

    const optimistic: AppSettings = {
      ...base,
      theme: normalizeThemeSettings(partial.theme ? { ...base.theme, ...partial.theme } : base.theme),
      sync: normalizeSyncSettings(partial.sync ? { ...base.sync, ...partial.sync } : base.sync),
      ai: partial.ai ? nextAi : base.ai,
      embedding: partial.embedding ? nextEmbedding : base.embedding,
      general: normalizeGeneralSettings(partial.general ? { ...base.general, ...partial.general } : base.general),
      runtime: normalizeRuntimeSettings(partial.runtime ? { ...base.runtime, ...partial.runtime } : base.runtime),
    };
    settingsRef.current = optimistic;
    setSettings(optimistic);

    const targetAiForKeyLookup = { ...nextAi, apiKey: '' };
    const targetAiApiKey = partial.ai?.apiKey ?? '';
    const targetAiProvider = optimistic.ai.provider;
    const targetEmbeddingForKeyLookup = { ...nextEmbedding, apiKey: '' };
    const targetEmbeddingApiKey = partial.embedding?.apiKey ?? '';
    const targetEmbedding = optimistic.embedding;

    return enqueuePersist(async () => {
      if (epoch !== persistEpochRef.current) return;
      let nextError: string | null = null;

      if (needsAiKeyLookup) {
        let savedAiKey = '';
        try {
          savedAiKey = await readSavedAiApiKeyPlaceholder(targetAiForKeyLookup);
        } catch {
          savedAiKey = '';
          nextError = keychainFailureText('read');
        }
        if (epoch === persistEpochRef.current && aiKeyEpoch === aiKeyEpochRef.current) {
          const latest = settingsRef.current;
          const patched: AppSettings = {
            ...latest,
            ai: { ...latest.ai, apiKey: savedAiKey },
          };
          settingsRef.current = patched;
          setSettings(patched);
        }
      }

      if (needsEmbeddingKeyLookup) {
        let savedEmbeddingKey = '';
        try {
          savedEmbeddingKey = await readSavedEmbeddingApiKeyPlaceholder(targetEmbeddingForKeyLookup);
        } catch {
          savedEmbeddingKey = '';
          nextError = mergeSettingsError(
            nextError,
            keychainFailureText('read'),
          );
        }
        if (epoch === persistEpochRef.current && embeddingKeyEpoch === embeddingKeyEpochRef.current) {
          const latest = settingsRef.current;
          const patched: AppSettings = {
            ...latest,
            embedding: { ...latest.embedding, apiKey: savedEmbeddingKey },
          };
          settingsRef.current = patched;
          setSettings(patched);
        }
      }

      if (epoch !== persistEpochRef.current) return;
      try {
        if (partialAiIncludesApiKey) {
          clearAiKeySaveTimer();
          try {
            if (epoch === persistEpochRef.current && aiKeyEpoch === aiKeyEpochRef.current) {
              setAiKeySaveStatus('saving');
            }
            await persistAiApiKey(targetAiApiKey, targetAiProvider);
            if (epoch === persistEpochRef.current && aiKeyEpoch === aiKeyEpochRef.current) {
              pendingAiKeyRef.current = null;
              setAiKeySaveStatus(targetAiApiKey.trim() ? 'saved' : 'idle');
            }
          } catch {
            if (epoch === persistEpochRef.current && aiKeyEpoch === aiKeyEpochRef.current) {
              setAiKeySaveStatus('error');
            }
            nextError = keychainFailureText('save');
          }
        }
        if (partialEmbeddingIncludesApiKey) {
          try {
            if (epoch === persistEpochRef.current && embeddingKeyEpoch === embeddingKeyEpochRef.current) {
              setEmbeddingKeySaveStatus('saving');
            }
            await persistEmbeddingApiKey(targetEmbeddingApiKey, targetEmbedding);
            if (epoch === persistEpochRef.current && embeddingKeyEpoch === embeddingKeyEpochRef.current) {
              setEmbeddingKeySaveStatus(targetEmbeddingApiKey.trim() ? 'saved' : 'idle');
            }
          } catch {
            if (epoch === persistEpochRef.current && embeddingKeyEpoch === embeddingKeyEpochRef.current) {
              setEmbeddingKeySaveStatus('error');
            }
            nextError = mergeSettingsError(
              nextError,
              keychainFailureText('save'),
            );
          }
        }
        await persistSettingsSnapshot(settingsRef.current);
      } catch {
        nextError = mergeSettingsError(nextError, SETTINGS_NOT_SAVED);
      }
      if (epoch === persistEpochRef.current) {
        setSettingsError(nextError);
      }
    });
  }

  async function updateTheme(theme: Partial<AppSettings['theme']>) {
    await updateSettings({ theme });
  }

  async function updateSync(sync: Partial<AppSettings['sync']>) {
    await updateSettings({ sync });
  }

  async function updateAI(ai: Partial<AppSettings['ai']>) {
    await updateSettings({ ai });
  }

  async function updateEmbedding(embedding: Partial<AppSettings['embedding']>) {
    await updateSettings({ embedding });
  }

  async function flushEmbeddingKey(apiKey = settingsRef.current.embedding.apiKey) {
    const epoch = persistEpochRef.current;
    const embeddingKeyEpoch = ++embeddingKeyEpochRef.current;
    return enqueuePersist(async () => {
      if (epoch !== persistEpochRef.current) return;
      const base = settingsRef.current;
      try {
        if (embeddingKeyEpoch === embeddingKeyEpochRef.current) {
          setEmbeddingKeySaveStatus('saving');
        }
        await persistEmbeddingApiKey(apiKey, base.embedding);
        const normalizedApiKey = apiKey.trim();
        if (epoch === persistEpochRef.current && embeddingKeyEpoch === embeddingKeyEpochRef.current) {
          const updated = {
            ...settingsRef.current,
            embedding: {
              ...settingsRef.current.embedding,
              apiKey: normalizedApiKey ? SAVED_AI_API_KEY_PLACEHOLDER : '',
            },
          };
          settingsRef.current = updated;
          setSettings(updated);
          setEmbeddingKeySaveStatus(normalizedApiKey ? 'saved' : 'idle');
          setSettingsError(null);
        }
        await persistSettingsSnapshot(settingsRef.current);
      } catch (error) {
        if (epoch === persistEpochRef.current && embeddingKeyEpoch === embeddingKeyEpochRef.current) {
          setEmbeddingKeySaveStatus('error');
          setSettingsError(keychainFailureText('save'));
        }
        throw error;
      }
    });
  }

  async function updateAIKeyDraft(apiKey: string) {
    aiKeyEpochRef.current += 1;
    const updated = {
      ...settingsRef.current,
      ai: normalizeAiSettings({ ...settingsRef.current.ai, apiKey }),
    };
    settingsRef.current = updated;
    setSettings(updated);
    scheduleAiKeySave(apiKey, updated.ai.provider);
    const epoch = persistEpochRef.current;
    await enqueuePersist(async () => {
      if (epoch !== persistEpochRef.current) return;
      try {
        await persistSettingsSnapshot(settingsRef.current);
        if (epoch === persistEpochRef.current) {
          setSettingsError(null);
        }
      } catch {
        if (epoch === persistEpochRef.current) {
          setSettingsError(SETTINGS_NOT_SAVED);
        }
      }
    });
  }

  async function flushAIKey(apiKey = settingsRef.current.ai.apiKey) {
    clearAiKeySaveTimer();
    try {
      setAiKeySaveStatus('saving');
      await persistAiApiKey(apiKey, settingsRef.current.ai.provider);
      pendingAiKeyRef.current = null;
      setAiKeySaveStatus(apiKey.trim() ? 'saved' : 'idle');
      setSettingsError(null);
    } catch (error) {
      setAiKeySaveStatus('error');
      setSettingsError(keychainFailureText('save'));
      throw error;
    }
  }

  async function updateGeneral(general: Partial<AppSettings['general']>) {
    await updateSettings({ general });
  }

  async function updateRuntime(runtime: Partial<AppSettings['runtime']>) {
    await updateSettings({ runtime });
  }

  async function resetSettings() {
    persistEpochRef.current += 1;
    aiKeyEpochRef.current += 1;
    embeddingKeyEpochRef.current += 1;
    settingsRef.current = DEFAULT_SETTINGS;
    setSettings(DEFAULT_SETTINGS);
    clearAiKeySaveTimer();
    const failures: string[] = [];

    try {
      await invoke('clear_ai_api_key', { provider: null });
    } catch {
      failures.push('没能清掉保存的 AI 密钥');
    }

    try {
      await invoke('clear_embedding_api_key');
    } catch {
      failures.push('没能清掉保存的检索服务密钥');
    }

    try {
      await invoke('clear_app_settings');
    } catch {
      failures.push('没能清掉设置文件');
    }

    try {
      localStorage.removeItem(STALE_SETTINGS_CACHE_KEY);
    } catch {
      failures.push('没能清掉设置的临时副本');
    }

    pendingAiKeyRef.current = null;
    pendingAiKeyProviderRef.current = DEFAULT_SETTINGS.ai.provider;
    setAiKeySaveStatus('idle');
    setEmbeddingKeySaveStatus('idle');

    if (failures.length > 0) {
      const message = `设置没能重置：${failures.join('；')}。再点一次；还不行就重启应用，再不行把这条截图发给开发者。`;
      setSettingsError(message);
      throw new Error(message);
    }

    setSettingsError(null);
  }

  function clearAiKeySaveTimer() {
    if (aiKeySaveTimerRef.current) {
      clearTimeout(aiKeySaveTimerRef.current);
      aiKeySaveTimerRef.current = null;
    }
  }

  function scheduleAiKeySave(apiKey: string, provider: AppSettings['ai']['provider']) {
    clearAiKeySaveTimer();
    pendingAiKeyRef.current = apiKey;
    pendingAiKeyProviderRef.current = provider;
    setAiKeySaveStatus(apiKey.trim() ? 'saving' : 'idle');
    aiKeySaveTimerRef.current = setTimeout(() => {
      aiKeySaveTimerRef.current = null;
      void persistAiApiKey(apiKey, provider)
        .then(() => {
          if (pendingAiKeyRef.current === apiKey) {
            pendingAiKeyRef.current = null;
            setAiKeySaveStatus(apiKey.trim() ? 'saved' : 'idle');
          }
        })
        .catch((error) => {
          setAiKeySaveStatus('error');
          setSettingsError(keychainFailureText('save'));
        });
    }, 500);
  }

  async function persistAiApiKey(apiKey: string, provider: AppSettings['ai']['provider']) {
    if (provider === 'none') {
      return;
    }
    const normalizedApiKey = apiKey.trim();
    if (isSavedAiApiKeyPlaceholder(normalizedApiKey)) {
      return;
    }
    if (normalizedApiKey) {
      await invoke('save_ai_api_key', { provider, apiKey: normalizedApiKey });
    } else {
      await invoke('clear_ai_api_key', { provider });
    }
  }

  async function persistEmbeddingApiKey(
    apiKey: string,
    embedding: AppSettings['embedding'],
  ) {
    const normalizedApiKey = apiKey.trim();
    if (isSavedAiApiKeyPlaceholder(normalizedApiKey)) {
      return;
    }
    if (normalizedApiKey) {
      await invoke('save_embedding_api_key', {
        provider: embedding.provider,
        baseUrl: embedding.baseUrl || null,
        apiKey: normalizedApiKey,
      });
    } else {
      await invoke('clear_embedding_api_key');
    }
  }

  async function persistSettingsSnapshot(nextSettings: AppSettings) {
    const sanitizedSettings = sanitizeSettingsForStorage(nextSettings);
    await invoke('save_app_settings', { settings: sanitizedSettings });
  }

  return {
    settings,
    isLoading,
    settingsError,
    aiKeySaveStatus,
    embeddingKeySaveStatus,
    updateSettings,
    updateTheme,
    updateSync,
    updateAI,
    updateEmbedding,
    updateAIKeyDraft,
    flushAIKey,
    flushEmbeddingKey,
    updateGeneral,
    updateRuntime,
    resetSettings,
  };
}

function mergeSettingsError(current: string | null, next: string): string {
  return current ? `${current}；${next}` : next;
}

function shouldReadSavedAiApiKey(ai: AppSettings['ai']) {
  return ai.provider !== 'none' && shouldFlushAiApiKey(ai);
}

async function readSavedAiApiKeyPlaceholder(ai: AppSettings['ai']) {
  if (!shouldReadSavedAiApiKey(ai)) {
    return '';
  }

  const hasSavedApiKey = await invoke<boolean>('has_ai_api_key', { provider: ai.provider });
  return hasSavedApiKey ? SAVED_AI_API_KEY_PLACEHOLDER : '';
}

function shouldReadSavedEmbeddingApiKey(embedding: AppSettings['embedding']) {
  return embedding.provider !== 'none' && shouldFlushEmbeddingApiKey(embedding);
}

async function readSavedEmbeddingApiKeyPlaceholder(embedding: AppSettings['embedding']) {
  const hasSavedApiKey = await invoke<boolean>('has_embedding_api_key', {
    provider: embedding.provider,
    baseUrl: embedding.baseUrl || null,
  });
  return hasSavedApiKey ? SAVED_AI_API_KEY_PLACEHOLDER : '';
}

async function loadPersistedSettingsSnapshot(): Promise<Partial<AppSettings> | null> {
  return invoke<Partial<AppSettings> | null>('get_app_settings');
}

function sanitizeSettingsForStorage(settings: AppSettings): AppSettings {
  return {
    ...settings,
    theme: normalizeThemeSettings(settings.theme),
    sync: normalizeSyncSettings(settings.sync),
    general: normalizeGeneralSettings(settings.general),
    runtime: normalizeRuntimeSettings(settings.runtime),
    ai: {
      ...settings.ai,
      apiKey: '',
    },
    embedding: {
      ...normalizeEmbeddingSettings(settings.embedding),
      apiKey: '',
    },
  };
}

function normalizeRuntimeSettings(runtime: AppSettings['runtime']): AppSettings['runtime'] {
  const record = runtime.lastSelfCheckRecord;
  if (!record || typeof record !== 'object') {
    return { lastSelfCheckRecord: null };
  }

  const completedAt = typeof record.completedAt === 'string' ? record.completedAt : '';
  if (!completedAt) {
    return { lastSelfCheckRecord: null };
  }

  return {
    lastSelfCheckRecord: {
      completedAt,
      passed: normalizeRuntimeCount(record.passed),
      failed: normalizeRuntimeCount(record.failed),
      skipped: normalizeRuntimeCount(record.skipped),
    },
  };
}

function normalizeRuntimeCount(value: number): number {
  const normalizedValue = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(normalizedValue) ? Math.max(0, Math.round(normalizedValue)) : 0;
}

function normalizeGeneralSettings(general: AppSettings['general']): AppSettings['general'] {
  return {
    showWelcomeOnStartup: Boolean(general.showWelcomeOnStartup),
  };
}

function normalizeSyncSettings(sync: AppSettings['sync']): AppSettings['sync'] {
  return {
    enableAutoSync: Boolean(sync.enableAutoSync),
    autoSyncInterval: normalizeSyncInterval(sync.autoSyncInterval),
  };
}

function normalizeSyncInterval(value: number): number {
  const normalizedValue = typeof value === 'number' ? value : Number(value);
  if (!Number.isFinite(normalizedValue)) {
    return DEFAULT_SETTINGS.sync.autoSyncInterval;
  }

  return Math.min(Math.max(Math.round(normalizedValue), 5), 1440);
}

function normalizeAiSettings(ai: AppSettings['ai']): AppSettings['ai'] {
  const provider = ai.provider === 'openai'
    || ai.provider === 'openai-compatible'
    || ai.provider === 'anthropic'
    || ai.provider === 'none'
    ? ai.provider
    : DEFAULT_SETTINGS.ai.provider;
  const isRemoteProvider = provider !== 'none';
  const baseUrl = typeof ai.baseUrl === 'string' ? ai.baseUrl.trim() : '';
  const model = typeof ai.model === 'string' ? ai.model.trim() : '';
  const providerPreset = normalizeAiProviderPreset(ai.providerPreset, provider);

  return {
    provider,
    providerPreset,
    baseUrl: isRemoteProvider ? baseUrl : DEFAULT_SETTINGS.ai.baseUrl,
    apiKey: typeof ai.apiKey === 'string' ? ai.apiKey : '',
    model: isRemoteProvider ? model : DEFAULT_SETTINGS.ai.model,
    enableAutoSummary: isRemoteProvider ? Boolean(ai.enableAutoSummary) : false,
  };
}

function normalizeAiProviderPreset(
  value: AppSettings['ai']['providerPreset'] | undefined,
  provider: AppSettings['ai']['provider'],
): AppSettings['ai']['providerPreset'] {
  const knownPresets: AppSettings['ai']['providerPreset'][] = [
    'openai',
    'anthropic',
    'openrouter',
    'deepseek',
    'moonshot',
    'qwen',
    'zhipu',
    'siliconflow',
    'ollama',
    'lmstudio',
    'custom-openai-compatible',
    'none',
  ];
  if (value && knownPresets.includes(value)) {
    return value;
  }
  if (provider === 'openai') return 'openai';
  if (provider === 'anthropic') return 'anthropic';
  if (provider === 'openai-compatible') return 'custom-openai-compatible';
  return 'none';
}

function normalizeThemeSettings(theme: AppSettings['theme']): AppSettings['theme'] {
  const mode = theme.mode === 'system' || theme.mode === 'light' || theme.mode === 'dark'
    ? theme.mode
    : DEFAULT_SETTINGS.theme.mode;
  const fontSize = theme.fontSize === 'small' || theme.fontSize === 'medium' || theme.fontSize === 'large'
    ? theme.fontSize
    : DEFAULT_SETTINGS.theme.fontSize;
  const brandColor = typeof theme.brandColor === 'string' && /^#[0-9a-f]{6}$/i.test(theme.brandColor)
    ? theme.brandColor
    : DEFAULT_SETTINGS.theme.brandColor;
  const colorPreset = typeof theme.colorPreset === 'string' && theme.colorPreset.trim()
    ? theme.colorPreset
    : DEFAULT_SETTINGS.theme.colorPreset;

  return {
    mode,
    brandColor,
    fontSize,
    colorPreset,
  };
}
