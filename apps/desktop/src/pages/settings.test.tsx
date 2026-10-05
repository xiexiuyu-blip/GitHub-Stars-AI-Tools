import { describe, expect, it } from 'vitest';
import SOURCE from '@/pages/settings.tsx?raw';

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
