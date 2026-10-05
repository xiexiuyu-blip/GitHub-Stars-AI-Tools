import { useCallback } from 'react';
import { useToast } from '@/components/kit';
import { COPY_FAILED_NOTICE } from '@/lib/item-format';

/** 复制一段文字到剪贴板，并给一句提示。复制失败时告诉用户手动复制。返回是否复制成功。 */
export function useCopy(): (text: string, title: string, sub?: string) => Promise<boolean> {
  const toast = useToast();
  return useCallback(async (text, title, sub) => {
    try {
      if (!navigator.clipboard?.writeText) throw new Error('clipboard');
      await navigator.clipboard.writeText(text);
      toast.show({ title, sub });
      return true;
    } catch {
      toast.show({ title: COPY_FAILED_NOTICE });
      return false;
    }
  }, [toast]);
}
