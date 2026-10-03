import { useEffect, useState } from 'react';
import { getRepositoryReadme, type RepositoryReadmeText } from '@/lib/api';

type Entry =
  | { state: 'loading' }
  | { state: 'ready'; readme: RepositoryReadmeText | null }
  | { state: 'error'; message: string };

const cache = new Map<string, Promise<RepositoryReadmeText | null>>();

function load(id: string): Promise<RepositoryReadmeText | null> {
  let pending = cache.get(id);
  if (!pending) {
    pending = getRepositoryReadme(id);
    cache.set(id, pending);
    // 失败不缓存，下次再试
    pending.catch(() => cache.delete(id));
  }
  return pending;
}

/** 读某个仓库的 README（带内存缓存：同一个仓库在今日、列表、详情里只读一次）。id 为空时什么都不做。 */
export function useRepoReadme(id: string | null | undefined): Entry {
  const [entry, setEntry] = useState<Entry>({ state: 'loading' });
  useEffect(() => {
    if (!id) {
      setEntry({ state: 'ready', readme: null });
      return;
    }
    let cancelled = false;
    setEntry({ state: 'loading' });
    load(id).then((readme) => {
      if (!cancelled) setEntry({ state: 'ready', readme });
    }).catch((reason) => {
      if (!cancelled) setEntry({ state: 'error', message: reason instanceof Error ? reason.message : String(reason) });
    });
    return () => {
      cancelled = true;
    };
  }, [id]);
  return entry;
}
