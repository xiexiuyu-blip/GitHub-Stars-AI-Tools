/** 「找方案」页的纯视图规则：勾选、提示文案、参考包请求。不碰 React，方便在 node 里测。 */

import type { PackFormat, PackRequest } from '@/lib/api';
import type { ScenarioSummary, SolveHit, SolveResult } from '@/lib/contracts';

export type MatchBadge = { tone: 'blue' | 'gray'; text: string };

export const formatOptions: readonly { value: PackFormat; label: string }[] = [
  { value: 'markdown', label: 'Markdown' },
  { value: 'agents', label: 'AGENTS.md' },
  { value: 'json', label: 'JSON' },
];

/** 默认勾选直接用 preselected，丢掉不在命中里的 id，并保持原顺序。 */
export function initialPicks(result: SolveResult): string[] {
  const known = new Set((result.hits ?? []).map((hit) => hit.id));
  const picked: string[] = [];
  const seen = new Set<string>();
  for (const id of result.preselected ?? []) {
    if (!known.has(id) || seen.has(id)) continue;
    seen.add(id);
    picked.push(id);
  }
  return picked;
}

/** 返回新数组，不改原数组。新勾选加在末尾，取消时其余项的相对顺序不变。 */
export function togglePick(picked: readonly string[], id: string): string[] {
  if (picked.includes(id)) return picked.filter((item) => item !== id);
  return [...picked, id];
}

/** 把某一项设成主参考：勾选里排第一的就是主参考。不在勾选里或已经是第一个，原样返回。 */
export function makeMain(picked: readonly string[], id: string): string[] {
  if (!picked.includes(id) || picked[0] === id) return [...picked];
  return [id, ...picked.filter((item) => item !== id)];
}

/** 按勾选顺序返回命中。不在命中里的 id 跳过。 */
export function orderedPicks(hits: readonly SolveHit[], picked: readonly string[]): SolveHit[] {
  const byId = new Map(hits.map((hit) => [hit.id, hit]));
  const seen = new Set<string>();
  const ordered: SolveHit[] = [];
  for (const id of picked) {
    if (seen.has(id)) continue;
    const hit = byId.get(id);
    if (!hit) continue;
    seen.add(id);
    ordered.push(hit);
  }
  return ordered;
}

/**
 * 有命中、没有任何一条对上全部意图、且意图词多于 1 个时，提示部分匹配。
 * 两个意图写成「A」和「B」，三个及以上写成「A」「B」和「C」。
 */
export function gapNote(result: SolveResult): string | null {
  const hits = result.hits ?? [];
  const intents = result.intents ?? [];
  if (hits.length === 0 || result.fullMatchCount !== 0 || intents.length <= 1) return null;
  return `你的收藏里没有同时对上${quoteIntentList(intents)}的工具，下面是部分匹配。`;
}

export function matchBadge(hit: SolveHit, intents: readonly string[]): MatchBadge | null {
  if (hit.fullMatch && (intents?.length ?? 0) > 1) {
    return { tone: 'blue', text: '都对上了' };
  }
  const matched = hit.matched ?? [];
  if (!hit.fullMatch && matched.length > 0) {
    return { tone: 'gray', text: `只对上${matched.map((item) => `「${item}」`).join('')}` };
  }
  return null;
}

/** suggestedScenarioId 为空，或场景列表里对不上名字时，不编一句。 */
export function crossNote(result: SolveResult, scenarios: readonly ScenarioSummary[]): string | null {
  const id = result.suggestedScenarioId?.trim() ?? '';
  if (!id) return null;
  const name = (scenarios ?? []).find((item) => item.id === id)?.nameZh?.trim() ?? '';
  if (!name) return null;
  const count = result.suggestedScenarioCount ?? 0;
  return `你在「${name}」方向收藏了 ${count} 个。想看更多同类的新项目？`;
}

export function pendingNote(result: SolveResult): string | null {
  if (!Number.isFinite(result.pendingMatches) || result.pendingMatches <= 0) return null;
  return `另有 ${result.pendingMatches} 个还没整理，整理好后可能对得上。`;
}

/** why 去掉空白后为空，或和一句话完全一样时，不再单独写一行。 */
export function whyLine(hit: SolveHit): string | null {
  const why = hit.why?.trim() ?? '';
  if (!why) return null;
  const oneLiner = hit.oneLiner?.trim() ?? '';
  if (why === oneLiner) return null;
  return why;
}

/** 认不出任务词、但仍有文字相近的命中时，提醒不要当成全部对上。 */
export function noIntentNote(result: SolveResult): string | null {
  const intents = result.intents ?? [];
  const hits = result.hits ?? [];
  if (intents.length === 0 && hits.length > 0) {
    return '没有认出具体的任务词，下面是文字上相近的收藏，请自己勾选你觉得对的。';
  }
  return null;
}

/** 第一个是主参考，其余是备选。why 原样带上。 */
export function buildPackRequest(query: string, hits: readonly SolveHit[]): PackRequest {
  return {
    title: `参考包：${query}`,
    problem: query,
    items: hits.map((hit, index) => ({
      repoId: hit.id,
      role: index === 0 ? 'main' : 'alternative',
      why: hit.why ?? '',
    })),
    localCapabilities: [],
  };
}

/** 本地日历日，YYYY-MM-DD。不用 UTC，避免跨日时差一天。 */
export function todayLocalDate(now: Date): string {
  if (!(now instanceof Date) || Number.isNaN(now.getTime())) return '';
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

export function packPlaceholder(format: PackFormat): string {
  void format;
  return '（还没有选中的工具）';
}

function quoteIntentList(intents: readonly string[]): string {
  const quoted = intents.map((intent) => `「${intent}」`);
  if (quoted.length <= 1) return quoted.join('');
  if (quoted.length === 2) return `${quoted[0]}和${quoted[1]}`;
  return `${quoted.slice(0, -1).join('')}和${quoted[quoted.length - 1]}`;
}
