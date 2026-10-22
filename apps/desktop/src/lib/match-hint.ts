/** 匹配度一共几档。界面画这么多根竖条。 */
export const MATCH_STAR_MAX = 4;

export type MatchStar = 1 | 2 | 3 | 4;

/** 只认 1、2、3、4。别的（含小数、字符串、空）都当成没有档位，界面不自己用分数推。 */
export function normalizeMatchStars(value: unknown): MatchStar | null {
  if (value === 1 || value === 2 || value === 3 || value === 4) return value;
  return null;
}

export const MATCH_STAR_HINTS: Record<MatchStar, string> = {
  4: '匹配度 4 / 4：搜的词主要出现在项目名里',
  3: '匹配度 3 / 4：搜的词主要出现在中文一句话说明里',
  2: '匹配度 2 / 4：搜的词主要出现在中文摘要或英文简介里',
  1: '匹配度 1 / 4：只是沾边，多在话题或关键词里',
};

const MAX_REASON_LABELS = 4;

/**
 * 「命中：项目名 / 中文说明」。只留非空字符串，去首尾空白，去重（保持原顺序），最多 4 个。
 * 一个有效的都没有就返回 null，调用方什么都不画。
 */
export function matchReasonsLine(reasons: readonly unknown[] | null | undefined): string | null {
  if (!reasons) return null;
  const seen = new Set<string>();
  const labels: string[] = [];
  for (const reason of reasons) {
    if (typeof reason !== 'string') continue;
    const text = reason.trim();
    if (!text || seen.has(text)) continue;
    seen.add(text);
    labels.push(text);
    if (labels.length === MAX_REASON_LABELS) break;
  }
  if (labels.length === 0) return null;
  return `命中：${labels.join(' / ')}`;
}
