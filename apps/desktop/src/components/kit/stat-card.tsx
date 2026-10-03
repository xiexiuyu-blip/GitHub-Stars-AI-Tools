import type { CSSProperties } from 'react';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

const TONE_HUE = { blue: 222, amber: 36, violet: 268, green: 148 } as const;

/**
 * 统计卡（兼筛选）。左边一块带颜色的图标底，右边是说明和大号数字；选中时描边加浅色底。
 * 不传 icon 时退回原来的纯文字样子。
 */
export function StatCard(props: {
  label: string;
  value: number | string;
  hint?: string;
  icon?: string;
  tone?: keyof typeof TONE_HUE;
  active?: boolean;
  onClick?: () => void;
}) {
  const hue = TONE_HUE[props.tone ?? 'blue'];
  const style = { '--h': hue } as CSSProperties;
  return (
    <button
      type="button"
      onClick={props.onClick}
      aria-pressed={props.active === true}
      style={style}
      className={cn(
        'flex items-center gap-3 rounded-xl border border-border bg-card px-4 py-3 text-left shadow-[0_1px_2px_rgba(31,35,41,0.04)] transition-all hover:-translate-y-px hover:border-input hover:shadow-[0_4px_12px_rgba(31,35,41,0.08)]',
        props.active && 'border-primary bg-[hsl(var(--h)_100%_98.5%)] shadow-[inset_0_0_0_1px_var(--color-primary)]',
      )}
    >
      {props.icon ? (
        <span className="hue-tile grid size-10 shrink-0 place-items-center rounded-[10px]">
          <Icon name={props.icon} size={22} fill />
        </span>
      ) : null}
      <span className="flex min-w-0 flex-col">
        <span className="text-[13px] text-on-surface-variant">{props.label}</span>
        <span className="text-[26px] leading-[1.25] font-bold tracking-tight text-foreground tabular-nums">
          {props.value}
          {props.hint ? <small className="ml-1.5 text-xs font-normal text-on-surface-variant">{props.hint}</small> : null}
        </span>
      </span>
    </button>
  );
}
