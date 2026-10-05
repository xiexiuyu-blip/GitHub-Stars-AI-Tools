import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { InboxItem, InboxPage } from '@/lib/contracts';
import type { InboxFilter } from '@/lib/capture-view';
import { InboxCard } from './inbox-card';

function textOf(html: string): string {
  return html
    .replace(/<span class="material-symbols-outlined[^>]*>[^<]*<\/span>/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

const COUNTS = { pending: 5, ready: 1, failed: 2, archived: 7 };

function item(id: string, status: InboxItem['status'] = 'pending'): InboxItem {
  return {
    id,
    title: `标题 ${id}`,
    originalUrl: `https://example.com/${id}`,
    normalizedUrl: `https://example.com/${id}`,
    itemType: 'web',
    platform: '网页',
    sourceContext: '微信群',
    userNote: '',
    status,
    repositoryId: null,
    createdAt: '2026-10-03T10:00:00Z',
  };
}

function render(options: {
  filter?: InboxFilter;
  page: InboxPage | null;
  phase?: 'loading' | 'ready' | 'error';
}): string {
  return renderToStaticMarkup(
    <InboxCard
      filter={options.filter ?? 'pending'}
      page={options.page}
      phase={options.phase ?? 'ready'}
      error={null}
      loadingMore={false}
      busyId={null}
      editingId={null}
      draft=""
      noteMessage={null}
      savingNote={false}
      fetching={false}
      collectingId={null}
      onCollectRepos={() => {}}
      onFilter={() => {}}
      onLoadMore={() => {}}
      onRetry={() => {}}
      onStartNote={() => {}}
      onDraft={() => {}}
      onSaveNote={() => {}}
      onCancelNote={() => {}}
      onArchive={() => {}}
      onRetryFetch={() => {}}
    />,
  );
}

/** 标题那一行（h3 和它后面紧跟着的小数字，如果有）。 */
function headingOf(html: string): string {
  const match = /<h3[^>]*>收集箱<\/h3>(<span[^>]*>[^<]*<\/span>)?/.exec(html);
  expect(match, '找不到「收集箱」标题').not.toBeNull();
  return match?.[0] ?? '';
}

describe('收集箱：标题不带数字', () => {
  it('标题就是「收集箱」，后面没有数字（那个数字只是当前页签里有几条，放在标题后面像是总数）', () => {
    const page: InboxPage = { items: [item('a'), item('b')], totalCount: 2, counts: COUNTS };
    const heading = headingOf(render({ page }));
    expect(heading).toBe('<h3 class="m-0 text-base font-semibold whitespace-nowrap text-foreground">收集箱</h3>');
    expect(textOf(heading)).toBe('收集箱');
  });

  it('读取中、读出错、没有数据时也一样不带数字', () => {
    expect(textOf(headingOf(render({ page: null, phase: 'loading' })))).toBe('收集箱');
    expect(textOf(headingOf(render({ page: null, phase: 'error' })))).toBe('收集箱');
    expect(textOf(headingOf(render({ page: { items: [], totalCount: 0, counts: COUNTS } })))).toBe('收集箱');
  });

  it('页签上的数字保留：待处理 5、失败 2、已归档 7', () => {
    const text = textOf(render({ page: { items: [item('a')], totalCount: 1, counts: COUNTS } }));
    expect(text).toContain('待处理 5');
    expect(text).toContain('失败 2');
    expect(text).toContain('已归档 7');
  });
});

describe('收集箱：空状态按页签说', () => {
  const empty: InboxPage = { items: [], totalCount: 0, counts: COUNTS };

  it('待处理页签：没有待处理的链接。', () => {
    const text = textOf(render({ filter: 'pending', page: empty }));
    expect(text).toContain('没有待处理的链接。');
    expect(text).toContain('上面粘贴后点「加入收集箱」，新链接会进来');
  });

  it('失败页签：没有失败的链接。', () => {
    expect(textOf(render({ filter: 'failed', page: empty }))).toContain('没有失败的链接。');
  });

  it('已归档页签：还没有归档的链接。', () => {
    expect(textOf(render({ filter: 'archived', page: empty }))).toContain('还没有归档的链接。');
  });

  it('不再有「这里没有内容」', () => {
    for (const filter of ['pending', 'failed', 'archived'] as const) {
      expect(textOf(render({ filter, page: empty })), filter).not.toContain('这里没有内容');
    }
  });

  it('有内容时不显示空状态', () => {
    const text = textOf(render({ filter: 'pending', page: { items: [item('a')], totalCount: 1, counts: COUNTS } }));
    expect(text).not.toContain('没有待处理的链接');
    expect(text).toContain('标题 a');
  });
});
