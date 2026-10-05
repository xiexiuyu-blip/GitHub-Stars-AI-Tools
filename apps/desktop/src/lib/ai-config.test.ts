import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS } from '@/types-settings';
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
  KEYCHAIN_PERMISSION_HELP,
  keychainFailureText,
  normalizeEmbeddingSettings,
  toBackendEmbeddingRequestConfig,
} from './ai-config';

describe('Embedding 配置', () => {
  it('新用户默认使用关闭的本地模型配置', () => {
    expect(DEFAULT_SETTINGS.embedding.enabled).toBe(false);
    expect(DEFAULT_SETTINGS.embedding.provider).toBe('local');
    expect(DEFAULT_SETTINGS.embedding.downloadSource).toBe('modelscope');
    expect(DEFAULT_SETTINGS.embedding.model).toBe('intfloat/multilingual-e5-small');
    expect(DEFAULT_SETTINGS.embedding.dimensions).toBe(384);
  });

  it('归一化维度、阈值和结果上限', () => {
    const normalized = normalizeEmbeddingSettings({
      ...DEFAULT_SETTINGS.embedding,
      enabled: true,
      provider: 'openai-compatible',
      baseUrl: '  http://127.0.0.1:11434/v1  ',
      model: '  embedding-model  ',
      dimensions: 9000,
      minScore: -1,
      maxResults: 20,
    });

    expect(normalized.baseUrl).toBe('http://127.0.0.1:11434/v1');
    expect(normalized.model).toBe('embedding-model');
    expect(normalized.dimensions).toBe(8192);
    expect(normalized.minScore).toBe(0);
    expect(normalized.maxResults).toBe(10);
  });

  it('旧版 none provider 迁移为关闭的本地模式', () => {
    const normalized = normalizeEmbeddingSettings({
      ...DEFAULT_SETTINGS.embedding,
      enabled: true,
      provider: 'none',
      minScore: 0.72,
    });

    expect(normalized.enabled).toBe(false);
    expect(normalized.provider).toBe('local');
    expect(normalized.model).toBe('intfloat/multilingual-e5-small');
    expect(normalized.dimensions).toBe(384);
    expect(normalized.minScore).toBe(0.8);
  });

  it('本地 provider 固定 multilingual-e5-small 参数并清空 Key', () => {
    const normalized = normalizeEmbeddingSettings({
      ...DEFAULT_SETTINGS.embedding,
      enabled: true,
      provider: 'local',
      apiKey: 'should-not-persist',
      model: 'fake-model',
      dimensions: 1536,
    });

    expect(normalized.provider).toBe('local');
    expect(normalized.model).toBe('intfloat/multilingual-e5-small');
    expect(normalized.dimensions).toBe(384);
    expect(normalized.apiKey).toBe('');
  });

  it('保留官方下载源并将未知下载源回退到国内源', () => {
    const official = normalizeEmbeddingSettings({
      ...DEFAULT_SETTINGS.embedding,
      downloadSource: 'huggingface',
    });
    const fallback = normalizeEmbeddingSettings({
      ...DEFAULT_SETTINGS.embedding,
      downloadSource: 'unknown' as 'modelscope',
    });

    expect(official.downloadSource).toBe('huggingface');
    expect(fallback.downloadSource).toBe('modelscope');
  });

  it('后端配置不携带明文密钥', () => {
    const backend = toBackendEmbeddingRequestConfig({
      ...DEFAULT_SETTINGS.embedding,
      enabled: true,
      provider: 'openai',
      baseUrl: 'https://api.openai.com/v1',
      apiKey: 'sk-secret',
    });

    expect(backend.apiKey).toBe('');
    expect(backend.downloadSource).toBe('modelscope');
    expect(backend.maxResults).toBe(8);
  });

  it('拒绝缺少远程密钥的启用配置', () => {
    const message = getEmbeddingConfigMessage({
      ...DEFAULT_SETTINGS.embedding,
      enabled: true,
      provider: 'openai',
      baseUrl: 'https://api.openai.com/v1',
      apiKey: '',
    });

    expect(message).toContain('Embedding 密钥');
  });
});

