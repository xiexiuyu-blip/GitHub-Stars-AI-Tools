export const TOAST_LIMIT = 4;
export const DEFAULT_TOAST_DURATION_MS = 4600;
/** 带「撤销」之类按钮的提示要留得久一点：来得及看清、移过去点（实测 4.6 秒经常点不到）。 */
export const ACTION_TOAST_DURATION_MS = 9000;
/** 鼠标移开后再留多久。 */
export const TOAST_HOVER_GRACE_MS = 2500;

export type ToastAction = {
  label: string;
  onClick: () => void;
};

export type ToastInput = {
  title: string;
  sub?: string;
  actions?: ToastAction[];
  durationMs?: number;
};

export type ToastRecord = ToastInput & {
  id: string;
};

/** 新提示放在最后。超过 4 条时丢掉最旧的。 */
export function pushToast(items: readonly ToastRecord[], toast: ToastRecord): ToastRecord[] {
  const next = [...items, toast];
  if (next.length <= TOAST_LIMIT) return next;
  return next.slice(next.length - TOAST_LIMIT);
}

export function removeToast(items: readonly ToastRecord[], id: string): ToastRecord[] {
  return items.filter((item) => item.id !== id);
}

/** 没写时长：普通提示 4.6 秒，带按钮的 9 秒。0 和负数表示不自动消失。 */
export function toastDuration(input: { durationMs?: number; actions?: readonly unknown[] }): number {
  if (input.durationMs == null) return input.actions && input.actions.length > 0 ? ACTION_TOAST_DURATION_MS : DEFAULT_TOAST_DURATION_MS;
  if (!Number.isFinite(input.durationMs) || input.durationMs <= 0) return 0;
  return input.durationMs;
}
