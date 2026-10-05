import type { ReadingGroup } from '@/lib/contracts';
import { regenerateNote, type AutomationStatus, type RegenerateNote } from '@/lib/automation-view';

const READING_LABEL: Record<ReadingGroup, string> = {
  unseen: '没看',
  want: '想试',
  tried: '试过',
  in_use: '在用',
  dropped: '不要了',
};

const HOW_PLACEHOLDER = /摘录中未|未提供|没有提供|未展示/;
const DAY_MS = 86_400_000;

/** 不到一万写整数；一万及以上写成「X 万」，保留一位小数并去掉末尾的 `.0`。 */
export function formatStars(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '—';
  if (value < 10000) return String(Math.trunc(value));
  return `${(value / 10000).toFixed(1).replace(/\.0$/, '')} 万`;
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

/**
 * `2026-10-03` → `10 月 3 日`；不是今年的带年份：`2025 年 10 月 3 日`。空值或不是日期返回「—」。
 * 只写日期，不写钟点：`starredAt` 是 GitHub 给的 UTC 时间，写上钟点会和你本机的时间差好几个小时。
 * 年月日直接取字符串里的，不做时区换算；字符串后面带的时间一律忽略。
 */
export function monthDay(iso: string | null | undefined, now: Date | number | string = new Date()): string {
  if (!iso) return '—';
  const match = /^(\d{4})-(\d{1,2})-(\d{1,2})/.exec(iso.trim());
  if (!match) return '—';
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (!Number.isInteger(month) || !Number.isInteger(day) || month < 1 || month > 12 || day < 1 || day > 31) {
    return '—';
  }
  const text = `${month} 月 ${day} 日`;
  // 现在的年份读不出来时（now 不是合法时间）宁可多写年份
  return year === new Date(now).getFullYear() ? text : `${year} 年 ${text}`;
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

const MATURITY_LABEL: Record<string, string> = {
  usable: '可用',
  mature: '成熟',
  stable: '稳定',
  production: '可正式使用',
  stale: '久未更新',
  beta: '测试版',
  alpha: '早期',
  experimental: '实验中',
  prototype: '原型',
};

/**
 * 「成熟度」做成短标签：认识的英文换成中文；已经是短词（不超过 6 个字）的原样；整句话
 * （「开源，提供安装脚本和文档，需自行部署。」）不显示——截半句只会误导，这些信息中文说明里已经有了。
 */
export function maturityLabel(raw: string | null | undefined): string | null {
  const text = raw?.trim();
  if (!text) return null;
  const mapped = MATURITY_LABEL[text.toLowerCase()];
  if (mapped) return mapped;
  return Array.from(text).length <= 6 ? text : null;
}

const EMOJI = /[\p{Extended_Pictographic}\u200d\ufe0f]/gu;
const SHORTCODE = /:[a-z0-9_+-]{2,}:/gi;
const URL_TEXT = /https?:\/\/\S+/gi;

/** GitHub 描述里常有表情、:shortcode:、链接：列表里读起来像噪音，去掉；折成一行。 */
export function cleanDescription(raw: string | null | undefined): string {
  if (!raw) return '';
  return raw
    .replace(URL_TEXT, ' ')
    .replace(SHORTCODE, ' ')
    .replace(EMOJI, '')
    .replace(/[\s\u3000]+/g, ' ')
    .replace(/^[\s|｜·\-–—:：]+/, '')
    .trim();
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
 *  「待 AI 整理」的数量已经在统计卡片里，这里不再重复。 */
export function buildTodayLead(weekNew: number, pickCount: number): string {
  const week = clampCount(weekNew);
  const picks = clampCount(pickCount);
  const tail = picks > 0
    ? `先从下面 ${picks} 个你可能用得上的看起。`
    : 'AI 整理好之后，这里会从还没看过的收藏里，优先挑和你关心的场景有关的。';
  return `这周你收藏了 ${week} 个新工具。${tail}`;
}

export type CodexFallbackInput = {
  fullName: string;
  htmlUrl?: string | null;
  oneLiner?: string | null;
  description?: string | null;
  howToTry?: string | null;
};

/**
 * 后端 `copy_item_for_codex` 还没接上时，用详情拼一段能粘给 Codex 的话。
 * 这是复制出去的正文，不是界面上的字：用词（README、上手提示、不要编造安装步骤）保持原样，别跟着界面文案改。
 */
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

/**
 * 「为什么今天推荐给你」：它被归在哪个场景 + 你还没看过。
 * 没有所属场景就返回 null（不编）。
 */
export function pickReason(scenarioNames: readonly string[]): string | null {
  const name = scenarioNames.map((item) => item.trim()).find(Boolean);
  if (!name) return null;
  return `它被归在你关心的「${name}」场景，你还没看过`;
}

/** 「不要了」悬停说明。图标按钮本身的名字仍是「不要了（X）」，这句长的只放在悬停提示里。 */
export const DROP_HINT = '标成「不要了」：只是今日里不再出现，不会取消 GitHub 收藏，可在资料库「我的状态 → 不要了」里找回';

/** 已经有中文说明、但「用之前注意」是空的时候，显示这一行灰字。 */
export const RISKS_EMPTY = '项目说明里没提限制，不代表没有';

/** 场景「偏少」的标签字和悬停说明。资料库左栏和发现页共用，只在这里定义一次。 */
export const THIN_TAG = '偏少';
export const THIN_TAG_HINT = '少于 14 个';

const README_ONLY_PREFIX = '只有 GitHub 简介，没有 README：';
const README_BASIS = '依据项目说明（README）';

/** 中文说明旁边那句依据。前缀是后端写在说明开头的，这里只判断、不改原文。 */
export function readmeBasisLabel(text: string | null | undefined): string {
  const trimmed = text?.trim() ?? '';
  if (trimmed.startsWith(README_ONLY_PREFIX)) return '只依据 GitHub 简介，没读到项目说明，别当真';
  return README_BASIS;
}

/**
 * 详情页「项目说明」页签上的字。「项目说明（README）」只在详情页第一次提到的地方带括号：
 * 有中文说明、依据的又是项目说明时，说明旁边那句已经带了，页签就只写「项目说明」；
 * 其余情况（还没有中文说明，或说明只依据 GitHub 简介）页签是第一次出现，由它带括号。
 */
export function readmeTabLabel(hasCard: boolean, summary: string | null | undefined): string {
  return hasCard && readmeBasisLabel(summary) === README_BASIS ? '项目说明' : '项目说明（README）';
}

/** 灵感展示：把单独的「Fox」换成「你」（Firefox 这类词里的不动）。大小写敏感。中文和「你」之间的空格收掉，避免「给 你 的」。 */
export function displayForYou(text: string | null | undefined): string {
  const trimmed = text?.trim() ?? '';
  if (!trimmed) return '';
  return trimmed
    .replace(/\bFox\b/g, '你')
    .replace(/([\u4e00-\u9fff])\s+你/g, '$1你')
    .replace(/你\s+([\u4e00-\u9fff])/g, '你$1');
}

/** 详情小标签：「成熟度：可用」。不认识的整句话仍然不显示。 */
export function maturityTagLabel(raw: string | null | undefined): string | null {
  const label = maturityLabel(raw);
  return label ? `成熟度：${label}` : null;
}

/** 「复制给 Codex」的按钮字：资料库和今日的详情用安装那条，发现页用评估那条。页面都从这里引用，不要各写一份。 */
export const COPY_INSTALL_BUTTON = '复制「帮我装好并试用」给 Codex';
export const COPY_EVALUATE_BUTTON = '复制「帮我评估」给 Codex';

/** 复制不了（剪贴板不让写）时的提示。 */
export const COPY_FAILED_NOTICE = '没能自动复制，请手动选中后按 ⌘C';

/** 复制出去的是哪一种话：装好并试用 / 只评估 / 去查别人怎么用。 */
export type CopyKind = 'install' | 'evaluate' | 'community';

const COPIED_NOTICE: Record<CopyKind, string> = {
  install: '已复制。粘到 Codex 后，它会先查你电脑上有没有，没有就装进独立文件夹，装完告诉你装在哪、怎么卸载。',
  evaluate: '已复制。它只会读资料、给你结论，不会安装。',
  // 只是让 Codex 去搜别人怎么用，不装东西，所以不能套用安装那一句
  community: '已复制，去 Codex 里粘贴',
};

/** 复制成功后的提示。粘出去的话让 Codex 做的事不一样，提示也不一样：会装软件的那种要说清楚。 */
export function copiedNotice(kind: CopyKind): string {
  return COPIED_NOTICE[kind];
}

/**
 * 「不对，请 AI 重写」点完之后的提示。
 * 自动整理没开（服务商配好了、没暂停、也不是一次性全部整理，只是开关关着）时，说清「不会重写」，并保留「去开启」；
 * 其余情况照 regenerateNote。
 */
export function regenerateToast(status: AutomationStatus | null): RegenerateNote {
  const note = regenerateNote(status);
  if (status && status.providerConfigured && !status.paused && status.unlimited !== true && !status.enabled) {
    return { title: '已记下，但自动整理没开，不会重写。', sub: '开启后才会重写。', actionLabel: note.actionLabel };
  }
  return note;
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