describe('getAiConfigMessage', () => {
  it('还没选服务商时，整句指向设置里的 AI 设置', () => {
    expect(getAiConfigMessage({ ...DEFAULT_SETTINGS.ai, provider: 'none' })).toBe(
      '还没设置 AI。去「设置 → AI 设置」，选一个服务商（比如 DeepSeek、通义、OpenAI），填上它给你的密钥和模型名，再点「测试连接」。',
    );
  });
});

describe('getAiConfigMessage：几种没填完整的情况都说人话', () => {
  it('自定义服务没填请求地址：指到「高级」，不说「OpenAI 兼容接口」', () => {
    const message = getAiConfigMessage({
      ...DEFAULT_SETTINGS.ai,
      provider: 'openai-compatible',
      providerPreset: 'custom-openai-compatible',
      baseUrl: '',
      model: 'x',
      apiKey: 'k',
    });
    expect(message).toBe('请先到「设置 → AI 设置 → 高级」里填写请求地址。');
  });

  it('地址不是 https:// 开头：说明本机或局域网里的服务可以用 http://', () => {
    const message = getAiConfigMessage({
      ...DEFAULT_SETTINGS.ai,
      provider: 'openai-compatible',
      baseUrl: 'http://example.com/v1',
      model: 'x',
      apiKey: 'k',
    });
    expect(message).toBe('AI 的请求地址要以 https:// 开头；本机或局域网里的服务可以用 http://。');
  });

  it('没填模型名、没填密钥：说「模型名」「密钥」，不说「模型 ID」「API Key」', () => {
    const ready = { ...DEFAULT_SETTINGS.ai, provider: 'openai' as const, baseUrl: 'https://api.openai.com/v1' };
    expect(getAiConfigMessage({ ...ready, model: '', apiKey: 'k' })).toBe('请先在「设置 → AI 设置」里填写模型名。');
    expect(getAiConfigMessage({ ...ready, model: 'gpt-x', apiKey: '' })).toBe('请先在「设置 → AI 设置」里填写密钥。');
    expect(getAiConfigMessage({ ...ready, model: 'gpt-x', apiKey: 'k' })).toBeNull();
  });

  it('本机服务可以不填密钥', () => {
    expect(
      getAiConfigMessage({
        ...DEFAULT_SETTINGS.ai,
        provider: 'openai-compatible',
        baseUrl: 'http://127.0.0.1:11434/v1',
        model: 'llama3.1',
        apiKey: '',
      }),
    ).toBeNull();
  });
});

