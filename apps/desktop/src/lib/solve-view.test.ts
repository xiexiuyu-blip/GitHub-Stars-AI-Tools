import { describe, expect, it } from 'vitest';
import type { RerankFit, RerankResult, ScenarioSummary, SolveHit, SolveResult } from '@/lib/contracts';
// 源码原文：检查「界面上不再有旧叫法」「提示只写在一处」用（Vite 的 ?raw，不用 node 的类型）
import formatPickerSource from '../components/solve/format-picker.tsx?raw';
import hitDrawerSource from '../components/solve/hit-drawer.tsx?raw';
import hitRowSource from '../components/solve/solve-hit-row.tsx?raw';
import solvePageSource from '../pages/v2/solve.tsx?raw';
import solveViewSource from './solve-view.ts?raw';
import { buildCommunityPrompt, buildInstallPrompt } from './codex-prompts';
import {
  applyRerank,
  promotedHits,
  RERANK_CANDIDATE_LIMIT,
  rerankCandidateIds,
  BACK_TO_PICK_LABEL,
  buildPackRequest,
  COPY_FOR_CODEX_TOAST,
  COPY_FOR_RESEARCH_TOAST,
  copyPromptToast,
  crossNote,
  DIRECT_FROM_PICKS_LABEL,
  defaultPickNote,
  DIRECT_LABEL,
  emptyResultHint,
  formatOptions,
  gapNote,
  initialPicks,
  makeMain,
  missingToolsNote,
  pickListNote,
  pickLocalCapabilities,
  matchBadge,
  noIntentNote,
  orderedPicks,
  packEmptyNote,
  packPlaceholder,
  packSizeText,
  pendingNote,
  previewDisplayText,
  RERANK_FIT_LABEL,
  RERANK_TOGGLE_LABEL,
  rerankPicks,
  rerankStatusText,
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

/** 源码去掉注释，剩下的才是会显示给人看的字（和写死的字符串）。注释里讲数据模型，可以说「仓库」「项目」。 */
function uiText(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:])\/\/[^\n]*/g, '$1');
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
  it('全都对上是默认情况，不标（八条里六七条都是，每条都标就等于没标）', () => {
    expect(matchBadge(
      hit({ id: 'a', fullMatch: true, matched: ['小红书', '封面'] }),
      ['小红书', '封面'],
    )).toBeNull();
  });

  it('只对上一部分是灰色，多个词连写', () => {
    expect(matchBadge(
      hit({ id: 'a', fullMatch: false, matched: ['小红书', '封面'] }),
      ['小红书', '封面', '配图'],
    )).toEqual({ tone: 'gray', text: '只对上「小红书」「封面」' });
  });

  it('只有一个意图词时，全对上也不标', () => {
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
    }), [scenario])).toBe('你在「小红书」场景收藏了 4 个。想看更多同类的新工具？');
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
    }), [scenario])).toBe('你在「小红书」场景收藏了 0 个。想看更多同类的新工具？');
  });
});

