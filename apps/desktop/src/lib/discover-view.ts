/** 「发现」页的纯视图规则：场景排序、提示文案、候选状态、给 Codex 的评估请求。不碰 React 和网络，方便单测。 */

import type { DiscoverCandidate, DiscoverRefreshReport, DiscoverScenario } from '@/lib/contracts';
import { relativeDays } from '@/lib/item-format';

/** 偏少的排前面；同样偏少或同样不偏少，保持场景自己的顺序。 */
export function sortScenarios(list: readonly DiscoverScenario[]): DiscoverScenario[] {
  return [...list].sort((a, b) => Number(b.thin) - Number(a.thin) || a.sortOrder - b.sortOrder);
}

/** 右边标题下面那句：你在这个方向收了几个、偏不偏少、上次什么时候去找过。 */
export function scenarioLead(scenario: DiscoverScenario | null | undefined, now: Date | number | string): string {
  if (!scenario) return '';
  const mine = `你在这个方向收藏了 ${scenario.myCount} 个${scenario.thin ? '，偏少' : ''}。`;
  const fetched = scenario.lastFetchedAt
    ? `上次去 GitHub 找是${relativeDays(scenario.lastFetchedAt, now)}。`
    : '还没去 GitHub 找过。';
  return `${mine}${fetched}`;
}

export type CandidateStatus = 'mine' | 'inbox' | 'new';

/** 候选现在处于哪种状态：已在 Stars / 已在收集箱（含这次刚收进去的）/ 还没收。 */
export function candidateStatus(candidate: DiscoverCandidate, addedNow: ReadonlySet<string>): CandidateStatus {
  if (candidate.mine) return 'mine';
  if (candidate.inInbox || addedNow.has(candidate.fullName)) return 'inbox';
  return 'new';
}

export type RefreshNotice = { tone: 'info' | 'warn'; text: string };

/** 联网刷新完以后给 Fox 看的一句话。限流 / 部分失败用 warn，其余是 info。 */
export function refreshNotice(report: DiscoverRefreshReport): RefreshNotice {
  const message = report.message?.trim();
  if (report.rateLimitResetAt) {
    return { tone: 'warn', text: message || 'GitHub 暂时限流了，过一会儿再点「重新找找」。已拿到的部分已经列在下面。' };
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

/** 复制给 Codex：让它读 README，评估这个项目值不值得收、适不适合 Fox 做的事。 */
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
