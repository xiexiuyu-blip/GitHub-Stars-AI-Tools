import { describe, expect, it } from 'vitest';
import {
  approxRepos,
  automationHint,
  automationState,
  BUDGET_PRESETS,
  estimateFullScanTokens,
  formatMinute,
  formatSize,
  formatTokens,
  FULL_SCAN_TOKENS_PER_REPO,
  fullScanPanel,
  fullScanPending,
  fullScanProgress,
  itemPendingNote,
  lastFullScanText,
  parseDailyBudget,
  readmeCheckText,
  type LastReadmeCheck,
  regenerateNote,
  roundedEstimate,
  TOKENS_PER_REPO,
  type AutomationStatus,
  type FullScan,
  type LastFullScan,
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

  it('大于 0 又小于 1.2 万的拦住：连一个仓库的预留都放不下，等于什么都不会整理', () => {
    expect(parseDailyBudget('1000')).toContain('至少要 12000 token');
    expect(parseDailyBudget('11999')).toContain('至少要 12000 token');
    expect(parseDailyBudget('12000')).toBe(12_000);
    expect(parseDailyBudget('0')).toBe(0);
  });
});

describe('approxRepos / formatTokens', () => {
  it('按每个仓库 6000 token 估（真实记录：成功的卡平均约 5.7–6.3 千），向下取整', () => {
    expect(TOKENS_PER_REPO).toBe(6000);
    expect(approxRepos(100_000)).toBe(16);
    expect(approxRepos(200_000)).toBe(33);
    expect(approxRepos(5999)).toBe(0);
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
    expect(hint?.detail).toBe('最近 24 小时已用约 4 万 token，约整理了 6 个。');
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

describe('regenerateNote', () => {
  it('读不到状态：只说已加入队列，不许诺', () => {
    expect(regenerateNote(null)).toEqual({ title: '已加入整理队列', actionLabel: null });
  });

  it('没开额度：明说要开启才会重做、不开不花钱，并给「去开启」', () => {
    const note = regenerateNote(status({ enabled: false, dailyBudgetTokens: 0, budgetRemaining: 0 }));
    expect(note.sub).toContain('开启后才会重新整理');
    expect(note.sub).toContain('不花钱');
    expect(note.actionLabel).toBe('去开启');
  });

  it('没配置服务 / 暂停 / 额度用完 / 在跑，各说各的', () => {
    expect(regenerateNote(status({ providerConfigured: false })).actionLabel).toBe('去看看');
    expect(regenerateNote(status({ paused: true })).sub).toContain('暂停');
    expect(regenerateNote(status({ budgetRemaining: 0 })).sub).toContain('明天');
    const running = regenerateNote(status({ queued: 2 }));
    expect(running.sub).toContain('后台重新整理');
    expect(running.actionLabel).toBeNull();
  });
});

function fullScan(overrides: Partial<FullScan> = {}): FullScan {
  return {
    startedAt: '2026-10-04T04:00:00Z',
    until: '2026-10-06T04:00:00Z',
    tokensUsed: 620_000,
    total: 388,
    remaining: 268,
    ...overrides,
  };
}

function lastScan(overrides: Partial<LastFullScan> = {}): LastFullScan {
  return {
    startedAt: '2026-10-04T04:00:00Z',
    finishedAt: '2026-10-04T12:30:00',
    tokensUsed: 620_000,
    reason: 'stopped',
    total: 388,
    remaining: 268,
    ...overrides,
  };
}

describe('estimateFullScanTokens', () => {
  it('按每个 6500 token 估算（真实记录：成功的卡平均约 5.7–6.3 千，略高一点留给偶尔的重试），向最接近的整数取整', () => {
    expect(FULL_SCAN_TOKENS_PER_REPO).toBe(6500);
    expect(estimateFullScanTokens(388)).toBe(2_522_000);
    expect(estimateFullScanTokens(1)).toBe(6500);
    expect(estimateFullScanTokens(1.2)).toBe(7800);
  });

  it('不是有限数或 ≤ 0 时返回 0', () => {
    expect(estimateFullScanTokens(0)).toBe(0);
    expect(estimateFullScanTokens(-3)).toBe(0);
    expect(estimateFullScanTokens(Number.NaN)).toBe(0);
    expect(estimateFullScanTokens(Number.POSITIVE_INFINITY)).toBe(0);
  });
});

describe('roundedEstimate', () => {
  it('100 万以上按 10 万取整，否则按 1 万取整，0 就是 0', () => {
    expect(roundedEstimate(2_017_600)).toBe('200 万');
    expect(roundedEstimate(2_065_000)).toBe('210 万');
    expect(roundedEstimate(35_000)).toBe('4 万');
    expect(roundedEstimate(0)).toBe('0');
  });

  it('不足 1 万的正数至少写成 1 万，非法值当 0', () => {
    expect(roundedEstimate(5200)).toBe('1 万');
    expect(roundedEstimate(1)).toBe('1 万');
    expect(roundedEstimate(Number.NaN)).toBe('0');
    expect(roundedEstimate(-20)).toBe('0');
  });
});

describe('fullScanPending', () => {
  it('有 needsOrganizing 就用它，0 也不退回排队数', () => {
    expect(fullScanPending(status({ needsOrganizing: 388, queued: 1, running: 1 }))).toBe(388);
    expect(fullScanPending(status({ needsOrganizing: 0, queued: 9, running: 2 }))).toBe(0);
  });

  it('缺失或 null 时用排队 + 进行中；不是有限数或小于 0 当 0', () => {
    expect(fullScanPending(status({ queued: 4, running: 1 }))).toBe(5);
    expect(fullScanPending(status({ needsOrganizing: null, queued: 2, running: 3 }))).toBe(5);
    expect(fullScanPending(status({ needsOrganizing: -1, queued: 8 }))).toBe(0);
    expect(fullScanPending(status({ queued: Number.NaN, running: 1 }))).toBe(0);
  });
});

describe('fullScanProgress', () => {
  it('正常进度：已整理夹在 0 和总数之间，百分比向下取整', () => {
    expect(fullScanProgress(fullScan())).toEqual({ done: 120, total: 388, percent: 30 });
    expect(fullScanProgress(fullScan({ total: 3, remaining: 2 }))).toEqual({ done: 1, total: 3, percent: 33 });
  });

  it('remaining 大于 total 时已整理是 0，百分比是 0', () => {
    expect(fullScanProgress(fullScan({ total: 10, remaining: 25 }))).toEqual({ done: 0, total: 10, percent: 0 });
  });

  it('total 为 0 时百分比是 0（即使 remaining 也是 0）', () => {
    expect(fullScanProgress(fullScan({ total: 0, remaining: 0 }))).toEqual({ done: 0, total: 0, percent: 0 });
    expect(fullScanProgress(fullScan({ total: 0, remaining: 4 }))).toEqual({ done: 0, total: 0, percent: 0 });
  });

  it('remaining 为 0 且 total 大于 0 时百分比一定是 100', () => {
    expect(fullScanProgress(fullScan({ total: 388, remaining: 0 }))).toEqual({ done: 388, total: 388, percent: 100 });
  });
});

describe('fullScanPanel', () => {
  it('还没读到状态是 loading', () => {
    expect(fullScanPanel(null)).toEqual({ kind: 'loading' });
  });

  it('没配置 AI 服务是 unavailable，原因让人去设置里保存密钥', () => {
    expect(fullScanPanel(status({ providerConfigured: false }))).toEqual({
      kind: 'unavailable',
      reason: '先到「设置 → 引擎」里选好服务商和模型、保存密钥，才能整理。',
    });
  });

  it('没有要整理的是 nothing，有上次结果就带上一行', () => {
    const last = lastScan({ reason: 'completed', remaining: 0, total: 388, tokensUsed: 2_018_000 });
    const panel = fullScanPanel(status({ needsOrganizing: 0, queued: 3, lastFullScan: last }));
    expect(panel.kind).toBe('nothing');
    if (panel.kind !== 'nothing') return;
    expect(panel.last).toBe('上次全量整理：2026-10-04 12:30 完成，整理了 388 个，用了约 201.8 万 token。');
  });

  it('有要整理的是 ready：388 个约 250 万 token', () => {
    const panel = fullScanPanel(status({ needsOrganizing: 388, lastFullScan: lastScan() }));
    expect(panel).toMatchObject({
      kind: 'ready',
      pending: 388,
      estimate: '250 万',
      last: '上次全量整理：2026-10-04 12:30 被停止，已整理 120 / 388 个，用了约 62 万 token。',
    });
  });

  it('正在全量整理是 running，带上进度、用量和暂停原因', () => {
    const panel = fullScanPanel(status({
      unlimited: true,
      paused: true,
      pauseReason: { kind: 'errors', message: '连续失败', at: '2026-10-04T05:00:00Z' },
      fullScan: fullScan(),
    }));
    expect(panel).toEqual({
      kind: 'running',
      done: 120,
      total: 388,
      percent: 30,
      tokensUsed: 620_000,
      failed: 0,
      startedAt: '2026-10-04T04:00:00Z',
      until: '2026-10-06T04:00:00Z',
      paused: true,
      pausedReason: '连续失败',
    });
  });

  it('后台给了失败个数就带上，缺失、负数、非数字都当 0', () => {
    const run = (failed: number | undefined) => {
      const panel = fullScanPanel(status({ unlimited: true, fullScan: fullScan({ failed }) }));
      return panel.kind === 'running' ? panel.failed : -1;
    };
    expect(run(7)).toBe(7);
    expect(run(undefined)).toBe(0);
    expect(run(-3)).toBe(0);
    expect(run(Number.NaN)).toBe(0);
  });

  it('有 fullScan 时，就算还没配好服务也是 running（进行中优先于没配服务）', () => {
    const panel = fullScanPanel(status({
      providerConfigured: false,
      unlimited: true,
      fullScan: fullScan({ total: 10, remaining: 10, tokensUsed: 0 }),
    }));
    expect(panel.kind).toBe('running');
  });

  it('unlimited 明确是 false 时，留着 fullScan 也不算在跑', () => {
    const panel = fullScanPanel(status({
      unlimited: false,
      needsOrganizing: 12,
      fullScan: fullScan(),
    }));
    expect(panel.kind).toBe('ready');
  });

  it('unlimited 缺失但 fullScan 还在：没有被明确关掉，面板仍是 running', () => {
    const panel = fullScanPanel(status({
      providerConfigured: false,
      fullScan: fullScan({ total: 4, remaining: 4 }),
    }));
    expect(panel.kind).toBe('running');
  });

  it('unlimited 明确关掉、又没配服务：按顺序落到 unavailable', () => {
    const panel = fullScanPanel(status({
      unlimited: false,
      providerConfigured: false,
      fullScan: fullScan(),
    }));
    expect(panel.kind).toBe('unavailable');
  });
});

describe('lastFullScanText', () => {
  it('完成、停止、到时间、用量超了，四句各不相同', () => {
    expect(lastFullScanText(lastScan({ reason: 'completed', remaining: 0, total: 388, tokensUsed: 2_018_000 }))).toBe(
      '上次全量整理：2026-10-04 12:30 完成，整理了 388 个，用了约 201.8 万 token。',
    );
    expect(lastFullScanText(lastScan({ reason: 'stopped' }))).toBe(
      '上次全量整理：2026-10-04 12:30 被停止，已整理 120 / 388 个，用了约 62 万 token。',
    );
    expect(lastFullScanText(lastScan({ reason: 'expired' }))).toBe(
      '上次全量整理：2026-10-04 12:30 到时间自动结束，已整理 120 / 388 个，用了约 62 万 token。',
    );
    expect(lastFullScanText(lastScan({ reason: 'ceiling' }))).toBe(
      '上次全量整理：2026-10-04 12:30 因为用量远超预计，已自动停下（已整理 120 / 388 个，用了约 62 万 token）。要继续可以再开一次。',
    );
  });

  it('有失败的，整理好的个数要扣掉失败，并单独说一句；没有失败字段（旧后台）和以前完全一样', () => {
    expect(lastFullScanText(lastScan({ reason: 'completed', remaining: 0, total: 388, tokensUsed: 2_018_000, failed: 24 }))).toBe(
      '上次全量整理：2026-10-04 12:30 完成，整理了 364 个，另有 24 个没整理成功（可以再开一次全量整理重试），用了约 201.8 万 token。',
    );
    expect(lastFullScanText(lastScan({ reason: 'stopped', failed: 5 }))).toBe(
      '上次全量整理：2026-10-04 12:30 被停止，已整理 115 / 388 个，另有 5 个没整理成功，用了约 62 万 token。',
    );
    expect(lastFullScanText(lastScan({ reason: 'ceiling', failed: 1 }))).toContain('另有 1 个没整理成功');
    expect(lastFullScanText(lastScan({ reason: 'stopped', failed: 0 }))).not.toContain('没整理成功');
    expect(lastFullScanText(lastScan({ reason: 'stopped', failed: -2 }))).not.toContain('没整理成功');
    // 全部失败：整理好的是 0，不是 3
    expect(lastFullScanText(lastScan({ reason: 'completed', total: 3, remaining: 0, failed: 3, tokensUsed: 30_000 }))).toBe(
      '上次全量整理：2026-10-04 12:30 完成，整理了 0 个，另有 3 个没整理成功（可以再开一次全量整理重试），用了约 3 万 token。',
    );
    // 失败数比处理数还大（数据不一致）：不出现负数
    expect(lastFullScanText(lastScan({ reason: 'stopped', total: 10, remaining: 8, failed: 9 }))).toContain('已整理 0 / 10 个');
  });

  it('失败太多被自动停下：说清原因、让人先检查模型', () => {
    expect(lastFullScanText(lastScan({ reason: 'failures', total: 388, remaining: 300, failed: 77, tokensUsed: 700_000 }))).toBe(
      '上次全量整理：2026-10-04 12:30 因为失败太多，已自动停下（已整理 11 / 388 个，另有 77 个没整理成功，用了约 70 万 token）。先到「设置 → 引擎」检查模型，再重试。',
    );
  });

  it('没有记录，或 reason 认不出，返回 null', () => {
    expect(lastFullScanText(null)).toBeNull();
    expect(lastFullScanText(undefined)).toBeNull();
    const unknown = lastScan();
    (unknown as { reason: string }).reason = 'other';
    expect(lastFullScanText(unknown)).toBeNull();
  });
});

describe('全量整理期间不说额度用完', () => {
  const during = () => status({
    unlimited: true,
    budgetRemaining: 0,
    queued: 10,
    tokens24h: 80_000,
    fullScan: fullScan({ tokensUsed: 620_000 }),
  });

  it('顶部状态是「全量整理中」，不提额度用完', () => {
    const state = automationState(during());
    expect(state).toEqual({
      label: '全量整理中（不限额）',
      hue: 148,
      detail: '在后台连续整理，不受每天额度限制；整理完会自动回到日常的增量整理。',
    });
    expect(state.detail).not.toContain('用完');
  });

  it('今日 / 资料库提示说还在全量整理，按钮是「查看进度」，个数用传入的 pending', () => {
    const hint = automationHint(during(), 5);
    expect(hint).toEqual({
      hue: 148,
      icon: 'autorenew',
      title: '正在全量整理（不限额），还剩 5 个',
      detail: '不受每天额度限制，已用约 62 万 token。整理完会自动回到日常整理。',
      actionLabel: '查看进度',
    });
    expect(hint?.title).not.toContain('额度用完');
    expect(hint?.detail).not.toContain('用完');
  });

  it('这一批已经排完、最后几个在收尾（队列是 0）：提示不落到「它们还没进整理队列」', () => {
    const hint = automationHint(status({ unlimited: true, budgetRemaining: 0, queued: 0, running: 0, fullScan: fullScan({ tokensUsed: 2_000_000 }) }), 3);
    expect(hint).toEqual({
      hue: 148,
      icon: 'autorenew',
      title: '正在全量整理（不限额）',
      detail: '队列里这一批已经排完，整理完会自动回到日常整理。已用约 200 万 token。',
      actionLabel: '查看进度',
    });
    expect(hint?.detail).not.toContain('还没进整理队列');
  });

  it('没有 fullScan.tokensUsed 时，提示改用最近 24 小时的用量', () => {
    const hint = automationHint(status({ unlimited: true, budgetRemaining: 0, queued: 2, tokens24h: 40_000, fullScan: null }), 7);
    expect(hint?.detail).toBe('不受每天额度限制，已用约 4 万 token。整理完会自动回到日常整理。');
  });

  it('详情不说额度用完，改说正在全量整理', () => {
    const note = itemPendingNote(during());
    expect(note).toEqual({
      text: '正在全量整理，整理好会自动出现在这里。先看 GitHub 上的原文：',
      actionLabel: null,
    });
    expect(note.text).not.toContain('额度');
  });

  it('队列是 0、额度也是 0 时，只要 unlimited，详情和顶上提示都不说额度用完', () => {
    const pausedBudget = status({ unlimited: true, budgetRemaining: 0, queued: 0, running: 0 });
    const note = itemPendingNote(pausedBudget);
    expect(note.text).toContain('正在全量整理');
    expect(note.text).not.toContain('额度用完');
    const hint = automationHint(pausedBudget, 3);
    expect(hint?.title ?? '').not.toContain('额度用完');
    expect(hint?.detail ?? '').not.toContain('用完');
    expect(automationState(pausedBudget).label).toBe('全量整理中（不限额）');
    expect(regenerateNote(pausedBudget).sub ?? '').not.toContain('额度');
  });

  it('重新整理的提示说会在全量整理里重做，不说额度用完', () => {
    const note = regenerateNote(during());
    expect(note).toEqual({
      title: '已加入整理队列',
      sub: '正在全量整理，会在后台重新整理，好了会自动更新',
      actionLabel: null,
    });
    expect(note.sub).not.toContain('额度');
  });

  it('暂停优先于 unlimited：徽章仍是「已暂停」', () => {
    const state = automationState(status({ paused: true, unlimited: true, budgetRemaining: 0, queued: 4 }));
    expect(state.label).toBe('已暂停');
    expect(state.hue).toBe(4);
    expect(automationHint(status({ paused: true, unlimited: true, budgetRemaining: 0, queued: 4 }), 5)?.title).toBe('自动整理被暂停了');
    expect(itemPendingNote(status({ paused: true, unlimited: true, budgetRemaining: 0 })).text).toContain('暂停');
    expect(regenerateNote(status({ paused: true, unlimited: true, budgetRemaining: 0 })).sub).toContain('暂停');
  });
});

describe('额度用完时指出可以开全量整理', () => {
  it('不是全量整理时，提示改去工具箱，并说明可以不限额整理完', () => {
    const hint = automationHint(status({ budgetRemaining: 0, queued: 3 }), 5);
    expect(hint?.title).toBe('今天的整理额度用完了');
    expect(hint?.actionLabel).toBe('去工具箱');
    expect(hint?.detail).toBe('还有 5 个没整理，明天会自动接着做；想现在就整理完，可以在工具箱里开启「全量整理」（不限额）。');
  });

  it('详情和重新整理的「额度用完」都补上一句可以开全量整理', () => {
    expect(itemPendingNote(status({ budgetRemaining: 0 })).text).toContain('也可以在工具箱里开启全量整理');
    expect(regenerateNote(status({ budgetRemaining: 0 })).sub).toContain('也可以在工具箱里开启全量整理');
  });

  it('unlimited 字段缺失时，仍走原来的「额度用完」，不说正在全量整理', () => {
    expect(automationState(status({ budgetRemaining: 0, queued: 10 })).label).toBe('今天的额度用完了');
    expect(automationHint(status({ budgetRemaining: 0, queued: 3 }), 5)?.title).toBe('今天的整理额度用完了');
    expect(itemPendingNote(status({ budgetRemaining: 0 })).text).toContain('明天');
    expect(itemPendingNote(status({ queued: 1 })).text).toContain('整理队列');
    expect(regenerateNote(status({ budgetRemaining: 0 })).sub).toContain('明天');
  });
});

describe('readmeCheckText', () => {
  const now = new Date(2026, 9, 5, 12, 0, 0);
  function check(overrides: Partial<LastReadmeCheck> = {}): LastReadmeCheck {
    return {
      checkedAt: new Date(2026, 9, 4, 18, 20, 0).toISOString(),
      checked: 23,
      refreshed: 8,
      material: 3,
      queued: 2,
      ...overrides,
    };
  }

  it('没检查过不显示', () => {
    expect(readmeCheckText(null, now)).toBeNull();
    expect(readmeCheckText(undefined, now)).toBeNull();
  });

  it('有明显改动时写出排队', () => {
    expect(readmeCheckText(check(), now)).toBe(
      'README 更新检查：昨天 18:20 · 看了 23 个 · 有明显改动 3 个，已排队重新整理',
    );
  });

  it('没有明显改动时不说排队', () => {
    expect(readmeCheckText(check({ material: 0, queued: 0 }), now)).toBe(
      'README 更新检查：昨天 18:20 · 看了 23 个 · 没有明显改动',
    );
    expect(readmeCheckText(check({ material: Number.NaN }), now)).toContain('没有明显改动');
    expect(readmeCheckText(check({ material: -2 }), now)).toContain('没有明显改动');
  });

  it('更早的日期只写月日，当天写今天，不到一小时写刚刚', () => {
    expect(readmeCheckText(check({ checkedAt: new Date(2026, 8, 1, 8, 0, 0).toISOString() }), now)).toContain('9月1日');
    expect(readmeCheckText(check({ checkedAt: new Date(2026, 9, 5, 9, 30, 0).toISOString() }), now)).toContain('今天 09:30');
    expect(readmeCheckText(check({ checkedAt: new Date(2026, 9, 5, 11, 30, 0).toISOString() }), now)).toContain('刚刚');
    expect(readmeCheckText(check({ checkedAt: new Date(2026, 9, 5, 11, 0, 0).toISOString() }), now)).toContain('今天 11:00');
  });

  it('看了 0 个也照实写', () => {
    expect(readmeCheckText(check({ checked: 0, material: 0 }), now)).toBe(
      'README 更新检查：昨天 18:20 · 看了 0 个 · 没有明显改动',
    );
  });

  it('时间读不懂就整行不显示', () => {
    expect(readmeCheckText(check({ checkedAt: '不是时间' }), now)).toBeNull();
  });
});
