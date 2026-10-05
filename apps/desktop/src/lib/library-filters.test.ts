import { describe, expect, it } from 'vitest';
import type { ReadingGroup } from '@/lib/contracts';
import {
  applyOptimisticReading,
  cardStateOptions,
  clearFilters,
  createLibraryUiState,
  formLabel,
  formLabelWithHint,
  formOptions,
  formTagLabel,
  hasActiveFilters,
  nextPageOffset,
  removeIfFilteredOut,
  shouldFallbackToAll,
  toFilters,
  toggleForm,
  toggleReading,
  toggleScenario,
  visibleScenarios,
  withInitial,
  type LibraryUiState,
} from './library-filters';

function state(patch: Partial<LibraryUiState> = {}): LibraryUiState {
  return { ...createLibraryUiState(), ...patch };
}

describe('资料库筛选状态', () => {
  it('默认看全部（没整理的也是收藏），按收藏时间，其余为空', () => {
    expect(createLibraryUiState()).toEqual({
      scenario: null,
      reading: null,
      form: null,
      cardState: 'all',
      keyword: '',
      sort: 'starred',
    });
  });

  it('空关键词和空白不传，offset 是页码乘每页条数', () => {
    const blank = toFilters(state({ keyword: '   ' }), 0, 80);
    expect(blank.keyword).toBeUndefined();
    expect(blank.offset).toBe(0);
    expect(blank.limit).toBe(80);
    expect(blank.cardState).toBe('all');
    expect(blank.sort).toBe('starred');
    expect(blank.scenario).toBeUndefined();
    expect(blank.reading).toBeUndefined();
    expect(blank.form).toBeUndefined();

    const page2 = toFilters(state(), 2, 80);
    expect(page2.offset).toBe(160);
    expect(page2.limit).toBe(80);

    const invalid = toFilters(state(), -1, 0);
    expect(invalid.offset).toBe(0);
    expect(invalid.limit).toBe(0);
  });

  it('有筛选时带上场景、状态、类型，关键词去掉首尾空格', () => {
    const filters = toFilters(state({
      scenario: 'ec',
      reading: 'want',
      form: 'skill',
      cardState: 'all',
      keyword: '  小红书  ',
      sort: 'stars',
    }), 1, 80);
    expect(filters).toEqual({
      scenario: 'ec',
      reading: 'want',
      form: 'skill',
      cardState: 'all',
      keyword: '小红书',
      sort: 'stars',
      limit: 80,
      offset: 80,
    });
  });

  it('场景再点同一项就取消，点另一项就替换', () => {
    const original = state();
    const picked = toggleScenario(original, 'ec');
    expect(picked.scenario).toBe('ec');
    expect(original.scenario).toBeNull();
    expect(toggleScenario(picked, 'ec').scenario).toBeNull();
    expect(toggleScenario(picked, 'seo').scenario).toBe('seo');
    expect(toggleScenario(picked, '').scenario).toBeNull();
  });

  it('我的状态再点同一项就取消，点另一项就替换', () => {
    const picked = toggleReading(state(), 'want');
    expect(picked.reading).toBe('want');
    expect(toggleReading(picked, 'want').reading).toBeNull();
    expect(toggleReading(picked, 'in_use').reading).toBe('in_use');
  });

  it('类型再点同一项就取消，点另一项就替换', () => {
    const picked = toggleForm(state(), 'skill');
    expect(picked.form).toBe('skill');
    expect(toggleForm(picked, 'skill').form).toBeNull();
    expect(toggleForm(picked, 'cli').form).toBe('cli');
  });

  it('清除筛选后整理状态回到全部，排序保留', () => {
    const cleared = clearFilters(state({
      scenario: 'ec',
      reading: 'want',
      form: 'mcp',
      cardState: 'all',
      keyword: '封面',
      sort: 'updated',
    }));
    expect(cleared).toEqual({
      scenario: null,
      reading: null,
      form: null,
      cardState: 'all',
      keyword: '',
      sort: 'updated',
    });
  });

  it('有场景、状态、类型、关键词或非默认整理状态时才显示清除筛选', () => {
    expect(hasActiveFilters(state())).toBe(false);
    expect(hasActiveFilters(state({ sort: 'stars' }))).toBe(false);
    expect(hasActiveFilters(state({ keyword: '   ' }))).toBe(false);
    expect(hasActiveFilters(state({ keyword: 'a' }))).toBe(true);
    expect(hasActiveFilters(state({ scenario: 'ec' }))).toBe(true);
    expect(hasActiveFilters(state({ reading: 'dropped' }))).toBe(true);
    expect(hasActiveFilters(state({ form: 'cli' }))).toBe(true);
    expect(hasActiveFilters(state({ cardState: 'pending' }))).toBe(true);
    expect(hasActiveFilters(state({ cardState: 'done' }))).toBe(true);
    expect(hasActiveFilters(state({ cardState: 'all' }))).toBe(false);
  });

  it('初始关键词和场景会带上，空白当成没有', () => {
    expect(withInitial()).toEqual(createLibraryUiState());
    expect(withInitial('  ', '  ')).toEqual(createLibraryUiState());
    expect(withInitial(null, null)).toEqual(createLibraryUiState());
    expect(withInitial('  小红书  ', ' ec ')).toMatchObject({
      keyword: '小红书',
      scenario: 'ec',
      cardState: 'all',
      sort: 'starred',
    });
  });
});

