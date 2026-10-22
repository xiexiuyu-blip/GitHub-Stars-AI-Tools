import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { LibraryItem, ScenarioSummary } from '@/lib/contracts';
import { DROP_HINT_LIBRARY } from '@/lib/item-format';
// 页面源码原文：守住「资料库传自己的说明、今日页保持原句」（Vite 的 ?raw，不用 node 的类型）
import librarySource from '../../pages/v2/library.tsx?raw';
import todaySource from '../../pages/v2/today.tsx?raw';
import { ItemRow } from './item-row';
import { PickCard } from './pick-card';

const ITEM: LibraryItem = {
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
};

const DROP_TITLE = '标成「不要了」：只是今日里不再出现，不会取消 GitHub 收藏，可在资料库「我的状态 → 不要了」里找回';

/** 悬停按钮：名字（aria-label）和悬停提示（title）。 */
function hoverButtons(html: string): { title: string; label: string }[] {
  return [...html.matchAll(/<button[^>]*title="([^"]*)"[^>]*aria-label="([^"]*)"/g)].map((match) => ({
    title: match[1],
    label: match[2],
  }));
}

describe('列表行上的悬停按钮', () => {
  const html = renderToStaticMarkup(
    <ItemRow item={ITEM} onSelect={() => {}} onWant={() => {}} onDrop={() => {}} />,
  );

  it('「不要了」悬停时说明后果：只是今日里不再出现，不会取消 GitHub 收藏，在哪里找回', () => {
    const drop = hoverButtons(html).find((button) => button.label.startsWith('不要了'));
    expect(drop?.title).toBe(DROP_TITLE);
    expect(drop?.title).toContain('不会取消 GitHub 收藏');
    expect(drop?.title).toContain('我的状态 → 不要了');
  });

  it('按钮的名字（读屏用）是简短的「不要了（X）」，带快捷键，不是整句长文案', () => {
    const drop = hoverButtons(html).find((button) => button.label.startsWith('不要了'));
    expect(drop?.label).toBe('不要了（X）');
  });

  it('「想试」没有长说明：悬停和名字都是「想试（T）」', () => {
    const want = hoverButtons(html).find((button) => button.label.startsWith('想试'));
    expect(want).toEqual({ title: '想试（T）', label: '想试（T）' });
  });

  it('两个按钮，没有多出别的', () => {
    expect(hoverButtons(html).map((button) => button.label)).toEqual(['想试（T）', '不要了（X）']);
  });
});

const LIBRARY_DROP_TITLE = '标成「不要了」：不会取消 GitHub 上的收藏，可以在「我的状态 → 不要了」里找回。';

describe('资料库里的列表行：「不要了」的悬停说明不是今日页那一句', () => {
  const html = renderToStaticMarkup(
    <ItemRow item={ITEM} dropHint={DROP_HINT_LIBRARY} onSelect={() => {}} onWant={() => {}} onDrop={() => {}} />,
  );

  it('说不会取消 GitHub 上的收藏、在「我的状态 → 不要了」里找回；名字仍是「不要了（X）」', () => {
    const drop = hoverButtons(html).find((button) => button.label.startsWith('不要了'));
    expect(drop).toEqual({ title: LIBRARY_DROP_TITLE, label: '不要了（X）' });
    expect(DROP_HINT_LIBRARY).toBe(LIBRARY_DROP_TITLE);
  });

  it('没有「只是今日里不再出现」——那是今日页的话，在资料库里说就不对了', () => {
    const drop = hoverButtons(html).find((button) => button.label.startsWith('不要了'));
    expect(drop?.title).not.toContain('今日');
    expect(drop?.title).not.toContain('只是');
  });

  it('不传说明的行（今日页的「最近 7 天新收藏」列表用它）仍是今日页的原句', () => {
    const plain = renderToStaticMarkup(<ItemRow item={ITEM} onSelect={() => {}} onWant={() => {}} onDrop={() => {}} />);
    expect(hoverButtons(plain).find((button) => button.label.startsWith('不要了'))?.title).toBe(DROP_TITLE);
  });

  it('资料库页给列表行传了资料库那一句，今日页没有传', () => {
    expect(librarySource).toContain('dropHint={DROP_HINT_LIBRARY}');
    expect(todaySource).not.toContain('DROP_HINT_LIBRARY');
    expect(todaySource).not.toContain('dropHint=');
  });
});

