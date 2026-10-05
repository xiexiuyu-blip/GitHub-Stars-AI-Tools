/// <reference types="node" />
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  approxRepos,
  autoSyncDescription,
  automationHint,
  automationState,
  backupKindLabel,
  backupTimeLine,
  BUDGET_PRESETS,
  buildReadinessRows,
  CLEAR_LOCAL_DATA_DONE,
  CLEAR_LOCAL_DATA_FAILED,
  CLEAR_LOCAL_DATA_WARNING,
  CONNECT_SLOW,
  CONNECT_SLOW_WELCOME,
  dailyBudgetConfirmLead,
  estimateFullScanTokens,
  EXPORT_SECTION_HINT,
  exportSavedText,
  formatMinute,
  formatSize,
  formatSyncComplete,
  formatSyncStatus,
  formatTokens,
  FULL_SCAN_TOKENS_PER_REPO,
  fullScanConfirmLines,
  fullScanPanel,
  fullScanPending,
  fullScanProgress,
  GIST_BACKUP_LIBRARY,
  GIST_BACKUP_MARKS,
  GIST_IMPORT_WARNING,
  GIST_NEEDS_SCOPE,
  GIST_SECTION_TITLE,
  gistImportHint,
  GITHUB_TOKEN_EXPLAINER,
  GITHUB_TOKEN_EXTRA_SCOPES,
  GITHUB_TOKEN_SAVED_NOTE,
  GITHUB_TOKEN_SCOPES,
  GITHUB_TOKEN_WILL_SAVE_NOTE,
  itemPendingNote,
  lastFullScanText,
  parseDailyBudget,
  readmeCheckText,
  recentDoneCount,
  recentUsageLine,
  recentUsageMetric,
  type LastReadmeCheck,
  regenerateNote,
  roundedEstimate,
  runStatusText,
  RUNTIME_CHECK_COST_NOTE,
  RUNTIME_CHECK_TITLES,
  spentByEarlierFullScan,
  SYNC_INTERVAL_HINT,
  TOKENS_PER_REPO,
  WELCOME_COPY,
  WELCOME_STARTUP_HINT,
  withRecentDone,
  type AutomationStatus,
  type FullScan,
  type LastFullScan,
  type ReadinessInput,
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

  it('大于 0 又小于 1.2 万的拦住：连一个工具的预留都放不下，等于什么都不会整理', () => {
    expect(parseDailyBudget('1000')).toContain('至少要 1.2 万 token');
    expect(parseDailyBudget('11999')).toContain('至少要 1.2 万 token');
    expect(parseDailyBudget('12000')).toBe(12_000);
    expect(parseDailyBudget('0')).toBe(0);
  });
});

