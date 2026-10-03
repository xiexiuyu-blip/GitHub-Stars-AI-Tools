/** 「找方案」页的纯视图规则：勾选、提示文案、参考包请求。不碰 React，方便在 node 里测。 */

import type { LocalCapabilityInput, PackFormat, PackRequest } from '@/lib/api';
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

/**
 * 任务词在本机能力里的近义说法：本机的 Skill 说明大多是英文，MCP 只有名字（`gpt-image`），
 * 光用中文任务词（「封面」「配图」）对不上已经装好的 `ai-draw`、`gpt-image`。
 * 只放意思真的相近的词，不放太短、太泛的词（`cut` 会命中 `execute`）。
 */
const LOCAL_SYNONYMS: Record<string, readonly string[]> = {
  封面: ['cover', 'thumbnail', 'poster', 'image', 'draw', '出图', '绘图', '画图', '生图'],
  海报: ['poster', 'banner', 'image', 'draw', '出图', '绘图', '画图', '生图'],
  配图: ['illustration', 'infographic', 'image', 'draw', '出图', '绘图', '画图', '生图'],
  插图: ['illustration', 'image', 'draw', '出图', '绘图', '画图', '生图'],
  图片: ['image', 'picture', 'draw', '出图', '绘图', '画图', '生图'],
  主图: ['image', 'draw', '出图', '绘图', '画图', '生图'],
  详情页: ['image', 'draw', '出图', '绘图', '画图', '生图'],
  视频: ['video'],
  短视频: ['video', 'shorts', '剪辑'],
  剪辑: ['video', 'editing'],
  字幕: ['subtitle', 'caption', 'srt', 'whisper', '转写', '逐字稿', '妙记'],
  配音: ['voiceover', 'speech', 'tts'],
  ppt: ['pptx', 'slides', '幻灯片', '演示'],
  文档: ['docx', 'document', 'word', 'pdf'],
  表格: ['xlsx', 'sheet', 'excel'],
  公众号: ['wechat', 'weixin', '微信'],
  微信: ['wechat', 'weixin'],
  小红书: ['xhs', 'xiaohongshu', 'rednote'],
  抖音: ['douyin', 'tiktok'],
  文案: ['copywriting', '写作'],
  翻译: ['translate', 'translation'],
  采集: ['crawl', 'scrape', 'fetch', '抓取'],
  爬虫: ['crawl', 'scrape', '抓取'],
  电商: ['ecommerce', 'commerce'],
};

function localTerms(intent: string): string[] {
  const base = intent.trim().toLowerCase();
  if (!base) return [];
  return [base, ...(LOCAL_SYNONYMS[base] ?? [])];
}

/**
 * 从本机已装的 Skill / MCP 里挑出该写进参考包的：
 * 1. 名字和选中的项目对得上的（说明这个工具本机已经装了，Codex 不用再装），排最前；
 * 2. 和任务相关的，最多 limit 个（「本机已有的相关能力」）：任务词（含近义说法）出现在名字里算 3 分、
 *    只出现在一句话里算 1 分，每个任务词只算一次，按总分从高到低挑，同分按名字排。
 * 全部本机能力有几十上百个，整个塞进去只会淹没重点。
 */
export function pickLocalCapabilities(
  all: readonly LocalCapabilityInput[],
  hits: readonly Pick<SolveHit, 'fullName'>[],
  intents: readonly string[],
  limit = 5,
): LocalCapabilityInput[] {
  const repoNames = new Set<string>();
  for (const hit of hits) {
    const full = hit.fullName.trim().toLowerCase();
    if (!full) continue;
    repoNames.add(full);
    const short = full.split('/').pop();
    if (short) repoNames.add(short);
  }
  const groups = intents.map(localTerms).filter((terms) => terms.length > 0);
  const installed: LocalCapabilityInput[] = [];
  const scored: { item: LocalCapabilityInput; score: number }[] = [];
  for (const item of all) {
    const name = item.name.trim().toLowerCase();
    if (!name) continue;
    if (repoNames.has(name)) {
      installed.push(item);
      continue;
    }
    const line = item.oneLiner.toLowerCase();
    let score = 0;
    for (const terms of groups) {
      if (terms.some((term) => name.includes(term))) score += 3;
      else if (terms.some((term) => line.includes(term))) score += 1;
    }
    if (score > 0) scored.push({ item, score });
  }
  scored.sort((left, right) => right.score - left.score || left.item.name.localeCompare(right.item.name));
  return [...installed, ...scored.slice(0, Math.max(0, limit)).map((entry) => entry.item)];
}

/** 第一个是主参考，其余是备选。why 原样带上。local 是本机已装的相关能力（见 pickLocalCapabilities）。 */
export function buildPackRequest(
  query: string,
  hits: readonly SolveHit[],
  local: readonly LocalCapabilityInput[] = [],
): PackRequest {
  return {
    title: `参考包：${query}`,
    problem: query,
    items: hits.map((hit, index) => ({
      repoId: hit.id,
      role: index === 0 ? 'main' : 'alternative',
      why: hit.why ?? '',
    })),
    localCapabilities: [...local],
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