describe('pendingNote', () => {
  it('还有在整理的就提示数量', () => {
    expect(pendingNote(result({ pendingMatches: 2 }))).toBe('另有 2 个待 AI 整理，整理好后可能对得上。');
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
    expect(request.title).toBe('交接单：做小红书封面');
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

  it('直接模式不带项目，mode 是 direct，标题换成直接交给 AI 助手', () => {
    const hits = [hit({ id: '1', why: '主因' }), hit({ id: '2', why: '备因' })];
    const request = buildPackRequest('批量改图尺寸和压缩', hits, [], true);
    expect(request.items).toEqual([]);
    expect(request.mode).toBe('direct');
    expect(request.title).toBe('直接交给 AI 助手：批量改图尺寸和压缩');
    expect(request.problem).toBe('批量改图尺寸和压缩');
    expect(request.localCapabilities).toEqual([]);
    expect(hits.map((item) => item.id)).toEqual(['1', '2']);
  });

  it('非直接模式不写 mode', () => {
    const request = buildPackRequest('做小红书封面', [hit({ id: '1' })]);
    expect(request.mode).toBeUndefined();
    expect(Object.prototype.hasOwnProperty.call(request, 'mode')).toBe(false);
    expect(request.title).toBe('交接单：做小红书封面');
  });

  it('直接模式带上本机能力，而且不和调用方共用数组', () => {
    const local = [{ name: 'sips', oneLiner: '改图片尺寸', path: '/usr/bin/sips' }];
    const request = buildPackRequest('批量改图尺寸和压缩', [hit({ id: '1' })], local, true);
    expect(request.mode).toBe('direct');
    expect(request.items).toEqual([]);
    expect(request.localCapabilities).toEqual(local);
    expect(request.localCapabilities).not.toBe(local);
  });

  it('三个直接交给 AI 助手的按钮用同一句', () => {
    expect(DIRECT_LABEL).toBe('不用这些工具，直接交给 AI 助手');
    expect(DIRECT_FROM_PICKS_LABEL).toBe('不用这些工具，直接交给 AI 助手');
    expect(DIRECT_FROM_PICKS_LABEL).toBe(DIRECT_LABEL);
  });

  it('改回挑工具的按钮文案', () => {
    expect(BACK_TO_PICK_LABEL).toBe('改回挑工具');
  });
});

describe('后备候选（本地排名 9–16）', () => {
  const ok = (items: Array<{ repoId: string; fit: 'best' | 'good' | 'maybe' | 'poor' }>): RerankResult => ({
    status: 'ok',
    items: items.map((item) => ({ ...item, reason: '理由' })),
    noneFit: false,
    note: null,
    message: null,
    tokens: 5000,
  });

  it('交给 AI 的候选 = 前 8 + 后备，去重，最多 16 个', () => {
    const hits = Array.from({ length: 8 }, (_, index) => hit({ id: `h${index}` }));
    const more = Array.from({ length: 12 }, (_, index) => hit({ id: `m${index}` }));
    const ids = rerankCandidateIds({ hits, moreHits: more });
    expect(ids).toHaveLength(RERANK_CANDIDATE_LIMIT);
    expect(ids.slice(0, 8)).toEqual(hits.map((item) => item.id));
    expect(ids[8]).toBe('m0');
    expect(ids[15]).toBe('m7');
    expect(rerankCandidateIds({ hits: [hit({ id: 'a' }), hit({ id: 'a' })], moreHits: [hit({ id: 'a' }), hit({ id: 'b' })] })).toEqual(['a', 'b']);
  });

  it('旧后端没有 moreHits：只用命中', () => {
    expect(rerankCandidateIds({ hits: [hit({ id: 'a' }), hit({ id: 'b' })], moreHits: undefined })).toEqual(['a', 'b']);
    expect(rerankCandidateIds({ hits: [], moreHits: [] })).toEqual([]);
  });

  it('AI 点名的后备工具（最合适 / 合适）拿出来，按 AI 的顺序', () => {
    const result = { hits: [hit({ id: 'a' }), hit({ id: 'b' })], moreHits: [hit({ id: 'm1' }), hit({ id: 'm2' }), hit({ id: 'm3' })] };
    const rerank = ok([
      { repoId: 'm2', fit: 'best' },
      { repoId: 'a', fit: 'good' },
      { repoId: 'm1', fit: 'good' },
      { repoId: 'm3', fit: 'maybe' },
    ]);
    expect(promotedHits(result, rerank).map((item) => item.id)).toEqual(['m2', 'm1']);
  });

  it('后备候选提升、精选排序和已选交接单都保留维护事实，不自动排除归档项目', () => {
    const main = hit({ id: 'a', pushedAt: '2020-01-01T00:00:00Z', archived: false });
    const archived = hit({ id: 'm1', pushedAt: '2026-10-22T12:00:00Z', archived: true, hasCard: false });
    const legacy = hit({ id: 'm2' });
    const result = { hits: [main], moreHits: [archived, legacy] };
    const rerank = ok([{ repoId: 'm1', fit: 'best' }, { repoId: 'm2', fit: 'good' }]);
    const promoted = promotedHits(result, rerank);
    expect(promoted).toEqual([archived, legacy]);
    const sorted = applyRerank([...result.hits, ...promoted], rerank);
    expect(sorted).toEqual([archived, legacy, main]);
    expect(sorted[0]).toBe(archived);
    expect(orderedPicks(sorted, ['a', 'm1'])).toEqual([main, archived]);
    expect(rerankPicks(rerank, sorted)).toEqual(['m1', 'm2']);
  });

  it('备选 / 不太合适的后备工具不拿出来，已经在列表里的不重复', () => {
    const result = { hits: [hit({ id: 'a' })], moreHits: [hit({ id: 'm1' }), hit({ id: 'a' })] };
    const rerank = ok([
      { repoId: 'm1', fit: 'maybe' },
      { repoId: 'a', fit: 'best' },
    ]);
    expect(promotedHits(result, rerank)).toEqual([]);
  });

  it('精选没成功 / 没有精选 / 没有后备候选时什么也不拿', () => {
    const result = { hits: [hit({ id: 'a' })], moreHits: [hit({ id: 'm1' })] };
    const failed: RerankResult = { status: 'failed', items: [], noneFit: false, note: null, message: '失败', tokens: null };
    expect(promotedHits(result, null)).toEqual([]);
    expect(promotedHits(result, failed)).toEqual([]);
    expect(promotedHits({ hits: result.hits, moreHits: undefined }, ok([{ repoId: 'm1', fit: 'best' }]))).toEqual([]);
  });

  it('cached 的结果也一样拿', () => {
    const result = { hits: [hit({ id: 'a' })], moreHits: [hit({ id: 'm1' })] };
    const cached: RerankResult = { ...ok([{ repoId: 'm1', fit: 'best' }]), status: 'cached' };
    expect(promotedHits(result, cached).map((item) => item.id)).toEqual(['m1']);
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
    expect(formatOptions.map((item) => item.label)).toEqual([
      '文档（Markdown，直接粘给 AI 助手，默认）',
      '长期规则（AGENTS.md，存成文件放进工作文件夹）',
      '程序用（JSON，一般不用选）',
    ]);
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

describe('pickLocalCapabilities', () => {
  const all = [
    { name: 'ai-draw', oneLiner: 'AI 绘图，生成封面和海报', path: '~/.agents/skills/ai-draw' },
    { name: 'guizang-social-card-skill', oneLiner: '小红书图文卡片', path: '~/.agents/skills/guizang-social-card-skill' },
    { name: 'code-review', oneLiner: '代码审查', path: '~/.agents/skills/code-review' },
    { name: 'gpt-image', oneLiner: 'MCP 工具', path: '~/.codex/config.toml' },
  ];

  it('名字和选中项目对得上的排在最前（本机已装），相关的跟在后面', () => {
    const picked = pickLocalCapabilities(all, [{ fullName: 'op7418/guizang-social-card-skill' }], ['封面']);
    expect(picked.map((item) => item.name)).toEqual(['guizang-social-card-skill', 'ai-draw', 'gpt-image']);
  });

  it('任务词出现在名字或一句话里的算相关，最多 limit 个；不相关的不放', () => {
    expect(pickLocalCapabilities(all, [], ['封面']).map((item) => item.name)).toEqual(['ai-draw', 'gpt-image']);
    expect(pickLocalCapabilities(all, [], ['小红书', '封面'], 1).map((item) => item.name)).toEqual(['ai-draw']);
    expect(pickLocalCapabilities(all, [], ['数据库'])).toEqual([]);
    expect(pickLocalCapabilities(all, [], ['封面'], 0)).toEqual([]);
  });

  it('近义说法也算：中文任务词能对上英文说明和只有名字的 MCP', () => {
    // 「配图」→ image / draw：MCP 只有名字（gpt-image），Skill 说明是英文
    const real = [
      { name: 'ai-draw', oneLiner: 'Route, generate, edit, save, and verify AI visual assets for…', path: '~/.agents/skills/ai-draw' },
      { name: 'gpt-image', oneLiner: 'MCP 工具', path: '~/.codex/config.toml' },
      { name: 'gemini-image', oneLiner: 'MCP 工具', path: '~/.codex/config.toml' },
      { name: 'ai-upscale', oneLiner: 'Upscale AI-generated or photographic images via fal-ai MCP', path: '~/.agents/skills/ai-upscale' },
      { name: 'lark-im', oneLiner: '飞书即时通讯：收发消息和管理群聊', path: '~/.agents/skills/lark-im' },
    ];
    // 名字里有 draw / image 的各 6 分（两个任务词各 3 分）；ai-upscale 只在一句话里有 image，2 分；lark-im 一分没有
    expect(pickLocalCapabilities(real, [], ['详情页', '图片']).map((item) => item.name)).toEqual([
      'ai-draw',
      'gemini-image',
      'gpt-image',
      'ai-upscale',
    ]);
  });

  it('名字里有任务词（含近义词）的排在只有一句话里有的前面；同分按名字', () => {
    const items = [
      { name: 'zzz-notes', oneLiner: '整理封面素材', path: 'p1' },
      { name: 'cover-maker', oneLiner: '工具', path: 'p2' },
      { name: 'aaa-poster', oneLiner: '工具', path: 'p3' },
    ];
    // zzz-notes 只在一句话里提到一次封面（1 分），不够 2 分，不放
    expect(pickLocalCapabilities(items, [], ['封面']).map((item) => item.name)).toEqual(['aaa-poster', 'cover-maker']);
    // 同一句话里提到两个任务词（2 分）就够了
    const twice = [{ name: 'zzz-notes', oneLiner: '整理封面素材和海报模板', path: 'p1' }];
    expect(pickLocalCapabilities(twice, [], ['封面', '海报']).map((item) => item.name)).toEqual(['zzz-notes']);
  });

  it('零星提到一次任务词的不算相关：「调整尺寸」的电子表格不是改图工具', () => {
    const items = [
      { name: 'lark-sheets', oneLiner: '飞书电子表格：创建和操作电子表格，支持增删行列、调整尺寸、冻结', path: '~/.agents/skills/lark-sheets' },
      { name: 'image-resize', oneLiner: 'MCP 工具', path: '~/.codex/config.toml' },
    ];
    expect(pickLocalCapabilities(items, [], ['改图', '压缩', '尺寸']).map((item) => item.name)).toEqual(['image-resize']);
  });

  it('新增的任务词有近义说法：压缩 / 周报 / 转写 / 抠图能对上英文名字', () => {
    const items = [
      { name: 'tinypng-compress', oneLiner: '工具', path: 'p1' },
      { name: 'weekly-report', oneLiner: '工具', path: 'p2' },
      { name: 'whisper', oneLiner: 'MCP 工具', path: 'p3' },
      { name: 'rembg', oneLiner: 'MCP 工具', path: 'p4' },
      { name: 'code-review', oneLiner: '代码审查', path: 'p5' },
    ];
    expect(pickLocalCapabilities(items, [], ['压缩']).map((item) => item.name)).toEqual(['tinypng-compress']);
    expect(pickLocalCapabilities(items, [], ['周报']).map((item) => item.name)).toEqual(['weekly-report']);
    expect(pickLocalCapabilities(items, [], ['转写']).map((item) => item.name)).toEqual(['whisper']);
    expect(pickLocalCapabilities(items, [], ['抠图']).map((item) => item.name)).toEqual(['rembg']);
  });

  it('没有本机能力、没有任务词，都是空', () => {
    expect(pickLocalCapabilities([], [{ fullName: 'a/b' }], ['封面'])).toEqual([]);
    expect(pickLocalCapabilities(all, [], [])).toEqual([]);
  });

  it('仓库全名或短名（不分大小写）都能对上', () => {
    expect(pickLocalCapabilities(all, [{ fullName: 'Foo/AI-Draw' }], []).map((item) => item.name)).toEqual(['ai-draw']);
  });
});

describe('交接单文案', () => {
  it('buildPackRequest 的标题前缀是交接单，直接模式仍是直接交给 AI 助手', () => {
    expect(buildPackRequest('做小红书封面', [hit({ id: '1' })]).title).toBe('交接单：做小红书封面');
    expect(buildPackRequest('做小红书封面', [hit({ id: '1' })], [], true).title).toBe('直接交给 AI 助手：做小红书封面');
  });

  it('统一后的按钮常量', () => {
    expect(DIRECT_LABEL).toBe('不用这些工具，直接交给 AI 助手');
    expect(DIRECT_FROM_PICKS_LABEL).toBe(DIRECT_LABEL);
    expect(BACK_TO_PICK_LABEL).toBe('改回挑工具');
  });

  it('格式选项只改显示文字，value 仍是 markdown / agents / json', () => {
    expect(formatOptions.map((item) => [item.value, item.label])).toEqual([
      ['markdown', '文档（Markdown，直接粘给 AI 助手，默认）'],
      ['agents', '长期规则（AGENTS.md，存成文件放进工作文件夹）'],
      ['json', '程序用（JSON，一般不用选）'],
    ]);
  });

  it('列表顶部那一行：有默认勾选才说，没有或直接模式不说', () => {
    expect(defaultPickNote(2, false)).toBe('已替你勾了最对口的 2 个，不合适可以取消');
    expect(defaultPickNote(0, false)).toBeNull();
    expect(defaultPickNote(2, true)).toBeNull();
  });
});

describe('格式选项：名字和一句说明', () => {
  it('每项的完整一句就是「名字（说明）」，value 和顺序不变', () => {
    expect(formatOptions.map((item) => item.value)).toEqual(['markdown', 'agents', 'json']);
    for (const option of formatOptions) {
      expect(option.name.length).toBeGreaterThan(0);
      expect(option.note.length).toBeGreaterThan(0);
      expect(option.label).toBe(`${option.name}（${option.note}）`);
      expect(option.name).not.toContain('（');
    }
    expect(formatOptions.map((item) => item.name)).toEqual(['文档', '长期规则', '程序用']);
    expect(formatOptions.map((item) => item.note)).toEqual([
      'Markdown，直接粘给 AI 助手，默认',
      'AGENTS.md，存成文件放进工作文件夹',
      'JSON，一般不用选',
    ]);
  });
});

describe('列表顶部那一行：N 和界面上勾着的个数对得上', () => {
  const hits = ['a', 'b', 'c', 'd'].map((id) => hit({ id }));
  // 后端默认勾了 a、b 两个
  const found = result({ hits, preselected: ['a', 'b'] });
  const aiFits = (items: Array<{ repoId: string; fit: RerankFit }>): RerankResult => rerank({
    items: items.map((item) => ({ ...item, reason: '理由' })),
  });
  const countOnScreen = (picked: readonly string[]) => orderedPicks(found.hits, picked).length;

  it('AI 精选之前：就是后端默认勾的个数', () => {
    const picked = initialPicks(found);
    expect(picked).toEqual(['a', 'b']);
    expect(pickListNote(found.hits, picked, false, false)).toBe('已替你勾了最对口的 2 个，不合适可以取消');
    expect(countOnScreen(picked)).toBe(2);
  });

  it('AI 精选改勾之后：N 跟着现在勾着的个数变，不再写后端当初的 2', () => {
    // 变少
    const one = rerankPicks(aiFits([{ repoId: 'c', fit: 'best' }, { repoId: 'a', fit: 'maybe' }]), found.hits);
    expect(one).toEqual(['c']);
    expect(pickListNote(found.hits, one ?? [], false, false)).toBe('已替你勾了最对口的 1 个，不合适可以取消');
    // 变多（最多 3 个）
    const three = rerankPicks(aiFits([
      { repoId: 'a', fit: 'best' },
      { repoId: 'b', fit: 'best' },
      { repoId: 'c', fit: 'good' },
      { repoId: 'd', fit: 'good' },
    ]), found.hits);
    expect(three).toEqual(['a', 'b', 'c']);
    expect(pickListNote(found.hits, three ?? [], false, false)).toBe('已替你勾了最对口的 3 个，不合适可以取消');
    // 每一种 N 都等于右边「已选 N 个」那个数
    expect(countOnScreen(one ?? [])).toBe(1);
    expect(countOnScreen(three ?? [])).toBe(3);
  });

  it('AI 说一个都不合适：勾选清空，顶上这一行不说；右边说「没有特别对口的，所以没替你默认勾选」', () => {
    const none = rerankPicks(rerank({ noneFit: true, items: [{ repoId: 'a', fit: 'best', reason: '写了也不信' }] }), found.hits);
    expect(none).toEqual([]);
    expect(pickListNote(found.hits, none ?? [], false, false)).toBeNull();
    // 后端当初默认勾了 a、b，是 AI 把它们清空的（用户没动过）：不是用户取消的，要解释
    expect(packEmptyNote(found, false)).toBe('没有特别对口的，所以没替你默认勾选。看一眼左边的列表自己勾，或者直接交给 AI 助手。');
    // 后端本来就没默认勾：一直是这句
    expect(packEmptyNote({ hits: found.hits, preselected: [] })).toContain('没有特别对口的，所以没替你默认勾选');
    expect(packEmptyNote({ hits: found.hits, preselected: [] }, false)).toContain('没有特别对口的，所以没替你默认勾选');
  });

  it('用户自己把默认勾的取消光：右边不解释（不传 edited 也按用户改过算）', () => {
    expect(packEmptyNote(found, true)).toBeNull();
    expect(packEmptyNote(found)).toBeNull();
    expect(pickListNote(found.hits, [], false, true)).toBe('已勾 0 个');
  });

  it('AI 精选没成功：勾选不动，N 还是默认勾的', () => {
    const picked = initialPicks(found);
    expect(rerankPicks(rerank({ status: 'failed' }), found.hits)).toBeNull();
    expect(pickListNote(found.hits, picked, false, false)).toBe('已替你勾了最对口的 2 个，不合适可以取消');
  });

  it('用户手动改勾之后：不再说「替你勾」，改成「已勾 N 个」，N 跟着变', () => {
    const afterAi = rerankPicks(aiFits([{ repoId: 'c', fit: 'best' }]), found.hits) ?? [];
    const added = togglePick(afterAi, 'a');
    expect(pickListNote(found.hits, added, false, true)).toBe('已勾 2 个');
    const removedOne = togglePick(added, 'c');
    expect(pickListNote(found.hits, removedOne, false, true)).toBe('已勾 1 个');
    const removedAll = togglePick(removedOne, 'a');
    expect(pickListNote(found.hits, removedAll, false, true)).toBe('已勾 0 个');
    for (const picked of [added, removedOne, removedAll]) {
      expect(pickListNote(found.hits, picked, false, true)).not.toContain('替你勾');
    }
  });

  it('直接交给 Codex 时不说，不管改没改过', () => {
    expect(pickListNote(found.hits, ['a', 'b'], true, false)).toBeNull();
    expect(pickListNote(found.hits, ['a', 'b'], true, true)).toBeNull();
  });

  it('只数命中里真有的，不在命中里的 id 不算', () => {
    expect(pickListNote(found.hits, ['a', 'gone', 'a'], false, false)).toBe('已替你勾了最对口的 1 个，不合适可以取消');
    expect(pickListNote(found.hits, ['gone'], false, true)).toBe('已勾 0 个');
  });

  it('defaultPickNote 本身：坏数字不说，改过的写已勾', () => {
    expect(defaultPickNote(Number.NaN, false)).toBeNull();
    expect(defaultPickNote(Number.NaN, false, true)).toBeNull();
    expect(defaultPickNote(-1, false)).toBeNull();
    expect(defaultPickNote(3, false, true)).toBe('已勾 3 个');
    expect(defaultPickNote(3, true, true)).toBeNull();
  });
});

describe('界面上残留的旧叫法已换成统一用词', () => {
  it('没找到时引导去「发现」的那句说场景；有待 AI 整理的接在后面', () => {
    expect(emptyResultHint(result({ pendingMatches: 0 }))).toBe('去「发现」按这个场景找找看');
    expect(emptyResultHint(result({ pendingMatches: 3 }))).toBe(
      '去「发现」按这个场景找找看。另有 3 个待 AI 整理，整理好后可能对得上。',
    );
  });

  it('交接单大小：不到一万字写具体数，一万字以上写「约 X 万字」', () => {
    expect(packSizeText(0)).toBe('约 0 字');
    expect(packSizeText(850)).toBe('约 850 字');
    expect(packSizeText(9999)).toBe('约 9999 字');
    expect(packSizeText(3457)).toBe('约 3457 字');
    expect(packSizeText(1000)).toBe('约 1000 字');
    expect(packSizeText(10000)).toBe('约 1 万字');
    expect(packSizeText(12345)).toBe('约 1.2 万字');
    expect(packSizeText(12950)).toBe('约 1.3 万字');
    expect(packSizeText(99999)).toBe('约 10 万字');
    expect(packSizeText(123456)).toBe('约 12.3 万字');
    expect(packSizeText(Number.NaN)).toBe('');
    expect(packSizeText(-5)).toBe('');
    // 一万字以上不再写成带逗号的「12,345」；四位数也不加千分位（「约 3,457 字」→「约 3457 字」）
    expect(packSizeText(12345)).not.toContain(',');
    for (const length of [1000, 3457, 9999]) {
      expect(packSizeText(length), String(length)).not.toContain(',');
    }
  });

  it('交接单里有工具已不在资料库里：写工具和资料库，不写项目 / 库', () => {
    expect(missingToolsNote(2)).toBe('有 2 个工具已不在资料库里（可能你取消了收藏）');
    expect(missingToolsNote(0)).toBeNull();
    expect(missingToolsNote(Number.NaN)).toBeNull();
  });

  it('找方案页的接线：用上面这些函数，不再自己拼数字和句子', () => {
    const page = uiText(solvePageSource);
    // 列表顶部那一行数的是界面上勾着的（picked），不是后端的 preselected
    expect(page).toContain('pickListNote(result.hits, picked, direct, picksEdited)');
    expect(page).not.toContain('result.preselected.length');
    // 用户改过勾选才算改过：点勾选、设主参考都走 updatePicked
    expect(page).toContain('setPicksEdited(true)');
    expect(page).toContain('setPicksEdited(false)');
    expect(page).toContain('packEmptyNote(result, picksEdited)');
    // 大小、缺失提示、空状态提示、格式列表
    expect(page).toContain('packSizeText(pack.content.length)');
    expect(page).not.toContain("toLocaleString('zh-CN')");
    expect(page).toContain('missingToolsNote(pack.missing.length)');
    expect(page).toContain('emptyResultHint(props.result)');
    expect(page).toContain('<FormatPicker');
    expect(page).not.toContain('Segmented');
  });

  it('界面源码（注释除外）里不再出现旧叫法', () => {
    const banned = ['参考包', '参考项目', '任务交接', '直接交接单', '工作目录说明', '仓库', '项目', '方向', '待整理', '还没整理', '理解卡', '知识库', '您'];
    const files: Array<[string, string]> = [
      ['pages/v2/solve.tsx', solvePageSource],
      ['lib/solve-view.ts', solveViewSource],
      ['components/solve/solve-hit-row.tsx', hitRowSource],
      ['components/solve/hit-drawer.tsx', hitDrawerSource],
      ['components/solve/format-picker.tsx', formatPickerSource],
    ];
    for (const [file, source] of files) {
      const text = uiText(source);
      expect(text.length, `${file} 读不到`).toBeGreaterThan(100);
      for (const term of banned) {
        expect(text, `${file} 里还有旧叫法「${term}」`).not.toContain(term);
      }
    }
  });
});

describe('复制成功的提示', () => {
  it('复制交接单和复制单个工具的话，用同一句（审读第 5 条）', () => {
    expect(COPY_FOR_CODEX_TOAST).toBe('已复制。粘给 AI 助手后，它会先查你电脑上有没有，没有就装进独立文件夹，装完告诉你装在哪、怎么卸载。');
  });

  it('「去查别人怎么用」那段话只读资料：提示不能说它会装东西；认不出的话按会装东西的那句说', () => {
    const target = { fullName: 'fox/cover-kit', htmlUrl: 'https://github.com/fox/cover-kit', form: 'cli', scenarioName: '小红书' };
    expect(copyPromptToast(buildInstallPrompt(target))).toBe(COPY_FOR_CODEX_TOAST);
    expect(copyPromptToast(buildCommunityPrompt(target, new Date(2026, 9, 5)))).toBe(COPY_FOR_RESEARCH_TOAST);
    expect(copyPromptToast('一段没见过的话')).toBe(COPY_FOR_CODEX_TOAST);
    expect(COPY_FOR_RESEARCH_TOAST).toContain('不会安装');
    expect(COPY_FOR_RESEARCH_TOAST).not.toContain('装进');
  });

  it('找方案页和详情抽屉都用这个常量，没有各抄一份，也不再有旧提示', () => {
    const page = solvePageSource;
    const drawer = hitDrawerSource;
    expect(page).toContain('COPY_FOR_CODEX_TOAST');
    expect(drawer).toContain('COPY_FOR_CODEX_TOAST');
    expect(drawer).toContain('copyPromptToast');
    for (const [name, text] of [['solve.tsx', page], ['hit-drawer.tsx', drawer]] as const) {
      expect(text, `${name} 里还有旧提示`).not.toContain('已复制，去 AI 助手里粘贴');
      expect(text, `${name} 里又写了一份提示原文`).not.toContain('粘给 AI 助手后');
    }
  });
});

describe('previewDisplayText：交接单排版预览里的星数和日期', () => {
  const now = new Date(2026, 9, 5, 12, 0, 0);
  const line = (url: string, stars: string, date: string) => `- 链接：${url} · ★${stars} · 最近更新 ${date}`;

  it('★27.2k → ★2.7 万，2026-07-01 → 7 月 1 日（今年的不带年份）', () => {
    const shown = previewDisplayText(line('https://github.com/a/big', '27.2k', '2026-07-01'), [], now);
    expect(shown).toBe('- 链接：https://github.com/a/big · ★2.7 万 · 最近更新 7 月 1 日');
  });

  it('不到一万的写原数字：认得出是哪个工具，就用它真实的个数（★1234，不是从 1.2k 倒推的 ★1200）', () => {
    const hits = [{ htmlUrl: 'https://github.com/a/mid', starsCount: 1234 }];
    expect(previewDisplayText(line('https://github.com/a/mid', '1.2k', '2026-07-01'), hits, now))
      .toBe('- 链接：https://github.com/a/mid · ★1234 · 最近更新 7 月 1 日');
    // 一万以上也用真实个数：27 249 → 2.7 万，不是 27.2k 倒推出来的 27 200
    const big = [{ htmlUrl: 'https://github.com/a/big', starsCount: 27249 }];
    expect(previewDisplayText(line('https://github.com/a/big', '27.2k', '2026-07-01'), big, now))
      .toContain('★2.7 万');
  });

  it('对不上哪个工具时，按「k」推回个数：1k → ★1000，12k → ★1.2 万，999 原样', () => {
    const none: never[] = [];
    expect(previewDisplayText(line('https://github.com/a/x', '999', '2026-07-01'), none, now)).toContain('★999 ·');
    expect(previewDisplayText(line('https://github.com/a/x', '1k', '2026-07-01'), none, now)).toContain('★1000 ·');
    expect(previewDisplayText(line('https://github.com/a/x', '1.2k', '2026-07-01'), none, now)).toContain('★1200 ·');
    expect(previewDisplayText(line('https://github.com/a/x', '12k', '2026-07-01'), none, now)).toContain('★1.2 万 ·');
    expect(previewDisplayText(line('https://github.com/a/x', '12.3k', '2026-07-01'), none, now)).toContain('★1.2 万 ·');
  });

  it('不是今年的日期带年份；不知道的日期原样写「未知」', () => {
    expect(previewDisplayText(line('https://github.com/a/x', '5', '2025-12-31'), [], now)).toContain('最近更新 2025 年 12 月 31 日');
    expect(previewDisplayText(line('https://github.com/a/x', '5', '未知'), [], now)).toContain('最近更新 未知');
  });

  it('整份交接单里只换工具那一行：别的行（包括正文里碰巧写了 ★ 和日期的）一个字不动，多个工具各换各的', () => {
    const markdown = [
      '# 交接单：做小红书封面',
      '我想 ★3.5k 的那种，2026-07-01 之前要做完。',
      '## 1. a/one（主参考）',
      line('https://github.com/a/one', '27.2k', '2026-07-01'),
      '- 许可证：MIT',
      '## 2. b/two（备选）',
      line('https://github.com/b/two', '42', '2024-01-02'),
      '- 是什么：封面工具 ★1k',
    ].join('\n');
    const shown = previewDisplayText(markdown, [], now).split('\n');
    expect(shown[0]).toBe('# 交接单：做小红书封面');
    expect(shown[1]).toBe('我想 ★3.5k 的那种，2026-07-01 之前要做完。');
    expect(shown[3]).toBe('- 链接：https://github.com/a/one · ★2.7 万 · 最近更新 7 月 1 日');
    expect(shown[4]).toBe('- 许可证：MIT');
    expect(shown[6]).toBe('- 链接：https://github.com/b/two · ★42 · 最近更新 2024 年 1 月 2 日');
    expect(shown[7]).toBe('- 是什么：封面工具 ★1k');
    expect(shown).toHaveLength(8);
  });

  it('没有这种行的文档（JSON、直接交给 AI 助手的交接单）原样返回', () => {
    const json = '{"items":[{"stars":27200,"pushedOn":"2026-07-01"}]}';
    expect(previewDisplayText(json, [], now)).toBe(json);
    expect(previewDisplayText('', [], now)).toBe('');
    expect(previewDisplayText('# 直接交给 AI 助手：把群聊整理成日报', [], now)).toBe('# 直接交给 AI 助手：把群聊整理成日报');
  });

  it('不改传进来的原文（复制的永远是原文）', () => {
    const original = line('https://github.com/a/one', '27.2k', '2026-07-01');
    const copy = `${original}`;
    previewDisplayText(original, [], now);
    expect(original).toBe(copy);
    expect(original).toContain('★27.2k');
  });
});

describe('找方案页的预览按钮和提示', () => {
  it('看原文 / 看排版 → 「看 Markdown 原文」「看排版效果」；列表顶上那句缩短成不会折行的一句', () => {
    const page = uiText(solvePageSource);
    expect(page).toContain("showRaw ? '看排版效果' : '看 Markdown 原文'");
    expect(page).not.toContain("'看排版'");
    expect(page).not.toContain("'看原文'");
    expect(page).toContain('勾选的写进交接单，点一行看详情');
    expect(page).not.toContain('勾选的会写进交接单');
  });

  it('排版预览把交接单交给 previewDisplayText 换写法；原文和复制不经过它', () => {
    const page = uiText(solvePageSource);
    expect(page).toContain('markdown={previewDisplayText(props.content, props.hits)}');
    // 「看原文」和 JSON 直接显示 props.content
    expect(page).toContain('<pre className={PREVIEW_CLASS}>{props.content}</pre>');
    // 复制的是后端给的原文，不是换过写法的
    expect(page).toContain('copy(pack.content, COPY_FOR_CODEX_TOAST, detail)');
  });
});

describe('packEmptyNote', () => {
  it('没搜过：不说话', () => {
    expect(packEmptyNote(null)).toBeNull();
  });

  it('一个都没搜到：说收藏里没找到', () => {
    expect(packEmptyNote({ hits: [], preselected: [] })).toBe('收藏里没找到相关的工具。');
  });

  it('搜到了但没有默认勾选：说为什么没勾，并指出可以直接交给 AI 助手', () => {
    const note = packEmptyNote({ hits: [{ id: 'a' }] as never, preselected: [] });
    expect(note).toContain('没替你默认勾选');
    expect(note).toContain('直接交给 AI 助手');
  });

  it('有默认勾选（用户自己取消的）：不解释', () => {
    expect(packEmptyNote({ hits: [{ id: 'a' }] as never, preselected: ['a'] })).toBeNull();
  });
});

function rerank(partial: Partial<RerankResult> = {}): RerankResult {
  return {
    status: partial.status ?? 'ok',
    items: partial.items ?? [],
    noneFit: partial.noneFit ?? false,
    note: partial.note ?? null,
    message: partial.message ?? null,
    tokens: partial.tokens === undefined ? null : partial.tokens,
  };
}

describe('applyRerank', () => {
  const hits = [hit({ id: 'a' }), hit({ id: 'b' }), hit({ id: 'c' })];

  it('不可用或没有条目时返回原数组', () => {
    expect(applyRerank(hits, null)).toBe(hits);
    expect(applyRerank(hits, rerank({ status: 'failed', items: [{ repoId: 'c', fit: 'best', reason: '好' }] }))).toBe(hits);
    expect(applyRerank(hits, rerank({ status: 'skipped' }))).toBe(hits);
    expect(applyRerank(hits, rerank({ status: 'ok', items: [] }))).toBe(hits);
  });

  it('按精选顺序重排', () => {
    const next = applyRerank(hits, rerank({
      items: [
        { repoId: 'c', fit: 'best', reason: '最对口' },
        { repoId: 'a', fit: 'good', reason: '能用' },
        { repoId: 'b', fit: 'maybe', reason: '沾边' },
      ],
    }));
    expect(next.map((item) => item.id)).toEqual(['c', 'a', 'b']);
    expect(next).not.toBe(hits);
  });

  it('精选里没有的命中按原顺序放在最后', () => {
    const next = applyRerank(hits, rerank({
      items: [{ repoId: 'c', fit: 'best', reason: '最对口' }],
    }));
    expect(next.map((item) => item.id)).toEqual(['c', 'a', 'b']);
  });

  it('精选里多出来的 id 忽略', () => {
    const next = applyRerank(hits, rerank({
      items: [
        { repoId: 'gone', fit: 'best', reason: '没有这个' },
        { repoId: 'b', fit: 'good', reason: '能用' },
      ],
    }));
    expect(next.map((item) => item.id)).toEqual(['b', 'a', 'c']);
  });

  it('不改入参', () => {
    const source = [hit({ id: 'a' }), hit({ id: 'b' })];
    const snapshot = source.map((item) => item.id);
    applyRerank(source, rerank({
      items: [
        { repoId: 'b', fit: 'best', reason: '好' },
        { repoId: 'a', fit: 'good', reason: '行' },
      ],
    }));
    expect(source.map((item) => item.id)).toEqual(snapshot);
  });
});

describe('rerankPicks', () => {
  const hits = [hit({ id: 'a' }), hit({ id: 'b' }), hit({ id: 'c' }), hit({ id: 'd' })];

  it('best 排在 good 前面', () => {
    const picks = rerankPicks(rerank({
      items: [
        { repoId: 'a', fit: 'good', reason: '能用' },
        { repoId: 'b', fit: 'best', reason: '最对口' },
        { repoId: 'c', fit: 'maybe', reason: '沾边' },
      ],
    }), hits);
    expect(picks).toEqual(['b', 'a']);
  });

  it('最多勾 3 个', () => {
    const picks = rerankPicks(rerank({
      items: [
        { repoId: 'a', fit: 'best', reason: '1' },
        { repoId: 'b', fit: 'best', reason: '2' },
        { repoId: 'c', fit: 'good', reason: '3' },
        { repoId: 'd', fit: 'good', reason: '4' },
      ],
    }), hits);
    expect(picks).toEqual(['a', 'b', 'c']);
  });

  it('没有合适的时候返回空数组', () => {
    expect(rerankPicks(rerank({
      noneFit: true,
      items: [{ repoId: 'a', fit: 'best', reason: '模型写了也不信' }],
    }), hits)).toEqual([]);
  });

  it('精选不可用时返回 null', () => {
    const items = [{ repoId: 'a', fit: 'best' as const, reason: '好' }];
    expect(rerankPicks(null, hits)).toBeNull();
    expect(rerankPicks(rerank({ status: 'failed', items }), hits)).toBeNull();
    expect(rerankPicks(rerank({ status: 'skipped', items }), hits)).toBeNull();
    expect(rerankPicks(rerank({ status: 'refused', items }), hits)).toBeNull();
  });

  it('只保留命中里有的 id', () => {
    expect(rerankPicks(rerank({
      items: [
        { repoId: 'gone', fit: 'best', reason: '没有' },
        { repoId: 'c', fit: 'good', reason: '有' },
      ],
    }), hits)).toEqual(['c']);
  });
});

describe('rerankStatusText', () => {
  it('关着就不说话', () => {
    expect(rerankStatusText({ phase: 'loading', result: null, enabled: false })).toBeNull();
    expect(rerankStatusText({ phase: 'done', result: rerank({ status: 'ok', tokens: 12 }), enabled: false })).toBeNull();
  });

  it('正在挑', () => {
    expect(rerankStatusText({ phase: 'loading', result: null, enabled: true })).toBe('AI 正在帮你挑…');
  });

  it('挑完并带上这次的用量', () => {
    expect(rerankStatusText({ phase: 'done', result: rerank({ status: 'ok', tokens: 12 }), enabled: true })).toBe('AI 已挑过 · 本次约 12 token');
  });

  it('没有用量时只说挑过', () => {
    expect(rerankStatusText({ phase: 'done', result: rerank({ status: 'ok' }), enabled: true })).toBe('AI 已挑过');
  });

  it('用量过千写成一位小数', () => {
    expect(rerankStatusText({ phase: 'done', result: rerank({ status: 'ok', tokens: 4200 }), enabled: true })).toBe('AI 已挑过 · 本次约 4.2 千 token');
  });

  it('用了上次的结果', () => {
    expect(rerankStatusText({ phase: 'done', result: rerank({ status: 'cached', tokens: 0 }), enabled: true })).toBe('AI 已挑过 · 用的是上次的结果，没花 token');
  });

  it('跳过时用返回的说明', () => {
    expect(rerankStatusText({
      phase: 'done',
      result: rerank({ status: 'skipped', message: '候选太少，不用 AI 再挑' }),
      enabled: true,
    })).toBe('候选太少，不用 AI 再挑');
  });

  it('额度用完时用返回的说明', () => {
    expect(rerankStatusText({
      phase: 'done',
      result: rerank({ status: 'refused', message: '今天的额度用完了，这次按本地排序' }),
      enabled: true,
    })).toBe('今天的额度用完了，这次按本地排序');
  });

  it('失败时用返回的说明', () => {
    expect(rerankStatusText({
      phase: 'done',
      result: rerank({ status: 'failed', message: 'AI 精选没成功，这次按本地排序' }),
      enabled: true,
    })).toBe('AI 精选没成功，这次按本地排序');
  });

  it('还没开始时不说话', () => {
    expect(rerankStatusText({ phase: 'idle', result: null, enabled: true })).toBeNull();
    expect(rerankStatusText({ phase: 'done', result: null, enabled: true })).toBeNull();
    expect(rerankStatusText({
      phase: 'done',
      result: rerank({ status: 'failed', message: '   ' }),
      enabled: true,
    })).toBeNull();
  });
});

describe('精选文案', () => {
  it('档位用白话', () => {
    expect(RERANK_FIT_LABEL).toEqual({ best: '最合适', good: '合适', maybe: '备选', poor: '不太合适' });
  });

  it('开关写出开或关', () => {
    expect(RERANK_TOGGLE_LABEL(true)).toBe('AI 精选：开');
    expect(RERANK_TOGGLE_LABEL(false)).toBe('AI 精选：关');
  });
});
