import { useCallback, useEffect, useRef, useState } from 'react';
import type { RepoCommunity } from '@/lib/contracts';
import { describeInvokeError, getRepoCommunity, isMissingCommandError } from '@/lib/api';

export type CommunityState = {
  /** loading 第一次读缓存；ready 有数据（可能是空的）；unavailable 后台还没这个命令；error 读缓存失败 */
  phase: 'loading' | 'ready' | 'unavailable' | 'error';
  data: RepoCommunity | null;
  /** 正在联网查（Hacker News + GitHub Issues，几秒） */
  refreshing: boolean;
  /** 联网查失败的话（读缓存失败也放这里） */
  error: string | null;
};

/**
 * 某个仓库的社区讨论：先读本地缓存（不联网），点「查一查」才联网。
 * 后台还没带上命令时 phase 是 unavailable，界面就不画这一块，不影响别的内容。
 */
export function useRepoCommunity(id: string): { state: CommunityState; refresh: () => void } {
  const [state, setState] = useState<CommunityState>({ phase: 'loading', data: null, refreshing: false, error: null });
  const latestId = useRef(id);
  latestId.current = id;

  useEffect(() => {
    let cancelled = false;
    setState({ phase: 'loading', data: null, refreshing: false, error: null });
    getRepoCommunity(id, false)
      .then((data) => {
        if (!cancelled) setState({ phase: 'ready', data, refreshing: false, error: null });
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        if (isMissingCommandError(reason)) {
          setState({ phase: 'unavailable', data: null, refreshing: false, error: null });
        } else {
          setState({ phase: 'error', data: null, refreshing: false, error: describeInvokeError(reason) });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [id]);

  const refresh = useCallback(() => {
    const target = id;
    setState((current) => ({ ...current, refreshing: true, error: null }));
    getRepoCommunity(target, true)
      .then((data) => {
        if (latestId.current !== target) return;
        setState({ phase: 'ready', data, refreshing: false, error: null });
      })
      .catch((reason: unknown) => {
        if (latestId.current !== target) return;
        setState((current) => ({ ...current, refreshing: false, error: describeInvokeError(reason) }));
      });
  }, [id]);

  return { state, refresh };
}
