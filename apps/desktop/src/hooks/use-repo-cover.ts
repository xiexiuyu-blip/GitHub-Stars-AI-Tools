import { useMemo } from 'react';
import { extractPreviewImages } from '@/lib/repo-assets';
import { useRepoReadme } from './use-repo-readme';

/** 封面图：README 里的第一张截图。没读到、没有图时返回 null，界面用渐变底。 */
export function useRepoCover(id: string | null | undefined, fullName: string): string | null {
  const entry = useRepoReadme(id);
  return useMemo(() => {
    if (entry.state !== 'ready' || !entry.readme) return null;
    return extractPreviewImages(entry.readme.rawMarkdown, fullName, 1)[0]?.url ?? null;
  }, [entry, fullName]);
}
