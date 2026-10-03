import { describe, expect, it } from 'vitest';
import type { ScenarioSummary, SolveHit, SolveResult } from '@/lib/contracts';
import {
  buildPackRequest,
  crossNote,
  formatOptions,
  gapNote,
  initialPicks,
  makeMain,
  matchBadge,
  noIntentNote,
  orderedPicks,
  packPlaceholder,
  pendingNote,
  todayLocalDate,
  togglePick,
  whyLine,
} from './solve-view';

function hit(partial: Partial<SolveHit> & { id: string }): SolveHit {
  return {
    id: partial.id,
    fullName: partial.fullName ?? `fox/${partial.id}`,
    htmlUrl: partial.htmlUrl ?? `https://github.com/fox/${partial.id}`,
    starsCount: partial.starsCount ?? 12,
    oneLiner: partial.oneLiner ?? '一句话',
    form: partial.form ?? '技能',
    scenarioIds: partial.scenarioIds ?? [],
    matched: partial.matched ?? [],
    missing: partial.missing ?? [],
    why: partial.why ?? '因为相关',
    score: partial.score ?? 1,
    fullMatch: partial.fullMatch ?? false,
    hasCard: partial.hasCard ?? true,
  };
}

function result(partial: Partial<SolveResult> = {}): SolveResult {
  return {
    query: partial.query ?? '做小红书封面',
    intents: partial.intents ?? ['小红书', '封面'],
    extras: partial.extras ?? [],
    hits: partial.hits ?? [],
    preselected: partial.preselected ?? [],
    fullMatchCount: partial.fullMatchCount ?? 0,
    gapIntents: partial.gapIntents ?? [],
    suggestedScenarioId: partial.suggestedScenarioId ?? null,
    suggestedScenarioCount: partial.suggestedScenarioCount ?? null,
    pendingMatches: partial.pendingMatches ?? 0,
  };
}

const scenario: ScenarioSummary = {
  id: 'xhs',
  slug: 'xhs',
  nameZh: '小红书',
  sortOrder: 1,
  count: 4,
  thin: true,
};

describe('initialPicks', () => {
  it('只保留命中里的 id，并保持 preselected 的顺序', () => {
    const preselected = ['b', 'gone', 'a'];
    const next = initialPicks(result({
      preselected,
      hits: [hit({ id: 'a' }), hit({ id: 'b' })],
    }));
    expect(next).toEqual(['b', 'a']);
    expect(preselected).toEqual(['b', 'gone', 'a']);
  });

  it('不把没出现在 preselected 里的部分匹配硬凑进来', () => {
    const next = initialPicks(result({
      hits: [
        hit({ id: 'part', fullMatch: false, matched: ['小红书'] }),
        hit({ id: 'full', fullMatch: true, matched: ['小红书', '封面'] }),
      ],
      preselected: ['full'],
    }));
    expect(next).toEqual(['full']);
  });

  it('重复的 id 只留第一次', () => {
    expect(initialPicks(result({
      hits: [hit({ id: 'a' })],
      preselected: ['a', 'a'],
    }))).toEqual(['a']);
  });
});

describe('togglePick', () => {
  it('加在末尾，不改原数组', () => {
    const original = ['a', 'b'];
    const next = togglePick(original, 'c');
    expect(next).toEqual(['a', 'b', 'c']);
    expect(next).not.toBe(original);
    expect(original).toEqual(['a', 'b']);
  });

  it('取消中间一项时，其余项顺序不变，也不改原数组', () => {
    const original = ['a', 'b', 'c'];
    const next = togglePick(original, 'b');
    expect(next).toEqual(['a', 'c']);
    expect(original).toEqual(['a', 'b', 'c']);
  });
});

describe('orderedPicks', () => {
  it('按勾选顺序返回，跳过不在命中里的 id', () => {
    const hits = [hit({ id: 'a' }), hit({ id: 'b' }), hit({ id: 'c' })];
    expect(orderedPicks(hits, ['c', 'missing', 'a']).map((item) => item.id)).toEqual(['c', 'a']);
  });
});

