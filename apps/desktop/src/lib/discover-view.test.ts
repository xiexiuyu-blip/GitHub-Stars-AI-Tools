import { describe, expect, it } from 'vitest';
import type { DiscoverCandidate, DiscoverRefreshReport, DiscoverScenario } from './contracts';
import { buildEvaluatePrompt, candidateStatus, refreshNotice, scenarioLead, sortScenarios } from './discover-view';

function scenario(overrides: Partial<DiscoverScenario> = {}): DiscoverScenario {
  return {
    id: 's1',
    slug: 'xhs',
    nameZh: '小红书',
    sortOrder: 1,
    myCount: 12,
    thin: false,
    candidateCount: 0,
    lastFetchedAt: null,
    ...overrides,
  };
}

function candidate(overrides: Partial<DiscoverCandidate> = {}): DiscoverCandidate {
  return {
    fullName: 'foo/bar',
    htmlUrl: 'https://github.com/foo/bar',
    description: 'A tool',
    language: 'Python',
    starsCount: 100,
    pushedAt: '2026-10-01T00:00:00Z',
    topics: [],
    mine: false,
    inInbox: false,
    fetchedAt: '2026-10-03T00:00:00Z',
    ...overrides,
  };
}

function report(overrides: Partial<DiscoverRefreshReport> = {}): DiscoverRefreshReport {
  return {
    scenarioId: 's1',
    queries: 2,
    failedQueries: 0,
    fetched: 5,
    skippedMine: 0,
    message: null,
    rateLimitResetAt: null,
    ...overrides,
  };
}

describe('sortScenarios', () => {
  it('偏少的排前面，其余保持场景自己的顺序，不改原数组', () => {
    const input = [
      scenario({ id: 'a', sortOrder: 1 }),
      scenario({ id: 'b', sortOrder: 2, thin: true }),
      scenario({ id: 'c', sortOrder: 3 }),
      scenario({ id: 'd', sortOrder: 0, thin: true }),
    ];
    expect(sortScenarios(input).map((item) => item.id)).toEqual(['d', 'b', 'a', 'c']);
    expect(input.map((item) => item.id)).toEqual(['a', 'b', 'c', 'd']);
  });
});

describe('scenarioLead', () => {
  const now = new Date('2026-10-03T12:00:00Z');

  it('没有场景时是空', () => {
    expect(scenarioLead(null, now)).toBe('');
  });

  it('没找过：明说还没找过', () => {
    expect(scenarioLead(scenario(), now)).toBe('你在这个方向收藏了 12 个。还没去 GitHub 找过。');
  });

  it('偏少会说偏少；找过的说多久以前', () => {
    const lead = scenarioLead(scenario({ thin: true, myCount: 3, lastFetchedAt: '2026-10-01T12:00:00Z' }), now);
    expect(lead).toBe('你在这个方向收藏了 3 个，偏少。上次去 GitHub 找是2 天前。');
  });
});

describe('candidateStatus', () => {
  it('已在 Stars > 已在收集箱 > 还没收', () => {
    const none = new Set<string>();
    expect(candidateStatus(candidate({ mine: true, inInbox: true }), none)).toBe('mine');
    expect(candidateStatus(candidate({ inInbox: true }), none)).toBe('inbox');
    expect(candidateStatus(candidate(), none)).toBe('new');
  });

  it('刚在这个页面收进去的也算已在收集箱', () => {
    expect(candidateStatus(candidate(), new Set(['foo/bar']))).toBe('inbox');
  });
});

describe('refreshNotice', () => {
  it('限流：用后端的人话；没有就用默认提示，都是 warn', () => {
    expect(refreshNotice(report({ rateLimitResetAt: '2026-10-03T12:01:00Z', message: 'GitHub 限流了，1 分钟后再试' }))).toEqual({
      tone: 'warn',
      text: 'GitHub 限流了，1 分钟后再试',
    });
    const fallback = refreshNotice(report({ rateLimitResetAt: '2026-10-03T12:01:00Z' }));
    expect(fallback.tone).toBe('warn');
    expect(fallback.text).toContain('限流');
  });

  it('后端带了别的提示就原样给', () => {
    expect(refreshNotice(report({ message: '网络不通' }))).toEqual({ tone: 'warn', text: '网络不通' });
  });

  it('部分搜索失败：说清几次里失败几次', () => {
    expect(refreshNotice(report({ failedQueries: 1 }))).toEqual({
      tone: 'warn',
      text: '2 次搜索里有 1 次没成功，先显示拿到的部分。',
    });
  });

  it('正常：说找到几个，跳过的说一下', () => {
    expect(refreshNotice(report())).toEqual({ tone: 'info', text: '找到 5 个候选。' });
    expect(refreshNotice(report({ skippedMine: 3 })).text).toBe('找到 5 个候选，另有 3 个你已经收藏过了。');
  });

  it('什么都没找到', () => {
    expect(refreshNotice(report({ fetched: 0 })).text).toBe('这次没有找到新的候选。');
    expect(refreshNotice(report({ fetched: 0, skippedMine: 2 })).text).toBe('这次没有找到新的候选，另有 2 个你已经收藏过了。');
  });
});

describe('buildEvaluatePrompt', () => {
  it('带项目名、链接、场景，要求中文回答并且不编', () => {
    const text = buildEvaluatePrompt(candidate(), '小红书');
    expect(text).toContain('foo/bar（https://github.com/foo/bar）');
    expect(text).toContain('我想用它做「小红书」');
    expect(text).toContain('不要编');
  });

  it('没有链接就用 fullName 拼；没有场景名就不写「我想用它做」', () => {
    const text = buildEvaluatePrompt({ fullName: 'a/b', htmlUrl: '' }, ' ');
    expect(text).toContain('https://github.com/a/b');
    expect(text).not.toContain('我想用它做');
  });
});