describe('approxRepos / formatTokens', () => {
  it('按每个工具 6000 token 估（真实记录：成功的卡平均约 5.7–6.3 千），向下取整', () => {
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

  it('开了、有额度、有等待或进行中：绿色「整理中」', () => {
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
  it('读不懂的时间原样返回', () => {
    expect(formatMinute('不是时间')).toBe('不是时间');
  });

  it('时间补零到分钟', () => {
    expect(formatMinute('2026-01-02T03:04:05')).toBe('1 月 2 日 03:04');
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

  it('没配置 AI 服务 > 暂停 > 没开启 > 额度用完 > 整理中 > 还没加入，按这个顺序挑', () => {
    expect(automationHint(status({ providerConfigured: false, paused: true, enabled: false }), 5)?.icon).toBe('key');
    expect(automationHint(status({ paused: true, enabled: false }), 5)?.title).toBe('自动整理被暂停了');
    expect(automationHint(status({ enabled: false, budgetRemaining: 0 }), 5)?.actionLabel).toBe('开启自动整理');
    expect(automationHint(status({ budgetRemaining: 0, queued: 3 }), 5)?.title).toBe('今天的整理额度用完了');
    expect(automationHint(status({ queued: 3 }), 5)?.title).toBe('正在后台整理，还剩 5 个');
    expect(automationHint(status(), 5)?.actionLabel).toBe('加入整理');
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
    expect(hint?.detail).toBe('最近 24 小时已用约 4 万 token。');
    expect(hint?.actionLabel).toBeNull();
  });
});

describe('itemPendingNote', () => {
  it('每种状态都说得清楚，而且都带「GitHub 上的原文」的衔接', () => {
    expect(itemPendingNote(null).text).toContain('GitHub 上的原文');
    expect(itemPendingNote(status({ providerConfigured: false })).actionLabel).toBe('去设置 AI');
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
    expect(regenerateNote(status({ providerConfigured: false })).actionLabel).toBe('去设置 AI');
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
  it('有 needsOrganizing 就用它，0 也不退回等待数', () => {
    expect(fullScanPending(status({ needsOrganizing: 388, queued: 1, running: 1 }))).toBe(388);
    expect(fullScanPending(status({ needsOrganizing: 0, queued: 9, running: 2 }))).toBe(0);
  });

  it('缺失或 null 时用等待数 + 进行中；不是有限数或小于 0 当 0', () => {
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
      reason: '先到「设置 → AI 设置」里选好服务商和模型、保存密钥，才能整理。',
    });
  });

  it('没有要整理的是 nothing，有上次结果就带上一行', () => {
    const last = lastScan({ reason: 'completed', remaining: 0, total: 388, tokensUsed: 2_018_000 });
    const panel = fullScanPanel(status({ needsOrganizing: 0, queued: 3, lastFullScan: last }));
    expect(panel.kind).toBe('nothing');
    if (panel.kind !== 'nothing') return;
    expect(panel.last).toBe('上次一次性全部整理：10 月 4 日 12:30 完成，整理了 388 个，用了约 201.8 万 token。');
  });

  it('有要整理的是 ready：388 个约 250 万 token', () => {
    const panel = fullScanPanel(status({ needsOrganizing: 388, lastFullScan: lastScan() }));
    expect(panel).toMatchObject({
      kind: 'ready',
      pending: 388,
      estimate: '250 万',
      last: '上次一次性全部整理：10 月 4 日 12:30 被停止，已整理 120 / 388 个，用了约 62 万 token。',
    });
  });

  it('正在一次性全部整理是 running，带上进度、花费和暂停原因', () => {
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
  it('完成、停止、到时间、花费超了，四句各不相同', () => {
    expect(lastFullScanText(lastScan({ reason: 'completed', remaining: 0, total: 388, tokensUsed: 2_018_000 }))).toBe(
      '上次一次性全部整理：10 月 4 日 12:30 完成，整理了 388 个，用了约 201.8 万 token。',
    );
    expect(lastFullScanText(lastScan({ reason: 'stopped' }))).toBe(
      '上次一次性全部整理：10 月 4 日 12:30 被停止，已整理 120 / 388 个，用了约 62 万 token。',
    );
    expect(lastFullScanText(lastScan({ reason: 'expired' }))).toBe(
      '上次一次性全部整理：10 月 4 日 12:30 到时间自动结束，已整理 120 / 388 个，用了约 62 万 token。',
    );
    expect(lastFullScanText(lastScan({ reason: 'ceiling' }))).toBe(
      '上次一次性全部整理：10 月 4 日 12:30 因为花费远超预计，已自动停下（已整理 120 / 388 个，用了约 62 万 token）。要继续可以再开一次。',
    );
  });

  it('有失败的，整理好的个数要扣掉失败，并单独说一句；没有失败字段（旧后台）和以前完全一样', () => {
    expect(lastFullScanText(lastScan({ reason: 'completed', remaining: 0, total: 388, tokensUsed: 2_018_000, failed: 24 }))).toBe(
      '上次一次性全部整理：10 月 4 日 12:30 完成，整理了 364 个，另有 24 个没整理成功（可以再开一次一次性全部整理重试），用了约 201.8 万 token。',
    );
    expect(lastFullScanText(lastScan({ reason: 'stopped', failed: 5 }))).toBe(
      '上次一次性全部整理：10 月 4 日 12:30 被停止，已整理 115 / 388 个，另有 5 个没整理成功，用了约 62 万 token。',
    );
    expect(lastFullScanText(lastScan({ reason: 'ceiling', failed: 1 }))).toContain('另有 1 个没整理成功');
    expect(lastFullScanText(lastScan({ reason: 'stopped', failed: 0 }))).not.toContain('没整理成功');
    expect(lastFullScanText(lastScan({ reason: 'stopped', failed: -2 }))).not.toContain('没整理成功');
    // 全部失败：整理好的是 0，不是 3
    expect(lastFullScanText(lastScan({ reason: 'completed', total: 3, remaining: 0, failed: 3, tokensUsed: 30_000 }))).toBe(
      '上次一次性全部整理：10 月 4 日 12:30 完成，整理了 0 个，另有 3 个没整理成功（可以再开一次一次性全部整理重试），用了约 3 万 token。',
    );
    // 失败数比处理数还大（数据不一致）：不出现负数
    expect(lastFullScanText(lastScan({ reason: 'stopped', total: 10, remaining: 8, failed: 9 }))).toContain('已整理 0 / 10 个');
  });

  it('失败太多被自动停下：说清原因、让人先检查模型', () => {
    expect(lastFullScanText(lastScan({ reason: 'failures', total: 388, remaining: 300, failed: 77, tokensUsed: 700_000 }))).toBe(
      '上次一次性全部整理：10 月 4 日 12:30 因为失败太多，已自动停下（已整理 11 / 388 个，另有 77 个没整理成功，用了约 70 万 token）。先到「设置 → AI 设置」检查模型，再重试。',
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

describe('一次性全部整理期间不说额度用完', () => {
  const during = () => status({
    unlimited: true,
    budgetRemaining: 0,
    queued: 10,
    tokens24h: 80_000,
    fullScan: fullScan({ tokensUsed: 620_000 }),
  });

  it('顶部状态是「一次性全部整理中」，不提额度用完', () => {
    const state = automationState(during());
    expect(state).toEqual({
      label: '一次性全部整理中',
      hue: 148,
      detail: '在后台连续整理，不受每天额度限制；整理完会自动回到每天自动整理。',
    });
    expect(state.detail).not.toContain('用完');
  });

  it('今日 / 资料库提示说还在一次性全部整理，按钮是「看整理进度」，个数用传入的 pending', () => {
    const hint = automationHint(during(), 5);
    expect(hint).toEqual({
      hue: 148,
      icon: 'autorenew',
      title: '正在一次性全部整理，还剩 5 个',
      detail: '不受每天额度限制，这一轮已整理 120 个（已用约 62 万 token）。整理完会自动回到每天自动整理。',
      actionLabel: '看整理进度',
    });
    expect(hint?.title).not.toContain('额度用完');
    expect(hint?.detail).not.toContain('用完');
  });

  it('这一批已经排完、最后几个在收尾（队列是 0）：提示不落到「它们还没进整理队列」', () => {
    const hint = automationHint(status({ unlimited: true, budgetRemaining: 0, queued: 0, running: 0, fullScan: fullScan({ tokensUsed: 2_000_000 }) }), 3);
    expect(hint).toEqual({
      hue: 148,
      icon: 'autorenew',
      title: '正在一次性全部整理',
      detail: '这一批已经做完，整理完会自动回到每天自动整理。这一轮已整理 120 个（已用约 200 万 token）。',
      actionLabel: '看整理进度',
    });
    expect(hint?.detail).not.toContain('还没进整理队列');
  });

  it('没有 fullScan.tokensUsed 时，提示改用最近 24 小时的花费', () => {
    const hint = automationHint(status({ unlimited: true, budgetRemaining: 0, queued: 2, tokens24h: 40_000, fullScan: null }), 7);
    expect(hint?.detail).toBe('不受每天额度限制，已用约 4 万 token。整理完会自动回到每天自动整理。');
  });

  it('详情不说额度用完，改说正在一次性全部整理', () => {
    const note = itemPendingNote(during());
    expect(note).toEqual({
      text: '正在一次性全部整理，整理好会自动出现在这里。先看 GitHub 上的原文：',
      actionLabel: null,
    });
    expect(note.text).not.toContain('额度');
  });

  it('队列是 0、额度也是 0 时，只要 unlimited，详情和顶上提示都不说额度用完', () => {
    const pausedBudget = status({ unlimited: true, budgetRemaining: 0, queued: 0, running: 0 });
    const note = itemPendingNote(pausedBudget);
    expect(note.text).toContain('正在一次性全部整理');
    expect(note.text).not.toContain('额度用完');
    const hint = automationHint(pausedBudget, 3);
    expect(hint?.title ?? '').not.toContain('额度用完');
    expect(hint?.detail ?? '').not.toContain('用完');
    expect(automationState(pausedBudget).label).toBe('一次性全部整理中');
    expect(regenerateNote(pausedBudget).sub ?? '').not.toContain('额度');
  });

  it('重新整理的提示说会在一次性全部整理里重做，不说额度用完', () => {
    const note = regenerateNote(during());
    expect(note).toEqual({
      title: '已加入整理队列',
      sub: '正在一次性全部整理，会在后台重新整理，好了会自动更新',
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

describe('额度用完时指出可以开一次性全部整理', () => {
  it('不是一次性全部整理时，提示去调整每天额度，并说明可以一次整理完', () => {
    const hint = automationHint(status({ budgetRemaining: 0, queued: 3 }), 5);
    expect(hint?.title).toBe('今天的整理额度用完了');
    expect(hint?.actionLabel).toBe('调整每天额度');
    expect(hint?.detail).toBe('还有 5 个没整理，明天会自动接着做；想现在就整理完，可以到「设置 → 自动整理与备份」开启「一次性全部整理」。');
  });

  it('详情和重新整理的「额度用完」都补上一句可以开一次性全部整理', () => {
    expect(itemPendingNote(status({ budgetRemaining: 0 })).text).toContain('也可以到「设置 → 自动整理与备份」开启一次性全部整理');
    expect(regenerateNote(status({ budgetRemaining: 0 })).sub).toContain('也可以到「设置 → 自动整理与备份」开启一次性全部整理');
  });

  it('unlimited 字段缺失时，仍走原来的「额度用完」，不说正在一次性全部整理', () => {
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

  it('有明显改动时写出已加入重新整理', () => {
    expect(readmeCheckText(check(), now)).toBe(
      '项目说明更新检查：昨天 18:20 · 看了 23 个 · 有明显改动 3 个，已加入重新整理',
    );
  });

  it('没有明显改动时不说重新整理', () => {
    expect(readmeCheckText(check({ material: 0, queued: 0 }), now)).toBe(
      '项目说明更新检查：昨天 18:20 · 看了 23 个 · 没有明显改动',
    );
    expect(readmeCheckText(check({ material: Number.NaN }), now)).toContain('没有明显改动');
    expect(readmeCheckText(check({ material: -2 }), now)).toContain('没有明显改动');
  });

  it('更早的日期只写月日，当天写今天，不到一小时写刚刚', () => {
    expect(readmeCheckText(check({ checkedAt: new Date(2026, 8, 1, 8, 0, 0).toISOString() }), now)).toContain('9 月 1 日');
    expect(readmeCheckText(check({ checkedAt: new Date(2026, 9, 5, 9, 30, 0).toISOString() }), now)).toContain('今天 09:30');
    expect(readmeCheckText(check({ checkedAt: new Date(2026, 9, 5, 11, 30, 0).toISOString() }), now)).toContain('刚刚');
    expect(readmeCheckText(check({ checkedAt: new Date(2026, 9, 5, 11, 0, 0).toISOString() }), now)).toContain('今天 11:00');
  });

  it('看了 0 个也照实写', () => {
    expect(readmeCheckText(check({ checked: 0, material: 0 }), now)).toBe(
      '项目说明更新检查：昨天 18:20 · 看了 0 个 · 没有明显改动',
    );
  });

  it('时间读不懂就整行不显示', () => {
    expect(readmeCheckText(check({ checkedAt: '不是时间' }), now)).toBeNull();
  });
});

describe('formatSyncStatus', () => {
  const now = new Date(2026, 9, 5, 18, 17, 0);

  it('今天只写时刻', () => {
    expect(formatSyncStatus(new Date(2026, 9, 5, 9, 5, 0).toISOString(), now)).toBe('上次同步：今天 09:05');
  });

  it('昨天写昨天和时刻', () => {
    expect(formatSyncStatus(new Date(2026, 9, 4, 18, 17, 0).toISOString(), now)).toBe('上次同步：昨天 18:17');
  });

  it('更早写成月日', () => {
    expect(formatSyncStatus(new Date(2026, 8, 3, 8, 7, 0).toISOString(), now)).toBe('上次同步：9 月 3 日 08:07');
  });

  it('空、null 和读不懂的时间都是还没同步过', () => {
    expect(formatSyncStatus(null, now)).toBe('还没同步过');
    expect(formatSyncStatus(undefined, now)).toBe('还没同步过');
    expect(formatSyncStatus('', now)).toBe('还没同步过');
    expect(formatSyncStatus('不是时间', now)).toBe('还没同步过');
  });
});

describe('formatSyncComplete', () => {
  it('按真实个数写成一句，取消收藏单独说明', () => {
    expect(formatSyncComplete({ activeCount: 497, createdCount: 14, removedCount: 0 })).toBe(
      '同步完成：你现在共收藏 497 个，新增 14 个，有 0 个是你在 GitHub 上取消收藏的（已从资料库隐藏）。',
    );
  });

  it('后台的 updatedCount 是「已有收藏的全部个数」，不是信息变了的个数：这一句不写它', () => {
    // 497 个里有 483 个本来就在：updatedCount 会是 483，写成「更新了 483 个」就是假话
    const sentence = formatSyncComplete({ activeCount: 497, createdCount: 14, updatedCount: 483, removedCount: 2 });
    expect(sentence).toBe('同步完成：你现在共收藏 497 个，新增 14 个，有 2 个是你在 GitHub 上取消收藏的（已从资料库隐藏）。');
    expect(sentence).not.toContain('更新');
    expect(sentence).not.toContain('483');
  });
});

describe('fullScanConfirmLines', () => {
  it('确认框固定三句：个数、不受每天额度限制、什么时候自动停', () => {
    expect(fullScanConfirmLines(12, '8 万')).toEqual([
      '还有 12 个收藏没有中文说明，预计整理约 12 个（约 8 万 token，具体花多少钱以你服务商的账单为准）。',
      '点确认后，应用在后台一直整理，不受每天额度限制，最多跑 48 小时，你随时可以点「停止」。',
      '服务商拒绝请求、失败太多，或花费超过预计的 4 倍时会自动停下。',
    ]);
  });
});

describe('dailyBudgetConfirmLead', () => {
  it('开头是每天最多整理约 N 个，N 按 6000 token 一个来估', () => {
    expect(dailyBudgetConfirmLead(100_000)).toBe('每天最多整理约 16 个');
    expect(dailyBudgetConfirmLead(0)).toBe('每天最多整理约 0 个');
  });
});

describe('recentDoneCount / withRecentDone：「整理了 N 个」只读后台算好的真数', () => {
  it('合法的个数向下取整，缺失、null、NaN、无穷、负数都当读不到', () => {
    expect(recentDoneCount(0)).toBe(0);
    expect(recentDoneCount(513)).toBe(513);
    expect(recentDoneCount(7.9)).toBe(7);
    expect(recentDoneCount(undefined)).toBeNull();
    expect(recentDoneCount(null)).toBeNull();
    expect(recentDoneCount(Number.NaN)).toBeNull();
    expect(recentDoneCount(Number.POSITIVE_INFINITY)).toBeNull();
    expect(recentDoneCount(-1)).toBeNull();
  });

  it('把 get_ai_queue_status 里的 succeeded24h 合进状态，其余字段原样', () => {
    const merged = withRecentDone(status({ tokens24h: 40_000, queued: 3 }), { succeeded24h: 12 });
    expect(merged.succeeded24h).toBe(12);
    expect(merged.tokens24h).toBe(40_000);
    expect(merged.queued).toBe(3);
  });

  it('队列状态读不到时是 null，不编数字', () => {
    expect(withRecentDone(status(), null).succeeded24h).toBeNull();
    expect(withRecentDone(status(), undefined).succeeded24h).toBeNull();
    expect(withRecentDone(status(), { succeeded24h: -3 }).succeeded24h).toBeNull();
  });
});

describe('spentByEarlierFullScan：「这是之前一次性整理用的」只在有证据时才说', () => {
  const now = new Date(2026, 9, 5, 12, 0, 0);
  const hoursAgo = (hours: number) => new Date(now.getTime() - hours * 60 * 60 * 1000).toISOString();
  const closed = { enabled: false, dailyBudgetTokens: 0, tokens24h: 3_083_000 };

  it('每天额度是 0、没有一次性整理在跑、上一轮在 24 小时内结束：说', () => {
    expect(spentByEarlierFullScan(status({ ...closed, lastFullScan: lastScan({ finishedAt: hoursAgo(2) }) }), now)).toBe(true);
    expect(spentByEarlierFullScan(status({ ...closed, lastFullScan: lastScan({ finishedAt: hoursAgo(23) }) }), now)).toBe(true);
  });

  it('暂停但没有一次性整理（暂停时 enabled 也是 false）：不说', () => {
    expect(spentByEarlierFullScan(status({ ...closed, paused: true, lastFullScan: null }), now)).toBe(false);
    expect(spentByEarlierFullScan(status({ ...closed, paused: true, lastFullScan: undefined }), now)).toBe(false);
    expect(spentByEarlierFullScan(status({ ...closed, paused: true, dailyBudgetTokens: 100_000, lastFullScan: null }), now)).toBe(false);
  });

  it('上一轮在 24 小时之前结束、时间读不懂、现在正在跑、每天额度开着、一分钱没花：都不说', () => {
    expect(spentByEarlierFullScan(status({ ...closed, lastFullScan: lastScan({ finishedAt: hoursAgo(25) }) }), now)).toBe(false);
    expect(spentByEarlierFullScan(status({ ...closed, lastFullScan: lastScan({ finishedAt: '不是时间' }) }), now)).toBe(false);
    expect(spentByEarlierFullScan(status({ ...closed, unlimited: true, lastFullScan: lastScan({ finishedAt: hoursAgo(1) }) }), now)).toBe(false);
    expect(spentByEarlierFullScan(status({ ...closed, dailyBudgetTokens: 200_000, lastFullScan: lastScan({ finishedAt: hoursAgo(1) }) }), now)).toBe(false);
    expect(spentByEarlierFullScan(status({ ...closed, tokens24h: 0, lastFullScan: lastScan({ finishedAt: hoursAgo(1) }) }), now)).toBe(false);
  });
});

describe('recentUsageLine / recentUsageMetric', () => {
  const now = new Date(2026, 9, 5, 12, 0, 0);
  const hoursAgo = (hours: number) => new Date(now.getTime() - hours * 60 * 60 * 1000).toISOString();

  it('读不到个数时整句不写个数，只说花了多少 token，不拿 token 除出来冒充', () => {
    expect(recentUsageLine(status({ tokens24h: 40_000 }), now)).toBe('最近 24 小时已用约 4 万 token。');
    expect(recentUsageLine(status({ tokens24h: 40_000, succeeded24h: null }), now)).toBe('最近 24 小时已用约 4 万 token。');
    expect(recentUsageLine(status({ tokens24h: 40_000 }), now)).not.toContain('整理了');
    for (const bad of [Number.NaN, -1, Number.POSITIVE_INFINITY]) {
      expect(recentUsageLine(status({ tokens24h: 40_000, succeeded24h: bad }), now)).toBe('最近 24 小时已用约 4 万 token。');
    }
  });

  it('读得到真实个数时写「整理了 N 个」，token 放进括号；0 也是真数', () => {
    expect(recentUsageLine(status({ tokens24h: 40_000, succeeded24h: 7 }), now)).toBe(
      '最近 24 小时整理了 7 个（已用约 4 万 token，含失败重试）。',
    );
    expect(recentUsageLine(status({ tokens24h: 12_000, succeeded24h: 0 }), now)).toBe(
      '最近 24 小时整理了 0 个（已用约 1.2 万 token，含失败重试）。',
    );
  });

  it('个数比「token 除以 6000」大得多时也照后台的真数写，不被估算带偏', () => {
    // 6 万 token 按 6000 一个只能整理 10 个；后台说整理了 42 个（旧的小卡），就写 42
    expect(recentUsageLine(status({ tokens24h: 60_000, succeeded24h: 42 }), now)).toContain('整理了 42 个');
  });

  it('关了每天额度、上一轮一次性整理刚结束：标明这是之前一次性整理用的', () => {
    const afterScan = status({
      enabled: false,
      dailyBudgetTokens: 0,
      budgetRemaining: 0,
      tokens24h: 3_083_000,
      lastFullScan: lastScan({ finishedAt: hoursAgo(2) }),
    });
    expect(recentUsageLine(afterScan, now)).toBe('最近 24 小时已用约 308.3 万 token（这是之前一次性整理用的）。');
    expect(recentUsageLine({ ...afterScan, succeeded24h: 513 }, now)).toBe(
      '最近 24 小时整理了 513 个（已用约 308.3 万 token，含失败重试；这是之前一次性整理用的）。',
    );
  });

  it('暂停了、没有一次性整理：不能出现「之前一次性整理用的」', () => {
    const paused = status({ enabled: false, paused: true, dailyBudgetTokens: 100_000, tokens24h: 3_083_000, lastFullScan: null });
    expect(recentUsageLine(paused, now)).toBe('最近 24 小时已用约 308.3 万 token。');
    const pausedNoBudget = status({ enabled: false, paused: true, dailyBudgetTokens: 0, tokens24h: 3_083_000 });
    expect(recentUsageLine(pausedNoBudget, now)).not.toContain('一次性');
    expect(recentUsageLine({ ...pausedNoBudget, succeeded24h: 5 }, now)).not.toContain('一次性');
    expect(recentUsageMetric(pausedNoBudget, now).sub).not.toContain('一次性');
  });

  it('没花过就不提一次性整理', () => {
    const idle = status({ enabled: false, dailyBudgetTokens: 0, tokens24h: 0, lastFullScan: lastScan({ finishedAt: hoursAgo(1) }) });
    expect(recentUsageLine(idle, now)).toBe('最近 24 小时已用约 0 token。');
  });

  it('工具箱那一格：知道个数就写「整理了 N 个」，token 放下面一行', () => {
    expect(recentUsageMetric(status({ tokens24h: 40_000, succeeded24h: 7 }), now)).toEqual({
      label: '24 小时内整理了',
      value: '7 个',
      sub: '已用约 4 万 token，含失败重试',
    });
  });

  it('工具箱那一格：不知道个数就只写 token，没有「约整理了」', () => {
    const metric = recentUsageMetric(status({ tokens24h: 40_000 }), now);
    expect(metric).toEqual({ label: '24 小时内已用', value: '4 万 token', sub: '' });
    expect(JSON.stringify(metric)).not.toContain('整理了');
  });

  it('工具箱那一格：关了额度又刚跑完一次性整理时，下面一行说清这笔花费的来历', () => {
    const afterScan = status({ enabled: false, dailyBudgetTokens: 0, tokens24h: 3_083_000, lastFullScan: lastScan({ finishedAt: hoursAgo(2) }) });
    expect(recentUsageMetric(afterScan, now)).toEqual({ label: '24 小时内已用', value: '308.3 万 token', sub: '这是之前一次性整理用的' });
    expect(recentUsageMetric({ ...afterScan, succeeded24h: 513 }, now)).toEqual({
      label: '24 小时内整理了',
      value: '513 个',
      sub: '已用约 308.3 万 token，含失败重试；这是之前一次性整理用的',
    });
  });

  it('今日 / 资料库横幅（每天整理中）带上真实个数', () => {
    const hint = automationHint(status({ queued: 2, tokens24h: 40_000, succeeded24h: 7 }), 7);
    expect(hint?.detail).toBe('最近 24 小时整理了 7 个（已用约 4 万 token，含失败重试）。');
  });
});

describe('一次性全部整理的横幅：「已整理 N 个」读后台给的开始个数和剩余个数', () => {
  it('处理过的减去失败的，不是估的', () => {
    const hint = automationHint(status({ unlimited: true, budgetRemaining: 0, queued: 4, fullScan: fullScan({ total: 388, remaining: 268, failed: 20, tokensUsed: 620_000 }) }), 5);
    expect(hint?.detail).toBe('不受每天额度限制，这一轮已整理 100 个（已用约 62 万 token）。整理完会自动回到每天自动整理。');
  });

  it('开始个数不是正数就不写个数，只写花了多少', () => {
    const hint = automationHint(status({ unlimited: true, budgetRemaining: 0, queued: 4, fullScan: fullScan({ total: 0, remaining: 0, tokensUsed: 620_000 }) }), 5);
    expect(hint?.detail).toBe('不受每天额度限制，已用约 62 万 token。整理完会自动回到每天自动整理。');
  });
});

describe('runStatusText：运行状态只用后台算好的「还没解决的失败数」', () => {
  it('0 个出错是运行正常，N 个出错写个数', () => {
    expect(runStatusText(0)).toBe('运行正常');
    expect(runStatusText(3)).toBe('有 3 个任务出错');
  });

  it('没读到就不下结论：不能写「运行正常」', () => {
    for (const unknown of [null, undefined, Number.NaN, -1]) {
      expect(runStatusText(unknown)).toBe('还没读到运行状态');
    }
  });
});

describe('exportSavedText', () => {
  it('写出个数和「下载」文件夹里的文件名', () => {
    expect(exportSavedText(497, 'fox-stars-lab-library.md')).toBe(
      '已导出 497 个工具到「下载」文件夹：fox-stars-lab-library.md',
    );
  });
});

describe('导出区块的说明（自动整理与备份页）', () => {
  it('讲清文件存在「下载」文件夹、同名会被覆盖，并补一句「用 Excel 打开中文不会乱码」', () => {
    expect(EXPORT_SECTION_HINT).toBe(
      '把资料库导成文件，方便存档或给别的工具读。文件会保存在「下载」文件夹里，同名的会被覆盖。导出的表格用 Excel 打开中文不会乱码。',
    );
    expect(EXPORT_SECTION_HINT).toContain('用 Excel 打开中文不会乱码');
  });

  it('页面上这段说明就是它（区块的 hint），两个导出按钮都在：文档（Markdown）、表格（CSV，可用 Excel 打开）', () => {
    const toolbox = withoutComments(sourceOf('../pages/v2/toolbox.tsx'));
    expect(toolbox).toContain('title="导出成文档 / 表格" hint={EXPORT_SECTION_HINT}');
    expect(toolbox).toContain('导出成文档（Markdown）');
    expect(toolbox).toContain('导出成表格（CSV，可用 Excel 打开）');
  });
});

describe('backupTimeLine', () => {
  it('有上次备份时写成月日时刻，并说明大约 20 小时', () => {
    expect(backupTimeLine('2026-10-03T18:17:00')).toBe(
      '上次备份：10 月 3 日 18:17。应用开着时，每隔约 20 小时自动备一次。',
    );
  });

  it('还没有备份时不编时间', () => {
    expect(backupTimeLine(null)).toBe('还没有备份。应用开着时，每隔约 20 小时自动备一次。');
    expect(backupTimeLine('不是时间')).toBe('还没有备份。应用开着时，每隔约 20 小时自动备一次。');
  });
});

describe('backupKindLabel', () => {
  it('四种类型都是人话', () => {
    expect(backupKindLabel('auto')).toBe('自动');
    expect(backupKindLabel('manual')).toBe('手动');
    expect(backupKindLabel('preRestore')).toBe('恢复前自动留的');
    expect(backupKindLabel('preMigrate')).toBe('升级前自动留的');
  });
});

describe('各状态按钮文案', () => {
  it('横幅、详情、重新整理的按钮用统一说法', () => {
    expect(automationHint(status({ providerConfigured: false }), 3)?.actionLabel).toBe('去设置 AI');
    expect(automationHint(status({ paused: true }), 3)?.actionLabel).toBe('继续整理');
    expect(automationHint(status({ unlimited: true, fullScan: fullScan() }), 3)?.actionLabel).toBe('看整理进度');
    expect(automationHint(status({ budgetRemaining: 0 }), 3)?.actionLabel).toBe('调整每天额度');
    expect(automationHint(status(), 3)?.actionLabel).toBe('加入整理');
    expect(itemPendingNote(status({ providerConfigured: false })).actionLabel).toBe('去设置 AI');
    expect(itemPendingNote(status({ paused: true })).actionLabel).toBe('继续整理');
    expect(itemPendingNote(status({ budgetRemaining: 0 })).actionLabel).toBe('调整每天额度');
    expect(regenerateNote(status({ providerConfigured: false })).actionLabel).toBe('去设置 AI');
    expect(regenerateNote(status({ paused: true })).actionLabel).toBe('继续整理');
  });
});

describe('buildReadinessRows：运行检查上面的清单读真实数据', () => {
  const base: ReadinessInput = {
    connected: true,
    login: 'fox',
    totalRepos: 497,
    totalReadmes: 497,
    totalAiSummaries: 497,
    aiConfigMessage: null,
    aiModel: 'deepseek-chat',
    syncing: false,
    fetchingReadmes: false,
    writingCards: false,
  };
  const row = (input: ReadinessInput, key: 'github' | 'sync' | 'readme' | 'ai') => {
    const found = buildReadinessRows(input).find((item) => item.key === key);
    if (!found) throw new Error(`没有 ${key} 这一行`);
    return found;
  };

  it('只有四行，没有「标签网络」那一行', () => {
    const rows = buildReadinessRows(base);
    expect(rows.map((item) => item.key)).toEqual(['github', 'sync', 'readme', 'ai']);
    expect(rows.map((item) => `${item.title}${item.detail}${item.actionLabel ?? ''}`).join('')).not.toContain('标签网络');
  });

  it('同步那一行写资料库里真有几个收藏，不再写「连接后执行同步」', () => {
    expect(row(base, 'sync').detail).toBe('已同步：资料库里有 497 个收藏');
    expect(row(base, 'sync').done).toBe(true);
    const empty = { ...base, totalRepos: 0, totalReadmes: 0, totalAiSummaries: 0 };
    expect(row(empty, 'sync').done).toBe(false);
    expect(row(empty, 'sync').detail).toBe('还没同步。连接后点「同步我的收藏」。');
  });

  it('AI 那一行写设置里的模型名：「AI 已设置：模型名」', () => {
    expect(row(base, 'ai').detail).toContain('AI 已设置：deepseek-chat');
    expect(row(base, 'ai').done).toBe(true);
  });

  it('中文说明还没写完时，AI 一行有「写说明」按钮，并说清会花钱、受每天额度限制', () => {
    const partial = { ...base, totalAiSummaries: 100 };
    const ai = row(partial, 'ai');
    expect(ai.action).toBe('writeCards');
    expect(ai.actionLabel).toBe('继续写还没有的中文说明');
    expect(ai.detail).toContain('已写好 100 个中文说明');
    expect(ai.detail).toContain('会花钱');
    expect(ai.detail).toContain('每天额度');
    const none = row({ ...base, totalAiSummaries: 0 }, 'ai');
    expect(none.actionLabel).toBe('给没有说明的收藏写中文说明');
  });

  it('都写完了就没有按钮', () => {
    expect(row(base, 'ai').action).toBeNull();
    expect(row(base, 'readme').action).toBeNull();
  });

  it('AI 还没设置完整：整句用设置里的提示，按钮是「去设置 AI」，不写成「已设置」', () => {
    const ai = row({ ...base, aiConfigMessage: '还没设置 AI。去「设置 → AI 设置」…' }, 'ai');
    expect(ai.done).toBe(false);
    expect(ai.detail).toBe('还没设置 AI。去「设置 → AI 设置」…');
    expect(ai.detail).not.toContain('AI 已设置');
    expect(ai.actionLabel).toBe('去设置 AI');
    expect(ai.action).toBe('openAiSettings');
  });

  it('没连接：GitHub 一行教人去粘贴令牌，同步按钮没有', () => {
    const offline = { ...base, connected: false, login: null };
    expect(row(offline, 'github').detail).toBe('还没连接。先在下面粘贴令牌，再点「连接 GitHub」。');
    expect(row(offline, 'github').done).toBe(false);
    expect(row(offline, 'sync').action).toBeNull();
    expect(row(base, 'github').detail).toBe('已连接 @fox');
  });

  it('项目说明：还差的可以继续补；正在下载时按钮转圈、其他按钮暂时点不了', () => {
    const missing = { ...base, totalReadmes: 300 };
    expect(row(missing, 'readme').detail).toBe('已下载 300 / 497 个项目说明，还差的可以继续补');
    expect(row(missing, 'readme').action).toBe('readme');
    const busy = { ...missing, fetchingReadmes: true };
    expect(row(busy, 'readme').actionLabel).toBe('下载中…');
    expect(row(busy, 'readme').busy).toBe(true);
    expect(row(busy, 'ai').actionDisabled).toBe(true);
    expect(row(busy, 'sync').busy).toBe(false);
  });
});

describe('运行检查：说明、费用提示和各项名字', () => {
  it('检查会真的调用 AI，要说清会花一点钱', () => {
    expect(RUNTIME_CHECK_COST_NOTE).toBe('如果已经设置了 AI，检查时会真实调用几次 AI，会产生很少的费用。');
  });

  it('检查结果每一项的名字都是人话，不再出现标签网络、Stars API、Token', () => {
    const titles = Object.values(RUNTIME_CHECK_TITLES).join('|');
    expect(titles).not.toMatch(/标签网络|Stars API|Token|README|数据库|SQLite/);
    expect(RUNTIME_CHECK_TITLES.github).toBe('GitHub 令牌');
  });
});

describe('设置页文案：令牌、钥匙串、清空本机数据、GitHub 备份', () => {
  it('GitHub 的叫令牌，保存处叫钥匙串，不再说系统凭据管理器', () => {
    expect(GITHUB_TOKEN_EXPLAINER).toBe('令牌是 GitHub 给你的一串密码，应用用它读你的收藏。');
    expect(GITHUB_TOKEN_SAVED_NOTE).toBe('令牌已保存在 Mac 的钥匙串里，界面不会显示它。');
    expect(GITHUB_TOKEN_WILL_SAVE_NOTE).toContain('只会保存在 Mac 的钥匙串里');
    for (const text of [GITHUB_TOKEN_SAVED_NOTE, GITHUB_TOKEN_WILL_SAVE_NOTE, GITHUB_TOKEN_SCOPES]) {
      expect(text).not.toMatch(/系统凭据管理器|Personal Access Token|Token/);
    }
  });

  it('令牌是什么是一句完整的话（不是半截句）；连接之后令牌区域那一句不再重复它', () => {
    expect(GITHUB_TOKEN_EXPLAINER.startsWith('令牌是')).toBe(true);
    expect(GITHUB_TOKEN_EXPLAINER.endsWith('。')).toBe(true);
    expect(GITHUB_TOKEN_EXPLAINER).not.toMatch(/^令牌（/);
    // 设置 → GitHub 页：最上面说一遍（含权限），「访问配置」里只说保存在哪，不重复「一串密码」
    const connectedPage = [GITHUB_TOKEN_EXPLAINER, GITHUB_TOKEN_SCOPES, GITHUB_TOKEN_SAVED_NOTE].join('');
    expect(connectedPage.split('一串密码').length - 1).toBe(1);
    expect(GITHUB_TOKEN_SAVED_NOTE).not.toContain('一串密码');
    // 欢迎引导里是第一次提到令牌，保存那一句带着它是什么
    expect(GITHUB_TOKEN_WILL_SAVE_NOTE.startsWith(GITHUB_TOKEN_EXPLAINER)).toBe(true);
  });

  it('权限说明：只读公开收藏一个都不用勾；还需要的话勾「repo」（读你的私有收藏）和「gist」（上传备份用）', () => {
    expect(GITHUB_TOKEN_SCOPES).toBe('创建令牌时，只读公开收藏的话一个权限都不用勾；还需要的话再勾「repo」（读你的私有收藏）和「gist」（上传备份用）。');
    // repo / gist 各带一句解释，同一件事只写一遍
    expect(GITHUB_TOKEN_SCOPES.split('「repo」').length - 1).toBe(1);
    expect(GITHUB_TOKEN_SCOPES.split('「gist」').length - 1).toBe(1);
    expect(GITHUB_TOKEN_SCOPES).toContain('「repo」（读你的私有收藏）');
    expect(GITHUB_TOKEN_SCOPES).toContain('「gist」（上传备份用）');
  });

  it('设置 → GitHub 页：令牌是什么那一句后面直接接权限，不再多一个句号（原来「令牌（…）。」是半截句）', () => {
    const settings = withoutComments(sourceOf('../pages/settings.tsx'));
    expect(settings).toContain('{GITHUB_TOKEN_EXPLAINER}{GITHUB_TOKEN_SCOPES}');
    expect(settings).not.toContain('{GITHUB_TOKEN_EXPLAINER}。');
  });

  it('连接慢的提示：设置里一句、欢迎引导里多一句「先进入」', () => {
    expect(CONNECT_SLOW).toBe('GitHub 这会儿比较慢，请再等一会儿');
    expect(CONNECT_SLOW_WELCOME).toBe('GitHub 这会儿比较慢，请再等一会儿，或先进入，连接会在后台继续');
    expect(CONNECT_SLOW).not.toContain('没有卡住');
  });

  it('清空本机数据：列出现在真会丢的东西，说清 GitHub 上的收藏不受影响，提醒先备份', () => {
    for (const piece of ['想试 / 在用', '收集箱里的链接', '中文说明（重做要重新花钱）', 'GitHub 令牌', 'AI 密钥', '你在 GitHub 上的收藏不受影响', '「设置 → 自动整理与备份」点「现在备份」', '刚安装的样子']) {
      expect(CLEAR_LOCAL_DATA_WARNING).toContain(piece);
    }
    expect(CLEAR_LOCAL_DATA_WARNING).not.toMatch(/SQLite|Stars|README|AI 摘要|API Key|Token/);
    expect(CLEAR_LOCAL_DATA_DONE).toContain('同步我的收藏');
    expect(CLEAR_LOCAL_DATA_FAILED).toContain('你在 GitHub 上的收藏不受影响');
    expect(CLEAR_LOCAL_DATA_FAILED).toContain('把这条截图发给开发者');
  });

  it('「自动同步」把设置里的间隔写进句子', () => {
    expect(autoSyncDescription(30)).toBe('打开应用时先同步一次，之后每隔 30 分钟再同步一次。');
    expect(autoSyncDescription(120)).toBe('打开应用时先同步一次，之后每隔 120 分钟再同步一次。');
    expect(autoSyncDescription(Number.NaN)).toBe('打开应用时先同步一次，之后每隔 30 分钟再同步一次。');
    expect(SYNC_INTERVAL_HINT).toBe('可填 5～1440 分钟。太频繁会被 GitHub 限制，一般不用改。');
    expect(WELCOME_STARTUP_HINT).toBe('没连接 GitHub 时，启动后显示欢迎引导。');
  });

  it('上传备份到 GitHub：标题讲清是私密 Gist，两个备份说清范围', () => {
    expect(GIST_SECTION_TITLE).toBe('上传备份到 GitHub（私密 Gist）');
    expect(GIST_BACKUP_MARKS.title).toBe('备份我的标记');
    expect(GIST_BACKUP_MARKS.body).toBe('把你标的「想试 / 在用 / 不要了」、标签和笔记上传到你 GitHub 账号下的一个私密 Gist（别人看不到）。');
    expect(GIST_BACKUP_LIBRARY.title).toBe('备份整个清单');
    expect(GIST_BACKUP_LIBRARY.body).toBe('同上，再加上收藏清单；不含中文说明、AI 密钥和本机设置。');
    expect(GIST_NEEDS_SCOPE).toContain('「gist」');
    const all = [GIST_SECTION_TITLE, GIST_BACKUP_MARKS.body, GIST_BACKUP_LIBRARY.body, GIST_IMPORT_WARNING, GIST_NEEDS_SCOPE].join('|');
    expect(all).not.toMatch(/GSAT|注解|导出所有仓库|Token/);
  });

  it('导入的覆盖警告一直在，不用等填了 Gist ID；随状态变的提示只说为什么现在点不了', () => {
    expect(GIST_IMPORT_WARNING).toBe('导入会覆盖这台电脑上同一工具的标记，请确认 Gist ID 是你自己上传的。');
    expect(gistImportHint(false, true)).toBe('请先连接 GitHub 账号后再导入。');
    expect(gistImportHint(false, false)).toBe('请先连接 GitHub 账号后再导入。');
    expect(gistImportHint(true, false)).toBe('输入 Gist ID 后才能导入。');
    expect(gistImportHint(true, true)).toBeNull();
    // 警告不能再藏在「填了 Gist ID」这个条件后面
    expect(gistImportHint(true, true) ?? '').not.toContain('覆盖');
  });
});

describe('欢迎引导文案', () => {
  it('标题、说明、三条提示都按新界面写', () => {
    expect(WELCOME_COPY.titleLines).toEqual(['让每一颗 Star，', '都变成找得到、看得懂的工具']);
    expect(WELCOME_COPY.subtitle).toBe('把你在 GitHub 上收藏的项目同步到这台电脑；需要时再让 AI 写中文说明（按服务商价格收费，默认不开）');
    expect(WELCOME_COPY.tips).toEqual([
      '在「资料库」按场景找工具',
      '在「找方案」说一句想做的事，挑工具交给 Codex',
      '在「今日」看新收藏',
    ]);
  });

  it('令牌怎么拿：点链接、点 Generate new token、名字随便填、权限一个都不用勾（只读公开收藏）、复制', () => {
    expect(WELCOME_COPY.tokenSteps).toHaveLength(5);
    expect(WELCOME_COPY.tokenSteps[0]).toBe('点这个链接');
    expect(WELCOME_COPY.tokenSteps[1]).toBe('点 Generate new token');
    expect(WELCOME_COPY.tokenSteps[2]).toBe('名字随便填');
    expect(WELCOME_COPY.tokenSteps[3]).toContain('权限一个都不用勾（只读公开收藏）');
    expect(WELCOME_COPY.tokenSteps[4]).toBe('复制');
  });

  it('怎么拿令牌里的权限一步：repo / gist 各带一句解释，和设置页是同一句话（只写一遍）', () => {
    expect(WELCOME_COPY.tokenSteps[3]).toBe('权限一个都不用勾（只读公开收藏）；还需要的话再勾「repo」（读你的私有收藏）和「gist」（上传备份用）');
    expect(WELCOME_COPY.tokenSteps[3]).toContain('「repo」（读你的私有收藏）');
    expect(WELCOME_COPY.tokenSteps[3]).toContain('「gist」（上传备份用）');
    expect(GITHUB_TOKEN_SCOPES).toContain(GITHUB_TOKEN_EXTRA_SCOPES);
    expect(WELCOME_COPY.tokenSteps[3]).toContain(GITHUB_TOKEN_EXTRA_SCOPES);
  });

  it('不再提仓库页、标签网络页、语义检索、AI 解析，也不自称「我」', () => {
    const everything = JSON.stringify(WELCOME_COPY);
    expect(everything).not.toMatch(/仓库页|标签网络|语义|AI 解析|中文定位|Personal Access Token|系统凭据|Stars/);
    expect(everything).not.toMatch(/我来|我会/);
  });

  it('进不去时不拼英文原文，告诉你资料还在、下一步做什么', () => {
    expect(WELCOME_COPY.enterFailed).toContain('你的资料都还在');
    expect(WELCOME_COPY.enterFailed).toContain('把这条截图发给开发者');
    expect(WELCOME_COPY.syncPartial).toContain('你的资料都还在');
  });
});

/* ───────── 文案守卫：直接读源码，旧叫法回来了就变红 ───────── */

function sourceOf(relative: string): string {
  return readFileSync(fileURLToPath(new URL(relative, import.meta.url)), 'utf8');
}

/** 含有某个词的那几行（失败时只打印这几行，不把整个文件打出来）。 */
function linesContaining(text: string, term: string): string[] {
  return text.split('\n').filter((line) => line.includes(term)).map((line) => line.trim());
}

/** 去掉注释，只留会显示给人看的字符串和 JSX 文字（注释里写旧叫法不算）。 */
function withoutComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .map((line) => line.replace(/(^|\s)\/\/.*$/, '$1'))
    .join('\n');
}

describe('文案守卫：设置页里不再出现旧叫法（卡 33 验收第 3 条）', () => {
  const settings = withoutComments(sourceOf('../pages/settings.tsx'));
  // 「向量」页签藏起来了：面板代码还在，但没有任何入口能打开它。守卫只看能被看见的那一部分。
  const hiddenStart = settings.indexOf('function EmbeddingSettingsPanel(');
  const hiddenEnd = settings.indexOf('function AiRequestDiagnosticsPanel(');
  const visible = hiddenStart >= 0 && hiddenEnd > hiddenStart ? settings.slice(0, hiddenStart) + settings.slice(hiddenEnd) : settings;

  it('隐藏的向量面板确实被排除在外，而且没有页签能打开它', () => {
    expect(hiddenStart).toBeGreaterThan(0);
    expect(hiddenEnd).toBeGreaterThan(hiddenStart);
    expect(settings).not.toMatch(/key: 'embedding', icon/);
  });

  it.each([
    '标签网络',
    '增量 AI 解析',
    '向量',
    'GSAT',
    '系统凭据管理器',
    'Personal Access Token',
    'API Key',
    'Base URL',
    '请求 Endpoint',
    '摘要 Prompt',
    '首字响应',
    '引擎',
    'README 缓存',
    '数据同步操作',
    '导出注解',
    '导出所有仓库',
    '注解',
    '仓库',
    '工具箱',
    '您',
    'weiloo',
    '模型 ID',
  ])('设置页看得见的文字里没有「%s」', (term) => {
    expect(linesContaining(visible, term)).toEqual([]);
  });

  it('页签是 GitHub / AI 设置 / 通用 / GitHub 备份 / 关于，向量页签没有了', () => {
    expect(settings).toContain("label: 'GitHub', shortLabel: 'GitHub'");
    expect(settings).toContain("label: 'AI 设置', shortLabel: 'AI 设置'");
    expect(settings).toContain("label: '通用', shortLabel: '通用'");
    expect(settings).toContain("label: 'GitHub 备份', shortLabel: 'GitHub 备份'");
    expect(settings).toContain("label: '关于', shortLabel: '关于'");
  });
});

describe('文案守卫：欢迎引导、更新、侧栏里不再出现旧叫法', () => {
  it.each([
    ['../components/welcome-flow.tsx', ['Personal Access Token', '系统凭据管理器', '标签网络', '仓库页', '语义检索', '中文定位', 'AI 解析', '连接没有卡住', '“', '”']],
    ['../providers/app-update-provider.tsx', ['latest.json', 'GitHub Release', '更新文件没找到 / 校验没通过']],
    ['../features/sidebar/connection-panel.tsx', ['Personal Access Token', '系统凭据管理器']],
    ['../features/sidebar/sync-panel.tsx', ['同步 Stars', '抓取 README', '导出注解', '导入注解']],
    ['../hooks/use-settings.ts', ['系统凭据管理器', 'Embedding Key', 'AI Key', '请检查本机存储权限']],
    ['../hooks/use-stars-workspace.ts', ['Personal Access Token', 'GitHub Stars', '选型状态', 'README 已缓存', 'AI 增量处理', '本地数据刷新失败']],
    ['../App.tsx', ['README 缓存失败', '重试 AI 搜索', '重试导出 Gist', '重试导入 Gist', '重试 README 缓存']],
  ] as const)('%s 里没有旧说法', (file, terms) => {
    const text = withoutComments(sourceOf(file));
    for (const term of terms) {
      expect(linesContaining(text, term), `${file} 里还有「${term}」`).toEqual([]);
    }
  });
});

describe('文案守卫：页签名、关于页、导入警告', () => {
  const settings = withoutComments(sourceOf('../pages/settings.tsx'));

  it('「工具箱」页签改叫「自动整理与备份」，页面标题同步（页签名现在只在 lib/nav.ts 里定义，App.tsx 不再覆盖）', () => {
    expect(withoutComments(sourceOf('./nav.ts'))).toContain("label: '自动整理与备份'");
    expect(withoutComments(sourceOf('../App.tsx'))).not.toContain("label: '自动整理与备份'");
    expect(withoutComments(sourceOf('../pages/v2/toolbox.tsx'))).toContain('title="设置 · 自动整理与备份"');
  });

  it('关于页只留版本号、数据放在哪个文件夹、许可证，其余折进「开发者信息」', () => {
    const about = settings.slice(settings.indexOf('function AboutSettings('), settings.indexOf('function normalizeSyncIntervalInput('));
    expect(about).toContain('label="版本号"');
    expect(about).toContain('label="数据放在哪个文件夹"');
    expect(about).toContain('>许可证</dt>');
    const developerInfoAt = about.indexOf('开发者信息');
    expect(developerInfoAt).toBeGreaterThan(0);
    // 身份明细（Bundle ID 之类）和链接都在「开发者信息」之后，不在外面
    expect(about.indexOf('<AppIdentityDetails')).toBeGreaterThan(developerInfoAt);
    expect(about.indexOf('href={PROJECT_REPOSITORY_URL}')).toBeGreaterThan(developerInfoAt);
    expect(settings).not.toMatch(/技术资产|凭据服务|本地优先客户端/);
  });

  it('导入的覆盖警告不放在任何条件后面：一直显示', () => {
    const at = settings.indexOf('{GIST_IMPORT_WARNING}');
    expect(at).toBeGreaterThan(0);
    const before = settings.slice(Math.max(0, at - 260), at);
    expect(before).not.toMatch(/&&\s*\(|\?\s*\(|\?\s*['"`]/);
  });

  it('同步完成的提示用统一的句子，不再自己拼「更新 N 个」', () => {
    expect(settings).toContain('formatSyncComplete(workspace.syncSummary)');
    expect(settings).not.toContain('updatedCount');
  });
});