describe('列表行：名字被省略时，悬停能看到全名（agency-agents-zh 和 agency-agents 都会被截成同一个前缀）', () => {
  function rowFor(fullName: string): string {
    return renderToStaticMarkup(
      <ItemRow item={{ ...ITEM, fullName }} onSelect={() => {}} onWant={() => {}} onDrop={() => {}} />,
    );
  }

  it('名字那一段带 title，内容是「所有者/名字」全名，页面上显示的仍是名字', () => {
    const html = rowFor('msitarzewski/agency-agents-zh');
    expect(html).toContain('title="msitarzewski/agency-agents-zh">agency-agents-zh</b>');
    expect(html).toContain('truncate');
  });

  it('两个前缀相同的名字，悬停的全名不一样', () => {
    const zh = /title="([^"]*)">agency-agents-zh<\/b>/.exec(rowFor('a/agency-agents-zh'))?.[1];
    const en = /title="([^"]*)">agency-agents<\/b>/.exec(rowFor('a/agency-agents'))?.[1];
    expect(zh).toBe('a/agency-agents-zh');
    expect(en).toBe('a/agency-agents');
    expect(zh).not.toBe(en);
  });
});

describe('列表行：维护状态只标放缓和疑似停更', () => {
  const NOW = '2026-10-22T12:00:00Z';
  const scenario: ScenarioSummary = {
    id: 'scn-xhs',
    slug: 'xhs',
    nameZh: '小红书',
    sortOrder: 1,
    count: 3,
    thin: false,
  };

  function row(pushedAt: string | null, now: string = NOW): string {
    const item: LibraryItem = {
      ...ITEM,
      pushedAt,
      starsCount: 42,
      scenarioIds: ['scn-xhs'],
      readingGroup: 'want',
      readingStatus: 'want_to_try',
    };
    return renderToStaticMarkup(
      <ItemRow item={item} now={now} scenarios={[scenario]} onSelect={() => {}} onWant={() => {}} onDrop={() => {}} />,
    );
  }

  function text(html: string): string {
    return html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
  }

  function keepsTheRow(html: string) {
    const visible = text(html);
    expect(visible).toContain('tool');
    expect(visible).toContain('做封面');
    expect(visible).toContain('小红书');
    expect(visible).toContain('42');
    expect(visible).toContain('想试');
  }

  it('放缓有灰色标签，疑似停更有红色标签；活跃、没有时间和坏日期都不画', () => {
    const slow = row('2026-04-01T12:00:00Z');
    expect(text(slow)).toContain('放缓');
    expect(slow).toContain('title="3 到 12 个月没有更新"');
    expect(text(slow)).not.toContain('疑似停更');
    expect(text(slow)).not.toContain('活跃');
    keepsTheRow(slow);

    const stale = row('2024-01-01T12:00:00Z');
    expect(text(stale)).toContain('疑似停更');
    expect(stale).toContain('title="超过 1 年没有更新，可能已经不维护了"');
    expect(text(stale)).not.toContain('放缓');
    keepsTheRow(stale);

    for (const pushedAt of ['2026-10-01T12:00:00Z', null, 'not-a-date']) {
      const html = row(pushedAt);
      const visible = text(html);
      expect(visible, String(pushedAt)).not.toContain('活跃');
      expect(visible, String(pushedAt)).not.toContain('放缓');
      expect(visible, String(pushedAt)).not.toContain('疑似停更');
      expect(html, String(pushedAt)).not.toContain('最近 3 个月内有更新');
      keepsTheRow(html);
    }
  });

  it('用传进来的「现在」算，不看电脑的真实时钟', () => {
    const past = '2025-06-01T12:00:00Z';
    const recent = row('2025-05-20T12:00:00Z', past);
    const visibleRecent = text(recent);
    expect(visibleRecent).not.toContain('活跃');
    expect(visibleRecent).not.toContain('放缓');
    expect(visibleRecent).not.toContain('疑似停更');
    expect(recent).not.toContain('最近 3 个月内有更新');

    const slow = row('2025-02-01T12:00:00Z', past);
    expect(text(slow)).toContain('放缓');
    expect(text(slow)).not.toContain('疑似停更');
    expect(slow).toContain('title="3 到 12 个月没有更新"');
  });

  it('已归档压过推送时间，没归档或没有这个字段时和以前一样，行里原有内容还在', () => {
    function marked(pushedAt: string | null, archived?: boolean): string {
      const item: LibraryItem = {
        ...ITEM,
        pushedAt,
        archived,
        starsCount: 42,
        scenarioIds: ['scn-xhs'],
        readingGroup: 'want',
        readingStatus: 'want_to_try',
        matchReasons: ['项目名'],
        matchStars: 2,
      };
      return renderToStaticMarkup(
        <ItemRow item={item} now={NOW} scenarios={[scenario]} onSelect={() => {}} onWant={() => {}} onDrop={() => {}} />,
      );
    }

    function keepsTheRow(html: string) {
      const visible = text(html);
      expect(visible).toContain('tool');
      expect(visible).toContain('做封面');
      expect(visible).toContain('小红书');
      expect(visible).toContain('42');
      expect(visible).toContain('想试');
      expect(visible).toContain('命中：项目名');
    }

    for (const pushedAt of ['2026-10-21T12:00:00Z', null]) {
      const html = marked(pushedAt, true);
      const visible = text(html);
      expect(visible, String(pushedAt)).toContain('已归档');
      expect(html, String(pushedAt)).toContain('title="作者已在 GitHub 上把它归档：只读，不会再更新"');
      expect(visible, String(pushedAt)).not.toContain('活跃');
      expect(visible, String(pushedAt)).not.toContain('放缓');
      expect(visible, String(pushedAt)).not.toContain('疑似停更');
      keepsTheRow(html);
    }

    const active = marked('2026-10-21T12:00:00Z', false);
    expect(text(active)).not.toContain('已归档');
    expect(text(active)).not.toContain('活跃');
    expect(text(active)).not.toContain('放缓');
    expect(text(active)).not.toContain('疑似停更');
    keepsTheRow(active);

    const slow = marked('2026-04-01T12:00:00Z', false);
    expect(text(slow)).toContain('放缓');
    expect(text(slow)).not.toContain('已归档');
    expect(text(slow)).not.toContain('疑似停更');
    keepsTheRow(slow);

    const stale = marked('2024-01-01T12:00:00Z');
    expect(text(stale)).toContain('疑似停更');
    expect(text(stale)).not.toContain('已归档');
    expect(text(stale)).not.toContain('放缓');
    keepsTheRow(stale);

    const missing = marked(null);
    expect(text(missing)).not.toContain('已归档');
    expect(text(missing)).not.toContain('活跃');
    expect(text(missing)).not.toContain('放缓');
    expect(text(missing)).not.toContain('疑似停更');
    keepsTheRow(missing);
  });
});

