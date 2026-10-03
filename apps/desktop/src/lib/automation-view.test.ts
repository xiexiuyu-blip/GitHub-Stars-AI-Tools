import { describe, expect, it } from 'vitest';
import {
  approxRepos,
  automationHint,
  automationState,
  BUDGET_PRESETS,
  formatMinute,
  formatSize,
  formatTokens,
  itemPendingNote,
  parseDailyBudget,
  TOKENS_PER_REPO,
  type AutomationStatus,
} from './automation-view';

function status(overrides: Partial<AutomationStatus> = {}): AutomationStatus {
  return {
    enabled: true,
    dailyBudgetTokens: 100_000,
    paused: false,
    pauseReason: null,
    tokens24h: 0,
    budgetRemaining: 100_000,
    queued: 0,
    running: 0,
    failed: 0,
    providerConfigured: true,
    model: 'gpt-x',
    ...overrides,
  };
}

describe('parseDailyBudget', () => {
  it('合法整数原样返回，0 也合法（等于关闭）', () => {
    expect(parseDailyBudget('0')).toBe(0);
    expect(parseDailyBudget(' 100000 ')).toBe(100_000);
  });

  it('空、负数、小数、字母都给出人话提示', () => {
    expect(parseDailyBudget('')).toBe('请先填写每天额度');
    expect(parseDailyBudget('   ')).toBe('请先填写每天额度');
    expect(parseDailyBudget('-5')).toBe('每天额度不能小于 0');
    expect(parseDailyBudget('1.5')).toBe('每天额度要填整数');
    expect(parseDailyBudget('10万')).toBe('每天额度要填整数');
  });

  it('大到不是安全整数的也拦住', () => {
    expect(parseDailyBudget('99999999999999999999')).toBe('每天额度要填整数');
  });
});

describe('approxRepos / formatTokens', () => {
  it('按每个仓库 4000 token 估，向下取整', () => {
    expect(TOKENS_PER_REPO).toBe(4000);
    expect(approxRepos(100_000)).toBe(25);
    expect(approxRepos(200_000)).toBe(50);
    expect(approxRepos(3999)).toBe(0);
    expect(approxRepos(-1)).toBe(0);
    expect(approxRepos(Number.NaN)).toBe(0);
  });

  it('档位文案里写的「约 N 个」和估算一致', () => {
    for (const preset of BUDGET_PRESETS.filter((item) => item.tokens > 0)) {
      expect(preset.hint).toContain(`约 ${approxRepos(preset.tokens)} 个`);
    }
  });

  it('1 万以上写成「万」，整万去掉小数点', () => {
    expect(formatTokens(0)).toBe('0');
    expect(formatTokens(9999)).toBe('9999');
    expect(formatTokens(10_000)).toBe('1 万');
    expect(formatTokens(100_000)).toBe('10 万');
    expect(formatTokens(15_000)).toBe('1.5 万');
    expect(formatTokens(600_000)).toBe('60 万');
  });

  it('负数和 NaN 当 0', () => {
    expect(formatTokens(-5)).toBe('0');
    expect(formatTokens(Number.NaN)).toBe('0');
  });
});

describe('automationState', () => {
  it('还没读到时只显示「读取中」', () => {
    expect(automationState(null)).toEqual({ label: '读取中', hue: 220, detail: '' });
  });

  it('没配置 AI 服务：提示去设置里配，优先级最高', () => {
    const state = automationState(status({ providerConfigured: false, paused: true }));
    expect(state.label).toBe('还没配置 AI 服务');
    expect(state.detail).toContain('设置');
    expect(state.hue).toBe(36);
  });

  it('被暂停：显示暂停原因，红色', () => {
    const state = automationState(
      status({ paused: true, pauseReason: { kind: 'auth', message: '密钥无效，已暂停', at: '2026-10-03T00:00:00Z' } }),
    );
    expect(state).toEqual({ label: '已暂停', hue: 4, detail: '密钥无效，已暂停' });
  });

  it('手动暂停没有原因时也有一句人话', () => {
    expect(automationState(status({ paused: true })).detail).toContain('继续整理');
  });

  it('额度是 0（未开启）：明确说不会花钱', () => {
    const state = automationState(status({ enabled: false, dailyBudgetTokens: 0, budgetRemaining: 0, queued: 401 }));
    expect(state.label).toBe('未开启');
    expect(state.detail).toContain('不会花钱');
  });

  it('开了但今天额度用完：黄色，说明明天继续', () => {
    const state = automationState(status({ budgetRemaining: 0, queued: 10 }));
    expect(state.label).toBe('今天的额度用完了');
    expect(state.hue).toBe(36);
    expect(state.detail).toContain('明天');
  });

  it('开了、有额度、有排队或进行中：绿色「整理中」', () => {
    expect(automationState(status({ queued: 3 })).label).toBe('整理中');
    expect(automationState(status({ running: 1 })).label).toBe('整理中');
    expect(automationState(status({ queued: 3 })).hue).toBe(148);
  });

  it('开了、有额度、没东西可整理：绿色待命', () => {
    const state = automationState(status());
    expect(state.label).toBe('已开启，暂时没有要整理的');
    expect(state.hue).toBe(148);
  });
});

