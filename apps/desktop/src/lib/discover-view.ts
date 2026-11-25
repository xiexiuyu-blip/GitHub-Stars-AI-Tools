/** 「发现」页的纯视图规则：场景排序、提示文案、候选状态、给 Codex 的评估请求。不碰 React 和网络，方便单测。 */

import type {
  DiscoverCandidate,
  DiscoverFeedId,
  DiscoverFeedRefreshReport,
  DiscoverFeedView,
  DiscoverRefreshReport,
  DiscoverScenario,
  DiscoverSearchSort,
  DiscoverSearchTimeRange,
} from '@/lib/contracts';
import { THIN_TAG, THIN_TAG_HINT, relativeDays } from '@/lib/item-format';

/** 偏少的排前面；同样偏少或同样不偏少，保持场景自己的顺序。 */
export function sortScenarios(list: readonly DiscoverScenario[]): DiscoverScenario[] {
  return [...list].sort((a, b) => Number(b.thin) - Number(a.thin) || a.sortOrder - b.sortOrder);
}

/** 右边标题下面那句：你在这个场景收了几个、偏不偏少、上次什么时候去找过。 */
export function scenarioLead(scenario: DiscoverScenario | null | undefined, now: Date | number | string): string {
  if (!scenario) return '';
  const mine = `你在这个场景收藏了 ${scenario.myCount} 个${scenario.thin ? `，${THIN_TAG}（${THIN_TAG_HINT}）` : ''}。`;
  const fetched = scenario.lastFetchedAt
    ? `上次去 GitHub 找是${relativeDays(scenario.lastFetchedAt, now)}。`
    : '还没去 GitHub 找过。';
  return `${mine}${fetched}`;
}

export type CandidateStatus = 'mine' | 'inbox' | 'new';

/** 候选现在处于哪种状态：已收藏 / 已在收集箱（含这次刚放进去的）/ 还没放。 */
export function candidateStatus(candidate: DiscoverCandidate, addedNow: ReadonlySet<string>): CandidateStatus {
  if (candidate.mine) return 'mine';
  if (candidate.inInbox || addedNow.has(candidate.fullName)) return 'inbox';
  return 'new';
}

const STATUS_RANK: Record<CandidateStatus, number> = { new: 0, inbox: 1, mine: 2 };

/**
 * 还没放进收集箱的排前面，已在收集箱的其次，已经收藏的沉到最后：「发现」是用来找没见过的，
 * 头两张就是「你已经收藏了」会让人以为这页没东西。同一档里保持后台给的顺序。
 */
export function sortCandidates<T extends DiscoverCandidate>(
  candidates: readonly T[],
  addedNow: ReadonlySet<string>,
): T[] {
  return candidates
    .map((candidate, index) => ({ candidate, index, rank: STATUS_RANK[candidateStatus(candidate, addedNow)] }))
    .sort((left, right) => left.rank - right.rank || left.index - right.index)
    .map((entry) => entry.candidate);
}

export type RefreshNotice = { tone: 'info' | 'warn'; text: string };

/** 查得太频繁时的那句。后端有自己的 message 时仍用后端的。（后端 `discover.rs` 的 `RATE_LIMIT_MESSAGE` 是同一句。） */
const RATE_LIMIT_NOTICE = 'GitHub 暂时不让查太频繁，过几分钟再点「重新找找」，已找到的先列在下面';

/** 放进收集箱以后的提示：新放进去的、和本来就在收集箱里的，说的不一样。 */
const ADDED_TO_INBOX = '已放进收集箱。想正式收藏，去 GitHub 点 Star，再回来点「同步」';
const ALREADY_IN_INBOX = '收集箱里已经有了';

export function addedToInboxToast(created: number, fullName: string): { title: string; sub: string } {
  return { title: created > 0 ? ADDED_TO_INBOX : ALREADY_IN_INBOX, sub: fullName };
}

/**
 * 场景右边的个数。没有候选时分两种：去 GitHub 找过但这次没有结果，和压根还没找过——以前都写「—」，分不出来。
 * 格子在 220px 的左栏里，字要短；整句放在 `candidateCountHint` 的悬停提示里。
 */
export function candidateCountText(count: number, lastFetchedAt?: string | null): string {
  if (Number.isFinite(count) && count > 0) return `${Math.floor(count)} 个`;
  return lastFetchedAt ? '没有结果' : '还没找过';
}

