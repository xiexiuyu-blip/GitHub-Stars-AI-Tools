import { useEffect, useState } from 'react';
import { getRepositoryReadme, type RepositoryReadmeText } from '@/lib/api';
import type { TaskProgressEvent } from '@/types';

type Entry =
  | { state: 'loading' }
  | { state: 'ready'; readme: RepositoryReadmeText | null }
  | { state: 'error'; message: string };

const cache = new Map<string, Promise<RepositoryReadmeText | null>>();
const subscribers = new Map<string, Set<() => void>>();

function load(id: string): Promise<RepositoryReadmeText | null> {
  let pending = cache.get(id);
  if (!pending) {
    pending = getRepositoryReadme(id);
    cache.set(id, pending);
    // 失败不缓存，下次再试
    pending.catch(() => {
      // 旧请求失败不能删掉失效后新建的缓存。
      if (cache.get(id) === pending) cache.delete(id);
    });
  }
  return pending;
}

/** 更新后清掉旧值（包括「没有 README」），同时刷新已经打开的详情。 */
export function invalidateRepoReadmes(id?: string): void {
  if (id) cache.delete(id);
  else cache.clear();
  const listeners = id ? [...(subscribers.get(id) ?? [])] : [...subscribers.values()].flatMap((items) => [...items]);
  listeners.forEach((refresh) => refresh());
}

/** 后端完成事件没有仓库 id：只在会更新 README 的任务收尾时失效，不跟随每条进度。 */
export function invalidateRepoReadmesForProgress(progress: TaskProgressEvent): void {
  const syncFinished = progress.taskId === 'sync-stars' && progress.status === 'succeeded';
  const readmeFinished = ['readme-follow', 'fetch-readmes', 'fetch-repository-readme'].includes(progress.taskId)
    && ['succeeded', 'partial', 'failed'].includes(progress.status);
  // README 抓取后续的评估可能失败，已写入的正文仍须刷新。
  if (syncFinished || readmeFinished) invalidateRepoReadmes();
}

/** 同仓库共享缓存；收到更新事件后重新读取。id 为空时不读取。 */
export function useRepoReadme(id: string | null | undefined): Entry {
  const [entry, setEntry] = useState<Entry>({ state: 'loading' });
  useEffect(() => {
    if (!id) {
      setEntry({ state: 'ready', readme: null });
      return;
    }
    let cancelled = false;
    let generation = 0;
    const refresh = () => {
      const current = ++generation;
      setEntry({ state: 'loading' });
      load(id).then((readme) => {
        if (!cancelled && current === generation) setEntry({ state: 'ready', readme });
      }).catch((reason) => {
        if (!cancelled && current === generation) setEntry({ state: 'error', message: reason instanceof Error ? reason.message : String(reason) });
      });
    };
    const listeners = subscribers.get(id) ?? new Set<() => void>();
    listeners.add(refresh);
    subscribers.set(id, listeners);
    refresh();
    return () => {
      cancelled = true;
      listeners.delete(refresh);
      if (!listeners.size) subscribers.delete(id);
    };
  }, [id]);
  return entry;
}
