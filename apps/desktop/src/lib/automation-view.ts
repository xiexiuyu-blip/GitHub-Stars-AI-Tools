/**
 * 「自动整理与备份」「设置」「顶栏」「今日 / 资料库横幅」里用到的纯函数：额度档位、一次性全部整理、状态文案、
 * 同步结果、备份时间、运行检查各行、数字和日期格式。不碰网络、不碰界面，方便单测。
 */

export type PauseReason = { kind: string; message: string; at: string };

/** 正在进行的一轮一次性全部整理。后台还没带上这个字段时，整个值是 null。 */
export type FullScan = {
  /** ISO 时间（UTC，带 Z） */
  startedAt: string;
  /** 兜底截止（开始后 48 小时），到点自动结束 */
  until: string;
  /** 开始以来已用 token（含在途的预留） */
  tokensUsed: number;
  /** 开始时要整理的个数 */
  total: number;
  /** 现在还剩（还在等的 + 进行中） */
  remaining: number;
  /** 开始以来失败了几个（旧后台没有这个字段，当 0） */
  failed?: number;
};

/** 上一轮一次性全部整理的结果。reason 认不出就当没有。 */
export type LastFullScan = {
  startedAt: string;
  finishedAt: string;
  tokensUsed: number;
  /** 整理完 / 你点了停止 / 到 48 小时 / 花费远超预计被自动停下 / 失败太多被自动停下 */
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
   * 一次性全部整理正在进行。字段缺失（旧后台）当作 false：用 `=== true` 判断，不要把缺失当成开着。
   * 面板是否显示「进行中」另看 `fullScanPanel`：有 `fullScan` 且这里不是明确的 false 才算在跑。
   */
  unlimited?: boolean;
  /** 缺失当作 null。 */
  fullScan?: FullScan | null;
  /** 缺失当作 null。 */
  lastFullScan?: LastFullScan | null;
  /** 上次 README 更新检查。缺失当作 null，界面不显示那一行。 */
  lastReadmeCheck?: LastReadmeCheck | null;
  /** 还没整理好的收藏数。缺失或 null 时用 queued + running 代替。 */
  needsOrganizing?: number | null;
  /** 有效收藏总数和说明分项。旧后台缺失时，不显示分项。 */
  activeRepos?: number | null;
  needsOrganizingMissing?: number | null;
  needsOrganizingOutdated?: number | null;
  /** 按收藏去重的待整理数（包含 README 变化后重新排队的）。 */
  awaiting?: number | null;
  /** 最近成功整理一张中文说明的平均 token（样本不足时为 null）。 */
  avgTokensPerCard?: number | null;
  /** 每天额度已用，不含「现在整理」；旧后台回退到 tokens24h。 */
  dailyTokensUsed?: number | null;
  /**
   * 最近 24 小时整理成功的个数。**不是** get_ai_automation 的字段：来自现成命令 get_ai_queue_status 的 `succeeded24h`，
   * 由 `withRecentDone` 合进来。读不到就缺失或 null，界面把「整理了 N 个」整句去掉，不拿 token 除一除去估。
   */
  succeeded24h?: number | null;
};

/** get_ai_queue_status 返回的状态里，界面用得到的那一个字段。 */
export type AiQueueStatus = {
  succeeded24h: number;
};

/** 一次 README 更新检查。后台还没带上这个字段时，整个值是 null。 */
export type LastReadmeCheck = {
  checkedAt: string;
  checked: number;
  refreshed: number;
  material: number;
  queued: number;
};

/** 整理一个工具大约用多少 token（2026-10-04 真实记录：输入约 4300 + 输出约 1500，成功的卡平均约 5.7–6.3 千，取 6000）。只用来估「约整理多少个」。 */
export const TOKENS_PER_REPO = 6000;

/** 真实记录：切库后全量整理约 500 张，实测每张约 9 千（含思考）；Rust 的 `FULL_SCAN_TOKENS_PER_CARD` 与前端的 `FULL_SCAN_TOKENS_PER_REPO` 必须一致，改一处要同时改另一处。只用于这一轮的预计花费，不用于每天额度折算。 */
export const FULL_SCAN_TOKENS_PER_REPO = 9000;

/** 额度档位：名字 + 每天约整理多少个。0 是关闭。 */
export const BUDGET_PRESETS: ReadonlyArray<{ tokens: number; title: string; hint: string }> = [
  { tokens: 0, title: '关闭', hint: '只关闭后台自动整理；手动整理、AI 问答等仍可能收费' },
  { tokens: 100_000, title: '试跑', hint: '每 24 小时最多约 16 个，先看看效果' },
  { tokens: 200_000, title: '日常', hint: '每 24 小时最多约 33 个' },
  { tokens: 600_000, title: '多整理些', hint: '每 24 小时最多约 100 个，新收藏很多时用' },
];

export const AUTO_ORGANIZE_DEFINITION = '后台自动给新收藏或说明有更新的项目写中文说明，不重复整理。';
export const AUTO_ORGANIZE_DETAIL = '自动整理：应用在后台给「新收藏」和「项目说明（README）有实质更新的收藏」写（或重写）中文说明——它能做什么、适合解决什么、怎么试。没变化的不会重复整理，也不会重复花钱。整理好的会出现在「今日」和「资料库」里。用的是你自己配置的 AI 服务，具体花多少钱以服务商的账单为准。';
export const DAILY_ORGANIZE_TITLE = '每天自动整理（按滚动 24 小时计额度）';
export const DAILY_ORGANIZE_DESCRIPTION = '第一步做完以后，平时不用管：每次同步到新收藏、或某个收藏的项目说明（README）有实质更新，应用会在后台自动整理它。「每天额度」按最近 24 小时滚动累计，不在零点重置；用完后，随着较早用量移出这个窗口，额度逐步恢复。手动「现在整理」另行确认费用。';
export const DAILY_ORGANIZE_DURING_SCAN = '整理正在进行，这一轮不受每天额度限制；整理完才按下面的额度。';
export const SLOW_ORGANIZE_COPY = {
  title: '只想慢慢整理（可选）',
  description: '这两个只是把它们排进队列，按每天额度慢慢做，不会马上花钱；想马上做完，用上面的「现在整理」。',
  missingLabel: '把还没有说明的排进队列',
  outdatedLabel: '把旧版说明排进队列重写',
};
export const FULL_SCAN_STARTED = '现在整理已开始';
export const FULL_SCAN_STOPPED = '已停止现在整理，剩下的会按每天额度接着整理';

