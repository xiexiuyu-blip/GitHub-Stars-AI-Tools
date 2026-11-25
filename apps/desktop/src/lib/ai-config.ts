import { DEFAULT_SETTINGS, type AISettings, type EmbeddingSettings } from '@/types-settings';

export const SAVED_AI_API_KEY_PLACEHOLDER = '__FSL_SAVED_AI_API_KEY__';

export type BackendAiRequestConfig = Pick<AISettings, 'provider' | 'baseUrl' | 'model'> & {
  apiKey: '';
};

export type BackendEmbeddingRequestConfig = Pick<
  EmbeddingSettings,
  'enabled' | 'provider' | 'downloadSource' | 'baseUrl' | 'model' | 'dimensions' | 'minScore' | 'maxResults'
> & {
  apiKey: '';
};

export function toBackendAiRequestConfig(ai: AISettings): BackendAiRequestConfig {
  return {
    provider: ai.provider,
    baseUrl: ai.baseUrl,
    apiKey: '',
    model: ai.model,
  };
}

export function toBackendEmbeddingRequestConfig(embedding: EmbeddingSettings): BackendEmbeddingRequestConfig {
  return {
    enabled: embedding.enabled,
    provider: embedding.provider,
    downloadSource: embedding.downloadSource,
    baseUrl: embedding.baseUrl,
    apiKey: '',
    model: embedding.model,
    dimensions: embedding.dimensions,
    minScore: embedding.minScore,
    maxResults: embedding.maxResults,
  };
}

export function normalizeEmbeddingSettings(embedding: EmbeddingSettings): EmbeddingSettings {
  const configuredProvider = embedding.provider === 'local'
    || embedding.provider === 'openai'
    || embedding.provider === 'openai-compatible'
    || embedding.provider === 'none'
    ? embedding.provider
    : DEFAULT_SETTINGS.embedding.provider;
  const provider = configuredProvider === 'none' ? 'local' : configuredProvider;
  const enabled = configuredProvider !== 'none' && Boolean(embedding.enabled);
  const downloadSource = embedding.downloadSource === 'huggingface' ? 'huggingface' : 'modelscope';
  if (provider === 'local') {
    return {
      ...DEFAULT_SETTINGS.embedding,
      enabled,
      downloadSource,
      apiKey: '',
      minScore: configuredProvider === 'none'
        ? DEFAULT_SETTINGS.embedding.minScore
        : normalizeBoundedNumber(embedding.minScore, DEFAULT_SETTINGS.embedding.minScore, 0, 1, false),
      maxResults: normalizeBoundedNumber(embedding.maxResults, DEFAULT_SETTINGS.embedding.maxResults, 1, 10, true),
    };
  }
  return {
    enabled,
    provider,
    downloadSource,
    baseUrl: typeof embedding.baseUrl === 'string' ? embedding.baseUrl.trim() : '',
    apiKey: typeof embedding.apiKey === 'string' ? embedding.apiKey : '',
    model: typeof embedding.model === 'string' ? embedding.model.trim() : '',
    dimensions: normalizeBoundedNumber(
      embedding.dimensions,
      DEFAULT_SETTINGS.embedding.dimensions,
      1,
      8192,
      true,
    ),
    minScore: normalizeBoundedNumber(
      embedding.minScore,
      DEFAULT_SETTINGS.embedding.minScore,
      0,
      1,
      false,
    ),
    maxResults: normalizeBoundedNumber(
      embedding.maxResults,
      DEFAULT_SETTINGS.embedding.maxResults,
      1,
      10,
      true,
    ),
  };
}

export function shouldFlushAiApiKey(ai: AISettings): boolean {
  const apiKey = ai.apiKey.trim();
  if (ai.provider === 'none' || isSavedAiApiKeyPlaceholder(apiKey)) {
    return false;
  }

  if (ai.provider === 'openai-compatible' && isLocalAiBaseUrl(ai.baseUrl) && !apiKey) {
    return false;
  }

  return true;
}

export function shouldFlushEmbeddingApiKey(embedding: EmbeddingSettings): boolean {
  const apiKey = embedding.apiKey.trim();
  if (embedding.provider === 'local' || embedding.provider === 'none' || isSavedAiApiKeyPlaceholder(apiKey)) {
    return false;
  }
  if (embedding.provider === 'openai-compatible' && isLocalAiBaseUrl(embedding.baseUrl) && !apiKey) {
    return false;
  }
  return true;
}

