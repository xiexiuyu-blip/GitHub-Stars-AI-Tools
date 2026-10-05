import { describe, expect, it } from 'vitest';
import type { CommunityItem, RepoCommunity } from './contracts';
import { communityNote, groupCommunity, itemMeta, stateLabel } from './community-view';

function item(overrides: Partial<CommunityItem> = {}): CommunityItem {
  return {
    source: 'hn',
    title: 't',
    url: 'https://example.com',
    points: null,
    comments: null,
    createdAt: null,
    state: null,
    ...overrides,
  };
}

function community(overrides: Partial<RepoCommunity> = {}): RepoCommunity {
  return { repoId: 'r', fetchedAt: '2026-10-03T11:00:00Z', stale: false, items: [], errors: [], ...overrides };
}

describe('groupCommunity', () => {
  it('HN 在前、Issues 在后；空组不返回', () => {
    const groups = groupCommunity([item({ source: 'github_issue', comments: 3 }), item({ source: 'hn', points: 5 })]);
    expect(groups.map((group) => group.source)).toEqual(['hn', 'github_issue']);
    expect(groupCommunity([item({ source: 'hn', points: 1 })]).map((group) => group.source)).toEqual(['hn']);
    expect(groupCommunity([])).toEqual([]);
  });

  it('组内按热度：HN 看分数，Issues 看评论数；不改原数组', () => {
    const input = [
      item({ title: 'a', points: 10 }),
      item({ title: 'b', points: 300 }),
      item({ source: 'github_issue', title: 'c', comments: 2 }),
      item({ source: 'github_issue', title: 'd', comments: 40 }),
    ];
    const groups = groupCommunity(input);
    expect(groups[0].items.map((entry) => entry.title)).toEqual(['b', 'a']);
    expect(groups[1].items.map((entry) => entry.title)).toEqual(['d', 'c']);
    expect(input.map((entry) => entry.title)).toEqual(['a', 'b', 'c', 'd']);
  });
});

describe('itemMeta / stateLabel', () => {
  it('分数和评论都有、只有一个、都没有', () => {
    expect(itemMeta(item({ points: 312, comments: 87 }))).toBe('312 分 · 87 条评论');
    expect(itemMeta(item({ comments: 24 }))).toBe('24 条评论');
    expect(itemMeta(item({ points: 0 }))).toBe('0 分');
    expect(itemMeta(item())).toBe('');
  });

  it('只有 Issue 有状态标签', () => {
    expect(stateLabel(item({ source: 'github_issue', state: 'open' }))).toBe('还没解决');
    expect(stateLabel(item({ source: 'github_issue', state: 'closed' }))).toBe('已解决');
    expect(stateLabel(item({ source: 'github_issue', state: 'weird' }))).toBeNull();
    expect(stateLabel(item({ source: 'hn', state: 'open' }))).toBeNull();
  });
});

describe('communityNote', () => {
  const now = new Date('2026-10-03T12:00:00Z');

  it('还没读到：没有提示', () => {
    expect(communityNote(null, now)).toBeNull();
  });

  it('从没查过（没有 fetchedAt）：没有提示，由界面给「去查」按钮', () => {
    expect(communityNote(community({ fetchedAt: null, stale: true }), now)).toBeNull();
  });

  it('查过但没有讨论', () => {
    expect(communityNote(community(), now)).toEqual({ tone: 'info', text: '今天查过，没有找到公开的讨论。' });
  });

  it('有讨论：说多久以前更新；过期了提醒重新查', () => {
    const items = [item({ points: 1 })];
    expect(communityNote(community({ items }), now)?.text).toBe('今天更新。');
    expect(communityNote(community({ items, stale: true, fetchedAt: '2026-09-20T00:00:00Z' }), now)?.text).toContain('重新查');
  });

  it('某个来源失败：warn，说哪个没连上，有内容说「先显示拿到的部分」', () => {
    const note = communityNote(
      community({ items: [item({ points: 1 })], errors: [{ source: 'hn', message: 'timeout' }] }),
      now,
    );
    expect(note).toEqual({ tone: 'warn', text: 'Hacker News 这次没连上，先显示拿到的部分。' });
    const none = communityNote(
      community({ errors: [{ source: 'hn', message: 'a' }, { source: 'github_issue', message: 'b' }] }),
      now,
    );
    expect(none?.text).toBe('Hacker News、GitHub Issues 这次没连上，稍后再试。');
  });
});

describe('卡 32 社区状态词', () => {
  it('还开着改成还没解决，已关闭改成已解决', () => {
    expect(stateLabel(item({ source: 'github_issue', state: 'open' }))).toBe('还没解决');
    expect(stateLabel(item({ source: 'github_issue', state: 'closed' }))).toBe('已解决');
    expect(stateLabel(item({ source: 'github_issue', state: 'open' }))).not.toBe('还开着');
    expect(stateLabel(item({ source: 'github_issue', state: 'closed' }))).not.toBe('已关闭');
  });
});