/** 开始整理一个工具前，先按「两次调用都用满」预留的量（Rust 的 `CLAIM_RESERVE_TOKENS`）。额度比它小，一个工具也领不到。 */
export const MIN_DAILY_BUDGET = 12_000;

/** 把输入框里的文字变成额度。合法返回数字，不合法返回给人看的错误提示。 */
export function parseDailyBudget(raw: string): number | string {
  const text = raw.trim();
  if (!text) return '请先填写每天额度';
  if (text.startsWith('-')) return '每天额度不能小于 0';
  if (!/^\d+$/.test(text)) return '每天额度要填整数';
  const value = Number(text);
  if (!Number.isSafeInteger(value)) return '每天额度要填整数';
  if (value > 0 && value < MIN_DAILY_BUDGET) return `每天额度至少要 ${formatTokens(MIN_DAILY_BUDGET)} token，再少连一个工具都整理不了（要关闭就填 0）`;
  return value;
}

/** 这些 token 大约够整理多少个工具。 */
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

/** 一次性全部整理预计要多少 token。优先用近期实测均值，缺省按 9000；个数不是有限数或 ≤ 0 时不估。 */
export function estimateFullScanTokens(count: number, avgTokensPerCard?: number | null): number {
  if (!Number.isFinite(count) || count <= 0) return 0;
  const perCard = avgTokensPerCard != null && Number.isFinite(avgTokensPerCard) && avgTokensPerCard > 0
    ? avgTokensPerCard
    : FULL_SCAN_TOKENS_PER_REPO;
  const tokens = Math.round(count * perCard);
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

/** 现在还有多少个要整理。依次用 awaiting、needsOrganizing、还在等的 + 进行中。 */
export function fullScanPending(status: AutomationStatus): number {
  const raw = status.awaiting ?? status.needsOrganizing ?? status.queued + status.running;
  if (!Number.isFinite(raw) || raw < 0) return 0;
  return raw;
}

function organizeCounts(status: AutomationStatus): { total: number; organized: number; outdated: number; missing: number } | null {
  const { activeRepos: total, needsOrganizingMissing: missing, needsOrganizingOutdated: outdated } = status;
  if (total == null || missing == null || outdated == null || !(total > 0)) return null;
  return { total, missing, outdated, organized: Math.max(0, total - outdated - missing) };
}

/** 旧后台的三个分项不齐时，不猜个数。 */
export function organizeCountsLine(status: AutomationStatus | null): string | null {
  const counts = status ? organizeCounts(status) : null;
  if (!counts) return null;
  const { total, organized, outdated, missing } = counts;
  const note = outdated > 0 ? '有部分说明是早期的简要格式，还能看，但建议更新成完整中文说明，「找方案」和资料库筛选才更准。' : '';
  return `现有 ${total} 个收藏：中文说明已是最新 ${organized} 个 · 中文说明要更新 ${outdated} 个 · 还没有中文说明 ${missing} 个。${note}`;
}

/** 只排队的结果；是否开始花钱仍由队列和授权决定。 */
export function enqueueCardsNote(count: number, unlimited: boolean): string {
  if (!(count > 0)) return '没有还要整理的了。';
  return unlimited
    ? `已排进队列 ${count} 个，整理正在进行，会一起处理。`
    : `已排进队列 ${count} 个，按每天额度慢慢整理；想马上做完，点上面的「现在整理」。`;
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

/** 这一轮一次性全部整理已经整理好几个（处理过的减去失败的）。没有这一轮、或开始个数不是正数，就是 null。 */
function fullScanDoneCount(scan: FullScan | null | undefined): number | null {
  if (scan == null) return null;
  const progress = fullScanProgress(scan);
  if (!(progress.total > 0)) return null;
  return Math.max(0, progress.done - failedCount(scan.failed));
}

/** 「自动整理与备份」里「上次一次性全部整理」那一行。没有、或 reason 认不出，就什么都不写。 */
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
    const retry = failed > 0 ? '（可以再点一次「现在整理」重试）' : '';
    return `上次整理现有收藏：${when} 完成，整理了 ${ok} 个${failedNote}${retry}，用了约 ${used} token。`;
  }
  if (last.reason === 'stopped') {
    return `上次整理现有收藏：${when} 被停止，已整理 ${ok} / ${total} 个${failedNote}，用了约 ${used} token。`;
  }
  if (last.reason === 'expired') {
    return `上次整理现有收藏：${when} 到时间自动结束，已整理 ${ok} / ${total} 个${failedNote}，用了约 ${used} token。`;
  }
  if (last.reason === 'ceiling') {
    return `上次整理现有收藏：${when} 因为花费远超预计，已自动停下（已整理 ${ok} / ${total} 个${failedNote}，用了约 ${used} token）。要继续可以再点一次「现在整理」。`;
  }
  if (last.reason === 'failures') {
    return `上次整理现有收藏：${when} 因为失败太多，已自动停下（已整理 ${ok} / ${total} 个${failedNote}，用了约 ${used} token）。先到「设置 → AI 设置」检查模型，再重试。`;
  }
  return null;
}

