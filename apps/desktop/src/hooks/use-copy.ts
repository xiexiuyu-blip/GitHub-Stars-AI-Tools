import { useCallback } from 'react';
import { useToast } from '@/components/kit';

/** 复制一段文字到剪贴板，并给一句提示。浏览器不让复制时告诉用户手动复制。返回是否复制成功。 */
export function useCopy(): (text: string, title: string, sub?: string) => Promise<boolean> {
  const toast = useToast();
  return useCallback(async (text, title, sub) => {
    try {
      if (!navigator.clipboard?.writeText) throw new Error('clipboard');
      await navigator.clipboard.writeText(text);
      toast.show({ title, sub });
      return true;
    } catch {
      toast.show({ title: '浏览器不允许自动复制', sub: '请手动选中后按 ⌘C' });
      return false;
    }
  }, [toast]);
}