export function getEmbeddingConfigMessage(embedding: EmbeddingSettings): string | null {
  if (!embedding.enabled) {
    return '这项检索尚未启用。';
  }
  if (embedding.provider === 'local') {
    return null;
  }
  if (embedding.provider === 'none') {
    return '请选择 OpenAI 或 OpenAI 兼容的 Embedding 服务。';
  }
  const baseUrl = embedding.baseUrl.trim();
  if (embedding.provider === 'openai-compatible' && !baseUrl) {
    return '请填写 Embedding 服务的 OpenAI 兼容请求地址。';
  }
  if (baseUrl && !isAllowedAiBaseUrl(baseUrl)) {
    return 'Embedding 请求地址必须使用 https://；本机或局域网服务可以使用 http://。';
  }
  if (!embedding.model.trim()) {
    return '请填写 Embedding 模型名。';
  }
  if (!Number.isInteger(embedding.dimensions) || embedding.dimensions < 1 || embedding.dimensions > 8192) {
    return 'Embedding 维度必须是 1 到 8192 的整数。';
  }
  if (!hasUsableAiApiKey(embedding.apiKey)
    && !(embedding.provider === 'openai-compatible' && isLocalAiBaseUrl(baseUrl))) {
    return '请填写 Embedding 密钥。';
  }
  return null;
}

/**
 * AI 还没设置完整时说什么。别处（GitHub 页的运行检查、欢迎引导、各个按钮的悬停提示）读到它，要指路到「设置 → AI 设置」；
 * 人已经在「设置 → AI 设置」这一页里时（`onAiSettingsPage`），再说「去设置 → AI 设置」就是在叫他去他正在的地方：
 * 这时改成指这一页上面的输入框（提示显示在页面最下面的「测试连接」旁边，要填的都在它上面）。
 */
export function getAiConfigMessage(ai: AISettings, options: { onAiSettingsPage?: boolean } = {}): string | null {
  const here = options.onAiSettingsPage === true;
  if (ai.provider === 'none') {
    return here
      ? '还没设置 AI。先在上面选一个服务商，填好密钥和模型名，再点「测试连接」。'
      : '还没设置 AI。去「设置 → AI 设置」，选一个服务商（比如 DeepSeek、通义、OpenAI），填上它给你的密钥和模型名，再点「测试连接」。';
  }

  const baseUrl = ai.baseUrl.trim();
  if (ai.provider === 'openai-compatible' && !baseUrl) {
    return here ? '请先在上面的「高级」里填写请求地址。' : '请先到「设置 → AI 设置 → 高级」里填写请求地址。';
  }

  if (baseUrl && !isAllowedAiBaseUrl(baseUrl)) {
    return 'AI 的请求地址要以 https:// 开头；本机或局域网里的服务可以用 http://。';
  }

  if (!ai.model.trim()) {
    return here ? '请先在上面填写模型名。' : '请先在「设置 → AI 设置」里填写模型名。';
  }

  if (!hasUsableAiApiKey(ai.apiKey) && !canUseAiWithoutApiKey(ai, baseUrl)) {
    return here ? '请先在上面填写密钥。' : '请先在「设置 → AI 设置」里填写密钥。';
  }

  return null;
}

export function isSavedAiApiKeyPlaceholder(apiKey: string) {
  return apiKey === SAVED_AI_API_KEY_PLACEHOLDER;
}

function hasUsableAiApiKey(apiKey: string) {
  return isSavedAiApiKeyPlaceholder(apiKey) || apiKey.trim().length > 0;
}

function canUseAiWithoutApiKey(ai: AISettings, baseUrl: string) {
  return ai.provider === 'openai-compatible' && isLocalAiBaseUrl(baseUrl);
}

function normalizeBoundedNumber(
  value: number,
  fallback: number,
  min: number,
  max: number,
  integer: boolean,
) {
  const parsed = typeof value === 'number' ? value : Number(value);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  const bounded = Math.min(max, Math.max(min, parsed));
  return integer ? Math.round(bounded) : bounded;
}

function isAllowedAiBaseUrl(baseUrl: string) {
  if (/^https:\/\//i.test(baseUrl)) {
    return true;
  }

  if (!/^http:\/\//i.test(baseUrl)) {
    return false;
  }

  try {
    const url = new URL(baseUrl);
    return isLocalAiHost(url.hostname) || isPrivateNetworkHost(url.hostname);
  } catch {
    return false;
  }
}

function isLocalAiBaseUrl(baseUrl: string) {
  try {
    const url = new URL(baseUrl);
    return isLocalAiHost(url.hostname);
  } catch {
    return false;
  }
}

function isLocalAiHost(hostname: string) {
  const host = normalizeHostname(hostname);
  return ['localhost', '127.0.0.1', '0.0.0.0', '::1'].includes(host);
}

function isPrivateNetworkHost(hostname: string) {
  const host = normalizeHostname(hostname);
  if (host === 'host.docker.internal') {
    return true;
  }

  const parts = host.split('.').map((part) => Number.parseInt(part, 10));
  if (parts.length !== 4 || parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) {
    return false;
  }

  return parts[0] === 10
    || (parts[0] === 172 && parts[1] >= 16 && parts[1] <= 31)
    || (parts[0] === 192 && parts[1] === 168);
}