function finiteCount(value: number): number {
  if (!Number.isFinite(value) || value < 0) return 0;
  return Math.floor(value);
}

function pad2(value: number): string {
  return String(value).padStart(2, '0');
}

function sameLocalDay(left: Date, right: Date): boolean {
  return left.getFullYear() === right.getFullYear() && left.getMonth() === right.getMonth() && left.getDate() === right.getDate();
}

/** 不到 1 小时、当天、昨天、更早（写成「10 月 3 日」）。时间读不懂就当没有这次检查。 */
function readmeWhen(iso: string, now: Date): string | null {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return null;
  const age = now.getTime() - date.getTime();
  if (age < 60 * 60 * 1000) return '刚刚';
  if (sameLocalDay(date, now)) return `今天 ${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  if (sameLocalDay(date, yesterday)) return `昨天 ${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
  return `${date.getMonth() + 1} 月 ${date.getDate()} 日`;
}

/** 「自动整理与备份」里「项目说明更新检查」那一行。没检查过、或时间读不懂，就什么都不写。 */
export function readmeCheckText(last: LastReadmeCheck | null | undefined, now = new Date()): string | null {
  if (!last) return null;
  const when = readmeWhen(last.checkedAt, now);
  if (!when) return null;
  const checked = finiteCount(last.checked);
  const material = finiteCount(last.material);
  const tail = material > 0 ? `有明显改动 ${material} 个，已加入重新整理` : '没有明显改动';
  return `项目说明更新检查：${when} · 看了 ${checked} 个 · ${tail}`;
}

export type FullScanPanel = { title: string; description: string | null } & (
  | { kind: 'loading' }
  | { kind: 'unavailable'; reason: string }
  | { kind: 'nothing'; last: string | null }
  | { kind: 'ready'; pending: number; estimate: string; last: string | null; body: string; buttonLabel: string }
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
    });

const FULL_SCAN_UNAVAILABLE = '先到「设置 → AI 设置」里选好服务商和模型、保存密钥，才能整理。';

/**
 * 「一次性全部整理」面板该画哪一种。顺序不能换：
 * 没读到 → 正在跑（有 fullScan，且没有被明确标成没在跑）→ 没配服务 → 没有要整理的 → 可以开始。
 * `unlimited === false` 时即使还留着 fullScan 也不算在跑。
 */
