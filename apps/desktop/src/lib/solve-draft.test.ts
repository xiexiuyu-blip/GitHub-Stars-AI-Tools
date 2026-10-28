import { afterEach, describe, expect, it } from 'vitest';
import { loadSolveDraft, saveSolveDraft, type SolveDraft } from '@/lib/solve-draft';

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
});