function normalizeHostname(hostname: string) {
  return hostname.trim().toLowerCase().replace(/^\[/u, '').replace(/\]$/u, '');
}

/* ───────── 设置 → AI 设置：服务商卡片和相关文案 ───────── */

export type AIProviderPreset = AISettings['providerPreset'];

export type AIProviderPresetOption = {
  id: AIProviderPreset;
  label: string;
  /** 卡片副标：只写「需要什么」。协议名（Chat Completions 之类）不在这里，折进「高级」。 */
  description: string;
  provider: AISettings['provider'];
  baseUrl: string;
  apiKeyLabel: string;
  modelPlaceholder: string;
  modelSuggestions: string[];
  icon: string;
};

const NEEDS_KEY_AND_MODEL = '需要：密钥 + 模型名';
const NEEDS_MODEL_LOCAL = '需要：模型名（本机服务，密钥可不填）';

export const AI_PROVIDER_PRESETS: AIProviderPresetOption[] = [
  {
    id: 'openai',
    label: 'OpenAI',
    description: NEEDS_KEY_AND_MODEL,
    provider: 'openai',
    baseUrl: 'https://api.openai.com/v1',
    apiKeyLabel: '密钥',
    modelPlaceholder: '填写 OpenAI 的模型名',
    modelSuggestions: ['gpt-5.5', 'gpt-5.5-mini', 'gpt-5.5-nano'],
    icon: 'auto_awesome',
  },
  {
    id: 'anthropic',
    label: 'Anthropic',
    description: NEEDS_KEY_AND_MODEL,
    provider: 'anthropic',
    baseUrl: 'https://api.anthropic.com/v1',
    apiKeyLabel: '密钥',
    modelPlaceholder: '填写 Claude 的模型名',
    modelSuggestions: ['claude-sonnet-5', 'claude-opus-4-8', 'claude-haiku-4-5'],
    icon: 'psychology',
  },
  {
    id: 'openrouter',
    label: 'OpenRouter',
    description: NEEDS_KEY_AND_MODEL,
    provider: 'openai-compatible',
    baseUrl: 'https://openrouter.ai/api/v1',
    apiKeyLabel: '密钥',
    modelPlaceholder: '例如 openai/gpt-5.5',
    modelSuggestions: ['openai/gpt-5.5', 'anthropic/claude-sonnet-5', 'deepseek/deepseek-chat'],
    icon: 'route',
  },
  {
    id: 'deepseek',
    label: 'DeepSeek',
    description: NEEDS_KEY_AND_MODEL,
    provider: 'openai-compatible',
    baseUrl: 'https://api.deepseek.com/v1',
    apiKeyLabel: '密钥',
    modelPlaceholder: '例如 deepseek-chat',
    modelSuggestions: ['deepseek-chat', 'deepseek-reasoner'],
    icon: 'travel_explore',
  },
  {
    id: 'moonshot',
    label: 'Moonshot / Kimi',
    description: NEEDS_KEY_AND_MODEL,
    provider: 'openai-compatible',
    baseUrl: 'https://api.moonshot.cn/v1',
    apiKeyLabel: '密钥',
    modelPlaceholder: '例如 kimi-k2-0711-preview',
    modelSuggestions: ['kimi-k2-0711-preview', 'moonshot-v1-8k', 'moonshot-v1-32k'],
    icon: 'dark_mode',
  },
  {
    id: 'qwen',
    label: '阿里通义 Qwen',
    description: NEEDS_KEY_AND_MODEL,
    provider: 'openai-compatible',
    baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    apiKeyLabel: '密钥',
    modelPlaceholder: '例如 qwen-plus',
    modelSuggestions: ['qwen-plus', 'qwen-max', 'qwen-turbo'],
    icon: 'cloud',
  },
  {
    id: 'zhipu',
    label: '智谱 GLM',
    description: NEEDS_KEY_AND_MODEL,
    provider: 'openai-compatible',
    baseUrl: 'https://open.bigmodel.cn/api/paas/v4',
    apiKeyLabel: '密钥',
    modelPlaceholder: '例如 glm-4.5',
    modelSuggestions: ['glm-4.5', 'glm-4.5-air', 'glm-4-flash'],
    icon: 'neurology',
  },
  {
    id: 'siliconflow',
    label: '硅基流动',
    description: NEEDS_KEY_AND_MODEL,
    provider: 'openai-compatible',
    baseUrl: 'https://api.siliconflow.cn/v1',
    apiKeyLabel: '密钥',
    modelPlaceholder: '填写 SiliconFlow 的模型名',
    modelSuggestions: ['Qwen/Qwen3-Coder-480B-A35B-Instruct', 'deepseek-ai/DeepSeek-V3', 'THUDM/GLM-4.1V-9B-Thinking'],
    icon: 'memory',
  },
  {
    id: 'ollama',
    label: 'Ollama',
    description: NEEDS_MODEL_LOCAL,
    provider: 'openai-compatible',
    baseUrl: 'http://127.0.0.1:11434/v1',
    apiKeyLabel: '密钥（可留空）',
    modelPlaceholder: '例如 llama3.1',
    modelSuggestions: ['llama3.1', 'qwen2.5-coder', 'deepseek-r1'],
    icon: 'dns',
  },
  {
    id: 'lmstudio',
    label: 'LM Studio',
    description: NEEDS_MODEL_LOCAL,
    provider: 'openai-compatible',
    baseUrl: 'http://127.0.0.1:1234/v1',
    apiKeyLabel: '密钥（可留空）',
    modelPlaceholder: '填写本机加载的模型名',
    modelSuggestions: ['local-model'],
    icon: 'desktop_windows',
  },
  {
    id: 'custom-openai-compatible',
    label: '自定义服务',
    description: '需要：请求地址 + 密钥 + 模型名',
    provider: 'openai-compatible',
    baseUrl: '',
    apiKeyLabel: '密钥（本机服务可留空）',
    modelPlaceholder: '填写服务商给你的模型名',
    modelSuggestions: [],
    icon: 'tune',
  },
  {
    id: 'none',
    label: '关闭 AI',
    description: '不使用 AI，也不会花钱。已有的中文说明和搜索照常能用。',
    provider: 'none',
    baseUrl: '',
    apiKeyLabel: '密钥',
    modelPlaceholder: '选好服务后填写模型名',
    modelSuggestions: [],
    icon: 'block',
  },
];