describe('gapNote', () => {
  const hits = [hit({ id: 'a', fullMatch: false, matched: ['小红书'] })];

  it('两个意图、没有全匹配时用「A」和「B」', () => {
    expect(gapNote(result({
      hits,
      fullMatchCount: 0,
      intents: ['小红书', '封面'],
      gapIntents: ['封面'],
    }))).toBe('你的收藏里没有同时对上「小红书」和「封面」的工具，下面是部分匹配。');
  });

  it('三个意图写成「A」「B」和「C」', () => {
    expect(gapNote(result({
      hits,
      fullMatchCount: 0,
      intents: ['小红书', '封面', '配图'],
    }))).toBe('你的收藏里没有同时对上「小红书」「封面」和「配图」的工具，下面是部分匹配。');
  });

  it('四个意图也是最后一个前面加「和」', () => {
    expect(gapNote(result({
      hits,
      fullMatchCount: 0,
      intents: ['小红书', '封面', '配图', '标题'],
    }))).toBe('你的收藏里没有同时对上「小红书」「封面」「配图」和「标题」的工具，下面是部分匹配。');
  });

  it('只有一个意图词时不提示', () => {
    expect(gapNote(result({
      hits,
      fullMatchCount: 0,
      intents: ['小红书'],
    }))).toBeNull();
  });

  it('没有任何命中时不提示', () => {
    expect(gapNote(result({
      hits: [],
      fullMatchCount: 0,
      intents: ['小红书', '封面'],
    }))).toBeNull();
  });

  it('已经有全匹配时不提示', () => {
    expect(gapNote(result({
      hits,
      fullMatchCount: 1,
      intents: ['小红书', '封面'],
    }))).toBeNull();
  });
});

describe('matchBadge', () => {
  it('多个意图都对上了是蓝色「都对上了」', () => {
    expect(matchBadge(
      hit({ id: 'a', fullMatch: true, matched: ['小红书', '封面'] }),
      ['小红书', '封面'],
    )).toEqual({ tone: 'blue', text: '都对上了' });
  });

  it('只对上一部分是灰色，多个词连写', () => {
    expect(matchBadge(
      hit({ id: 'a', fullMatch: false, matched: ['小红书', '封面'] }),
      ['小红书', '封面', '配图'],
    )).toEqual({ tone: 'gray', text: '只对上「小红书」「封面」' });
  });

  it('只有一个意图词时，即使全对上也不标「都对上了」', () => {
    expect(matchBadge(
      hit({ id: 'a', fullMatch: true, matched: ['小红书'] }),
      ['小红书'],
    )).toBeNull();
  });

  it('没对上任何词时不标', () => {
    expect(matchBadge(
      hit({ id: 'a', fullMatch: false, matched: [] }),
      ['小红书', '封面'],
    )).toBeNull();
  });

  it('单个意图只对上一部分仍显示灰色标签', () => {
    expect(matchBadge(
      hit({ id: 'a', fullMatch: false, matched: ['小红书'] }),
      ['小红书'],
    )).toEqual({ tone: 'gray', text: '只对上「小红书」' });
  });
});

describe('crossNote', () => {
  it('有场景时写出收藏数，并问要不要看新的', () => {
    expect(crossNote(result({
      suggestedScenarioId: 'xhs',
      suggestedScenarioCount: 4,
    }), [scenario])).toBe('你在「小红书」方向收藏了 4 个。想看更多同类的新项目？');
  });

  it('没有建议场景时返回空', () => {
    expect(crossNote(result({ suggestedScenarioId: null, suggestedScenarioCount: 4 }), [scenario])).toBeNull();
    expect(crossNote(result({ suggestedScenarioId: '  ' }), [scenario])).toBeNull();
  });

  it('场景 id 对不上名字时不编造', () => {
    expect(crossNote(result({ suggestedScenarioId: 'missing', suggestedScenarioCount: 2 }), [scenario])).toBeNull();
  });

  it('数量缺失时按 0 写', () => {
    expect(crossNote(result({
      suggestedScenarioId: 'xhs',
      suggestedScenarioCount: null,
    }), [scenario])).toBe('你在「小红书」方向收藏了 0 个。想看更多同类的新项目？');
  });
});

describe('pendingNote', () => {
  it('还有在整理的就提示数量', () => {
    expect(pendingNote(result({ pendingMatches: 2 }))).toBe('另有 2 个还没整理，整理好后可能对得上。');
  });

  it('没有或不是正数时不提示', () => {
    expect(pendingNote(result({ pendingMatches: 0 }))).toBeNull();
    expect(pendingNote(result({ pendingMatches: -1 }))).toBeNull();
  });
});

describe('buildPackRequest', () => {
  it('标题和问题用原句，第一项是主参考，其余是备选，并带上 why', () => {
    const hits = [
      hit({ id: '1', why: '主因' }),
      hit({ id: '2', why: '备因' }),
      hit({ id: '3', why: '再备' }),
    ];
    const request = buildPackRequest('做小红书封面', hits);
    expect(request.title).toBe('参考包：做小红书封面');
    expect(request.problem).toBe('做小红书封面');
    expect(request.localCapabilities).toEqual([]);
    expect(request.items).toEqual([
      { repoId: '1', role: 'main', why: '主因' },
      { repoId: '2', role: 'alternative', why: '备因' },
      { repoId: '3', role: 'alternative', why: '再备' },
    ]);
    expect(hits.map((item) => item.id)).toEqual(['1', '2', '3']);
  });

  it('没有勾选时 items 为空，也不共用同一个 capabilities 数组', () => {
    const first = buildPackRequest('空', []);
    const second = buildPackRequest('空', []);
    expect(first.items).toEqual([]);
    expect(first.localCapabilities).toEqual([]);
    expect(first.localCapabilities).not.toBe(second.localCapabilities);
  });
});