export function fullScanPanel(status: AutomationStatus | null): FullScanPanel {
  if (!status) return { kind: 'loading', title: '整理现有收藏', description: null };
  const scan = status.fullScan;
  if (scan != null && status.unlimited !== false) {
    const progress = fullScanProgress(scan);
    const tokensUsed = Number.isFinite(scan.tokensUsed) ? scan.tokensUsed : 0;
    return {
      kind: 'running',
      title: '正在整理（不受每天额度限制）',
      description: '应用在后台一个个整理现有收藏。这一轮不受每天额度限制，整理完自动回到每天自动整理。',
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
  if (!status.providerConfigured) return { kind: 'unavailable', title: '整理现有收藏', description: null, reason: FULL_SCAN_UNAVAILABLE };
  const last = lastFullScanText(status.lastFullScan);
  const pending = fullScanPending(status);
  if (pending <= 0) return {
    kind: 'nothing',
    title: '现有收藏都整理好了',
    description: '以后新收藏、README 有实质更新的，会交给下面的「每天自动整理」。',
    last,
  };
  const counts = organizeCounts(status);
  const firstStep = counts ? counts.organized * 2 < counts.total : status.lastFullScan == null;
  const estimate = roundedEstimate(estimateFullScanTokens(pending, status.avgTokensPerCard));
  return {
    kind: 'ready',
    title: firstStep ? '第一步：把现有收藏整理一遍' : `现在就整理 ${pending} 个`,
    description: firstStep
      ? '只在第一次用、或说明的格式升级之后做一次：一口气把现有收藏整理完，不受每天额度限制。整理完，以后就只剩下面的「每天自动整理」。'
      : '不想等每天自动整理的时候用：这一轮不受每天额度限制，整理完自动回到每天自动整理。',
    body: firstStep
      ? `需要整理 ${pending} 个，预计约 ${estimate} token（具体花多少钱以你服务商的账单为准）。`
      : `有 ${pending} 个收藏在等整理，预计约 ${estimate} token（具体花多少钱以你服务商的账单为准）。`,
    buttonLabel: firstStep ? `现在整理全部 ${pending} 个` : `现在整理 ${pending} 个`,
    pending,
    estimate,
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
    return { label: '还没配置 AI 服务', hue: 36, detail: '先到「设置 → AI 设置」里选好服务商和模型，并保存密钥。' };
  }
  if (status.paused) {
    return { label: '已暂停', hue: 4, detail: status.pauseReason?.message ?? '整理被暂停了，点「继续整理」才会接着做。' };
  }
  if (status.unlimited === true) {
    return { label: '现在整理中', hue: 148, detail: '在后台连续整理，不受每天额度限制；整理完会自动回到每天自动整理。' };
  }
  if (!status.enabled) {
    return { label: '未开启', hue: 220, detail: '后台自动整理已关闭。手动「现在整理」、单张重新整理、AI 问答和连接测试等仍可能产生费用，按各自的确认与额度规则执行。' };
  }
  if (status.budgetRemaining <= 0) {
    const queued = status.queued > 0 ? `；现在有 ${status.queued} 个在排队` : '';
    return { label: '每天额度用完了', hue: 36, detail: `每天自动整理的额度用完了（按最近 24 小时累计），过一阵额度恢复会自动继续${queued}。想现在就整理，点下面的「现在整理」，不受额度限制；也可以在下面调高每天额度。` };
  }
  if (status.queued + status.running > 0) {
    return { label: '整理中', hue: 148, detail: '在后台一个个整理，整理好的会出现在「今日」和「资料库」里。' };
  }
  const pending = fullScanPending(status);
  if (pending > 0) {
    return {
      label: '已开启，后台暂时没有排队的',
      hue: 148,
      detail: `现有收藏里还有 ${pending} 个说明要整理，点下面的「现在整理」一次做完；新收藏、或你点了「加入整理」的，会自动开始。`,
    };
  }
  return { label: '已开启，暂时没有要整理的', hue: 148, detail: '有新收藏，或者你点了「加入整理」，就会自动开始。' };
}

export type AutomationHint = {
  hue: number;
  icon: string;
  title: string;
  detail: string;
  /** 没有动作就不画按钮（比如正在整理中，没什么要你做的） */
  actionLabel: string | null;
  /** 只有未配置 AI 时直达配置页，其余动作去自动整理。 */
  actionTarget?: 'ai-settings';
};

/**
 * 今日 / 资料库顶上的一行提示：还有 pending 个收藏没有中文说明，为什么，以及你能做什么。
 * 没有要整理的就不打扰；读不到状态也不打扰。
 */
export function automationHint(status: AutomationStatus | null, pending: number): AutomationHint | null {
  if (!status || !Number.isFinite(pending) || pending <= 0) return null;
  const lacking = `还有 ${pending} 个收藏没有中文说明`;
  if (!status.providerConfigured) {
    return { hue: 36, icon: 'key', title: lacking, detail: '还没配置 AI 服务，配置好才能自动整理。', actionLabel: '去设置 AI', actionTarget: 'ai-settings' };
  }
  if (status.paused) {
    return {
      hue: 4,
      icon: 'pause_circle',
      title: '自动整理被暂停了',
      detail: status.pauseReason?.message ?? '点「继续整理」才会接着做。',
      actionLabel: '继续整理',
    };
  }
  if (status.unlimited === true) {
    const used = status.fullScan?.tokensUsed ?? status.tokens24h;
    // 这一轮已经整理了几个：用后台给的开始个数和剩余个数算（真数），没有 fullScan 就只说花了多少，不估
    const done = fullScanDoneCount(status.fullScan);
    const spent = done === null
      ? `已用约 ${formatTokens(used)} token`
      : `这一轮已整理 ${done} 个（已用约 ${formatTokens(used)} token）`;
    if (status.queued + status.running > 0) {
      return {
        hue: 148,
        icon: 'autorenew',
        title: `「现在整理」进行中，还剩 ${pending} 个`,
        detail: `不受每天额度限制，${spent}。整理完会自动回到每天自动整理。`,
        actionLabel: '看整理进度',
      };
    }
    // 这一批已经排完、最后几个在收尾：别落到「它们还没进整理队列」
    return {
      hue: 148,
      icon: 'autorenew',
      title: '「现在整理」进行中',
      detail: `这一批已经做完，整理完会自动回到每天自动整理。${spent}。`,
      actionLabel: '看整理进度',
    };
  }
  if (!status.enabled) {
    return {
      hue: 220,
      icon: 'auto_awesome',
      title: lacking,
      detail: '自动整理还没开启，它们先显示 GitHub 上的原文。开启后会在后台逐个整理，额度按最近 24 小时滚动累计。',
      actionLabel: '开启自动整理',
    };
  }
  // 走到这里一定不是一次性全部整理（上面已经返回了），所以「额度用完」只指每天额度
  if (status.budgetRemaining <= 0) {
    return {
      hue: 36,
      icon: 'schedule',
      title: '每天自动整理的额度用完了',
      detail: `还有 ${pending} 个没整理，过一阵额度恢复会自动接着做；想现在就整理，到「设置 → 自动整理」点「现在整理」。`,
      actionLabel: '去整理',
    };
  }
  if (status.queued + status.running > 0) {
    return {
      hue: 148,
      icon: 'autorenew',
      title: `正在后台整理，还剩 ${pending} 个`,
      detail: recentUsageLine(status),
      actionLabel: null,
    };
  }
  return { hue: 220, icon: 'playlist_add', title: lacking, detail: '它们还没加入整理。', actionLabel: '加入整理' };
}

/** 详情里「还没有中文说明」那一块：这个工具为什么还没整理。 */
export function itemPendingNote(status: AutomationStatus | null): { text: string; actionLabel: string | null; actionTarget?: 'ai-settings' } {
  const original = '先看 GitHub 上的原文：';
  if (!status) return { text: `整理好之后，这里会出现中文说明、能做什么、怎么试。${original}`, actionLabel: null };
  if (!status.providerConfigured) return { text: `还没配置 AI 服务，所以没法自动整理。${original}`, actionLabel: '去设置 AI', actionTarget: 'ai-settings' };
  if (status.paused) {
    const reason = status.pauseReason?.message;
    return { text: `自动整理被暂停了${reason ? `（${reason}）` : ''}。${original}`, actionLabel: '继续整理' };
  }
  // 一次性全部整理时不要说「额度用完」。每天自动整理（unlimited 不是 true）仍用下面原来的「已经在整理队列里了」，原有测试靠这句。
  if (status.unlimited === true) {
    return { text: `「现在整理」进行中，整理好会自动出现在这里。${original}`, actionLabel: null };
  }
  if (!status.enabled) return { text: `自动整理还没开启，所以这里还没有说明。${original}`, actionLabel: '开启自动整理' };
  if (status.budgetRemaining <= 0) {
    return { text: `每天自动整理的额度用完了，过一阵会接着整理；也可以到「设置 → 自动整理」点「现在整理」。${original}`, actionLabel: '去整理' };
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
  if (!status.providerConfigured) return { title: queued, sub: '还没配置 AI 服务，配置好才会重新整理', actionLabel: '去设置 AI' };
  if (status.paused) return { title: queued, sub: '自动整理现在是暂停的，继续后才会重新整理', actionLabel: '继续整理' };
  if (status.unlimited === true) {
    return { title: queued, sub: '「现在整理」进行中，会在后台重新整理，好了会自动更新', actionLabel: null };
  }
  if (!status.enabled) return { title: queued, sub: '自动整理还没开启，开启后才会重新整理（这部分不开启就不收费）', actionLabel: '去开启' };
  if (status.budgetRemaining <= 0) {
    return { title: queued, sub: '每天自动整理的额度用完了，过一阵会接着整理，也可以到「设置 → 自动整理」点「现在整理」', actionLabel: null };
  }
  return { title: queued, sub: '会在后台重新整理，好了会自动更新', actionLabel: null };
}

/** 给人看的时间：`10 月 3 日 18:17`。读不懂就原样返回。 */
export function formatMinute(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getMonth() + 1} 月 ${date.getDate()} 日 ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export type SyncCounts = {
  activeCount: number;
  createdCount: number;
  removedCount: number;
  /** 后端的 updatedCount 是「这次同步里已有的收藏的全部个数」，不是「信息变了的个数」，所以界面一个字都不用它。 */
  updatedCount?: number;
};

/** 顶栏「上次同步」。没有、或时间读不懂，就写还没同步过。 */
export function formatSyncStatus(iso: string | null | undefined, now = new Date()): string {
  if (!iso) return '还没同步过';
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '还没同步过';
  const clock = `${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
  if (sameLocalDay(date, now)) return `上次同步：今天 ${clock}`;
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  if (sameLocalDay(date, yesterday)) return `上次同步：昨天 ${clock}`;
  return `上次同步：${date.getMonth() + 1} 月 ${date.getDate()} 日 ${clock}`;
}

/** 同步完成那一句。个数照后台给的来，不另估；不写「更新了 N 个」（后台没有逐个比对，那个数是已有收藏的全部个数）。 */
export function formatSyncComplete(summary: SyncCounts): string {
  const active = finiteCount(summary.activeCount);
  const created = finiteCount(summary.createdCount);
  const removed = finiteCount(summary.removedCount);
  return `同步完成：你现在共收藏 ${active} 个，新增 ${created} 个，有 ${removed} 个是你在 GitHub 上取消收藏的（已从资料库隐藏）。`;
}

/** 一次性全部整理的确认框，固定三句。estimate 是已经取整的 token 文字，例如「250 万」。 */
export function fullScanConfirmLines(pending: number, estimate: string): [string, string, string] {
  const count = finiteCount(pending);
  const tokens = estimate.trim() || '0';
  return [
    `需要整理 ${count} 个，预计约 ${tokens} token（具体花多少钱以你服务商的账单为准）。`,
    '点确认后，应用在后台一直整理，不受每天额度限制，最多跑 48 小时，你随时可以点「停止」。',
    '服务商拒绝请求、失败太多，或花费超过预计的 4 倍时会自动停下。',
  ];
}

/** 每天额度确认框的第一句。N 是预计能整理的个数，不是已经整理的个数。 */
export function dailyBudgetConfirmLead(tokens: number): string {
  return `每 24 小时最多整理约 ${approxRepos(tokens)} 个`;
}

/** 「最近 24 小时」那几句话要用到的字段。`succeeded24h` 不在 get_ai_automation 里，见 `withRecentDone`。 */
export type RecentUsage = Pick<AutomationStatus, 'tokens24h' | 'dailyTokensUsed' | 'dailyBudgetTokens' | 'unlimited' | 'lastFullScan'> & {
  succeeded24h?: number | null;
};

const EARLIER_FULL_SCAN_NOTE = '这是之前整理现有收藏用的，不占每天额度';
const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * 把 get_ai_queue_status 里的 `succeeded24h` 合进自动整理状态。队列状态读不到（null）就带 null，状态本身照常用。
 * 不新增后端字段：两个命令都是现成的。
 */
export function withRecentDone(status: AutomationStatus, queue: AiQueueStatus | null | undefined): AutomationStatus {
  return { ...status, succeeded24h: queue ? recentDoneCount(queue.succeeded24h) : null };
}

/** 最近 24 小时整理成功的个数。缺失、null、不是有限数、小于 0 都当「读不到」。 */
export function recentDoneCount(value: number | null | undefined): number | null {
  if (value == null || !Number.isFinite(value) || value < 0) return null;
  return Math.floor(value);
}

/**
 * 最近 24 小时的花费能不能说成「这是之前一次性整理用的」。只在有证据时才说：
 * 现在没有一次性整理在跑、没开每天额度，而且上一轮一次性整理在最近 24 小时内结束过。
 * 不能看 `enabled`：暂停时它也是 false，但暂停不代表用过一次性整理。
 */
export function spentByEarlierFullScan(status: Pick<RecentUsage, 'tokens24h' | 'dailyBudgetTokens' | 'unlimited' | 'lastFullScan'>, now = new Date()): boolean {
  if (status.unlimited === true) return false;
  if (!(status.tokens24h > 0)) return false;
  if (status.dailyBudgetTokens > 0) return false;
  const finishedAt = status.lastFullScan?.finishedAt;
  if (!finishedAt) return false;
  const finished = new Date(finishedAt).getTime();
  if (Number.isNaN(finished)) return false;
  return now.getTime() - finished <= DAY_MS;
}

/** 最近 24 小时整理现有收藏用的 token。两个用量都可靠、每天用量非负且差值至少 1 万时才说。 */
export function fullScanSpendIn24h(status: Pick<RecentUsage, 'tokens24h' | 'dailyTokensUsed'>): number {
  const { tokens24h, dailyTokensUsed } = status;
  if (!Number.isFinite(tokens24h) || dailyTokensUsed == null || !Number.isFinite(dailyTokensUsed) || dailyTokensUsed < 0) return 0;
  const spent = tokens24h - dailyTokensUsed;
  return spent >= 10_000 ? spent : 0;
}

/** 先保留上一轮整理的原提示，再根据后台用量差值说明不占每天额度的部分。 */
function recentUsageNote(status: RecentUsage, now: Date): string | null {
  if (spentByEarlierFullScan(status, now)) return EARLIER_FULL_SCAN_NOTE;
  const spent = fullScanSpendIn24h(status);
  return spent > 0 ? `其中约 ${formatTokens(spent)} token 是整理现有收藏用的，不占每天额度` : null;
}

/**
 * 今日 / 资料库横幅里「最近 24 小时」那一句：整理了几个是后台算好的真数，token 放进括号。
 * 读不到个数时整句不写个数，只说花了多少 token，不拿 token 除一除去估。
 */
export function recentUsageLine(status: RecentUsage, now = new Date()): string {
  const tokens = formatTokens(status.tokens24h);
  const done = recentDoneCount(status.succeeded24h);
  const note = recentUsageNote(status, now);
  if (done !== null) {
    return `最近 24 小时整理了 ${done} 个（已用约 ${tokens} token，含失败重试${note ? `；${note}` : ''}）。`;
  }
  return note ? `最近 24 小时已用约 ${tokens} token（${note}）。` : `最近 24 小时已用约 ${tokens} token。`;
}

/** 自动整理里「24 小时内」那一格：知道整理了几个就写个数，token 放到下面一行；不知道就只写 token。 */
export function recentUsageMetric(status: RecentUsage, now = new Date()): { label: string; value: string; sub: string } {
  const tokens = formatTokens(status.tokens24h);
  const done = recentDoneCount(status.succeeded24h);
  const note = recentUsageNote(status, now);
  if (done !== null) {
    return {
      label: '24 小时内整理了',
      value: `${done} 个`,
      sub: `已用约 ${tokens} token，含失败重试${note ? `；${note}` : ''}`,
    };
  }
  return { label: '24 小时内已用', value: `${tokens} token`, sub: note ?? '' };
}

/** 自动整理额度那一格。手动整理的用量不从每天额度里扣。 */
export function dailyBudgetMetric(status: AutomationStatus): { label: string; value: string; sub: string } {
  const label = '今天额度还剩';
  if (status.unlimited === true) return { label, value: '不受每天额度限制', sub: '正在整理中' };
  if (!status.enabled) return { label, value: '—', sub: '没有开启每天自动整理' };
  const used = formatTokens(status.dailyTokensUsed ?? status.tokens24h);
  return {
    label,
    value: `${formatTokens(status.budgetRemaining)} token`,
    sub: `约还能整理 ${approxRepos(status.budgetRemaining)} 个 · 最近 24 小时已用 ${used} / 额度 ${formatTokens(status.dailyBudgetTokens)}（整理现有收藏用的不算）`,
  };
}

/** 「运行状态」那一行。用后台算好的「还没解决的失败数」；没读到就不下结论。 */
export function runStatusText(unresolvedFailures: number | null | undefined): string {
  if (unresolvedFailures == null || !Number.isFinite(unresolvedFailures) || unresolvedFailures < 0) return '还没读到运行状态';
  const count = Math.floor(unresolvedFailures);
  return count > 0 ? `有 ${count} 个任务出错` : '运行正常';
}

/** 导出完成的提示。文件固定在「下载」文件夹。 */
export function exportSavedText(count: number, fileName: string): string {
  return `已导出 ${finiteCount(count)} 个工具到「下载」文件夹：${fileName}`;
}

/** 「导出成文档 / 表格」区块下面的说明：存在哪、同名会怎样，以及表格用 Excel 打开中文不会乱码（表格文件开头带了编码标记）。 */
export const EXPORT_SECTION_HINT = '把资料库导成文件，方便存档或给别的工具读。文件会保存在「下载」文件夹里，同名的会被覆盖。导出的表格用 Excel 打开中文不会乱码。';

/** 本机备份的时间行。没有备份、或时间读不懂，就说还没有。 */
export function backupTimeLine(lastAutoAt: string | null | undefined): string {
  const tail = '应用开着时，每隔约 20 小时自动备一次。';
  if (!lastAutoAt) return `还没有备份。${tail}`;
  const when = formatMinute(lastAutoAt);
  if (when === lastAutoAt) return `还没有备份。${tail}`;
  return `上次备份：${when}。${tail}`;
}

const BACKUP_KIND_LABEL: Record<string, string> = {
  auto: '自动',
  manual: '手动',
  preRestore: '恢复前自动留的',
  preMigrate: '升级前自动留的',
};

/** 备份类型给人看的名字。认不出就原样返回，避免编一个。 */
export function backupKindLabel(kind: string): string {
  return BACKUP_KIND_LABEL[kind] ?? kind;
}

export function formatSize(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/* ───────── 设置页：运行检查 ───────── */

export type ReadinessInput = {
  connected: boolean;
  login: string | null;
  /** 资料库里的收藏个数（get_dashboard_stats 的 totalRepos） */
  totalRepos: number;
  /** 已下载的项目说明个数 */
  totalReadmes: number;
  /** 已经写好的中文说明个数 */
  totalAiSummaries: number;
  /** getAiConfigMessage 的结果：null 表示 AI 已经设置完整 */
  aiConfigMessage: string | null;
  aiModel: string;
  syncing: boolean;
  fetchingReadmes: boolean;
  writingCards: boolean;
};

export type ReadinessRow = {
  key: 'github' | 'sync' | 'readme' | 'ai';
  title: string;
  done: boolean;
  detail: string;
  actionLabel: string | null;
  /** 点按钮该做什么；null 表示这一行没有按钮 */
  action: 'sync' | 'readme' | 'writeCards' | 'openAiSettings' | null;
  actionDisabled: boolean;
  /** 按钮对应的事正在做，按钮前面转圈 */
  busy: boolean;
};

/**
 * 「运行检查」上面那四行清单：每行都读真实数据（资料库里有几个收藏、AI 设置的是哪个模型），不写死「连接后执行同步」这种和现状不符的话。
 * 标签网络那一行已经去掉：那个功能的页面在新导航里没有了，点了只花钱、看不到结果。
 */
export function buildReadinessRows(input: ReadinessInput): ReadinessRow[] {
  const repos = finiteCount(input.totalRepos);
  const readmes = finiteCount(input.totalReadmes);
  const cards = finiteCount(input.totalAiSummaries);
  const hasRepos = repos > 0;
  const readmeNeedsTopUp = input.connected && hasRepos && readmes < repos;
  const aiReady = !input.aiConfigMessage;
  const cardsNeedTopUp = input.connected && hasRepos && (cards === 0 || (readmes > 0 && cards < readmes));
  const busy = input.syncing || input.fetchingReadmes || input.writingCards;
  const model = input.aiModel.trim() || '还没填模型名';
  return [
    {
      key: 'github',
      title: 'GitHub 连接',
      done: input.connected,
      detail: input.connected && input.login ? `已连接 @${input.login}` : '还没连接。先在下面粘贴令牌，再点「连接 GitHub」。',
      actionLabel: null,
      action: null,
      actionDisabled: false,
      busy: false,
    },
    {
      key: 'sync',
      title: '同步我的收藏',
      done: hasRepos,
      detail: hasRepos ? `已同步：资料库里有 ${repos} 个收藏` : '还没同步。连接后点「同步我的收藏」。',
      actionLabel: input.syncing ? '同步中…' : '同步我的收藏',
      action: input.connected ? 'sync' : null,
      actionDisabled: input.syncing,
      busy: input.syncing,
    },
    {
      key: 'readme',
      title: '项目说明',
      done: readmes > 0,
      detail: readmes > 0
        ? `已下载 ${readmes} / ${Math.max(repos, readmes)} 个项目说明${readmeNeedsTopUp ? '，还差的可以继续补' : ''}`
        : '同步后先下载项目说明，AI 写中文说明要读它',
      actionLabel: input.fetchingReadmes ? '下载中…' : readmes > 0 ? '补下载还没有的项目说明' : '下载项目说明',
      action: readmeNeedsTopUp ? 'readme' : null,
      actionDisabled: busy,
      busy: input.fetchingReadmes,
    },
    {
      key: 'ai',
      title: 'AI 设置',
      done: aiReady,
      detail: aiReady
        ? `AI 已设置：${model}${cards > 0 ? `；已写好 ${cards} 个中文说明` : ''}${cardsNeedTopUp ? '。给还没有说明的收藏写中文说明会花钱，受「设置 → 自动整理」里的每天额度限制' : ''}`
        : input.aiConfigMessage ?? '还没设置 AI',
      actionLabel: aiReady
        ? input.writingCards ? '整理中…' : cards > 0 ? '继续写还没有的中文说明' : '给没有说明的收藏写中文说明'
        : '去设置 AI',
      action: aiReady ? (cardsNeedTopUp ? 'writeCards' : null) : 'openAiSettings',
      actionDisabled: busy,
      busy: input.writingCards,
    },
  ];
}

/** 「运行检查」下面那一行小字。检查只会真的调用一次 AI（后台 check_runtime_readiness 里的连接测试），要说清会花一点钱。 */
export const RUNTIME_CHECK_COST_NOTE = '如果已经设置了 AI，检查时会真实调用一次 AI，会产生很少的费用。';

/**
 * 「运行检查」检查结果里每一项的名字。
 * 后台只检查前六项，其中「AI 服务」是整个检查里唯一会调用 AI 的一项。
 */
export const RUNTIME_CHECK_TITLES = {
  storage: '资料库文件',
  settings: '应用设置',
  github: 'GitHub 令牌',
  stars: '读取我的收藏',
  readme: '下载项目说明',
  ai: 'AI 服务',
} as const;

/* ───────── 设置页：GitHub 令牌 / 钥匙串（欢迎引导和设置共用） ───────── */

/** 令牌是什么：一句完整的话。设置页的 GitHub 页只在最上面说一遍，下面的几句不再重复它。 */
export const GITHUB_TOKEN_EXPLAINER = '令牌是 GitHub 给你的一串密码，应用用它读你的收藏。';
/** 创建令牌时勾什么权限：repo / gist 各是干什么用的，写在括号里。设置页和欢迎引导里的「怎么拿令牌」共用这一处。 */
export const GITHUB_TOKEN_EXTRA_SCOPES = '勾「repo」（读你的私有收藏）和「gist」（上传备份用）';
export const GITHUB_TOKEN_SCOPES = `创建令牌时，只读公开收藏的话一个权限都不用勾；还需要的话再${GITHUB_TOKEN_EXTRA_SCOPES}。`;
/** 已经连接上之后，令牌区域那一句。令牌是什么上面已经说过，这里不重复。 */
export const GITHUB_TOKEN_SAVED_NOTE = '令牌已保存在 Mac 的钥匙串里，界面不会显示它。';
/** 还没连接时（欢迎引导里），输入框下面那一句。这里是第一次提到令牌，所以带着它是什么。 */
export const GITHUB_TOKEN_WILL_SAVE_NOTE = `${GITHUB_TOKEN_EXPLAINER}它只会保存在 Mac 的钥匙串里，不会上传到任何服务器。`;
export const CONNECT_VERIFYING = '正在验证 GitHub 令牌…';
export const CONNECT_SAVING_KEYCHAIN = '正在把令牌存进钥匙串。如果弹出授权框，请点「允许」。';
export const CONNECT_SLOW = 'GitHub 这会儿比较慢，请再等一会儿';
export const CONNECT_SLOW_WELCOME = 'GitHub 这会儿比较慢，请再等一会儿，或先进入，连接会在后台继续';

/* ───────── 设置页：通用 ───────── */

export const WELCOME_STARTUP_HINT = '没连接 GitHub 时，启动后显示欢迎引导。';
export const SYNC_INTERVAL_HINT = '可填 5～1440 分钟。太频繁会被 GitHub 限制，一般不用改。';

/** 「自动同步」下面那一句：把设置里的间隔写进去，不让人去猜「按设定间隔」是多久。 */
export function autoSyncDescription(minutes: number): string {
  const value = Number.isFinite(minutes) && minutes > 0 ? Math.floor(minutes) : 30;
  return `打开应用时先同步一次，之后每隔 ${value} 分钟再同步一次。`;
}

/** 「清空本机数据」区块和确认框里的说明：列出现在真会丢的东西，说清 GitHub 上的收藏不受影响，提醒先备份。 */
export const CLEAR_LOCAL_DATA_WARNING = '会清空这台电脑上的全部资料：你标的「想试 / 在用」、收集箱里的链接、已整理好的中文说明（重做要重新花钱）、GitHub 令牌和 AI 密钥。你在 GitHub 上的收藏不受影响。建议先到「设置 → 备份与导出」点「现在备份」。清空后应用回到刚安装的样子。';
export const CLEAR_LOCAL_DATA_DONE = '本机数据已清空。重新连接 GitHub 后，点「同步我的收藏」就能重新开始。';
export const CLEAR_LOCAL_DATA_FAILED = '没能清空。你在 GitHub 上的收藏不受影响。再点一次「清空本机数据」；还不行就重启应用，再不行把这条截图发给开发者。';

/* ───────── 设置页：上传备份到 GitHub（私密 Gist） ───────── */

export const GIST_SECTION_TITLE = '上传备份到 GitHub（私密 Gist）';
export const GIST_NEEDS_SCOPE = '请先连接 GitHub 账号。上传备份用的 GitHub 令牌要勾选「gist」这一项：到 GitHub 的 Settings → Developer settings → Personal access tokens 生成或修改，再回到「账号与同步」重新粘贴。';
export const GIST_BACKUP_MARKS = {
  title: '备份我的标记',
  body: '把你标的「想试 / 在用 / 不要了」、标签和笔记上传到你 GitHub 账号下的一个私密 Gist（别人看不到）。',
};
export const GIST_BACKUP_LIBRARY = {
  title: '备份整个清单',
  body: '同上，再加上收藏清单；不含中文说明、AI 密钥和本机设置。',
};
/** 导入区一直显示的警告，不等填了 Gist ID 才出现。 */
export const GIST_IMPORT_WARNING = '导入会覆盖这台电脑上同一工具的标记，请确认 Gist ID 是你自己上传的。';

/** 导入按钮下面随状态变的提示。可以点了就不再多说（警告一直在上面）。 */
export function gistImportHint(connected: boolean, hasGistId: boolean): string | null {
  if (!connected) return '请先连接 GitHub 账号后再导入。';
  if (!hasGistId) return '输入 Gist ID 后才能导入。';
  return null;
}

/* ───────── 欢迎引导 ───────── */

export const WELCOME_COPY = {
  titleLines: ['让你的每一份收藏，', '都变成找得到、看得懂的工具'],
  subtitle: '把你在 GitHub 上收藏的项目同步到这台电脑；需要时再让 AI 写中文说明（按服务商价格收费，默认不开）',
  features: [
    { title: '本地优先', description: '你的收藏只保存在这台电脑上' },
    { title: '快速查找', description: '搜名字、说明或关键词，也能按场景筛选' },
    { title: '中文说明', description: '需要时让 AI 写，按服务商价格收费，默认不开' },
  ],
  connectTitle: '连接 GitHub',
  connectIntro: '需要一个令牌，才能把你的收藏同步过来',
  tokenHowTo: '如何获取令牌？',
  /** 令牌怎么拿：每一步一行，第 1 步里的「这个链接」由页面做成链接。 */
  tokenSteps: [
    '点这个链接',
    '点 Generate new token',
    '名字随便填',
    `权限一个都不用勾（只读公开收藏）；还需要的话再${GITHUB_TOKEN_EXTRA_SCOPES}`,
    '复制',
  ],
  tokenPlaceholder: '粘贴 GitHub 令牌',
  syncTitle: '同步我的收藏',
  syncIntro: '首次同步可能需要几分钟，请耐心等待',
  syncBody: '连接已完成，接下来把你在 GitHub 上收藏的项目同步到这台电脑，并下载每个工具的项目说明（README）。项目说明是 AI 写中文说明时要读的资料；要不要让 AI 写、每天最多花多少，之后到「设置 → 自动整理」里设（默认不开）。',
  syncing: '收藏已同步，正在下载项目说明…',
  syncPartial: '收藏已同步，但项目说明没有全部下载完。你的资料都还在。进入后可以到「设置 → 账号与同步」再点「下载项目说明」。',
  completeTitle: '一切就绪',
  completeIntro: '你的收藏已经同步完成，现在可以开始用了',
  tips: [
    '在「资料库」按场景找工具',
    '在「找方案」说一句想做的事，挑工具交给 Codex',
    '在「今日」看新收藏',
  ],
  enterFailed: '没能进入。你的资料都还在。点「进入工作台」再试一次；还不行就重启应用，再不行把这条截图发给开发者。',
} as const;
