import { cn } from '@/lib/utils';

export function StatCard(props: {
  label: string;
  value: number | string;
  hint?: string;
  active?: boolean;
  onClick?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={props.onClick}
      aria-pressed={props.active === true}
      className={cn(
        'flex flex-col gap-0.5 rounded-lg border border-border bg-card px-3.5 py-2.5 text-left transition-colors hover:border-input',
        props.active && 'border-primary shadow-[inset_0_0_0_1px_var(--color-primary)]',
      )}
    >
      <span className="text-sm text-on-surface-variant">{props.label}</span>
      <span className="text-[20px] leading-[1.4] font-semibold text-foreground">
        {props.value}
        {props.hint ? <small className="ml-1.5 text-xs font-normal text-on-surface-variant">{props.hint}</small> : null}
      </span>
    </button>
  );
}
