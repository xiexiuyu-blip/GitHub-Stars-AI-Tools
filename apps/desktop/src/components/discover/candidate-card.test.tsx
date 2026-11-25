import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { DiscoverCardItem } from '@/lib/contracts';
import type { CandidateStatus } from '@/lib/discover-view';
import { CandidateCard } from './candidate-card';

function textOf(html: string): string {
  return html
    .replace(/<span class="material-symbols-outlined[^>]*>[^<]*<\/span>/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

const CANDIDATE: DiscoverCardItem = {
  fullName: 'foo/bar',
  htmlUrl: 'https://github.com/foo/bar',
  description: 'A tool',
  language: 'Python',
  starsCount: 100,
  pushedAt: '2026-10-01T00:00:00Z',
  topics: ['cli'],
  mine: false,
  inInbox: false,
  fetchedAt: '2026-10-03T00:00:00Z',
};

function render(status: CandidateStatus, overrides: Partial<DiscoverCardItem> = {}): string {
  return renderToStaticMarkup(
    <CandidateCard
      candidate={{ ...CANDIDATE, ...overrides }}
      status={status}
      now={new Date('2026-10-05T00:00:00Z')}
      onAdd={() => {}}
      onCopyEvaluate={() => {}}
      onOpenInbox={() => {}}
    />,
  );
}

describe('发现页候选卡', () => {
  it('还没放进收集箱的：按钮写「先放进收集箱」，不写「收进收集箱」', () => {
    const text = textOf(render('new'));
    expect(text).toContain('先放进收集箱');
    expect(text).not.toContain('收进收集箱');
  });

  it('复制按钮写明只是评估：复制「帮我评估」给 AI 助手', () => {
    const text = textOf(render('new'));
    expect(text).toContain('复制「帮我评估」给 AI 助手');
    expect(text).not.toContain('复制给 Codex 评估');
    expect(text).not.toContain('帮我装好并试用');
  });

  it('已经收藏的：写「你已经收藏了」，不写「Stars」，也不再给放进收集箱的按钮', () => {
    const text = textOf(render('mine'));
    expect(text).toContain('你已经收藏了');
    expect(text).not.toContain('Stars');
    expect(text).not.toContain('先放进收集箱');
  });

  it('已在收集箱的：写「已在收集箱」', () => {
    const text = textOf(render('inbox'));
    expect(text).toContain('已在收集箱');
    expect(text).not.toContain('先放进收集箱');
  });

  it('最近更新说明是项目作者更新的：「项目最近更新：」', () => {
    expect(textOf(render('new'))).toContain('项目最近更新：4 天前');
  });

  it('趋势榜展示排名、热度增量、来源，HN 条目不显示 0 星标', () => {
    const trending = textOf(
      render('new', {
        rank: 1,
        deltaLabel: '+1,826 今日',
        sourceNote: 'GitHub 趋势榜',
      }),
    );
    expect(trending).toContain('#1');
    expect(trending).toContain('+1,826 今日');
    expect(trending).toContain('GitHub 趋势榜');

    const hn = textOf(
      render('new', {
        rank: 2,
        starsCount: 0,
        deltaLabel: '240 HN 赞',
        sourceNote: 'Show HN · 60 条讨论',
        pushedAt: null,
        createdAt: '2026-10-02T00:00:00Z',
      }),
    );
    expect(hn).toContain('#2');
    expect(hn).toContain('240 HN 赞');
    expect(hn).toContain('Show HN · 60 条讨论');
    expect(hn).toContain('新建于：3 天前');
  });
});