describe('筛选栏的联动计数', () => {
  it('场景个数用联动计数，缺键当 0，不用场景自带的个数', () => {
    const rows = visibleScenarios(
      [
        { id: 'ec', nameZh: '电商', thin: true },
        { id: 'seo', nameZh: 'SEO·增长', thin: true },
        { id: 'code', nameZh: '写代码', thin: false },
      ],
      { scenarios: { ec: 2, code: 0 } },
    );
    expect(rows).toEqual([
      { id: 'ec', nameZh: '电商', count: 2, thin: true },
      { id: 'seo', nameZh: 'SEO·增长', count: 0, thin: true },
      { id: 'code', nameZh: '写代码', count: 0, thin: false },
    ]);
    const withOwnCount: Array<{ id: string; nameZh: string; thin: boolean; count: number }> = [
      { id: 'ec', nameZh: '电商', thin: false, count: 99 },
    ];
    expect(visibleScenarios(withOwnCount, null)).toEqual([
      { id: 'ec', nameZh: '电商', count: 0, thin: false },
    ]);
  });

  it('类型按个数从多到少，个数相同按 id，并换成中文名', () => {
    const rows = formOptions({
      skill: 2,
      api: 2,
      mcp: 5,
      cli: 2,
      plugin: 5,
      app: 0,
      library: 1,
      service: 1,
      collection: 1,
      other: 1,
    });
    expect(rows.map((row) => `${row.id}:${row.label}:${row.count}`)).toEqual([
      'mcp:MCP 插件（给 AI 接外部工具）:5',
      'plugin:plugin:5',
      'api:接口（程序之间对接）:2',
      'cli:命令行工具（要让 Codex 帮你装）:2',
      'skill:技能包（装进 Codex 用）:2',
      'collection:资源清单:1',
      'library:开发用代码库（一般用不上）:1',
      'other:其他:1',
      'service:在线服务:1',
      'app:应用:0',
    ]);
    expect(formOptions(null)).toEqual([]);
  });

  it('整理状态三个选项，全部排最前，数量是 AI 已整理加待 AI 整理', () => {
    expect(cardStateOptions({ done: 3, pending: 2 })).toEqual([
      { value: 'all', label: '全部 5' },
      { value: 'done', label: 'AI 已整理 3' },
      { value: 'pending', label: '待 AI 整理 2' },
    ]);
    expect(cardStateOptions(null)).toEqual([
      { value: 'all', label: '全部 0' },
      { value: 'done', label: 'AI 已整理 0' },
      { value: 'pending', label: '待 AI 整理 0' },
    ]);
    expect(cardStateOptions({ done: Number.NaN, pending: Number.POSITIVE_INFINITY }).map((item) => item.label)).toEqual([
      '全部 0',
      'AI 已整理 0',
      '待 AI 整理 0',
    ]);
  });
});

