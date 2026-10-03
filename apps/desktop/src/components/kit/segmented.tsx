import { cn } from '@/lib/utils';

export function Segmented<T extends string>(props: {
  options: readonly { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
  ariaLabel: string;
}) {
  return (
    <div
      role="group"
      aria-label={props.ariaLabel}
      className="inline-flex max-w-full shrink-0 flex-wrap gap-0.5 rounded-md bg-muted p-0.5"
    >
      {props.options.map((option) => {
        const selected = option.value === props.value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={selected}
            onClick={() => props.onChange(option.value)}
            className={cn(
              'h-7 shrink-0 rounded-sm px-2.5 text-sm whitespace-nowrap text-on-surface-variant',
              selected && 'bg-card font-medium text-foreground shadow-sm',
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
