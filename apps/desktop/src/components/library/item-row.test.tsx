import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { LibraryItem } from '@/lib/contracts';
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
