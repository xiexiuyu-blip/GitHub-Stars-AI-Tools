/** 工具箱里「AI 自动整理」那一块用到的纯函数：额度档位、全量整理、状态文案、数字格式。不碰网络、不碰界面，方便单测。 */

export type PauseReason = { kind: string; message: string; at: string };

/** 正在进行的一轮全量整理。后台还没带上这个字段时，整个值是 null。 */
export type FullScan = {
  /** ISO 时间（UTC，带 Z） */
  startedAt: string;
  /** 兜底截止（开始后 48 小时），到点自动结束 */
  until: string;
  /** 开始以来已用 token（含在途的预留） */
  tokensUsed: number;
  /** 开始时要整理的个数 */
  total: number;
  /** 现在还剩（排队 + 进行中） */
  remaining: number;
  /** 开始以来失败了几个（旧后台没有这个字段，当 0） */
  failed?: number;
};

/** 上一轮全量整理的结果。reason 认不出就当没有。 */
export type LastFullScan = {
  startedAt: string;
  finishedAt: string;
  tokensUsed: number;
  /** 整理完 / 他点了停止 / 到 48 小时 / 用量远超预计被自动停下 / 失败太多被自动停下 */
  reason: 'completed' | 'stopped' | 'expired' | 'ceiling' | 'failures';
  total: number;
  /** 结束时还剩几个没整理 */
  remaining: number;
  /** 这一轮失败了几个（旧后台没有这个字段，当 0） */
  failed?: number;
};

export type AutomationStatus = {
  enabled: boolean;
  dailyBudgetTokens: number;
  paused: boolean;
  pauseReason: PauseReason | null;
  tokens24h: number;
  budgetRemaining: number;
  queued: number;
  running: number;
  failed: number;
  providerConfigured: boolean;
  model: string | null;
  /**
   * 全量整理正在进行。字段缺失（旧后台）当作 false：用 `=== true` 判断，不要把缺失当成开着。
   * 面板是否显示「进行中」另看 `fullScanPanel`：有 `fullScan` 且这里不是明确的 false 才算在跑。
   */
  unlimited?: boolean;
  /** 缺失当作 null。 */
  fullScan?: FullScan | null;
  /** 缺失当作 null。 */
  lastFullScan?: LastFullScan | null;
  /** 还没整理好的收藏数。缺失或 null 时用 queued + running 代替。 */
  needsOrganizing?: number | null;
};

/** 整理一个仓库大约用多少 token（输入约 3000 + 输出约 600，取整到 4000）。只用来估「约整理多少个」。 */
export const TOKENS_PER_REPO = 4000;

/** 全量整理估算：2026-10-04 真实记录里成功的卡平均约 6.3 千 token（含思考），偶尔被截断要重试，取 7500。只用于全量整理的预计花费，不用于日常额度折算。 */
export const FULL_SCAN_TOKENS_PER_REPO = 7500;

/** 额度档位：名字 + 每天约整理多少个。0 是关闭。 */
export const BUDGET_PRESETS: ReadonlyArray<{ tokens: number; title: string; hint: string }> = [
  { tokens: 0, title: '关闭', hint: '不自动整理，一分钱不花' },
  { tokens: 100_000, title: '试跑', hint: '每天约 25 个，先看看效果' },
  { tokens: 200_000, title: '日常', hint: '每天约 50 个' },
  { tokens: 600_000, title: '集中补齐', hint: '每天约 150 个，尽快补完旧收藏' },
];

/** 开始整理一个仓库前，先按「两次调用都用满」预留的量（Rust 的 `CLAIM_RESERVE_TOKENS`）。额度比它小，一个仓库也领不到。 */
export const MIN_DAILY_BUDGET = 12_000;

/** 把输入框里的文字变成额度。合法返回数字，不合法返回给人看的错误提示。 */
export function parseDailyBudget(raw: string): number | string {
  const text = raw.trim();
  if (!text) return '请先填写每天额度';
  if (text.startsWith('-')) return '每天额度不能小于 0';
  if (!/^\d+$/.test(text)) return '每天额度要填整数';
  const value = Number(text);
  if (!Number.isSafeInteger(value)) return '每天额度要填整数';
  if (value > 0 && value < MIN_DAILY_BUDGET) return `每天额度至少要 ${MIN_DAILY_BUDGET} token，再少连一个仓库都整理不了（要关闭就填 0）`;
  return value;
}

/** 这些 token 大约够整理多少个仓库。 */
export function approxRepos(tokens: number): number {
  if (!Number.isFinite(tokens)) return 0;
  return Math.floor(Math.max(0, tokens) / TOKENS_PER_REPO);
}

/** 1 万以上写成「10 万」「1.5 万」，以下原样。 */
export function formatTokens(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '0';
  if (value >= 10_000) return `${(value / 10_000).toFixed(value % 10_000 === 0 ? 0 : 1)} 万`;
  return String(Math.floor(value));
}

