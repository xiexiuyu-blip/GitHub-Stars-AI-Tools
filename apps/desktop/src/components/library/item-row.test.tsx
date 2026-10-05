import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { LibraryItem } from '@/lib/contracts';
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
