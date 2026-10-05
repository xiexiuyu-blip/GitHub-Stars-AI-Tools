import { useCallback, useEffect, useState } from 'react';
import { getAiAutomationWithRecent } from '@/lib/api';
import type { AutomationStatus } from '@/lib/automation-view';

/** 上一次读到的状态：换页面、换项目时先用它，不让提示闪一下再变。 */
let lastKnown: AutomationStatus | null = null;

/**
 * 读 AI 自动整理的状态，用来在今日 / 资料库 / 详情里说清「为什么还没整理」。
 * 状态里带着「最近 24 小时整理了几个」（来自现成命令 get_ai_queue_status）；那个个数读不到时只是缺失，不影响状态。
 * 状态本身读不到（命令不存在、出错）就是 null，界面当作没有提示，不影响别的功能。
 */
export function useAiAutomation(): { status: AutomationStatus | null; reload: () => void } {
  const [status, setStatus] = useState<AutomationStatus | null>(lastKnown);
  const [reloadKey, setReloadKey] = useState(0);
  useEffect(() => {
    let cancelled = false;
    getAiAutomationWithRecent()
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