describe('todayLocalDate', () => {
  it('本地 0 点整是当天，0 点前一秒是前一天', () => {
    const midnight = new Date(2026, 9, 3, 0, 0, 0, 0);
    const justBefore = new Date(midnight.getTime() - 1);
    expect(todayLocalDate(midnight)).toBe('2026-10-03');
    expect(todayLocalDate(justBefore)).toBe('2026-10-02');
    expect(todayLocalDate(new Date(2026, 9, 3, 0, 0, 1))).toBe('2026-10-03');
  });

  it('月和日都补成两位', () => {
    expect(todayLocalDate(new Date(2026, 0, 1, 0, 0, 0))).toBe('2026-01-01');
  });

  it('跟本地年月日一致，不跟 UTC 日历日绑死', () => {
    const midnight = new Date(2026, 9, 3, 0, 0, 0, 0);
    const month = String(midnight.getMonth() + 1).padStart(2, '0');
    const day = String(midnight.getDate()).padStart(2, '0');
    expect(todayLocalDate(midnight)).toBe(`${midnight.getFullYear()}-${month}-${day}`);
  });
});

describe('whyLine', () => {
  it('why 为空或只有空白时不显示', () => {
    expect(whyLine(hit({ id: 'a', why: '', oneLiner: '一句话' }))).toBeNull();
    expect(whyLine(hit({ id: 'a', why: '  \n\t', oneLiner: '一句话' }))).toBeNull();
  });

  it('oneLiner 为 null 或 undefined 时，非空 why 仍然显示', () => {
    expect(whyLine(hit({ id: 'a', oneLiner: null, why: '因为相关' }))).toBe('因为相关');
    const missing = hit({ id: 'b', why: '另一句' });
    Reflect.deleteProperty(missing, 'oneLiner');
    expect(whyLine(missing)).toBe('另一句');
  });

  it('why 和 oneLiner 完全相同时不显示', () => {
    expect(whyLine(hit({ id: 'a', why: '一句话', oneLiner: '一句话' }))).toBeNull();
  });

  it('why 和 oneLiner 只差首尾空白时不显示', () => {
    expect(whyLine(hit({ id: 'a', why: '  一句话  ', oneLiner: '一句话' }))).toBeNull();
    expect(whyLine(hit({ id: 'a', why: '一句话', oneLiner: '  一句话\n' }))).toBeNull();
  });

  it('why 和 oneLiner 不同时返回去掉空白的 why', () => {
    expect(whyLine(hit({ id: 'a', why: '  更具体的原因  ', oneLiner: '一句话' }))).toBe('更具体的原因');
  });
});

describe('noIntentNote', () => {
  it('有命中但没有意图词时给出说明', () => {
    expect(noIntentNote(result({ intents: [], hits: [hit({ id: 'a' })] }))).toBe(
      '没有认出具体的任务词，下面是文字上相近的收藏，请自己勾选你觉得对的。',
    );
  });

  it('有意图词时不说明', () => {
    expect(noIntentNote(result({ intents: ['语音'], hits: [hit({ id: 'a' })] }))).toBeNull();
  });

  it('没有命中时不说明', () => {
    expect(noIntentNote(result({ intents: [], hits: [] }))).toBeNull();
  });
});

describe('packPlaceholder 和 formatOptions', () => {
  it('三种格式的占位文字相同', () => {
    expect(packPlaceholder('markdown')).toBe('（还没有选中的工具）');
    expect(packPlaceholder('agents')).toBe('（还没有选中的工具）');
    expect(packPlaceholder('json')).toBe('（还没有选中的工具）');
  });

  it('格式选项的值和文案', () => {
    expect(formatOptions.map((item) => item.value)).toEqual(['markdown', 'agents', 'json']);
    expect(formatOptions.map((item) => item.label)).toEqual(['Markdown', 'AGENTS.md', 'JSON']);
  });
});

describe('makeMain', () => {
  it('把勾选里的某一项挪到最前，其余顺序不变', () => {
    expect(makeMain(['a', 'b', 'c'], 'c')).toEqual(['c', 'a', 'b']);
    expect(makeMain(['a', 'b', 'c'], 'b')).toEqual(['b', 'a', 'c']);
  });

  it('已经是第一个、或者没勾选，原样返回（新数组）', () => {
    const picked = ['a', 'b'];
    expect(makeMain(picked, 'a')).toEqual(['a', 'b']);
    expect(makeMain(picked, 'a')).not.toBe(picked);
    expect(makeMain(picked, 'x')).toEqual(['a', 'b']);
    expect(makeMain([], 'a')).toEqual([]);
  });
});
