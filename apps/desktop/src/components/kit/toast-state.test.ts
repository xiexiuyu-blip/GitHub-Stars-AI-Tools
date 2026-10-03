import { describe, expect, it } from 'vitest';
import {
  DEFAULT_TOAST_DURATION_MS,
  pushToast,
  removeToast,
  toastDuration,
  type ToastRecord,
} from './toast-state';

function toast(id: string): ToastRecord {
  return { id, title: id };
}

describe('toast-state', () => {
  it('不到 4 条时按加入顺序保留', () => {
    const next = pushToast([toast('a')], toast('b'));
    expect(next.map((item) => item.id)).toEqual(['a', 'b']);
  });

  it('第 5 条挤掉最旧的一条', () => {
    let items: ToastRecord[] = [];
    for (const id of ['a', 'b', 'c', 'd', 'e']) {
      items = pushToast(items, toast(id));
    }
    expect(items.map((item) => item.id)).toEqual(['b', 'c', 'd', 'e']);
  });

  it('按 id 移除，没有这条时原样返回', () => {
    const items = [toast('a'), toast('b')];
    expect(removeToast(items, 'a').map((item) => item.id)).toEqual(['b']);
    expect(removeToast(items, 'missing')).toEqual(items);
  });

  it('默认 4.6 秒，0 和负数不自动关闭', () => {
    expect(toastDuration({})).toBe(DEFAULT_TOAST_DURATION_MS);
    expect(DEFAULT_TOAST_DURATION_MS).toBe(4600);
    expect(toastDuration({ durationMs: 0 })).toBe(0);
    expect(toastDuration({ durationMs: -5 })).toBe(0);
    expect(toastDuration({ durationMs: Number.NaN })).toBe(0);
    expect(toastDuration({ durationMs: 1200 })).toBe(1200);
  });
});