describe('加载更多和空白回退', () => {
  it('满页之后还有条目时，下一页 offset 就是已加载条数', () => {
    expect(nextPageOffset(80, 81, 80)).toBe(80);
    expect(nextPageOffset(80, 160, 80)).toBe(80);
    expect(nextPageOffset(160, 200, 80)).toBe(160);
  });

  it('取完、超过总数、还没加载、页大小非法或这一页没取满时，没有下一页', () => {
    expect(nextPageOffset(80, 80, 80)).toBeNull();
    expect(nextPageOffset(90, 80, 80)).toBeNull();
    expect(nextPageOffset(0, 100, 80)).toBeNull();
    expect(nextPageOffset(80, 0, 80)).toBeNull();
    expect(nextPageOffset(80, 160, 0)).toBeNull();
    expect(nextPageOffset(80, 160, -1)).toBeNull();
    expect(nextPageOffset(Number.NaN, 160, 80)).toBeNull();
    expect(nextPageOffset(50, 100, 80)).toBeNull();
  });

  it('当前停在「AI 已整理」、已整理数为 0 且还有待 AI 整理、用户没动过时，改看全部', () => {
    const counts = { cardState: { done: 0, pending: 4 } };
    expect(shouldFallbackToAll(counts, state({ cardState: 'done' }), false)).toBe(true);
    expect(shouldFallbackToAll({ cardState: { done: 0, pending: 1 } }, state({ cardState: 'done' }), false)).toBe(true);
    expect(shouldFallbackToAll(counts, state(), false)).toBe(false);
  });

  it('用户动过、已经不是默认、没有待 AI 整理、或已整理数大于 0 时，不自动切换', () => {
    const counts = { cardState: { done: 0, pending: 4 } };
    expect(shouldFallbackToAll(counts, state(), true)).toBe(false);
    expect(shouldFallbackToAll(counts, state({ cardState: 'all' }), false)).toBe(false);
    expect(shouldFallbackToAll(counts, state({ cardState: 'pending' }), false)).toBe(false);
    expect(shouldFallbackToAll({ cardState: { done: 0, pending: 0 } }, state(), false)).toBe(false);
    expect(shouldFallbackToAll({ cardState: { done: 2, pending: 4 } }, state(), false)).toBe(false);
    expect(shouldFallbackToAll(null, state(), false)).toBe(false);
    expect(shouldFallbackToAll({ cardState: { done: 0, pending: -1 } }, state(), false)).toBe(false);
  });
});

describe('改状态的乐观更新', () => {
  it('改状态返回新数组，不改原数组，没点到的条目保持原样', () => {
    const items = [
      { id: 'a', readingGroup: 'unseen' as ReadingGroup, name: '甲' },
      { id: 'b', readingGroup: 'want' as ReadingGroup, name: '乙' },
    ];
    const next = applyOptimisticReading(items, 'a', 'want');
    expect(next).not.toBe(items);
    expect(next[0]).toEqual({ id: 'a', readingGroup: 'want', name: '甲' });
    expect(next[0]).not.toBe(items[0]);
    expect(next[1]).toBe(items[1]);
    expect(items[0].readingGroup).toBe('unseen');
    expect(applyOptimisticReading(items, 'missing', 'dropped')[0]).toBe(items[0]);
    expect(items.map((item) => item.readingGroup)).toEqual(['unseen', 'want']);
  });

  it('筛了状态组且新状态对不上时移除，原数组不变', () => {
    const items = [
      { id: 'a', readingGroup: 'dropped' as ReadingGroup },
      { id: 'b', readingGroup: 'want' as ReadingGroup },
    ];
    const next = removeIfFilteredOut(items, 'a', 'want');
    expect(next.map((item) => item.id)).toEqual(['b']);
    expect(items.map((item) => item.id)).toEqual(['a', 'b']);
    expect(next).not.toBe(items);
  });

  it('没有状态筛选，或新状态仍然符合，就留下', () => {
    const items = [
      { id: 'a', readingGroup: 'want' as ReadingGroup },
      { id: 'b', readingGroup: 'in_use' as ReadingGroup },
    ];
    const unfiltered = removeIfFilteredOut(items, 'a', null);
    expect(unfiltered.map((item) => item.id)).toEqual(['a', 'b']);
    expect(unfiltered).not.toBe(items);
    expect(items).toHaveLength(2);

    const stillMatches = removeIfFilteredOut(items, 'a', 'want');
    expect(stillMatches.map((item) => item.id)).toEqual(['a', 'b']);
    expect(removeIfFilteredOut(items, 'missing', 'want').map((item) => item.id)).toEqual(['a', 'b']);
  });
});

