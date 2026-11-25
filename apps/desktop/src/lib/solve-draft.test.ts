import { afterEach, describe, expect, it } from 'vitest';
import type { LibraryDetail } from '@/lib/contracts';
import { addDetailToSolveDraft, loadSolveDraft, saveSolveDraft, type SolveDraft } from '@/lib/solve-draft';

function detail(id: string, fullName: string): LibraryDetail {
  return {
    id,
    fullName,
    htmlUrl: `https://github.com/${fullName}`,
    description: 'desc',
    language: 'TypeScript',
    starsCount: 100,
    starredAt: '2026-10-01T00:00:00Z',
    pushedAt: '2026-10-02T00:00:00Z',
    oneLiner: `一句话：${fullName}`,
    form: 'cli',
    scenarioIds: ['scn-1'],
    readingStatus: 'unread',
    readingGroup: 'unseen',
    hasCard: true,
    confidence: 0.9,
    cardReview: 'suggested',
    whatItIs: null,
    capabilities: [],
    problemsSolved: [],
    useWhen: [],
    notFor: [],
    howToTry: null,
    integratesWith: [],
    alternativesHint: [],
    inspirationForFox: null,
    risks: [],
    license: null,
    maturity: null,
    zhFriendly: null,
    keywords: [],
    topics: [],
    forksCount: 1,
    readmeExcerpt: null,
  };
}

function draft(): SolveDraft {
  return {
    query: '做封面', activeQuery: '做封面', picked: ['b', 'a'], format: 'json', direct: false,
    result: {
      query: '做封面', intents: ['封面'], extras: [], hits: [], preselected: ['a', 'b'],
      fullMatchCount: 0, gapIntents: [], suggestedScenarioId: null, suggestedScenarioCount: null, pendingMatches: 0,
    },
  };
}

afterEach(() => saveSolveDraft(null));

describe('找方案会话草稿：新任务与普通返回', () => {
  it('没有待消费问题时恢复勾选顺序、格式和任务结果', () => {
    saveSolveDraft(draft());
    expect(loadSolveDraft()).toEqual(draft());
    expect(loadSolveDraft('   ')?.picked).toEqual(['b', 'a']);
  });

  it.each(['另一个问题', '做封面'])('明确带入「%s」是新任务，不能加载已有草稿', (question) => {
    saveSolveDraft(draft());
    expect(loadSolveDraft(question)).toBeNull();
  });

  it('消费带入问题、保存用户调整后，离开返回恢复这份草稿', () => {
    expect(loadSolveDraft('做封面')).toBeNull();
    saveSolveDraft({ ...draft(), picked: ['b'], format: 'agents', direct: true });
    expect(loadSolveDraft('')).toMatchObject({ picked: ['b'], format: 'agents', direct: true });
  });

  it('新搜索开始时清空旧草稿，未完成就离开不会复活上一任务', () => {
    saveSolveDraft(draft());
    saveSolveDraft(null);
    expect(loadSolveDraft('')).toBeNull();
  });

  it('addDetailToSolveDraft：无草稿新建、已有追加、已选中提示重复', () => {
    const d1 = detail('r1', 'acme/first');
    const d2 = detail('r2', 'acme/second');

    expect(addDetailToSolveDraft(d1)).toEqual({ mode: 'created' });
    const created = loadSolveDraft('');
    expect(created?.picked).toEqual(['r1']);
    expect(created?.result.hits.map((h) => h.id)).toEqual(['r1']);

    expect(addDetailToSolveDraft(d1)).toEqual({ mode: 'already' });
    expect(addDetailToSolveDraft(d2)).toEqual({ mode: 'appended' });
    const updated = loadSolveDraft('');
    expect(updated?.picked).toEqual(['r1', 'r2']);
    expect(updated?.result.hits.map((h) => h.id)).toEqual(['r1', 'r2']);
  });
});
