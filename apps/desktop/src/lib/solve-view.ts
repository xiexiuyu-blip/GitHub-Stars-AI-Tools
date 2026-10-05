/** 「找方案」页的纯视图规则：勾选、提示文案、交接单请求。不碰 React，方便在 node 里测。 */

import type { LocalCapabilityInput, PackFormat, PackRequest } from '@/lib/api';
import type { RerankFit, RerankResult, ScenarioSummary, SolveHit, SolveResult } from '@/lib/contracts';

export type MatchBadge = { tone: 'blue' | 'gray'; text: string };

/** 交接单的格式选项。value 是传给后端的值，不能改；其余只是给人看的字。 */
export type FormatOption = {
  value: PackFormat;
  /** 完整的一句：名字（说明）。读屏和别处引用时用 */
  label: string;
  /** 名字：一眼看出是什么 */
  name: string;
  /** 一句说明：什么时候用它 */
  note: string;
};

function formatOption(value: PackFormat, name: string, note: string): FormatOption {
  return { value, name, note, label: `${name}（${note}）` };
}

export const formatOptions: readonly FormatOption[] = [
  formatOption('markdown', '文档', 'Markdown，直接粘给 Codex，默认'),
  formatOption('agents', '长期规则', 'AGENTS.md，存成文件放进工作文件夹'),
  formatOption('json', '程序用', 'JSON，一般不用选'),
];

/**
 * 复制成功后的提示：复制交接单（找方案）和复制单个工具的话（详情抽屉）粘到 Codex 之后，做的是同一件事，
 * 所以是同一句。别在别处再写一份。
 */
export const COPY_FOR_CODEX_TOAST = '已复制。粘到 Codex 后，它会先查你电脑上有没有，没有就装进独立文件夹，装完告诉你装在哪、怎么卸载。';

/** 「让 Codex 去查别人怎么用」那段话只查资料、不装东西，提示不能说它会装。 */
export const COPY_FOR_RESEARCH_TOAST = '已复制。粘到 Codex 后，它只会读资料、给你结论，不会安装。';

/**
 * 详情抽屉里复制「给 Codex 的话」之后的提示。抽屉里同一个回调接了两种话：「帮我装好并试用」（会装东西）
 * 和「去查别人怎么用」（只读资料）。认得出是后者才说「不会安装」；认不出的一律按会装东西的那句说，
 * 宁可多提醒，不能把会装东西的说成没事。
 */
export function copyPromptToast(text: string): string {
  return text.includes('请用 web-search 调查') ? COPY_FOR_RESEARCH_TOAST : COPY_FOR_CODEX_TOAST;
}

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
  // 全都对上是默认情况：八条里六七条都是，每条都标「都对上了」就等于没标，反而把真正有区别的「只对上…」淹没了。
  // 所以只标「只对上了一部分」的。
  void intents;
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
  return `你在「${name}」场景收藏了 ${count} 个。想看更多同类的新工具？`;
}

export function pendingNote(result: SolveResult): string | null {
  if (!Number.isFinite(result.pendingMatches) || result.pendingMatches <= 0) return null;
  return `另有 ${result.pendingMatches} 个待 AI 整理，整理好后可能对得上。`;
}

/** 一个都没找到时，空状态里引导去「发现」的那句话；有待 AI 整理的，后面接上 pendingNote。 */
export function emptyResultHint(result: SolveResult): string {
  const pending = pendingNote(result);
  return pending ? `去「发现」按这个场景找找看。${pending}` : '去「发现」按这个场景找找看';
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
  字幕: ['subtitle', 'caption', 'srt', 'whisper', '转写'],
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
  压缩: ['compress', 'optimize', 'minify'],
  尺寸: ['resize', 'crop', 'scale'],
  缩放: ['resize', 'scale'],
  裁剪: ['crop'],
  改图: ['image', 'retouch', 'edit'],
  修图: ['retouch', 'image'],
  抠图: ['rembg', 'matting', 'background', 'remove-bg'],
  水印: ['watermark'],
  日报: ['daily', 'digest'],
  周报: ['weekly', 'report'],
  月报: ['monthly', 'report'],
  转写: ['transcribe', 'whisper', 'asr', 'stt'],
  转录: ['transcribe', 'whisper', 'asr', 'stt'],
  文字稿: ['transcript', 'transcribe'],
  录音: ['audio', 'recording'],
  会议: ['meeting'],
  播客: ['podcast'],
  表情包: ['sticker', 'meme', 'emoji'],
  头像: ['avatar'],
  提示词: ['prompt'],
  抓取: ['crawl', 'scrape', 'fetch'],
  流程图: ['flowchart', 'mermaid', 'diagram'],
  思维导图: ['mindmap', 'mind-map'],
  二维码: ['qrcode', 'qr-code'],
  数据看板: ['dashboard'],
  看板: ['kanban'],
  用户画像: ['persona'],
};

