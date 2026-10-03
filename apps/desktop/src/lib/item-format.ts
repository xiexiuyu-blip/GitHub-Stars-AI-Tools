import type { ReadingGroup } from '@/lib/contracts';

const READING_LABEL: Record<ReadingGroup, string> = {
  unseen: '没看',
  want: '想试',
  tried: '试过',
  in_use: '在用',
  dropped: '不要了',
};

const HOW_PLACEHOLDER = /摘录中未|未提供|没有提供|未展示/;
const DAY_MS = 86_400_000;

/** 小于 1000 原样；大于等于 1000 保留一位小数并去掉末尾的 `.0`。 */
export function formatStars(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '—';
  if (value < 1000) return String(Math.trunc(value));
  return `${(value / 1000).toFixed(1).replace(/\.0$/, '')}k`;
}

/**
 * 距现在多久。规则与原型 `ago` 一致：按毫秒差四舍五入到天。
 * 空值、坏日期返回「—」；未来（负数天）算「今天」。
 */
export function relativeDays(iso: string | null | undefined, now: Date | number | string): string {
  if (!iso || !iso.trim()) return '—';
  const target = new Date(iso).getTime();
  const current = now instanceof Date ? now.getTime() : new Date(now).getTime();
  if (Number.isNaN(target) || Number.isNaN(current)) return '—';
  const days = Math.round((current - target) / DAY_MS);
  if (days <= 0) return '今天';
  if (days === 1) return '昨天';
  if (days < 30) return `${days} 天前`;
  if (days < 365) return `${Math.floor(days / 30)} 个月前`;
  return `${Math.floor(days / 365)} 年前`;
}

/** `2026-10-03` → `10/3`。空值或不是日期返回「—」。 */
export function monthDay(iso: string | null | undefined): string {
  if (!iso) return '—';
  const match = /^(\d{4})-(\d{1,2})-(\d{1,2})/.exec(iso.trim());
  if (!match) return '—';
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (!Number.isInteger(month) || !Number.isInteger(day) || month < 1 || month > 12 || day < 1 || day > 31) {
    return '—';
  }
  return `${month}/${day}`;
}

/** 按「。」切句，取前 n 句并补回句号。n 小于 1 时返回空串。 */
export function firstSentences(text: string | null | undefined, count: number): string {
  const parts = (text ?? '').split('。').filter(Boolean);
  if (parts.length === 0 || !Number.isFinite(count) || count <= 0) return '';
  return `${parts.slice(0, count).join('。')}。`;
}

/** 空，或含占位话（摘录中未 / 未提供 / 没有提供 / 未展示）时返回空串，否则原样。 */
export function cleanHowToTry(text: string | null | undefined): string {
  if (!text || !text.trim()) return '';
  if (HOW_PLACEHOLDER.test(text)) return '';
  return text;
}

export function readingGroupLabel(group: ReadingGroup): string {
  return READING_LABEL[group] ?? '没看';
}

/** 5–10 早上好，11–13 中午好，14–17 下午好，其余（含 0 点、负数、非数字）晚上好。 */
export function greetingFor(hour: number): string {
  if (!Number.isFinite(hour)) return '晚上好';
  const whole = Math.trunc(hour);
  if (whole >= 5 && whole <= 10) return '早上好';
  if (whole >= 11 && whole <= 13) return '中午好';
  if (whole >= 14 && whole <= 17) return '下午好';
  return '晚上好';
}

/** 今日页问候下面那一句。数字按非负整数用；没有推荐时换一句说明。
 *  「待整理」的全库数量已经在统计卡片里，这里不再重复（它是全库口径，放进「这周」那句里会对不上）。 */
export function buildTodayLead(weekNew: number, pickCount: number): string {
  const week = clampCount(weekNew);
  const picks = clampCount(pickCount);
  const tail = picks > 0
    ? `先从下面 ${picks} 个你可能用得上的看起。`
    : 'AI 整理好之后，这里会按你常做的事挑出值得先看的。';
  return `这周你收藏了 ${week} 个新工具。${tail}`;
}

export type CodexFallbackInput = {
  fullName: string;
  htmlUrl?: string | null;
  oneLiner?: string | null;
  description?: string | null;
  howToTry?: string | null;
};

/** 后端 `copy_item_for_codex` 还没接上时，用详情拼一段能粘给 Codex 的话。 */
export function codexFallbackText(input: CodexFallbackInput): string {
  const url = input.htmlUrl?.trim() || `https://github.com/${input.fullName}`;
  const what = input.oneLiner?.trim() || input.description?.trim() || '（暂无说明）';
  const how = cleanHowToTry(input.howToTry);
  const lines = [
    `请帮我试用这个工具：${input.fullName}（${url}）`,
    `它是：${what}`,
    '我想用它来：（请先问我想做什么）',
  ];
  if (how) lines.push(`AI 整理的上手提示：${how}`);
  lines.push('请先阅读 README，确认安装方式和使用前提，再带我一步步装好并跑通一个最小例子。不确定的地方先问我，不要编造安装步骤。');
  return lines.join('\n');
}

export function splitRepoName(fullName: string): { owner: string; repo: string; initial: string } {
  const trimmed = fullName.trim();
  const slash = trimmed.indexOf('/');
  const repo = (slash >= 0 ? trimmed.slice(slash + 1) : trimmed).trim();
  const owner = slash >= 0 ? trimmed.slice(0, slash).trim() : '';
  const first = Array.from(repo)[0];
  return { owner, repo, initial: first ? first.toUpperCase() : '?' };
}

/** 按字符截断。不超过上限时原样返回。 */
export function clipText(text: string, max: number): string {
  if (!Number.isFinite(max) || max <= 0) return '';
  const chars = Array.from(text);
  if (chars.length <= max) return text;
  return `${chars.slice(0, max).join('')}…`;
}

/** 「今天值得先看」列表行第三行。只在有 `highlight` 且和一句话不同时才显示，否则整行不出现（不要把一句话重复一遍）。 */
export function pickWhyLine(item: { oneLiner?: string | null; highlight?: string | null }): string | null {
  const highlight = item.highlight?.trim();
  if (!highlight) return null;
  if (highlight === item.oneLiner?.trim()) return null;
  return `适合：${highlight}`;
}

export function applyReadingOverrides<T extends { id: string; readingGroup: string }>(
  items: readonly T[],
  overrides: Readonly<Record<string, string>>,
): T[] {
  return items.map((item) => {
    const group = overrides[item.id];
    if (!group || group === item.readingGroup) return item;
    return { ...item, readingGroup: group } as T;
  });
}

/**
 * 「本周新收藏」两组：推荐里已经标过状态的不显示；
 * 本周列表去掉仍出现在推荐里的，以及标成「不要了」的。
 */
export function assembleWeekGroups<T extends { id: string; readingGroup: string }>(
  picks: readonly T[],
  weekNew: readonly T[],
): { picks: T[]; rest: T[] } {
  const seen = new Set<string>();
  const visiblePicks: T[] = [];
  for (const item of picks) {
    if (item.readingGroup !== 'unseen' || seen.has(item.id)) continue;
    seen.add(item.id);
    visiblePicks.push(item);
  }
  const rest = weekNew.filter((item) => !seen.has(item.id) && item.readingGroup !== 'dropped');
  return { picks: visiblePicks, rest };
}

export function itemsForReadingFilter<T extends { readingGroup: string }>(
  items: readonly T[],
  group: string,
): T[] {
  return items.filter((item) => item.readingGroup === group);
}

function clampCount(value: number): number {
  if (!Number.isFinite(value) || value <= 0) return 0;
  return Math.floor(value);
}