describe('「今天值得先看」卡片上的悬停按钮', () => {
  const html = renderToStaticMarkup(
    <PickCard item={ITEM} onSelect={() => {}} onWant={() => {}} onDrop={() => {}} />,
  );

  it('「不要了」悬停时说明后果，名字仍是「不要了（X）」', () => {
    const drop = hoverButtons(html).find((button) => button.label.startsWith('不要了'));
    expect(drop).toEqual({ title: DROP_TITLE, label: '不要了（X）' });
  });

  it('「想试」名字和悬停都是「想试（T）」', () => {
    const want = hoverButtons(html).find((button) => button.label.startsWith('想试'));
    expect(want).toEqual({ title: '想试（T）', label: '想试（T）' });
  });

  it('推荐理由说「场景」，不说「方向」', () => {
    const withReason = renderToStaticMarkup(
      <PickCard item={ITEM} reason="它被归在你关心的「小红书」场景，你还没看过" onSelect={() => {}} onWant={() => {}} onDrop={() => {}} />,
    );
    expect(withReason).toContain('它被归在你关心的「小红书」场景，你还没看过');
    expect(withReason).not.toContain('方向');
  });
});

/**
 * 测试跑在 Node 里，没有 DOM。这里把静态 HTML 收成一棵元素树，按文档顺序实现
 * `compareDocumentPosition`（跟随 4、在前 2、包含对方 16、被对方包含 8），只为核对行内顺序。
 */
