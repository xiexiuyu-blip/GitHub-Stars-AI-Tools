/** 「发现」页的纯视图规则：场景排序、提示文案、候选状态、给 Codex 的评估请求。不碰 React 和网络，方便单测。 */

import type { DiscoverCandidate, DiscoverRefreshReport, DiscoverScenario } from '@/lib/contracts';
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
export function sortCandidates(candidates: readonly DiscoverCandidate[], addedNow: ReadonlySet<string>): DiscoverCandidate[] {
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
