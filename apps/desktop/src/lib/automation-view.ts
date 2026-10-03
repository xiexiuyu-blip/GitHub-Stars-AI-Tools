/** 工具箱里「AI 自动整理」那一块用到的纯函数：额度档位、状态文案、数字格式。不碰网络、不碰界面，方便单测。 */

export type PauseReason = { kind: string; message: string; at: string };

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
};

/** 整理一个仓库大约用多少 token（输入约 3000 + 输出约 600，取整到 4000）。只用来估「约整理多少个」。 */
export const TOKENS_PER_REPO = 4000;

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
  if (!status.enabled) {
    return {
      hue: 220,
      icon: 'auto_awesome',
      title: lacking,
      detail: '自动整理还没开启，它们先显示 GitHub 上的原文。开启后会在后台逐个整理，每天最多花你设定的额度。',
      actionLabel: '开启自动整理',
    };
  }
  if (status.budgetRemaining <= 0) {
    return { hue: 36, icon: 'schedule', title: '今天的整理额度用完了', detail: `还有 ${pending} 个没整理，明天会自动接着做。`, actionLabel: '调整额度' };
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
  if (!status.enabled) return { text: `自动整理还没开启，所以这里还没有说明。${original}`, actionLabel: '开启自动整理' };
  if (status.budgetRemaining <= 0) return { text: `今天的整理额度用完了，明天会接着整理。${original}`, actionLabel: '调整额度' };
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
  if (!status.enabled) return { title: queued, sub: '自动整理还没开启，开启后才会重新整理（不开启就不花钱）', actionLabel: '去开启' };
  if (status.budgetRemaining <= 0) return { title: queued, sub: '今天的额度用完了，明天会接着整理', actionLabel: null };
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
