import type { CSSProperties } from 'react';
import type { AutomationHint } from '@/lib/automation-view';
import { Button } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

/** 今日 / 资料库顶上的一行提示：有多少没整理、为什么、能做什么。hint 为空就什么都不画。 */
export function AutomationBanner(props: { hint: AutomationHint | null; onAction?: () => void; className?: string }) {
  const { hint } = props;
  if (!hint) return null;
  return (
    <div className={cn('flex shrink-0 items-center gap-3 rounded-lg border border-border bg-card px-3.5 py-2.5 shadow-xs', props.className)}>
      <span className="hue-tile grid size-8 shrink-0 place-items-center rounded-lg" style={{ '--h': hint.hue } as CSSProperties}>
        <Icon name={hint.icon} size={18} fill />
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-sm leading-[1.4] font-medium text-foreground">{hint.title}</div>
        <div className="line-clamp-2 text-xs leading-[1.5] text-on-surface-variant">{hint.detail}</div>
      </div>
      {hint.actionLabel && props.onAction ? (
        <Button size="sm" onClick={props.onAction}>{hint.actionLabel}</Button>
      ) : null}
    </div>
  );
}