/** 个数格子的悬停提示（整句）。 */
export function candidateCountHint(count: number, lastFetchedAt?: string | null): string {
  if (Number.isFinite(count) && count > 0) return `${Math.floor(count)} 个你还没收藏的候选`;
  return lastFetchedAt ? '找过了，这次没有结果' : '还没去 GitHub 找过';
}

/** 右边没有候选可列时的空状态：一个场景都没有 / 找过但没有 / 还没找过。 */
export function discoverEmptyCopy(
  current: Pick<DiscoverScenario, 'lastFetchedAt'> | null | undefined,
): { title: string; description: string } {
  if (!current) {
    return { title: '还没有场景', description: '先去「资料库」同步一次收藏，场景会自动分好。' };
  }
  if (current.lastFetchedAt) {
    return { title: '这个场景上没有找到你还没收藏的', description: '可以换个场景，或者过几天再点「重新找找」。' };
  }
  return {
    title: '这个场景还没去找过',
    description: '点右上角「去 GitHub 找找」，会按这个场景搜一遍，把你还没收藏的列出来。不用登录，只会读 GitHub 公开的信息。',
  };
}

/** 联网刷新完以后给用户看的一句话。查得太频繁 / 部分失败用 warn，其余是 info。 */
export function refreshNotice(report: DiscoverRefreshReport): RefreshNotice {
  const message = report.message?.trim();
  if (report.rateLimitResetAt) {
    return { tone: 'warn', text: message || RATE_LIMIT_NOTICE };
  }
  if (message) return { tone: 'warn', text: message };
  if (report.failedQueries > 0) {
    return {
      tone: 'warn',
      text: `${report.queries} 次搜索里有 ${report.failedQueries} 次没成功，先显示拿到的部分。`,
    };
  }
  const skipped = report.skippedMine > 0 ? `，另有 ${report.skippedMine} 个你已经收藏过了` : '';
  if (report.fetched === 0) return { tone: 'info', text: `这次没有找到新的候选${skipped}。` };
  return { tone: 'info', text: `找到 ${report.fetched} 个候选${skipped}。` };
}

/** 复制给 Codex 的评估正文。正文本身不改。 */
export function buildEvaluatePrompt(candidate: Pick<DiscoverCandidate, 'fullName' | 'htmlUrl'>, scenarioName: string): string {
  const url = candidate.htmlUrl?.trim() || `https://github.com/${candidate.fullName}`;
  const area = scenarioName.trim() ? `我想用它做「${scenarioName.trim()}」。` : '';
  return [
    `请帮我评估这个 GitHub 项目：${candidate.fullName}（${url}）。${area}`,
    '先读它的 README 和最近的提交，然后用中文告诉我：',
    '1. 它是做什么的，能解决什么问题；',
    '2. 怎么安装、怎么试，需要什么环境；',
    '3. 有没有明显的坑（不再维护、许可证限制、依赖收费服务）；',
    '4. 值不值得我收藏，适合拿来做什么。',
    '不确定的地方直接说不确定，不要编。',
  ].join('\n');
}

/* ───────────── 热门趋势榜单 & 自由搜索 ───────────── */

export type DiscoverFeedMeta = {
  id: DiscoverFeedId;
  label: string;
  subtitle: string;
  supportsLanguage: boolean;
  emptyTitle: string;
  emptyDescription: string;
};