describe('设置 → AI 设置：服务商卡片', () => {
  const cloud = AI_PROVIDER_PRESETS.filter((preset) => !['none', 'ollama', 'lmstudio', 'custom-openai-compatible'].includes(preset.id));

  it('云端服务的副标统一写「需要：密钥 + 模型名」', () => {
    expect(cloud.length).toBeGreaterThanOrEqual(8);
    for (const preset of cloud) {
      expect(preset.description, preset.id).toBe('需要：密钥 + 模型名');
    }
  });

  it('本机服务只要模型名，自定义服务还要请求地址，关闭 AI 说不花钱', () => {
    const byId = (id: string) => AI_PROVIDER_PRESETS.find((preset) => preset.id === id);
    expect(byId('ollama')?.description).toBe('需要：模型名（本机服务，密钥可不填）');
    expect(byId('lmstudio')?.description).toBe('需要：模型名（本机服务，密钥可不填）');
    expect(byId('custom-openai-compatible')?.description).toBe('需要：请求地址 + 密钥 + 模型名');
    expect(byId('none')?.description).toContain('不会花钱');
  });

  it('卡片上的名字、副标、密钥标签、模型占位都没有协议名和旧叫法', () => {
    const everything = AI_PROVIDER_PRESETS.map((preset) => [preset.label, preset.description, preset.apiKeyLabel, preset.modelPlaceholder].join('|')).join('\n');
    expect(everything).not.toMatch(/Chat Completions|Messages API|OpenAI 兼容|兼容接口|API Key|模型 ID|Key（|知识搜索/);
    for (const preset of AI_PROVIDER_PRESETS) {
      expect(preset.apiKeyLabel, preset.id).toMatch(/^密钥/);
    }
  });

  it('每个服务商卡片都配得上现有的设置：云端有 https 地址，本机服务是 http', () => {
    for (const preset of cloud) {
      expect(preset.baseUrl, preset.id).toMatch(/^https:\/\//);
    }
    expect(AI_PROVIDER_PRESETS.find((preset) => preset.id === 'ollama')?.baseUrl).toMatch(/^http:\/\/127\.0\.0\.1/);
    expect(new Set(AI_PROVIDER_PRESETS.map((preset) => preset.id)).size).toBe(AI_PROVIDER_PRESETS.length);
  });

  it('「现在：…」：关闭时只说关了，不把「关闭 AI」当服务商名字读', () => {
    expect(describeCurrentAiService('关闭 AI', true)).toBe('现在：AI 已关闭');
    expect(describeCurrentAiService('DeepSeek', false)).toBe('现在：DeepSeek。换一个服务商后，要填它对应的密钥和模型名。');
    expect(describeCurrentAiService('关闭 AI', true)).not.toContain('当前使用');
  });

  it('测试连接旁说清会真实调用一次 AI、会产生很少的费用', () => {
    expect(AI_TEST_COST_NOTE).toBe('会真实调用一次 AI，会产生很少的费用。');
  });

  it('自动整理的开关换成指路：在「设置 → 自动整理与备份」里设', () => {
    expect(AI_AUTO_ORGANIZE_POINTER).toBe('要不要自动整理、每天最多花多少，在「设置 → 自动整理与备份」里设');
  });

  it('协议名只在「高级」里的请求地址帮助里出现', () => {
    expect(aiBaseUrlHelp('openai-compatible')).toContain('Chat Completions');
    expect(aiBaseUrlHelp('openai')).not.toContain('Chat Completions');
    expect(aiBaseUrlHelp('anthropic')).not.toContain('Messages');
  });
});

describe('密钥和钥匙串文案', () => {
  it('密钥帮助：已保存、本机服务可留空、普通情况各一句，都说钥匙串', () => {
    expect(aiKeyHelpText({ hasSavedKey: true, provider: 'openai' })).toBe('已保存密钥；输入新密钥可替换，或清除后重新设置。');
    expect(aiKeyHelpText({ hasSavedKey: false, provider: 'openai-compatible' })).toContain('本机服务时可以留空');
    const plain = aiKeyHelpText({ hasSavedKey: false, provider: 'anthropic' });
    expect(plain).toBe('密钥（AI 服务商给你的一串密码）只保存在 Mac 的钥匙串里，设置文件里只记请求地址和模型名。');
    for (const text of [plain, aiKeyHelpText({ hasSavedKey: false, provider: 'openai-compatible' })]) {
      expect(text).toContain('钥匙串');
      expect(text).not.toMatch(/系统凭据管理器|API Key|模型 ID/);
    }
  });

  it('读写钥匙串失败：让人点「允许」或到「钥匙串访问」里检查权限，不再说系统凭据管理器', () => {
    expect(KEYCHAIN_PERMISSION_HELP).toBe('没能读写钥匙串。请在弹出的授权框里点「允许」，或到「钥匙串访问」里检查 Fox Stars Lab 的权限。');
    expect(keychainFailureText('read')).toBe(`密钥状态读取失败，${KEYCHAIN_PERMISSION_HELP}`);
    expect(keychainFailureText('save')).toBe(`密钥保存失败，${KEYCHAIN_PERMISSION_HELP}`);
    expect(`${keychainFailureText('read')}${keychainFailureText('save')}`).not.toMatch(/系统凭据管理器|API Key/);
  });

  it('密钥输入框下面随保存状态变的小字', () => {
    expect(aiKeySaveStatusText('idle')).toBeNull();
    expect(aiKeySaveStatusText('saving')).toBe('正在把密钥存进钥匙串…');
    expect(aiKeySaveStatusText('saved')).toBe('密钥已保存在 Mac 的钥匙串里。');
    expect(aiKeySaveStatusText('error')).toBe(KEYCHAIN_PERMISSION_HELP);
  });
});
