export const TOAST_LIMIT = 4;
export const DEFAULT_TOAST_DURATION_MS = 4600;

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

/** 没写时长用 4.6 秒。0 和负数表示不自动消失。 */
export function toastDuration(input: { durationMs?: number }): number {
  if (input.durationMs == null) return DEFAULT_TOAST_DURATION_MS;
  if (!Number.isFinite(input.durationMs) || input.durationMs <= 0) return 0;
  return input.durationMs;
}
