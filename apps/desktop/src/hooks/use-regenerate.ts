import { useCallback } from 'react';
import { useToast } from '@/components/kit';
import { describeInvokeError, getAiAutomation, regenerateLibraryCard } from '@/lib/api';
import { regenerateToast } from '@/lib/item-format';

/**
 * 「不对，请 AI 重写」：把这条收藏排进整理队列，并按自动整理现在的真实状态说清楚会不会真的重写。
 * 说什么由 `regenerateToast` 决定（纯函数，有单测）；没开额度时不许说「整理好会更新」。
 * onOpenToolbox 不传就不给「去开启」按钮。
 */
export function useRegenerate(onOpenToolbox?: () => void): (id: string) => Promise<void> {
  const toast = useToast();
  return useCallback(async (id: string) => {
    try {
      await regenerateLibraryCard(id);
    } catch (reason) {
      toast.show({ title: describeInvokeError(reason) });
      return;
    }
    const status = await getAiAutomation().catch(() => null);
    const note = regenerateToast(status);
    toast.show({
      title: note.title,
      sub: note.sub,
      actions: note.actionLabel && onOpenToolbox ? [{ label: note.actionLabel, onClick: onOpenToolbox }] : undefined,
    });
  }, [toast, onOpenToolbox]);
}