/** 一句话里零星提到一个任务词（1 分）不算相关：「飞书电子表格……调整尺寸」不是改图工具。名字里有（3 分）或两处以上才算。 */
const MIN_LOCAL_SCORE = 2;

function localTerms(intent: string): string[] {
  const base = intent.trim().toLowerCase();
  if (!base) return [];
  return [base, ...(LOCAL_SYNONYMS[base] ?? [])];
}

/**
 * 从本机已装的 Skill / MCP 里挑出该写进交接单的：
 * 1. 名字和选中的项目对得上的（说明这个工具本机已经装了，Codex 不用再装），排最前；
 * 2. 和任务相关的，最多 limit 个（「本机已有的相关能力」）：任务词（含近义说法）出现在名字里算 3 分、
 *    只出现在一句话里算 1 分，每个任务词只算一次，总分不到 2 的不要（零星提到一次不算相关），按总分从高到低挑，同分按名字排。
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
    if (score >= MIN_LOCAL_SCORE) scored.push({ item, score });
  }
  scored.sort((left, right) => right.score - left.score || left.item.name.localeCompare(right.item.name));
  return [...installed, ...scored.slice(0, Math.max(0, limit)).map((entry) => entry.item)];
}

/** 一个工具都没勾，或勾了又都不合适时，进「直接交给 Codex」。两处同一句。 */
export const DIRECT_LABEL = '不用这些工具，直接交给 Codex';
/** 和 DIRECT_LABEL 同一句。勾选留着，「改回挑工具」能回去。 */
export const DIRECT_FROM_PICKS_LABEL = '不用这些工具，直接交给 Codex';
/** 从直接交给 Codex 回到勾选工具。 */
export const BACK_TO_PICK_LABEL = '改回挑工具';

/**
 * 第一个是主参考，其余是备选。why 原样带上。local 是本机已装的相关能力（见 pickLocalCapabilities）。
 * direct 为真时不带任何工具：标题换成「直接交给 Codex」，mode 写成 direct。为假时不写 mode。
 */
