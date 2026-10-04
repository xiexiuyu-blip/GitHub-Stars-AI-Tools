import { describe, expect, it } from 'vitest';
import type { RerankResult, ScenarioSummary, SolveHit, SolveResult } from '@/lib/contracts';
import {
  applyRerank,
  promotedHits,
  RERANK_CANDIDATE_LIMIT,
  rerankCandidateIds,
  BACK_TO_PICK_LABEL,
  buildPackRequest,
  crossNote,
  DIRECT_FROM_PICKS_LABEL,
  DIRECT_LABEL,
  formatOptions,
  gapNote,
  initialPicks,
  makeMain,
  pickLocalCapabilities,
  matchBadge,
  noIntentNote,
  orderedPicks,
  packEmptyNote,
  packPlaceholder,
  pendingNote,
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

  it('直接模式不带项目，mode 是 direct，标题换成直接交给 Codex', () => {
    const hits = [hit({ id: '1', why: '主因' }), hit({ id: '2', why: '备因' })];
    const request = buildPackRequest('批量改图尺寸和压缩', hits, [], true);
    expect(request.items).toEqual([]);
    expect(request.mode).toBe('direct');
    expect(request.title).toBe('直接交给 Codex：批量改图尺寸和压缩');
    expect(request.problem).toBe('批量改图尺寸和压缩');
    expect(request.localCapabilities).toEqual([]);
    expect(hits.map((item) => item.id)).toEqual(['1', '2']);
  });

  it('非直接模式不写 mode', () => {
    const request = buildPackRequest('做小红书封面', [hit({ id: '1' })]);
    expect(request.mode).toBeUndefined();
    expect(Object.prototype.hasOwnProperty.call(request, 'mode')).toBe(false);
    expect(request.title).toBe('参考包：做小红书封面');
  });

  it('直接模式带上本机能力，而且不和调用方共用数组', () => {
    const local = [{ name: 'sips', oneLiner: '改图片尺寸', path: '/usr/bin/sips' }];
    const request = buildPackRequest('批量改图尺寸和压缩', [hit({ id: '1' })], local, true);
    expect(request.mode).toBe('direct');
    expect(request.items).toEqual([]);
    expect(request.localCapabilities).toEqual(local);
    expect(request.localCapabilities).not.toBe(local);
  });

  it('直接交给 Codex 的按钮文案', () => {
    expect(DIRECT_LABEL).toBe('没有合适的？直接交给 Codex');
  });

  it('勾了工具之后的直接交接入口文案，和没勾时的不同', () => {
    expect(DIRECT_FROM_PICKS_LABEL).toBe('都不合适？直接交给 Codex');
    expect(DIRECT_FROM_PICKS_LABEL).not.toBe(DIRECT_LABEL);
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

describe('packEmptyNote', () => {
  it('没搜过：不说话', () => {
    expect(packEmptyNote(null)).toBeNull();
  });

  it('一个都没搜到：说收藏里没找到', () => {
    expect(packEmptyNote({ hits: [], preselected: [] })).toBe('收藏里没找到相关的工具。');
  });

  it('搜到了但没有默认勾选：说为什么没勾，并指出可以直接交给 Codex', () => {
    const note = packEmptyNote({ hits: [{ id: 'a' }] as never, preselected: [] });
    expect(note).toContain('没替你默认勾选');
    expect(note).toContain('直接交给 Codex');
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
