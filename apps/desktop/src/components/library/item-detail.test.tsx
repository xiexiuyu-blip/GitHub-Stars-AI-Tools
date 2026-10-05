import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { LibraryDetail, ScenarioSummary } from '@/lib/contracts';
import { ItemDetail } from './item-detail';
// 源码原文：守住「说什么由 item-format 决定」（Vite 的 ?raw，不用 node 的类型）
import itemDetailSource from './item-detail.tsx?raw';

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
    expect(text).toContain('类型：命令行工具（要让 Codex 帮你装）');
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

  it('「说得对吗」：说清是哪段话、不对会请 AI 重写、会花一点钱', () => {
    const text = textOf(render(make()));
    expect(text).toContain('上面这段说明说得对吗？');
    expect(text).toContain('不对，请 AI 重写（会花一点钱）');
    expect(text).not.toContain('重新整理');
  });

  it('确认过的说明不再问对不对', () => {
    const text = textOf(render(make({ cardReview: 'confirmed' })));
    expect(text).toContain('你确认过了');
    expect(text).not.toContain('说得对吗');
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
    expect(text).toContain('去「怎么试」页，让 Codex 帮你装 →');
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

describe('详情：底部的复制按钮', () => {
  it('写清粘出去会让 Codex 装好并试用', () => {
    const text = textOf(render(make()));
    expect(text).toContain('复制「帮我装好并试用」给 Codex');
    expect(text).not.toContain('复制给 Codex');
  });

  it('类型是「合集」（资源清单）时也写「帮我装好并试用」：粘出去的话是同一套，按钮不能和它说的不一样', () => {
    const text = textOf(render(make({ form: 'collection' })));
    expect(text).toContain('复制「帮我装好并试用」给 Codex');
    expect(text).not.toContain('帮我看看怎么用');
  });

  it('其余类型都不变：技能、MCP、命令行、应用、代码库、接口、服务、其他、没填', () => {
    for (const form of ['skill', 'mcp', 'cli', 'app', 'library', 'api', 'service', 'other', '']) {
      const text = textOf(render(make({ form })));
      expect(text, form).toContain('复制「帮我装好并试用」给 Codex');
      expect(text, form).not.toContain('帮我看看怎么用');
    }
  });

  it('按钮仍是页面主按钮，点它交出去的还是同一个动作（只是字不一样）', () => {
    const html = render(make({ form: 'collection' }));
    expect(html).toMatch(/<button[^>]*>(?:(?!<\/button>).)*复制「帮我装好并试用」给 Codex<\/button>/);
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

  it('下面的链接仍在：让 Codex 帮你装', () => {
    expect(textOf(render(make({ howToTry: null })))).toContain('去「怎么试」页，让 Codex 帮你装 →');
  });

  it('接线：用 noInstallStepsNote(readme)，按项目说明读到哪一步说话', () => {
    expect(itemDetailSource).toContain('noInstallStepsNote(readme)');
    expect(itemDetailSource).not.toContain('项目说明里没有现成的安装步骤');
  });
});