describe('formLabel', () => {
  it('常见类型写成短名字（行内小标签、找方案的命中行、复制给 Codex 的正文都用这一套）', () => {
    expect(formLabel('skill')).toBe('技能');
    expect(formLabel(' app ')).toBe('应用');
    expect(formLabel('cli')).toBe('命令行');
    expect(formLabel('mcp')).toBe('MCP');
    expect(formLabel('library')).toBe('代码库');
    expect(formLabel('api')).toBe('接口');
    expect(formLabel('service')).toBe('服务');
    expect(formLabel('collection')).toBe('合集');
  });

  it('短名字里不带括号和解释', () => {
    for (const id of ['skill', 'mcp', 'cli', 'app', 'library', 'api', 'service', 'collection']) {
      expect(formLabel(id), id).not.toMatch(/[（）]/);
      expect(Array.from(formLabel(id) ?? '').length, id).toBeLessThanOrEqual(4);
    }
  });

  it('没填、空白、「其他」都不显示标签；不认识的原样给', () => {
    expect(formLabel(null)).toBeNull();
    expect(formLabel(undefined)).toBeNull();
    expect(formLabel('  ')).toBeNull();
    expect(formLabel('other')).toBeNull();
    expect(formLabel('template')).toBe('template');
  });

  it('带解释的长写法只给筛选面板和详情页的「类型」一行用：和短名字是两套，没填 / 其他同样不显示，不认识的原样给', () => {
    expect(formLabelWithHint('skill')).toBe('技能包（装进 Codex 用）');
    expect(formLabelWithHint(' cli ')).toBe('命令行工具（要让 Codex 帮你装）');
    expect(formLabelWithHint('mcp')).toBe('MCP 插件（给 AI 接外部工具）');
    expect(formLabelWithHint('library')).toBe('开发用代码库（一般用不上）');
    expect(formLabelWithHint('api')).toBe('接口（程序之间对接）');
    expect(formLabelWithHint('service')).toBe('在线服务');
    expect(formLabelWithHint('collection')).toBe('资源清单');
    expect(formLabelWithHint('app')).toBe('应用');
    expect(formLabelWithHint('other')).toBeNull();
    expect(formLabelWithHint('  ')).toBeNull();
    expect(formLabelWithHint(undefined)).toBeNull();
    expect(formLabelWithHint('template')).toBe('template');
    // 改长写法不能把短名字带长
    expect(formLabel('cli')).not.toBe(formLabelWithHint('cli'));
  });

  it('详情小标签写成「类型：…」，用长写法；其他和空的不显示', () => {
    expect(formTagLabel('app')).toBe('类型：应用');
    expect(formTagLabel('skill')).toBe('类型：技能包（装进 Codex 用）');
    expect(formTagLabel('cli')).toBe('类型：命令行工具（要让 Codex 帮你装）');
    expect(formTagLabel('other')).toBeNull();
    expect(formTagLabel(null)).toBeNull();
    expect(formTagLabel('template')).toBe('类型：template');
  });
});