export const DISCOVER_FEEDS: readonly DiscoverFeedMeta[] = [
  {
    id: 'trending_daily',
    label: '今日飙升',
    subtitle: 'GitHub 今日热度飙升榜，抓不到趋势页时自动改用近期高星项目兜底。',
    supportsLanguage: true,
    emptyTitle: '今日飙升榜还没有拉取过',
    emptyDescription: '点右上角「拉取榜单」，看看今天 GitHub 上哪些项目涨得最快。',
  },
  {
    id: 'trending_weekly',
    label: '本周热门',
    subtitle: 'GitHub 本周趋势榜，适合看这一周持续升温的开源工具。',
    supportsLanguage: true,
    emptyTitle: '本周热门榜还没有拉取过',
    emptyDescription: '点右上角「拉取榜单」，看看这周讨论度与增量最高的项目。',
  },
  {
    id: 'rising_7d',
    label: '本周新锐',
    subtitle: '最近 7 天刚新建、快速起飞的新工具（20 星以上）。',
    supportsLanguage: true,
    emptyTitle: '本周新锐榜还没有拉取过',
    emptyDescription: '点右上角「拉取榜单」，挖掘最近 7 天刚发布就受关注的新项目。',
  },
  {
    id: 'rising_30d',
    label: '本月黑马',
    subtitle: '最近 30 天新建且热度破百的新锐项目。',
    supportsLanguage: true,
    emptyTitle: '本月黑马榜还没有拉取过',
    emptyDescription: '点右上角「拉取榜单」，看看最近一个月冒出来的黑马项目。',
  },
  {
    id: 'hn_show',
    label: 'HN Show 热门',
    subtitle: 'Hacker News 社区近两周热议的 Show HN 开源发布（仅保留 GitHub 项目）。',
    supportsLanguage: false,
    emptyTitle: 'HN Show 热门还没有拉取过',
    emptyDescription: '点右上角「拉取榜单」，看看海外开发者最近在 Show 什么新工具。',
  },
];

export const DISCOVER_LANGUAGES: ReadonlyArray<{ id: string; label: string }> = [
  { id: '', label: '全部语言' },
  { id: 'python', label: 'Python' },
  { id: 'typescript', label: 'TypeScript' },
  { id: 'rust', label: 'Rust' },
  { id: 'go', label: 'Go' },
];

export const DISCOVER_SEARCH_TIME_RANGES: ReadonlyArray<{ id: DiscoverSearchTimeRange; label: string }> = [
  { id: '30d', label: '近 30 天新建' },
  { id: '7d', label: '近 7 天新建' },
  { id: '90d', label: '近 3 个月新建' },
  { id: '180d', label: '近半年活跃' },
  { id: 'all', label: '不限时间' },
];

export const DISCOVER_SEARCH_SORTS: ReadonlyArray<{ id: DiscoverSearchSort; label: string }> = [
  { id: 'stars', label: '按星数排序' },
  { id: 'updated', label: '按最近更新' },
];

export const DISCOVER_SEARCH_PRESETS: readonly string[] = [
  'ai agent',
  'claude skill',
  'mcp server',
  'workflow automation',
  'cli tool',
];

export function feedMeta(feedId: DiscoverFeedId): DiscoverFeedMeta {
  return DISCOVER_FEEDS.find((item) => item.id === feedId) ?? DISCOVER_FEEDS[0];
}

export function feedLead(view: DiscoverFeedView | null | undefined, feedId: DiscoverFeedId, now: Date | number | string): string {
  const meta = feedMeta(feedId);
  if (!view?.lastFetchedAt) return meta.subtitle;
  const fallbackNote = view.usedFallback ? '（当前为近期高星兜底）' : '';
  return `${meta.subtitle} 上次拉取：${relativeDays(view.lastFetchedAt, now)}${fallbackNote}。`;
}

export function feedEmptyCopy(
  feedId: DiscoverFeedId,
  lastFetchedAt?: string | null,
): { title: string; description: string } {
  if (lastFetchedAt) {
    return {
      title: '这个榜单这次没有拿到可用项目',
      description: '可以换个语言或榜单试试，也可以稍后再点「刷新榜单」。',
    };
  }
  const meta = feedMeta(feedId);
  return { title: meta.emptyTitle, description: meta.emptyDescription };
}

export function feedRefreshNotice(report: DiscoverFeedRefreshReport): RefreshNotice {
  const message = report.message?.trim();
  if (report.rateLimitResetAt) {
    return {
      tone: 'warn',
      text: message || 'GitHub 暂时不让查太频繁，过几分钟再点「刷新」，已找到的先列在下面',
    };
  }
  if (message) return { tone: 'warn', text: message };
  const skipped = report.skippedMine > 0 ? `，另有 ${report.skippedMine} 个你已经收藏过了` : '';
  if (report.fetched === 0) return { tone: 'info', text: `这次没有拿到新的项目${skipped}。` };
  return { tone: 'info', text: `已更新 ${report.fetched} 个项目${skipped}。` };
}

export function filterDiscoverItems<T extends Pick<DiscoverCandidate, 'mine'>>(
  items: readonly T[],
  hideMine: boolean,
): T[] {
  return hideMine ? items.filter((item) => !item.mine) : [...items];
}
