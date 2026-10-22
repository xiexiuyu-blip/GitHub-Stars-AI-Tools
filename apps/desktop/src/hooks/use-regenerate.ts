import { useCallback } from 'react';
import { useToast } from '@/components/kit';
import { describeInvokeError, regenerateLibraryCard } from '@/lib/api';
import { regenerateFailureNote, regenerateStartedNote } from '@/lib/item-format';

/**
 * 「确认，重新整理」之后才调用。命令带 confirmed: true，只开这一张的额度闸。
 * 只能在用户点了确认条上的「确认，重新整理」之后调用。
 * onOpenToolbox 不传就不给「去自动整理」按钮。
 */
export function useRegenerate(onOpenToolbox?: () => void): (id: string) => Promise<void> {
  const toast = useToast();
  return useCallback(async (id: string) => {
    try {
      // 只能在用户点了确认条上的「确认，重新整理」之后调用。
      await regenerateLibraryCard(id, true);
    } catch (reason) {
      const message = describeInvokeError(reason);
      const failure = regenerateFailureNote(message);
      toast.show({
        title: message,
        actions: failure.actionLabel && onOpenToolbox ? [{ label: failure.actionLabel, onClick: onOpenToolbox }] : undefined,
      });
      return;
    }
    const note = regenerateStartedNote();
    toast.show({ title: note.title, sub: note.sub });
  }, [toast, onOpenToolbox]);
}
