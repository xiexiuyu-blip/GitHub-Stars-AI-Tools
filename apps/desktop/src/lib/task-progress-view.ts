import type { TaskProgressEvent } from '@/types';

/**
 * 后台任务条上的「正在做什么」。不依赖 React，方便在 node 里测。
 *
 * 后台报的阶段是内部代号（fetch / parse / save…），任务条上原来写成「阶段：请求数据」「已完成 3/12 批」，都是开发话。
 * 现在写成一句：「正在做：读取收藏…（第 3 步，共 12 步）」。
 */

/** 阶段代号 → 「正在做：」后面接的话。认不出的阶段不显示（不把英文代号露给人看）。 */
const DOING_LABELS: Record<string, string> = {
  auth: '验证账号',
  queue: '排队等候',
  request: '准备',
  prepare: '做准备',
  batch: '批量处理',
  check: '检查收藏',
  plan: '安排任务',
  fetch: '读取收藏',
  'fetch-readme': '下载项目说明',
  'github-search': '去 GitHub 查找',
  'github-star': '更新收藏',
  parse: '整理读到的内容',
  save: '保存到这台电脑',
  summarize: '写中文说明',
  analyze: '写中文说明',
};

/** 「正在做：」后面接的话；这个阶段没有对应的说法（包括 done / error 这类已经结束的）就是 null。 */
export function taskDoingLabel(stage: string | null | undefined): string | null {
  const key = stage?.trim();
  if (!key) return null;
  return DOING_LABELS[key] ?? null;
}

/**
 * 「第 3 步，共 12 步」。总数不知道（0 或没有）就是 null；还没开始（第 0 步）只说「共 12 步」。
 * 数字超过总数时按总数算，不写「第 13 步，共 12 步」。
 */
export function taskStepText(current: number, total: number): string | null {
  if (!Number.isFinite(total) || total <= 0) return null;
  const whole = Math.floor(total);
  if (whole <= 0) return null;
  const step = Number.isFinite(current) ? Math.min(Math.max(0, Math.floor(current)), whole) : 0;
  return step <= 0 ? `共 ${whole} 步` : `第 ${step} 步，共 ${whole} 步`;
}

/**
 * 任务条上标出「正在做什么」的那一小条：「正在做：读取收藏…（第 3 步，共 12 步）」。
 * 只有任务还在跑（`running`）才有；已经结束的任务，标题已经写了「任务完成 / 任务失败 / 任务部分完成」，不再重复一条阶段。
 * 阶段没有对应的说法时，只写步数：「第 3 步，共 12 步」。两样都没有就是 null。
 */
export function taskDoingText(progress: Pick<TaskProgressEvent, 'status' | 'stage' | 'current' | 'total'>): string | null {
  if (progress.status !== 'running') return null;
  const label = taskDoingLabel(progress.stage);
  const step = taskStepText(progress.current, progress.total);
  if (label) return step ? `正在做：${label}…（${step}）` : `正在做：${label}…`;
  return step;
}

/** 进度条右下角的百分比：「25%」。总数不知道就是 null（页面写「正在处理…」）。 */
export function taskPercentText(progress: Pick<TaskProgressEvent, 'current' | 'total'>): string | null {
  if (!Number.isFinite(progress.total) || progress.total <= 0) return null;
  const done = Number.isFinite(progress.current) ? Math.min(Math.max(0, progress.current), progress.total) : 0;
  return `${Math.min(100, Math.round((done / progress.total) * 100))}%`;
}