/** 全量整理预计要多少 token。个数不是有限数或 ≤ 0 时不估。 */
export function estimateFullScanTokens(count: number): number {
  if (!Number.isFinite(count) || count <= 0) return 0;
  const tokens = Math.round(count * FULL_SCAN_TOKENS_PER_REPO);
  if (!Number.isFinite(tokens) || tokens <= 0) return 0;
  return tokens;
}

/**
 * 给人看的「约」数：≥ 100 万按 10 万取整，否则按 1 万取整（至少 1 万）。
 * 半个万沿用 Math.round（0.5 向正无穷）。0 和非法值返回「0」。
 */
export function roundedEstimate(tokens: number): string {
  if (!Number.isFinite(tokens) || tokens <= 0) return '0';
  const step = tokens >= 1_000_000 ? 100_000 : 10_000;
  let rounded = Math.round(tokens / step) * step;
  if (tokens < 1_000_000 && rounded < 10_000) rounded = 10_000;
  return formatTokens(rounded);
}

/** 现在还有多少个要整理。优先用后台给的 needsOrganizing；没给就用排队 + 进行中。 */
export function fullScanPending(status: AutomationStatus): number {
  const raw = status.needsOrganizing ?? status.queued + status.running;
  if (!Number.isFinite(raw) || raw < 0) return 0;
  return raw;
}

/** 进度：done 夹在 0 和 total 之间；total ≤ 0 时百分比是 0；remaining 正好是 0 且 total > 0 时一定是 100。 */
export function fullScanProgress(scan: FullScan): { done: number; total: number; percent: number } {
  const total = Number.isFinite(scan.total) ? scan.total : 0;
  const remaining = Number.isFinite(scan.remaining) ? scan.remaining : 0;
  const done = Math.max(0, Math.min(total, total - remaining));
  if (!(total > 0)) return { done: 0, total: 0, percent: 0 };
  const percent = remaining === 0 ? 100 : Math.min(100, Math.max(0, Math.floor((done / total) * 100)));
  return { done, total, percent };
}

function failedCount(raw: number | undefined): number {
  if (raw == null || !Number.isFinite(raw) || raw < 0) return 0;
  return Math.floor(raw);
}

function organizedCount(total: number, remaining: number): number {
  if (!Number.isFinite(total) || !Number.isFinite(remaining)) return 0;
  const done = total - remaining;
  if (!Number.isFinite(done) || done < 0) return 0;
  return done;
}

/** 工具箱里「上次全量整理」那一行。没有、或 reason 认不出，就什么都不写。 */
export function lastFullScanText(last: LastFullScan | null | undefined): string | null {
  if (!last) return null;
  const when = formatMinute(last.finishedAt);
  const used = formatTokens(last.tokensUsed);
  const total = Number.isFinite(last.total) ? last.total : 0;
  // 处理过的（含失败）；失败的单独说，别让人以为都整理成功了
  const processed = organizedCount(last.total, last.remaining);
  const failed = Math.min(failedCount(last.failed), processed);
  const ok = processed - failed;
  const failedNote = failed > 0 ? `，另有 ${failed} 个没整理成功` : '';
  if (last.reason === 'completed') {
    const retry = failed > 0 ? '（可以再开一次全量整理重试）' : '';
    return `上次全量整理：${when} 完成，整理了 ${ok} 个${failedNote}${retry}，用了约 ${used} token。`;
  }
  if (last.reason === 'stopped') {
    return `上次全量整理：${when} 被停止，已整理 ${ok} / ${total} 个${failedNote}，用了约 ${used} token。`;
  }
  if (last.reason === 'expired') {
    return `上次全量整理：${when} 到时间自动结束，已整理 ${ok} / ${total} 个${failedNote}，用了约 ${used} token。`;
  }
  if (last.reason === 'ceiling') {
    return `上次全量整理：${when} 因为用量远超预计，已自动停下（已整理 ${ok} / ${total} 个${failedNote}，用了约 ${used} token）。要继续可以再开一次。`;
  }
  if (last.reason === 'failures') {
    return `上次全量整理：${when} 因为失败太多，已自动停下（已整理 ${ok} / ${total} 个${failedNote}，用了约 ${used} token）。先到「设置 → 引擎」检查模型，再重试。`;
  }
  return null;
}

export type FullScanPanel =
  | { kind: 'loading' }
  | { kind: 'unavailable'; reason: string }
  | { kind: 'nothing'; last: string | null }
  | { kind: 'ready'; pending: number; estimate: string; last: string | null }
  | {
      kind: 'running';
      done: number;
      total: number;
      percent: number;
      tokensUsed: number;
      failed: number;
      startedAt: string;
      until: string;
      paused: boolean;
      pausedReason: string | null;
    };

