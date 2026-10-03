import { useCallback, useEffect, useState } from 'react';
import { getAiAutomation } from '@/lib/api';
import type { AutomationStatus } from '@/lib/automation-view';

/** 上一次读到的状态：换页面、换项目时先用它，不让提示闪一下再变。 */
let lastKnown: AutomationStatus | null = null;

/**
 * 读 AI 自动整理的状态，用来在今日 / 资料库 / 详情里说清「为什么还没整理」。
 * 读不到（命令不存在、出错）就是 null，界面当作没有提示，不影响别的功能。
 */
export function useAiAutomation(): { status: AutomationStatus | null; reload: () => void } {
  const [status, setStatus] = useState<AutomationStatus | null>(lastKnown);
  const [reloadKey, setReloadKey] = useState(0);
  useEffect(() => {
    let cancelled = false;
    getAiAutomation()
      .then((next) => {
        lastKnown = next;
        if (!cancelled) setStatus(next);
      })
      .catch(() => {
        if (!cancelled) setStatus(null);
      });
    return () => {
      cancelled = true;
    };
  }, [reloadKey]);
  const reload = useCallback(() => setReloadKey((key) => key + 1), []);
  return { status, reload };
}
