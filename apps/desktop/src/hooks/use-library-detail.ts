import { useCallback, useEffect, useState } from 'react';
import type { LibraryDetail, ReadingGroup } from '@/lib/contracts';
import { describeInvokeError, getLibraryDetail } from '@/lib/api';

export type DetailState = {
  detail: LibraryDetail | null;
  loading: boolean;
  error: string | null;
};

/** 读一个仓库的详情。id 为空时什么都不读。可以就地改状态（想试 / 在用……）而不重新读。 */
export function useLibraryDetail(id: string | null): DetailState & { retry: () => void; paintReading: (group: ReadingGroup) => void } {
  const [state, setState] = useState<DetailState>({ detail: null, loading: false, error: null });
  const [reloadKey, setReloadKey] = useState(0);
  useEffect(() => {
    if (!id) {
      setState({ detail: null, loading: false, error: null });
      return;
    }
    let cancelled = false;
    setState({ detail: null, loading: true, error: null });
    getLibraryDetail(id)
      .then((detail) => {
        if (!cancelled) setState({ detail, loading: false, error: null });
      })
      .catch((reason: unknown) => {
        if (!cancelled) setState({ detail: null, loading: false, error: describeInvokeError(reason) });
      });
    return () => {
      cancelled = true;
    };
  }, [id, reloadKey]);
  const retry = useCallback(() => setReloadKey((key) => key + 1), []);
  const paintReading = useCallback((group: ReadingGroup) => {
    setState((current) => (current.detail ? { ...current, detail: { ...current.detail, readingGroup: group } } : current));
  }, []);
  return { ...state, retry, paintReading };
}