export function buildPackRequest(
  query: string,
  hits: readonly SolveHit[],
  local: readonly LocalCapabilityInput[] = [],
  direct = false,
): PackRequest {
  if (direct) {
    return {
      title: `直接交给 Codex：${query}`,
      problem: query,
      items: [],
      localCapabilities: [...local],
      mode: 'direct',
    };
  }
  return {
    title: `交接单：${query}`,
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

/**
 * 列表顶部那一行，说勾了几个。`pickedCount` 必须是界面上此刻真勾着的个数（右边「已选 N 个」那个数），
 * 不是后端当初默认勾的个数：AI 精选会改勾，用户也会自己改。
 * - 直接交给 Codex：勾选没在用，不说。
 * - 勾选是应用替你勾的（`edited` 为假；AI 精选改勾之后也是）：有勾才说「已替你勾了最对口的 N 个」；
 *   一个都没勾不说，右边有「没有特别对口的，所以没替你默认勾选」（见 packEmptyNote）。
 * - 用户自己改过之后（`edited` 为真）：不再说「替你勾」，只说「已勾 N 个」，勾到 0 个也说，免得这一行忽有忽无、把列表顶得上下跳。
 */
export function defaultPickNote(pickedCount: number, direct: boolean, edited = false): string | null {
  if (direct || !Number.isFinite(pickedCount)) return null;
  const count = Math.max(0, Math.trunc(pickedCount));
  if (edited) return `已勾 ${count} 个`;
  if (count === 0) return null;
  return `已替你勾了最对口的 ${count} 个，不合适可以取消`;
}

/** 页面用的：数的是命中里真勾着的（和右边「已选 N 个」同一个数），不是后端当初的 preselected。 */
export function pickListNote(
  hits: readonly SolveHit[],
  picked: readonly string[],
  direct: boolean,
  edited: boolean,
): string | null {
  return defaultPickNote(orderedPicks(hits, picked).length, direct, edited);
}

/** 交接单大约多少字。不到一万字写具体的数，一万字以上写「约 1.2 万字」。 */
export function packSizeText(length: number): string {
  if (!Number.isFinite(length) || length < 0) return '';
  const count = Math.round(length);
  if (count < 10000) return `约 ${String(count).replace(/\B(?=(\d{3})+(?!\d))/g, ',')} 字`;
  return `约 ${Math.round(count / 1000) / 10} 万字`;
}

/** 交接单里有工具已经不在资料库里（多半是你在 GitHub 上取消了收藏）。没有就不说。 */
export function missingToolsNote(count: number): string | null {
  if (!Number.isFinite(count) || count <= 0) return null;
  return `有 ${count} 个工具已不在资料库里（可能你取消了收藏）`;
}

/**
 * 交接单面板里一个工具都没勾时，说一句为什么没替你勾：搜到了但没有特别对口的（后端默认勾选是空的），
 * 或者一个都没搜到。还没搜、或者是自己取消了默认勾选的，不用解释，返回 null。
 *
 * AI 精选说「都不合适」、把默认勾选清空了，也是应用没替你勾，一样要说：这时后端的默认勾选不是空的，
 * 所以要靠 `edited` 区分——传 false 表示勾选是应用清空的、用户没动过。不传按「用户改过」，只看后端有没有默认勾。
 */
export function packEmptyNote(result: Pick<SolveResult, 'hits' | 'preselected'> | null, edited = true): string | null {
  if (!result) return null;
  if (result.hits.length === 0) return '收藏里没找到相关的工具。';
  if (result.preselected.length === 0 || !edited) {
    return '没有特别对口的，所以没替你默认勾选。看一眼左边的列表自己勾，或者直接交给 Codex。';
  }
  return null;
}

export function packPlaceholder(format: PackFormat): string {
  void format;
  return '（还没有选中的工具）';
}

export const RERANK_FIT_LABEL: Record<RerankFit, string> = {
  best: '最合适',
  good: '合适',
  maybe: '备选',
  poor: '不太合适',
};

export function RERANK_TOGGLE_LABEL(on: boolean): string {
  return on ? 'AI 精选：开' : 'AI 精选：关';
}

const RERANK_USABLE = new Set(['ok', 'cached']);

/** 一次精选最多把多少个候选交给 AI（和后端 `MAX_CANDIDATES` 一致）。 */
export const RERANK_CANDIDATE_LIMIT = 16;

/**
 * 交给 AI 精选的候选 id：本地排名前 8 的命中，加上后备的 9–16 名（`moreHits`，界面默认不显示）。去重，最多 16 个。
 * 后备候选让 AI 能把本地排在后面的、真正对口的工具捞上来。
 */
export function rerankCandidateIds(result: Pick<SolveResult, 'hits' | 'moreHits'>): string[] {
  const seen = new Set<string>();
  const ids: string[] = [];
  for (const hit of [...(result.hits ?? []), ...(result.moreHits ?? [])]) {
    if (!hit || typeof hit.id !== 'string' || !hit.id || seen.has(hit.id)) continue;
    seen.add(hit.id);
    ids.push(hit.id);
    if (ids.length >= RERANK_CANDIDATE_LIMIT) break;
  }
  return ids;
}

/**
 * AI 从后备候选里点名的「最合适 / 合适」的工具：本地排在 9–16 名、默认不显示，AI 觉得对口就拿出来放进列表。
 * 按 AI 给的顺序；已经在 `hits` 里的不重复；精选不可用时没有。
 */
export function promotedHits(result: Pick<SolveResult, 'hits' | 'moreHits'>, rerank: RerankResult | null): SolveHit[] {
  if (!rerank || !RERANK_USABLE.has(rerank.status)) return [];
  const known = new Set((result.hits ?? []).map((hit) => hit.id));
  const extras = new Map((result.moreHits ?? []).map((hit) => [hit.id, hit]));
  const promoted: SolveHit[] = [];
  for (const item of rerank.items) {
    if (item.fit !== 'best' && item.fit !== 'good') continue;
    const hit = extras.get(item.repoId);
    if (!hit || known.has(hit.id)) continue;
    known.add(hit.id);
    promoted.push(hit);
  }
  return promoted;
}

/** 按 AI 的顺序排列表。用不上时返回原数组，不改入参。 */
export function applyRerank(hits: SolveHit[], rerank: RerankResult | null): SolveHit[] {
  if (!rerank || !RERANK_USABLE.has(rerank.status) || rerank.items.length === 0) return hits;
  const byId = new Map(hits.map((item) => [item.id, item]));
  const seen = new Set<string>();
  const ordered: SolveHit[] = [];
  for (const item of rerank.items) {
    if (seen.has(item.repoId)) continue;
    const hit = byId.get(item.repoId);
    if (!hit) continue;
    seen.add(item.repoId);
    ordered.push(hit);
  }
  for (const hit of hits) {
    if (!seen.has(hit.id)) ordered.push(hit);
  }
  return ordered;
}

/**
 * ok / cached 时给出默认勾选：best 在前，然后 good，最多 3 个，且必须在命中里。
 * 一个都不合适时返回空数组。精选不可用时返回 null，调用方不要改勾选。
 */
export function rerankPicks(rerank: RerankResult | null, hits: readonly SolveHit[]): string[] | null {
  if (!rerank || !RERANK_USABLE.has(rerank.status)) return null;
  if (rerank.noneFit) return [];
  const known = new Set(hits.map((hit) => hit.id));
  const picked: string[] = [];
  const seen = new Set<string>();
  const take = (fit: RerankFit) => {
    for (const item of rerank.items) {
      if (picked.length >= 3) return;
      if (item.fit !== fit || !known.has(item.repoId) || seen.has(item.repoId)) continue;
      seen.add(item.repoId);
      picked.push(item.repoId);
    }
  };
  take('best');
  take('good');
  return picked;
}

export function rerankStatusText(state: {
  phase: 'idle' | 'loading' | 'done';
  result: RerankResult | null;
  enabled: boolean;
}): string | null {
  if (!state.enabled) return null;
  if (state.phase === 'loading') return 'AI 正在帮你挑…';
  if (state.phase !== 'done' || !state.result) return null;
  const { result } = state;
  if (result.status === 'ok') {
    if (result.tokens == null || !Number.isFinite(result.tokens)) return 'AI 已挑过';
    return `AI 已挑过 · 本次约 ${formatRerankTokens(result.tokens)} token`;
  }
  if (result.status === 'cached') return 'AI 已挑过 · 用的是上次的结果，没花 token';
  if (result.status === 'skipped' || result.status === 'refused' || result.status === 'failed') {
    const message = result.message?.trim() ?? '';
    return message || null;
  }
  return null;
}

function formatRerankTokens(tokens: number): string {
  if (tokens < 1000) return String(Math.trunc(tokens));
  return `${(tokens / 1000).toFixed(1)} 千`;
}

function quoteIntentList(intents: readonly string[]): string {
  const quoted = intents.map((intent) => `「${intent}」`);
  if (quoted.length <= 1) return quoted.join('');
  if (quoted.length === 2) return `${quoted[0]}和${quoted[1]}`;
  return `${quoted.slice(0, -1).join('')}和${quoted[quoted.length - 1]}`;
}