const POSITION_PRECEDING = 2;
const POSITION_FOLLOWING = 4;
const POSITION_CONTAINS = 8;
const POSITION_CONTAINED_BY = 16;

type DomEl = {
  tag: string;
  attrs: Record<string, string>;
  text: string;
  children: DomEl[];
  order: number;
  end: number;
  compareDocumentPosition: (other: DomEl) => number;
  textContent: () => string;
};

const VOID_TAGS = new Set(['area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'source', 'track', 'wbr']);

function decodeEntities(value: string): string {
  return value
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&');
}

function parseFragment(html: string): DomEl {
  const root = makeEl('#root');
  const stack: DomEl[] = [root];
  let index = 0;
  while (index < html.length) {
    if (html[index] !== '<') {
      const next = html.indexOf('<', index);
      const raw = html.slice(index, next === -1 ? html.length : next);
      stack[stack.length - 1].text += decodeEntities(raw);
      index = next === -1 ? html.length : next;
      continue;
    }
    if (html.startsWith('<!--', index)) {
      const end = html.indexOf('-->', index + 4);
      index = end === -1 ? html.length : end + 3;
      continue;
    }
    const end = tagEnd(html, index);
    if (end < 0) break;
    const source = html.slice(index + 1, end);
    index = end + 1;
    const closing = source.startsWith('/');
    const body = closing ? source.slice(1) : source;
    const name = tagName(body);
    if (!name) continue;
    if (closing) {
      while (stack.length > 1 && stack[stack.length - 1].tag !== name) stack.pop();
      if (stack.length > 1 && stack[stack.length - 1].tag === name) stack.pop();
      continue;
    }
    const element = makeEl(name);
    element.attrs = tagAttrs(body);
    const selfClosing = /\/\s*$/.test(source) || VOID_TAGS.has(name);
    stack[stack.length - 1].children.push(element);
    if (!selfClosing) stack.push(element);
  }
  assignOrder(root, { n: 0 });
  return root;
}

function makeEl(tag: string): DomEl {
  const element: DomEl = {
    tag,
    attrs: {},
    text: '',
    children: [],
    order: 0,
    end: 0,
    compareDocumentPosition(other) {
      if (other === element) return 0;
      if (element.order < other.order && other.end <= element.end) return POSITION_CONTAINED_BY | POSITION_FOLLOWING;
      if (other.order < element.order && element.end <= other.end) return POSITION_CONTAINS | POSITION_PRECEDING;
      if (element.order < other.order) return POSITION_FOLLOWING;
      return POSITION_PRECEDING;
    },
    textContent() {
      return element.text + element.children.map((child) => child.textContent()).join('');
    },
  };
  return element;
}

function assignOrder(node: DomEl, state: { n: number }) {
  node.order = state.n;
  state.n += 1;
  for (const child of node.children) assignOrder(child, state);
  node.end = state.n;
}

function tagEnd(html: string, start: number): number {
  let quote: '"' | "'" | null = null;
  for (let index = start + 1; index < html.length; index += 1) {
    const ch = html[index];
    if (quote) {
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      continue;
    }
    if (ch === '>') return index;
  }
  return -1;
}

function tagName(body: string): string {
  const match = /^([A-Za-z][\w:-]*)/.exec(body.trim());
  return match ? match[1].toLowerCase() : '';
}

function tagAttrs(body: string): Record<string, string> {
  const attrs: Record<string, string> = {};
  const name = tagName(body);
  let rest = body.trim().slice(name.length);
  while (rest.length > 0) {
    rest = rest.trimStart();
    if (!rest || rest.startsWith('/')) break;
    const nameMatch = /^([^\s=/>]+)/.exec(rest);
    if (!nameMatch) break;
    const key = nameMatch[1];
    rest = rest.slice(key.length);
    if (!rest.startsWith('=')) {
      attrs[key] = '';
      continue;
    }
    rest = rest.slice(1);
    if (rest.startsWith('"') || rest.startsWith("'")) {
      const quote = rest[0];
      const close = rest.indexOf(quote, 1);
      attrs[key] = decodeEntities(close < 0 ? rest.slice(1) : rest.slice(1, close));
      rest = close < 0 ? '' : rest.slice(close + 1);
      continue;
    }
    const bare = /^[^\s>]+/.exec(rest);
    attrs[key] = decodeEntities(bare?.[0] ?? '');
    rest = rest.slice(bare?.[0].length ?? 0);
  }
  return attrs;
}

function findEl(node: DomEl, pred: (el: DomEl) => boolean): DomEl | null {
  if (node.tag !== '#root' && pred(node)) return node;
  for (const child of node.children) {
    const found = findEl(child, pred);
    if (found) return found;
  }
  return null;
}

describe('列表行：搜索时在中文说明和语言之间显示命中原因', () => {
  const scenario: ScenarioSummary = {
    id: 'scn-xhs',
    slug: 'xhs',
    nameZh: '小红书',
    sortOrder: 1,
    count: 1,
    thin: false,
  };

  it('命中文字和匹配度竖条，排在中文说明之后、语言 / 场景那一行之前', () => {
    const probe = parseFragment('<div class="line-clamp-2">做封面</div><span class="truncate">命中：项目名</span><div class="flex-wrap">TypeScript</div>');
    const probeSummary = findEl(probe, (el) => el.attrs.class?.includes('line-clamp-2') === true);
    const probeHint = findEl(probe, (el) => el.textContent().includes('命中：'));
    const probeMeta = findEl(probe, (el) => el.attrs.class?.includes('flex-wrap') === true);
    expect(probeSummary?.compareDocumentPosition(probeHint!)).toBe(POSITION_FOLLOWING);
    expect(probeHint?.compareDocumentPosition(probeMeta!)).toBe(POSITION_FOLLOWING);

    const html = renderToStaticMarkup(
      <ItemRow
        item={{ ...ITEM, matchReasons: ['项目名', '中文说明'], matchStars: 3, scenarioIds: ['scn-xhs'] }}
        scenarios={[scenario]}
        onSelect={() => {}}
        onWant={() => {}}
        onDrop={() => {}}
      />,
    );
    expect(html).toContain('命中：项目名 / 中文说明');
    expect(html).toContain('role="img"');
    expect(html).toContain('aria-label="匹配度 3 / 4"');
    const root = parseFragment(html);
    const summary = findEl(root, (el) => el.attrs.class?.includes('line-clamp-2') === true && el.textContent().includes('做封面'));
    const hint = findEl(root, (el) => el.attrs.class?.includes('truncate') === true && el.textContent().includes('命中：'));
    const meta = findEl(root, (el) => el.attrs.class?.includes('flex-wrap') === true && el.textContent().includes('TypeScript'));
    expect(summary, html).not.toBeNull();
    expect(hint, html).not.toBeNull();
    expect(meta, html).not.toBeNull();
    expect(summary!.compareDocumentPosition(hint!)).toBe(POSITION_FOLLOWING);
    expect(hint!.compareDocumentPosition(meta!)).toBe(POSITION_FOLLOWING);
  });

  it('不带命中字段，或原因是空数组、档位是空的，行里什么都不加', () => {
    const plain = renderToStaticMarkup(<ItemRow item={ITEM} onSelect={() => {}} onWant={() => {}} onDrop={() => {}} />);
    expect(plain).not.toContain('命中：');
    expect(plain).not.toContain('role="img"');
    expect(plain).toContain('做封面');

    const empty = renderToStaticMarkup(
      <ItemRow item={{ ...ITEM, matchReasons: [], matchStars: null }} onSelect={() => {}} onWant={() => {}} onDrop={() => {}} />,
    );
    expect(empty).not.toContain('命中：');
    expect(empty).not.toContain('role="img"');
    expect(empty).toContain('做封面');
  });
});
