import { describe, expect, it, vi } from 'vitest';
import type { ReactElement, ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import type { LibraryDetail, ScenarioSummary } from '@/lib/contracts';
import { ItemDetail, PendingNote } from './item-detail';
import { AutomationBanner } from './automation-banner';
import { automationHint, type AutomationStatus } from '@/lib/automation-view';
const automation = vi.hoisted(() => ({ status: null as AutomationStatus | null }));
vi.mock('@/hooks/use-ai-automation', () => ({ useAiAutomation: () => ({ status: automation.status }) }));
// 源码原文：守住「说什么由 item-format 决定」（Vite 的 ?raw，不用 node 的类型）
import itemDetailSource from './item-detail.tsx?raw';

function actionOf(node: ReactNode): (() => void) | undefined {
  if (Array.isArray(node)) return node.map(actionOf).find(Boolean);
  if (!node || typeof node !== 'object' || !('props' in node)) return undefined;
  const props = (node as ReactElement<Record<string, unknown>>).props;
  if (typeof props.onClick === 'function') return props.onClick as () => void;
  return actionOf(props.children as ReactNode);
}

describe('未整理提示按钮的真实去向', () => {
  it('横幅与详情：没配置 AI 去 AI 设置；暂停/额度仍去自动整理', () => {
    const onOpenAiSettings = vi.fn();
    const onOpenToolbox = vi.fn();
    try {
      for (const [providerConfigured, paused, budgetRemaining] of [[false, false, 0], [true, true, 10], [true, false, 0]] as const) {
        automation.status = { providerConfigured, paused, budgetRemaining, enabled: true, queued: 0, running: 0 } as AutomationStatus;
        const hint = automationHint(automation.status, 3);
        const banner = AutomationBanner({ hint, onOpenAiSettings, onAction: onOpenToolbox });
        const note = PendingNote({ fullName: 'fox/tool', description: null, onOpenAiSettings, onOpenToolbox });
        const expected = providerConfigured ? onOpenToolbox : onOpenAiSettings;
        expect(actionOf(banner)).toBe(expected);
        expect(actionOf(note)).toBe(expected);
        actionOf(banner)?.(); actionOf(note)?.();
      }
      expect(onOpenAiSettings).toHaveBeenCalledTimes(2);
      expect(onOpenToolbox).toHaveBeenCalledTimes(4);
    } finally {
      automation.status = null;
    }
  });

  it('缺 AI 配置入口时不把「去设置 AI」错接到自动整理', () => {
    const hint = automationHint({ providerConfigured: false } as AutomationStatus, 3);
    expect(actionOf(AutomationBanner({ hint, onAction: vi.fn() }))).toBeUndefined();
  });
});

/** 去掉图标字（Material Symbols 的图标名是文字）和标签，留下用户看得见的字。 */
function textOf(html: string): string {
  return html
    .replace(/<span class="material-symbols-outlined[^>]*>[^<]*<\/span>/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function tabLabels(html: string): string[] {
  const labels: string[] = [];
  for (const match of html.matchAll(/<button[^>]*role="tab"[^>]*>(.*?)<\/button>/g)) {
    labels.push(textOf(match[1]));
  }
  return labels;
}

function make(overrides: Partial<LibraryDetail> = {}): LibraryDetail {
  return {
    id: 'r1',
    fullName: 'fox/tool',
    htmlUrl: 'https://github.com/fox/tool',
    description: 'A cover maker',
    language: 'TypeScript',
    starsCount: 12345,
    starredAt: '2026-10-03T18:17:00Z',
    pushedAt: '2026-09-30T00:00:00Z',
    oneLiner: '做封面',
    form: 'cli',
    scenarioIds: [],
    readingStatus: 'unread',
    readingGroup: 'unseen',
    hasCard: true,
    confidence: 0.9,
    cardReview: 'suggested',
    whatItIs: '这是一个做封面的工具。第二句。',
    capabilities: ['批量做封面'],
    problemsSolved: ['每天要出十张封面'],
    useWhen: ['要出十张图时'],
    notFor: [],
    howToTry: 'npm i tool',
    integratesWith: [],
    alternativesHint: [],
    inspirationForFox: null,
    risks: [],
    license: null,
    maturity: 'usable',
    zhFriendly: null,
    keywords: [],
    topics: [],
    forksCount: 1,
    readmeExcerpt: null,
    ...overrides,
  };
}

function render(detail: LibraryDetail | null, extra: { scenarios?: ScenarioSummary[] } = {}): string {
  return renderToStaticMarkup(
    <ItemDetail
      detail={detail}
      scenarios={extra.scenarios}
      onReadingChange={() => {}}
      onConfirm={() => {}}
      onRegenerate={() => {}}
      onCopyCodex={() => {}}
      onCopyHow={() => {}}
    />,
  );
}

function scenario(id: string, nameZh: string): ScenarioSummary {
  return { id, slug: id, nameZh, sortOrder: 1, count: 3, thin: false };
}

describe('详情：收藏于', () => {
  // 用本机时间造出 UTC 串（GitHub 给的 starredAt 是 UTC）：不管测试在哪个时区跑，它在本机日历上都是同一天
  const utcAtLocal = (year: number, month: number, day: number, hour: number, minute = 0) =>
    new Date(year, month - 1, day, hour, minute).toISOString();

  it('只写日期、不写钟点（收藏时间是 UTC，钟点和本机时间差好几个小时）', () => {
    const thisYear = new Date().getFullYear();
    const html = render(make({ starredAt: utcAtLocal(thisYear, 10, 3, 18, 17) }));
    const text = textOf(html);
    expect(text).toContain('收藏于 10 月 3 日');
    expect(text).not.toContain('18:17');
    expect(text).not.toMatch(/10 月 3 日 \d{1,2}:\d{2}/);
  });

  it('不是今年的带年份', () => {
    const text = textOf(render(make({ starredAt: utcAtLocal(2024, 10, 3, 18, 17) })));
    expect(text).toContain('收藏于 2024 年 10 月 3 日');
    expect(text).not.toContain('18:17');
  });

  it('写的是本机日历上的日期：本机凌晨 / 深夜收藏的，UTC 串里的日期比本机早 / 晚一天，也写本机的当天', () => {
    const thisYear = new Date().getFullYear();
    expect(textOf(render(make({ starredAt: utcAtLocal(thisYear, 10, 3, 1, 30) })))).toContain('收藏于 10 月 3 日');
    expect(textOf(render(make({ starredAt: utcAtLocal(thisYear, 10, 3, 23, 30) })))).toContain('收藏于 10 月 3 日');
  });

  it('读不出日期就写破折号，不写「undefined」之类', () => {
    const text = textOf(render(make({ starredAt: '' })));
    expect(text).toContain('收藏于 —');
  });
});

describe('详情：标签', () => {
  it('类型和成熟度都带分组名；类型用长写法（这里有地方解释）', () => {
    const text = textOf(render(make({ form: 'cli', maturity: 'usable' })));
    expect(text).toContain('类型：命令行工具（可让 AI 助手帮你装）');
    expect(text).toContain('成熟度：可用');
  });

  it('可上生产换成可正式使用，stale 换成久未更新，不再露出英文', () => {
    expect(textOf(render(make({ maturity: 'production' })))).toContain('成熟度：可正式使用');
    const stale = textOf(render(make({ maturity: 'stale' })));
    expect(stale).toContain('成熟度：久未更新');
    expect(stale).not.toContain('stale');
    expect(textOf(render(make({ maturity: 'production' })))).not.toContain('可上生产');
  });

  it('整句话的成熟度不显示；类型是其他 / 没填也不画这个标签', () => {
    const text = textOf(render(make({ maturity: '开源项目，提供安装脚本和文档，需自行部署。', form: 'other' })));
    expect(text).not.toContain('成熟度：');
    expect(text).not.toContain('类型：');
  });

  it('场景多于 3 个时写「还有 N 个场景」，不写「+N」', () => {
    const scenarios = ['a', 'b', 'c', 'd', 'e'].map((id) => scenario(id, `场景${id}`));
    const text = textOf(render(make({ scenarioIds: ['a', 'b', 'c', 'd', 'e'] }), { scenarios }));
    expect(text).toContain('还有 2 个场景');
    expect(text).not.toContain('+2');
  });

  it('「项目最近更新」说明是项目作者更新的', () => {
    expect(textOf(render(make()))).toContain('项目最近更新：');
  });
});

describe('详情：维护状态', () => {
  const day = 86_400_000;
  const nowMs = Date.now();
  const iso = (daysAgo: number) => new Date(nowMs - daysAgo * day).toISOString();

  it('三档都画小标签和悬停说明；没有最近推送时间不画', () => {
    const active = render(make({ pushedAt: iso(1) }));
    expect(textOf(active)).toContain('活跃');
    expect(active).toContain('title="最近 3 个月内有更新"');
    expect(textOf(active)).toContain('项目最近更新：');

    const slow = render(make({ pushedAt: iso(200) }));
    expect(textOf(slow)).toContain('放缓');
    expect(slow).toContain('title="3 到 12 个月没有更新"');
    expect(textOf(slow)).not.toContain('疑似停更');

    const stale = render(make({ pushedAt: iso(400), maturity: null }));
    expect(textOf(stale)).toContain('疑似停更');
    expect(stale).toContain('title="超过 1 年没有更新，可能已经不维护了"');
    expect(textOf(stale)).not.toContain('放缓');

    const missing = render(make({ pushedAt: null }));
    const missingText = textOf(missing);
    expect(missingText).toContain('项目最近更新：');
    expect(missingText).not.toContain('活跃');
    expect(missingText).not.toContain('放缓');
    expect(missingText).not.toContain('疑似停更');
    expect(missing).not.toContain('最近 3 个月内有更新');
  });

  it('已归档有红色小标签，没有推送时间也画；没归档时三档照旧', () => {
    const archivedRecent = render(make({ archived: true, pushedAt: iso(1) }));
    expect(textOf(archivedRecent)).toContain('已归档');
    expect(archivedRecent).toContain('title="作者已在 GitHub 上把它归档：只读，不会再更新"');
    expect(archivedRecent).toContain('bg-error-container');
    expect(textOf(archivedRecent)).not.toContain('活跃');

    const archivedMissing = render(make({ archived: true, pushedAt: null }));
    expect(textOf(archivedMissing)).toContain('已归档');
    expect(textOf(archivedMissing)).toContain('项目最近更新：');
    expect(archivedMissing).toContain('title="作者已在 GitHub 上把它归档：只读，不会再更新"');

    const active = render(make({ archived: false, pushedAt: iso(1) }));
    expect(textOf(active)).toContain('活跃');
    expect(textOf(active)).not.toContain('已归档');

    const slow = render(make({ archived: false, pushedAt: iso(200) }));
    expect(textOf(slow)).toContain('放缓');
    expect(textOf(slow)).not.toContain('已归档');

    const stale = render(make({ archived: false, pushedAt: iso(400), maturity: null }));
    expect(textOf(stale)).toContain('疑似停更');
    expect(textOf(stale)).not.toContain('已归档');
    expect(textOf(stale)).not.toContain('放缓');
  });
});

describe('详情：中文说明和它的依据', () => {
  it('依据项目说明：第一次出现写成「项目说明（README）」', () => {
    const text = textOf(render(make()));
    expect(text).toContain('中文说明');
    expect(text).toContain('依据项目说明（README）');
    expect(text).not.toContain('AI 说明');
  });

  it('说明只依据 GitHub 简介时，提醒别当真，不说「依据项目说明」', () => {
    const text = textOf(render(make({ whatItIs: '只有 GitHub 简介，没有 README：一个做封面的工具。' })));
    expect(text).toContain('只依据 GitHub 简介，没读到项目说明，别当真');
    expect(text).not.toContain('依据项目说明（README）');
    expect(text).not.toContain('依据项目说明');
  });

  it('确认按钮明确防覆盖语义：未确认说明可点「确认（不再自动覆盖）」，不太准可请 AI 重写', () => {
    const html = render(make());
    const text = textOf(html);
    expect(text).toContain('这段说明准确吗？');
    expect(text).toContain('确认（不再自动覆盖）');
    expect(html).toContain('title="确认后，后续自动整理不会覆盖这条说明"');
    expect(text).toContain('不太准，让 AI 重写（会花一点钱）');
    expect(text).not.toContain('重新整理');
  });

  it('确认过的说明显示「已确认（不会被自动覆盖）」且不再询问是否准确', () => {
    const text = textOf(render(make({ cardReview: 'confirmed' })));
    expect(text).toContain('已确认（不会被自动覆盖）');
    expect(text).not.toContain('这段说明准确吗');
  });
});

describe('详情：重新整理这一张', () => {
  it('初始渲染没有确认条；没确认过的看到重写入口，确认过的看到重新整理', () => {
    const suggested = render(make());
    expect(suggested).not.toContain('data-redo-confirm');
    expect(textOf(suggested)).toContain('不太准，让 AI 重写（会花一点钱）');
    expect(textOf(suggested)).not.toContain('重新整理这一张');

    const confirmed = render(make({ cardReview: 'confirmed' }));
    expect(confirmed).not.toContain('data-redo-confirm');
    expect(textOf(confirmed)).toContain('已确认（不会被自动覆盖）');
    expect(textOf(confirmed)).toContain('重新整理这一张');
    expect(textOf(confirmed)).not.toContain('不太准，让 AI 重写');
  });

  it('源码里 props.onRegenerate 只出现在「确认，重新整理」的处理函数里', () => {
    expect(itemDetailSource.split('props.onRegenerate').length - 1).toBe(1);
    const callAt = itemDetailSource.indexOf('props.onRegenerate');
    const buttonStart = itemDetailSource.lastIndexOf('<Button', callAt);
    const buttonEnd = itemDetailSource.indexOf('</Button>', callAt);
    const button = itemDetailSource.slice(buttonStart, buttonEnd);
    expect(button).toContain('data-confirm-redo');
    expect(button).toContain('confirmLabel');
    expect(button).toContain('props.onRegenerate()');

    const entry = (label: string) => {
      const end = itemDetailSource.indexOf(`>${label}</Button>`);
      expect(end).toBeGreaterThan(-1);
      const start = itemDetailSource.lastIndexOf('<Button', end);
      return itemDetailSource.slice(start, end);
    };
    const rewrite = entry('不太准，让 AI 重写（会花一点钱）');
    const again = entry('重新整理这一张');
    expect(rewrite).not.toContain('onRegenerate');
    expect(again).not.toContain('onRegenerate');
    expect(rewrite).toContain('setAskingRedo(true)');
    expect(again).toContain('setAskingRedo(true)');
  });

  it('点确认先收起确认条，再调用重新整理', () => {
    const callAt = itemDetailSource.indexOf('props.onRegenerate()');
    const buttonStart = itemDetailSource.lastIndexOf('<Button', callAt);
    const buttonEnd = itemDetailSource.indexOf('</Button>', callAt);
    const button = itemDetailSource.slice(buttonStart, buttonEnd);
    const closeAt = button.indexOf('setAskingRedo(false)');
    const regenerateAt = button.indexOf('props.onRegenerate()');
    expect(closeAt).toBeGreaterThan(-1);
    expect(regenerateAt).toBeGreaterThan(-1);
    expect(closeAt).toBeLessThan(regenerateAt);
  });

  it('「先不了」只收起确认条，不调用重新整理', () => {
    const end = itemDetailSource.indexOf('{redoCopy.cancelLabel}</Button>');
    expect(end).toBeGreaterThan(-1);
    const start = itemDetailSource.lastIndexOf('<Button', end);
    const button = itemDetailSource.slice(start, end);
    expect(button).toContain('setAskingRedo(false)');
    expect(button).not.toContain('onRegenerate');
    expect(button).not.toContain('setAskingRedo(true)');
  });

  it('确认文案按有没有点过「对」分支', () => {
    expect(itemDetailSource).toContain("redoConfirmCopy(detail.cardReview === 'confirmed')");
    expect(itemDetailSource).not.toContain('redoConfirmCopy(false)');
    expect(itemDetailSource).not.toContain('redoConfirmCopy(true)');
  });
});

describe('详情：页签', () => {
  it('「安装」页签改叫「怎么试」，和概览里「看完整的怎么试」对得上', () => {
    const html = render(make());
    const labels = tabLabels(html);
    expect(labels).toContain('怎么试');
    expect(labels).not.toContain('安装');
    expect(textOf(html)).toContain('看完整的怎么试 →');
  });

  it('没有现成命令时，概览里的链接也叫「怎么试」页', () => {
    const text = textOf(render(make({ howToTry: null })));
    expect(text).toContain('去「怎么试」页，让 AI 助手帮你装 →');
    expect(text).not.toContain('去「安装」页');
  });

  it('「项目说明（README）」只在第一次出现的地方带括号：有说明时括号在「依据」那一句，页签不带', () => {
    const html = render(make());
    expect(tabLabels(html)).toEqual(['概览', '怎么试', '预览', '项目说明', '社区']);
    expect(textOf(html).split('项目说明（README）').length - 1).toBe(1);
    expect(textOf(html).indexOf('项目说明（README）')).toBeLessThan(textOf(html).indexOf('概览'));
  });

  it('还没有中文说明时，页签是第一次出现，由它带括号', () => {
    const html = render(make({ hasCard: false, whatItIs: null, oneLiner: null }));
    expect(tabLabels(html)).toEqual(['概览', '怎么试', '预览', '项目说明（README）', '社区']);
    expect(textOf(html).split('项目说明（README）').length - 1).toBe(1);
  });

  it('说明只依据简介时，那句整句不带括号，页签补上', () => {
    const html = render(make({ whatItIs: '只有 GitHub 简介，没有 README：一个做封面的工具。' }));
    expect(tabLabels(html)).toContain('项目说明（README）');
    expect(textOf(html).split('项目说明（README）').length - 1).toBe(1);
  });
});

describe('详情：概览里的几块', () => {
  it('「适合解决」和「什么时候用它」合成「适合谁 / 什么时候用」，重复的只列一次', () => {
    const text = textOf(render(make({ problemsSolved: ['每天要出十张封面', '要出十张图时'], useWhen: ['要出十张图时', '临时补一张'] })));
    expect(text).toContain('适合谁 / 什么时候用');
    expect(text).not.toContain('适合解决');
    expect(text).not.toContain('什么时候用它');
    expect(text.split('要出十张图时').length - 1).toBe(1);
    expect(text).toContain('临时补一张');
  });

  it('用之前注意为空：有中文说明时显示一行灰字，不让整块消失', () => {
    const html = render(make({ risks: [] }));
    expect(textOf(html)).toContain('用之前注意');
    expect(textOf(html)).toContain('项目说明里没提限制，不代表没有');
    // 灰字，不是列表项
    expect(html).toMatch(/<p class="[^"]*text-on-surface-variant[^"]*">项目说明里没提限制，不代表没有<\/p>/);
  });

  it('用之前注意有内容：列出来，不再显示那行灰字', () => {
    const text = textOf(render(make({ risks: ['要付费 API'] })));
    expect(text).toContain('要付费 API');
    expect(text).not.toContain('项目说明里没提限制');
  });

  it('还没有中文说明、也没有注意事项：不凭空画「用之前注意」', () => {
    const text = textOf(render(make({ hasCard: false, risks: [], whatItIs: null })));
    expect(text).not.toContain('用之前注意');
    expect(text).not.toContain('项目说明里没提限制');
  });
});

describe('详情：灵感', () => {
  it('灵感里的 Fox 显示成你', () => {
    const text = textOf(render(make({ inspirationForFox: '给 Fox 做小红书封面时可以直接用。' })));
    expect(text).toContain('灵感');
    expect(text).toContain('给你做小红书封面时可以直接用。');
    expect(text).not.toContain('Fox');
  });

  it('降成一小段灰字：不是带边框的卡片，放在「怎么试」下面', () => {
    const html = render(make({ inspirationForFox: '给 Fox 的灵感' }));
    const match = /<section class="([^"]*)" aria-label="灵感">(.*?)<\/section>/.exec(html);
    expect(match).not.toBeNull();
    const classes = (match?.[1] ?? '').split(/\s+/);
    expect(classes).not.toContain('border');
    expect(classes).not.toContain('rounded-lg');
    expect(classes).not.toContain('p-4');
    expect(match?.[2]).toMatch(/<h3 class="[^"]*text-xs[^"]*">灵感<\/h3>/);
    expect(match?.[2]).toMatch(/<p class="[^"]*text-xs[^"]*text-on-surface-variant[^"]*">给你的灵感<\/p>/);
    // 位置：在「怎么试」那一块后面
    expect(html.indexOf('>灵感</h3>')).toBeGreaterThan(html.indexOf('>怎么试</h3>'));
    // 灵感不再用带图标底的小标题（SectionTitle）
    expect(html).not.toContain('>lightbulb<');
  });

  it('内容为空整段不显示', () => {
    for (const value of [null, '', '   ']) {
      const text = textOf(render(make({ inspirationForFox: value })));
      expect(text, String(value)).not.toContain('灵感');
    }
  });
});