const FULL_SCAN_UNAVAILABLE = '先到「设置 → 引擎」里选好服务商和模型、保存密钥，才能整理。';

/**
 * 工具箱「全量整理」面板该画哪一种。顺序不能换：
 * 没读到 → 正在跑（有 fullScan，且没有被明确标成没在跑）→ 没配服务 → 没有要整理的 → 可以开始。
 * `unlimited === false` 时即使还留着 fullScan 也不算在跑。
 */
export function fullScanPanel(status: AutomationStatus | null): FullScanPanel {
  if (!status) return { kind: 'loading' };
  const scan = status.fullScan;
  if (scan != null && status.unlimited !== false) {
    const progress = fullScanProgress(scan);
    const tokensUsed = Number.isFinite(scan.tokensUsed) ? scan.tokensUsed : 0;
    return {
      kind: 'running',
      done: progress.done,
      total: progress.total,
      percent: progress.percent,
      tokensUsed,
      failed: failedCount(scan.failed),
      startedAt: typeof scan.startedAt === 'string' ? scan.startedAt : '',
      until: typeof scan.until === 'string' ? scan.until : '',
      paused: status.paused === true,
      pausedReason: status.pauseReason?.message ?? null,
    };
  }
  if (!status.providerConfigured) return { kind: 'unavailable', reason: FULL_SCAN_UNAVAILABLE };
  const last = lastFullScanText(status.lastFullScan);
  const pending = fullScanPending(status);
  if (pending <= 0) return { kind: 'nothing', last };
  return {
    kind: 'ready',
    pending,
    estimate: roundedEstimate(estimateFullScanTokens(pending)),
    last,
  };
}

export type AutomationState = {
  label: string;
  /** 状态点和徽章的色相：蓝 220 = 平静，绿 148 = 在工作，黄 36 = 需要你看一眼，红 4 = 被暂停。 */
  hue: number;
  detail: string;
};

/** 自动整理现在处在什么状态：一个标签 + 一句人话。按「最需要你知道的」排优先级。 */
export function automationState(status: AutomationStatus | null): AutomationState {
  if (!status) return { label: '读取中', hue: 220, detail: '' };
  if (!status.providerConfigured) {
    return { label: '还没配置 AI 服务', hue: 36, detail: '先到「设置 → 引擎」里选好服务商和模型，并保存密钥。' };
  }
  if (status.paused) {
    return { label: '已暂停', hue: 4, detail: status.pauseReason?.message ?? '整理被暂停了，点「继续整理」才会接着做。' };
  }
  if (status.unlimited === true) {
    return { label: '全量整理中（不限额）', hue: 148, detail: '在后台连续整理，不受每天额度限制；整理完会自动回到日常的增量整理。' };
  }
  if (!status.enabled) {
    return { label: '未开启', hue: 220, detail: '没有授权额度时，应用不会调用 AI，也不会花钱。' };
  }
  if (status.budgetRemaining <= 0) {
    return { label: '今天的额度用完了', hue: 36, detail: '明天会自动继续，也可以在下面调高每天的额度。' };
  }
  if (status.queued + status.running > 0) {
    return { label: '整理中', hue: 148, detail: '在后台一个个整理，整理好的会出现在「今日」和「资料库」里。' };
  }
  return { label: '已开启，暂时没有要整理的', hue: 148, detail: '有新收藏，或者你点了「排队」，就会自动开始。' };
}

export type AutomationHint = {
  hue: number;
  icon: string;
  title: string;
  detail: string;
  /** 没有动作就不画按钮（比如正在整理中，没什么要你做的） */
  actionLabel: string | null;
};

/**
 * 今日 / 资料库顶上的一行提示：还有 pending 个收藏没有中文说明，为什么，以及你能做什么。
 * 没有要整理的就不打扰；读不到状态也不打扰。
 */
