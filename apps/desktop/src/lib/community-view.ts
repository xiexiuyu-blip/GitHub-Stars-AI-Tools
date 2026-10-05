/** 「社区」标签页里讨论摘要的纯视图规则：分组、排序、文案。不碰 React 和网络，方便单测。 */

import type { CommunityItem, RepoCommunity } from '@/lib/contracts';
import { relativeDays } from '@/lib/item-format';

export type CommunitySource = CommunityItem['source'];

export const SOURCE_LABEL: Record<CommunitySource, string> = {
  hn: 'Hacker News',
  github_issue: 'GitHub Issues',
};

export type CommunityGroup = { source: CommunitySource; label: string; items: CommunityItem[] };

/** 按来源分组（HN 在前、Issues 在后），组内「讨论最热」的在前：HN 看分数，Issues 看评论数。空的组不返回。 */
export function groupCommunity(items: readonly CommunityItem[]): CommunityGroup[] {
  const heat = (item: CommunityItem) => (item.source === 'hn' ? (item.points ?? 0) : (item.comments ?? 0));
  const order: CommunitySource[] = ['hn', 'github_issue'];
  return order
    .map((source) => ({
      source,
      label: SOURCE_LABEL[source],
      items: items.filter((item) => item.source === source).sort((a, b) => heat(b) - heat(a) || (b.comments ?? 0) - (a.comments ?? 0)),
    }))
    .filter((group) => group.items.length > 0);
}

/** 一条讨论右边的数字：「312 分 · 87 条评论」「24 条评论」。没有就返回空串。 */
export function itemMeta(item: CommunityItem): string {
  const parts: string[] = [];
  if (item.points != null) parts.push(`${item.points} 分`);
  if (item.comments != null) parts.push(`${item.comments} 条评论`);
  return parts.join(' · ');
}

/** issue 的状态标签：open → 「还没解决」、closed → 「已解决」，别的（HN 或不认识的）不显示。 */
export function stateLabel(item: CommunityItem): string | null {
  if (item.source !== 'github_issue') return null;
  if (item.state === 'open') return '还没解决';
  if (item.state === 'closed') return '已解决';
  return null;
}

export type CommunityNote = { tone: 'info' | 'warn'; text: string };

/** 面板顶上的一句话：上次什么时候更新的、缓存过期没、哪个来源这次没连上。 */
export function communityNote(community: RepoCommunity | null, now: Date | number | string): CommunityNote | null {
  if (!community) return null;
  const failed = community.errors.map((error) => SOURCE_LABEL[error.source]);
  if (failed.length > 0) {
    const uniq = [...new Set(failed)];
    const suffix = community.items.length > 0 ? '，先显示拿到的部分。' : '，稍后再试。';
    return { tone: 'warn', text: `${uniq.join('、')} 这次没连上${suffix}` };
  }
  if (!community.fetchedAt) return null;
  const when = relativeDays(community.fetchedAt, now);
  if (community.items.length === 0) return { tone: 'info', text: `${when}查过，没有找到公开的讨论。` };
  return { tone: 'info', text: `${when}更新${community.stale ? '，可能有新的了，点「重新查」。' : '。'}` };
}
