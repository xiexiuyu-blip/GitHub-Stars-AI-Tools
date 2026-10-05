import { describe, expect, it } from 'vitest';
import type { DiscoverCandidate, DiscoverRefreshReport, DiscoverScenario } from './contracts';
import {
  addedToInboxToast,
  buildEvaluatePrompt,
  candidateCountHint,
  candidateCountText,
  candidateStatus,
  discoverEmptyCopy,
  refreshNotice,
  scenarioLead,
  sortCandidates,
  sortScenarios,
} from './discover-view';

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
    expect(scenarioLead(scenario(), now)).toBe('你在这个场景收藏了 12 个。还没去 GitHub 找过。');
  });

  it('偏少会说偏少；找过的说多久以前', () => {
    const lead = scenarioLead(scenario({ thin: true, myCount: 3, lastFetchedAt: '2026-10-01T12:00:00Z' }), now);
    expect(lead).toBe('你在这个场景收藏了 3 个，偏少（14 个以内）。上次去 GitHub 找是2 天前。');
  });
});

describe('candidateStatus', () => {
  it('已收藏 > 已在收集箱 > 还没放', () => {
    const none = new Set<string>();
    expect(candidateStatus(candidate({ mine: true, inInbox: true }), none)).toBe('mine');
    expect(candidateStatus(candidate({ inInbox: true }), none)).toBe('inbox');
    expect(candidateStatus(candidate(), none)).toBe('new');
  });

  it('刚在这个页面放进去的也算已在收集箱', () => {
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
    expect(fallback.text).toBe('GitHub 暂时不让查太频繁，过几分钟再点「重新找找」，已找到的先列在下面');
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

describe('sortCandidates', () => {
  const make = (fullName: string, flags: Partial<DiscoverCandidate> = {}): DiscoverCandidate => ({
    fullName,
    htmlUrl: `https://github.com/${fullName}`,
    description: null,
    language: null,
    starsCount: 1,
    pushedAt: null,
    topics: [],
    mine: false,
    inInbox: false,
    fetchedAt: '2026-10-04T00:00:00Z',
    ...flags,
  });

  it('还没放的在前，已在收集箱的其次，已收藏的沉底；同一档保持原来的顺序', () => {
    const list = [
      make('a/mine1', { mine: true }),
      make('b/new1'),
      make('c/inbox1', { inInbox: true }),
      make('d/mine2', { mine: true }),
      make('e/new2'),
    ];
    expect(sortCandidates(list, new Set()).map((item) => item.fullName)).toEqual(['b/new1', 'e/new2', 'c/inbox1', 'a/mine1', 'd/mine2']);
  });

  it('这次刚放进收集箱的也算「已在收集箱」', () => {
    const list = [make('a/new1'), make('b/new2')];
    expect(sortCandidates(list, new Set(['a/new1'])).map((item) => item.fullName)).toEqual(['b/new2', 'a/new1']);
  });

  it('不改原数组', () => {
    const list = [make('a/mine', { mine: true }), make('b/new')];
    sortCandidates(list, new Set());
    expect(list.map((item) => item.fullName)).toEqual(['a/mine', 'b/new']);
  });
});

describe('发现页的新文案', () => {
  it('查得太频繁：后端没给原句时用审定的那句（不说限流、不写秒数），后端给了就原样给', () => {
    const fallback = refreshNotice(report({ rateLimitResetAt: '2026-10-03T12:01:00Z', message: '  ' }));
    expect(fallback).toEqual({
      tone: 'warn',
      text: 'GitHub 暂时不让查太频繁，过几分钟再点「重新找找」，已找到的先列在下面',
    });
    expect(fallback.text).not.toContain('限流');
    expect(fallback.text).not.toMatch(/\d/);
    expect(refreshNotice(report({ rateLimitResetAt: '2026-10-03T12:01:00Z', message: 'GitHub 限流了，1 分钟后再试' })).text)
      .toBe('GitHub 限流了，1 分钟后再试');
  });

  it('场景右边的个数：有候选写 N 个；没有候选时分「还没找过」和「找过了」，不再都写「—」', () => {
    const fetchedAt = '2026-10-01T12:00:00Z';
    expect(candidateCountText(3, null)).toBe('3 个');
    expect(candidateCountText(3, fetchedAt)).toBe('3 个');
    expect(candidateCountText(0, null)).toBe('还没找过');
    expect(candidateCountText(0, fetchedAt)).toBe('没有结果');
    expect(candidateCountText(-1, null)).toBe('还没找过');
    expect(candidateCountText(Number.NaN, fetchedAt)).toBe('没有结果');
    expect(candidateCountText(0, null)).not.toBe('—');
    expect(candidateCountText(0, fetchedAt)).not.toBe('—');
  });

  it('个数格子的悬停提示写整句：找过但没有结果和没找过要说得不一样', () => {
    const fetchedAt = '2026-10-01T12:00:00Z';
    expect(candidateCountHint(0, fetchedAt)).toBe('找过了，这次没有结果');
    expect(candidateCountHint(0, null)).toBe('还没去 GitHub 找过');
    expect(candidateCountHint(12, fetchedAt)).toBe('12 个你还没收藏的候选');
  });

  it('右边没有候选时的空状态：一个场景都没有 / 找过但没有 / 还没找过，各说各的', () => {
    expect(discoverEmptyCopy(null)).toEqual({
      title: '还没有场景',
      description: '先去「资料库」同步一次收藏，场景会自动分好。',
    });
    expect(discoverEmptyCopy(undefined).title).toBe('还没有场景');
    expect(discoverEmptyCopy({ lastFetchedAt: '2026-10-01T12:00:00Z' })).toEqual({
      title: '这个场景上没有找到你还没收藏的',
      description: '可以换个场景，或者过几天再点「重新找找」。',
    });
    const never = discoverEmptyCopy({ lastFetchedAt: null });
    expect(never.title).toBe('这个场景还没去找过');
    expect(never.description).toContain('去 GitHub 找找');
    expect(never.description).not.toContain('我');
    // 场景是启动时按关键词规则分好的，不用先整理收藏
    expect(discoverEmptyCopy(null).description).not.toContain('第一次整理');
  });

  it('放进收集箱之后：新放进去的提示要去 GitHub 点 Star 再同步；本来就在收集箱里的只说已经有了', () => {
    expect(addedToInboxToast(1, 'foo/bar')).toEqual({
      title: '已放进收集箱。想正式收藏，去 GitHub 点 Star，再回来点「同步」',
      sub: 'foo/bar',
    });
    expect(addedToInboxToast(0, 'foo/bar')).toEqual({ title: '收集箱里已经有了', sub: 'foo/bar' });
    expect(addedToInboxToast(1, 'foo/bar').title).not.toContain('收进');
  });
});