export function automationHint(status: AutomationStatus | null, pending: number): AutomationHint | null {
  if (!status || !Number.isFinite(pending) || pending <= 0) return null;
  const lacking = `还有 ${pending} 个收藏没有中文说明`;
  if (!status.providerConfigured) {
    return { hue: 36, icon: 'key', title: lacking, detail: '还没配置 AI 服务，配置好才能自动整理。', actionLabel: '去看看' };
  }
  if (status.paused) {
    return {
      hue: 4,
      icon: 'pause_circle',
      title: '自动整理被暂停了',
      detail: status.pauseReason?.message ?? '点「继续整理」才会接着做。',
      actionLabel: '去处理',
    };
  }
  if (status.unlimited === true) {
    const used = status.fullScan?.tokensUsed ?? status.tokens24h;
    if (status.queued + status.running > 0) {
      return {
        hue: 148,
        icon: 'autorenew',
        title: `正在全量整理（不限额），还剩 ${pending} 个`,
        detail: `不受每天额度限制，已用约 ${formatTokens(used)} token。整理完会自动回到日常整理。`,
        actionLabel: '查看进度',
      };
    }
    // 这一批已经排完、最后几个在收尾：别落到「它们还没进整理队列」
    return {
      hue: 148,
      icon: 'autorenew',
      title: '正在全量整理（不限额）',
      detail: `队列里这一批已经排完，整理完会自动回到日常整理。已用约 ${formatTokens(used)} token。`,
      actionLabel: '查看进度',
    };
  }
  if (!status.enabled) {
    return {
      hue: 220,
      icon: 'auto_awesome',
      title: lacking,
      detail: '自动整理还没开启，它们先显示 GitHub 上的原文。开启后会在后台逐个整理，每天最多花你设定的额度。',
      actionLabel: '开启自动整理',
    };
  }
  // 走到这里一定不是全量整理（上面已经返回了），所以「额度用完」只指日常额度
  if (status.budgetRemaining <= 0) {
    return {
      hue: 36,
      icon: 'schedule',
      title: '今天的整理额度用完了',
      detail: `还有 ${pending} 个没整理，明天会自动接着做；想现在就整理完，可以在工具箱里开启「全量整理」（不限额）。`,
      actionLabel: '去工具箱',
    };
  }
  if (status.queued + status.running > 0) {
    return {
      hue: 148,
      icon: 'autorenew',
      title: `正在后台整理，还剩 ${pending} 个`,
      detail: `最近 24 小时已用约 ${formatTokens(status.tokens24h)} token，约整理了 ${approxRepos(status.tokens24h)} 个。`,
      actionLabel: null,
    };
  }
  return { hue: 220, icon: 'playlist_add', title: lacking, detail: '它们还没进整理队列。', actionLabel: '去排队' };
}

/** 详情里「还没有中文说明」那一块：这个项目为什么还没整理。 */
export function itemPendingNote(status: AutomationStatus | null): { text: string; actionLabel: string | null } {
  const original = '先看 GitHub 上的原文：';
  if (!status) return { text: `整理好之后，这里会出现中文说明、能做什么、怎么试。${original}`, actionLabel: null };
  if (!status.providerConfigured) return { text: `还没配置 AI 服务，所以没法自动整理。${original}`, actionLabel: '去看看' };
  if (status.paused) {
    const reason = status.pauseReason?.message;
    return { text: `自动整理被暂停了${reason ? `（${reason}）` : ''}。${original}`, actionLabel: '去处理' };
  }
  // 全量整理时不要说「额度用完」。日常排队（unlimited 不是 true）仍用下面原来的「已经在整理队列里了」，原有测试靠这句。
  if (status.unlimited === true) {
    return { text: `正在全量整理，整理好会自动出现在这里。${original}`, actionLabel: null };
  }
  if (!status.enabled) return { text: `自动整理还没开启，所以这里还没有说明。${original}`, actionLabel: '开启自动整理' };
  if (status.budgetRemaining <= 0) {
    return { text: `今天的整理额度用完了，明天会接着整理。也可以在工具箱里开启全量整理。${original}`, actionLabel: '调整额度' };
  }
  if (status.queued + status.running > 0) return { text: `已经在整理队列里了，整理好会自动出现在这里。${original}`, actionLabel: null };
  return { text: `整理好之后，这里会出现中文说明、能做什么、怎么试。${original}`, actionLabel: null };
}

export type RegenerateNote = { title: string; sub?: string; actionLabel: string | null };

/**
 * 点「不对，重新整理」之后的提示。这只是把它排进队列，真正重做要等自动整理在跑、并且还有额度：
 * 没开额度时不能说「整理好会更新」（那是花钱的动作，也是一句不会兑现的承诺）。
 */
export function regenerateNote(status: AutomationStatus | null): RegenerateNote {
  const queued = '已加入整理队列';
  if (!status) return { title: queued, actionLabel: null };
  if (!status.providerConfigured) return { title: queued, sub: '还没配置 AI 服务，配置好才会重新整理', actionLabel: '去看看' };
  if (status.paused) return { title: queued, sub: '自动整理现在是暂停的，继续后才会重新整理', actionLabel: '去处理' };
  if (status.unlimited === true) {
    return { title: queued, sub: '正在全量整理，会在后台重新整理，好了会自动更新', actionLabel: null };
  }
  if (!status.enabled) return { title: queued, sub: '自动整理还没开启，开启后才会重新整理（不开启就不花钱）', actionLabel: '去开启' };
  if (status.budgetRemaining <= 0) {
    return { title: queued, sub: '今天的额度用完了，明天会接着整理，也可以在工具箱里开启全量整理', actionLabel: null };
  }
  return { title: queued, sub: '会在后台重新整理，好了会自动更新', actionLabel: null };
}

export function formatMinute(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function formatSize(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