describe('formatMinute / formatSize', () => {
  it('无法解析的时间原样返回', () => {
    expect(formatMinute('不是时间')).toBe('不是时间');
  });

  it('时间补零到分钟', () => {
    expect(formatMinute('2026-01-02T03:04:05')).toBe('2026-01-02 03:04');
  });

  it('大小保留一位小数的 MB', () => {
    expect(formatSize(13_100_000)).toBe('12.5 MB');
    expect(formatSize(0)).toBe('0.0 MB');
  });
});

describe('automationHint', () => {
  it('读不到状态、或者没有要整理的：不打扰', () => {
    expect(automationHint(null, 10)).toBeNull();
    expect(automationHint(status(), 0)).toBeNull();
    expect(automationHint(status(), Number.NaN)).toBeNull();
  });

  it('没配置 AI 服务 > 暂停 > 没开启 > 额度用完 > 整理中 > 没排队，按这个顺序挑', () => {
    expect(automationHint(status({ providerConfigured: false, paused: true, enabled: false }), 5)?.icon).toBe('key');
    expect(automationHint(status({ paused: true, enabled: false }), 5)?.title).toBe('自动整理被暂停了');
    expect(automationHint(status({ enabled: false, budgetRemaining: 0 }), 5)?.actionLabel).toBe('开启自动整理');
    expect(automationHint(status({ budgetRemaining: 0, queued: 3 }), 5)?.title).toBe('今天的整理额度用完了');
    expect(automationHint(status({ queued: 3 }), 5)?.title).toBe('正在后台整理，还剩 5 个');
    expect(automationHint(status(), 5)?.actionLabel).toBe('去排队');
  });

  it('没开启时明说数量和原因，并给「开启自动整理」', () => {
    const hint = automationHint(status({ enabled: false, dailyBudgetTokens: 0, budgetRemaining: 0 }), 318);
    expect(hint?.title).toBe('还有 318 个收藏没有中文说明');
    expect(hint?.detail).toContain('还没开启');
    expect(hint?.actionLabel).toBe('开启自动整理');
  });

  it('暂停时带上原因', () => {
    const hint = automationHint(
      status({ paused: true, pauseReason: { kind: 'auth', message: '密钥无效', at: '2026-10-03T00:00:00Z' } }),
      5,
    );
    expect(hint?.detail).toBe('密钥无效');
  });

  it('整理中显示今天用了多少，没有按钮', () => {
    const hint = automationHint(status({ queued: 2, tokens24h: 40_000 }), 7);
    expect(hint?.detail).toBe('今天已用约 4 万 token，约整理了 10 个。');
    expect(hint?.actionLabel).toBeNull();
  });
});

describe('itemPendingNote', () => {
  it('每种状态都说得清楚，而且都带「GitHub 上的原文」的衔接', () => {
    expect(itemPendingNote(null).text).toContain('GitHub 上的原文');
    expect(itemPendingNote(status({ providerConfigured: false })).actionLabel).toBe('去看看');
    expect(itemPendingNote(status({ paused: true })).text).toContain('暂停');
    expect(itemPendingNote(status({ enabled: false })).text).toContain('还没开启');
    expect(itemPendingNote(status({ budgetRemaining: 0 })).text).toContain('明天');
    expect(itemPendingNote(status({ queued: 1 })).text).toContain('整理队列');
  });

  it('暂停原因放在括号里', () => {
    const note = itemPendingNote(status({ paused: true, pauseReason: { kind: 'x', message: '密钥无效', at: '' } }));
    expect(note.text).toContain('（密钥无效）');
  });

  it('只有需要你动手的状态才有按钮', () => {
    expect(itemPendingNote(status({ queued: 1 })).actionLabel).toBeNull();
    expect(itemPendingNote(status()).actionLabel).toBeNull();
    expect(itemPendingNote(status({ enabled: false })).actionLabel).toBe('开启自动整理');
  });
});