/** 「现在：…」那一句。关闭时只说关了，不把「关闭 AI」当成一个服务商名字读出来。 */
export function describeCurrentAiService(label: string, disabled: boolean): string {
  if (disabled) return '现在：AI 已关闭';
  return `现在：${label}。换一个服务商后，要填它对应的密钥和模型名。`;
}

/** 「测试连接」会真的发一次请求，要说清会花一点点钱。 */
export const AI_TEST_COST_NOTE = '会真实调用一次 AI，会产生很少的费用。';

/** 自动整理的开关和额度不在这一页：只指路，不再放一个会花钱的开关。 */
export const AI_AUTO_ORGANIZE_POINTER = '要不要自动整理、每天最多花多少，在「设置 → 自动整理」里设';

/** 请求地址下面的帮助：协议名只在「高级」里出现。 */
export function aiBaseUrlHelp(provider: AISettings['provider']): string {
  return provider === 'openai-compatible'
    ? '这个服务按 OpenAI 的 Chat Completions 协议对接，请填服务商给你的请求地址。'
    : '也可以直接填完整的接口地址，应用会自动处理，不会重复拼接。';
}

/** 密钥输入框下面的帮助。「密钥」指 AI 服务商给的，GitHub 给的叫「令牌」；保存的地方叫「钥匙串」。 */
export function aiKeyHelpText(input: { hasSavedKey: boolean; provider: AISettings['provider'] }): string {
  if (input.hasSavedKey) return '已保存密钥；输入新密钥可替换，或清除后重新设置。';
  if (input.provider === 'openai-compatible') {
    return '连接 Ollama、LM Studio 等本机服务时可以留空；填了的话，只保存在 Mac 的钥匙串里。';
  }
  return '密钥（AI 服务商给你的一串密码）只保存在 Mac 的钥匙串里，设置文件里只记请求地址和模型名。';
}

/** 读写钥匙串失败时要让人照着做的那一句。 */
export const KEYCHAIN_PERMISSION_HELP = '没能读写钥匙串。请在弹出的授权框里点「允许」，或到「钥匙串访问」里检查 Fox Stars Lab 的权限。';

export function keychainFailureText(action: 'read' | 'save'): string {
  return `${action === 'read' ? '密钥状态读取失败' : '密钥保存失败'}，${KEYCHAIN_PERMISSION_HELP}`;
}

export type AiKeySaveStatus = 'idle' | 'saving' | 'saved' | 'error';

/** 密钥输入框下面随保存状态变的小字。 */
export function aiKeySaveStatusText(status: AiKeySaveStatus): string | null {
  switch (status) {
    case 'saving':
      return '正在把密钥存进钥匙串…';
    case 'saved':
      return '密钥已保存在 Mac 的钥匙串里。';
    case 'error':
      return KEYCHAIN_PERMISSION_HELP;
    case 'idle':
    default:
      return null;
  }
}