describe('详情：底部的复制按钮与加入方案', () => {
  it('写清粘出去会让 AI 助手装好并试用', () => {
    const text = textOf(render(make()));
    expect(text).toContain('复制「帮我装好并试用」给 AI 助手');
    expect(text).not.toContain('复制给 Codex');
  });

  it('传入 onAddToSolve 时展示「加入当前方案」按钮', () => {
    const html = renderToStaticMarkup(
      <ItemDetail
        detail={make()}
        onReadingChange={() => {}}
        onConfirm={() => {}}
        onRegenerate={() => {}}
        onCopyCodex={() => {}}
        onCopyHow={() => {}}
        onAddToSolve={() => {}}
      />,
    );
    expect(textOf(html)).toContain('加入当前方案');
  });

  it('类型是「合集」（资源清单）时也写「帮我装好并试用」：粘出去的话是同一套，按钮不能和它说的不一样', () => {
    const text = textOf(render(make({ form: 'collection' })));
    expect(text).toContain('复制「帮我装好并试用」给 AI 助手');
    expect(text).not.toContain('帮我看看怎么用');
  });

  it('其余类型都不变：技能、MCP、命令行、应用、代码库、接口、服务、其他、没填', () => {
    for (const form of ['skill', 'mcp', 'cli', 'app', 'library', 'api', 'service', 'other', '']) {
      const text = textOf(render(make({ form })));
      expect(text, form).toContain('复制「帮我装好并试用」给 AI 助手');
      expect(text, form).not.toContain('帮我看看怎么用');
    }
  });

  it('按钮仍是页面主按钮，点它交出去的还是同一个动作（只是字不一样）', () => {
    const html = render(make({ form: 'collection' }));
    expect(html).toMatch(/<button[^>]*>(?:(?!<\/button>).)*复制「帮我装好并试用」给 AI 助手<\/button>/);
  });
});

describe('详情：概览里「怎么试」没有现成命令时', () => {
  it('项目说明还在读取：只说正在读取，不先下结论说「没有现成的安装步骤」（读完可能就有）', () => {
    // 服务端渲染不跑 effect，项目说明就停在「读取中」
    const text = textOf(render(make({ howToTry: null })));
    expect(text).toContain('正在读取项目说明…');
    expect(text).not.toContain('项目说明里没有现成的安装步骤');
    expect(text).not.toContain('还没下载项目说明，暂时看不到安装步骤');
  });

  it('下面的链接仍在：让 AI 助手帮你装', () => {
    expect(textOf(render(make({ howToTry: null })))).toContain('去「怎么试」页，让 AI 助手帮你装 →');
  });

  it('接线：用 noInstallStepsNote(readme)，按项目说明读到哪一步说话', () => {
    expect(itemDetailSource).toContain('noInstallStepsNote(readme)');
    expect(itemDetailSource).not.toContain('项目说明里没有现成的安装步骤');
  });
});
