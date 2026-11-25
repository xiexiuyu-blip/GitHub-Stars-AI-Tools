import { useEffect, useRef } from 'react';

export type ListKeyTarget = {
  tagName?: string;
  isContentEditable?: boolean;
  metaKey?: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
} | null | undefined;

/**
 * j / ArrowDown 向下、k / ArrowUp 向上，到头不绕圈。
 * 当前没有选中、或不在列表里时，选第一个。列表为空返回 null。其他键返回 null。
 */
export function nextSelection(ids: readonly string[], currentId: string | null, key: string): string | null {
  if (ids.length === 0) return null;
  const normalized = key === 'ArrowDown' ? 'j' : key === 'ArrowUp' ? 'k' : key.toLowerCase();
  if (normalized !== 'j' && normalized !== 'k') return null;
  const index = currentId ? ids.indexOf(currentId) : -1;
  if (index < 0) return ids[0] ?? null;
  if (normalized === 'j') return ids[Math.min(ids.length - 1, index + 1)] ?? null;
  return ids[Math.max(0, index - 1)] ?? null;
}

/** 焦点在输入控件里，或按着 ⌘ / Ctrl / Alt 时，不处理列表快捷键。 */
export function shouldIgnoreKey(target: ListKeyTarget): boolean {
  if (!target) return false;
  if (target.metaKey || target.ctrlKey || target.altKey) return true;
  const tag = target.tagName?.toLowerCase() ?? '';
  if (tag === 'input' || tag === 'textarea' || tag === 'select') return true;
  return Boolean(target.isContentEditable);
}

type ListKeyOptions = {
  ids: readonly string[];
  currentId: string | null;
  enabled?: boolean;
  onMove: (id: string) => void;
  onStatus: (group: 'want' | 'dropped') => void;
  onCopy: () => void;
};

/** 监听 J/K 移动、T 想试、X 不要了、C 复制。输入框和修饰键留给浏览器。 */
export function useListKeys(options: ListKeyOptions) {
  const optionsRef = useRef(options);
  optionsRef.current = options;

  useEffect(() => {
    if (options.enabled === false) return undefined;

    function onKeyDown(event: KeyboardEvent) {
      if (event.isComposing) return;
      const current = optionsRef.current;
      const element = event.target instanceof Element ? event.target : null;
      if (shouldIgnoreKey({
        tagName: element?.tagName,
        isContentEditable: element instanceof HTMLElement ? element.isContentEditable : false,
        metaKey: event.metaKey,
        ctrlKey: event.ctrlKey,
        altKey: event.altKey,
      })) {
        return;
      }

      const key = event.key.toLowerCase();
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp' || key === 'j' || key === 'k') {
        const next = nextSelection(current.ids, current.currentId, event.key);
        if (!next) return;
        event.preventDefault();
        current.onMove(next);
        return;
      }
      if (!current.currentId) return;
      if (key === 't') {
        event.preventDefault();
        current.onStatus('want');
      } else if (key === 'x') {
        event.preventDefault();
        current.onStatus('dropped');
      } else if (key === 'c') {
        event.preventDefault();
        current.onCopy();
      }
    }

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [options.enabled]);
}
