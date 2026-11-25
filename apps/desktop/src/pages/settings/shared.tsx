import { useEffect, useState, type CSSProperties, type ReactNode } from 'react';
import { Button, Card } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { describeInvokeError } from '@/lib/api';
import { cn } from '@/lib/utils';

export function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function errorText(reason: unknown): string {
  return describeInvokeError(reason);
}

export function normalizeSyncIntervalInput(value: string) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return 30;
  }
  return Math.min(Math.max(parsed, 5), 1440);
}

export function Section(props: {
  icon: string;
  hue: number;
  title: string;
  hint?: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Card className="p-5">
      <div className="mb-3.5 flex items-start gap-3">
        <span className="hue-tile grid size-9 shrink-0 place-items-center rounded-[10px]" style={{ '--h': props.hue } as CSSProperties}>
          <Icon name={props.icon} size={20} fill />
        </span>
        <div className="min-w-0 flex-1">
          <h3 className="m-0 text-base font-semibold text-foreground">{props.title}</h3>
          {props.hint ? <p className="mt-0.5 mb-0 text-[13px] leading-[1.6] text-on-surface-variant">{props.hint}</p> : null}
        </div>
        {props.actions}
      </div>
      {props.children}
    </Card>
  );
}

export function Notice(props: { tone?: 'info' | 'warn'; children: ReactNode }) {
  return (
    <div className={cn('mb-3 flex flex-wrap items-center gap-2 rounded-lg px-3.5 py-2.5 text-sm', props.tone === 'warn' ? 'bg-error-container text-on-error-container' : 'bg-accent text-foreground')}>
      {props.children}
    </div>
  );
}

/** 一行口令或令牌：说明它能做什么，一个「复制」、一个要点两下才生效的「重置」。 */
export function TokenRow(props: { title: string; hint: string; onCopy: () => void; onReset: () => void }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return undefined;
    const timer = window.setTimeout(() => setArmed(false), 4000);
    return () => window.clearTimeout(timer);
  }, [armed]);
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 rounded-lg border border-border px-3.5 py-2.5">
      <div className="min-w-[220px] flex-1">
        <b className="block text-sm font-medium text-foreground">{props.title}</b>
        <span className="text-xs leading-[1.6] text-on-surface-variant">{props.hint}</span>
      </div>
      <div className="flex items-center gap-2">
        <Button size="sm" onClick={props.onCopy}>复制</Button>
        <Button
          size="sm"
          variant="text"
          onClick={() => {
            if (!armed) {
              setArmed(true);
              return;
            }
            setArmed(false);
            props.onReset();
          }}
        >
          {armed ? '再点一次确认重置' : '重置'}
        </Button>
      </div>
    </div>
  );
}

export function Metric(props: { label: string; value: string; sub: string; title?: string }) {
  return (
    <div className="rounded-lg bg-surface-container-low px-3.5 py-3" title={props.title}>
      <div className="text-xs text-on-surface-variant">{props.label}</div>
      <div className="mt-0.5 text-lg leading-[1.3] font-bold tracking-tight text-foreground tabular-nums">{props.value}</div>
      <div className="mt-0.5 text-xs leading-[1.5] text-subtle">{props.sub}</div>
    </div>
  );
}

export function ToggleSwitch({
  checked,
  disabled = false,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  onChange?: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => {
        if (disabled) return;
        onChange?.(!checked);
      }}
      className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 ${
        checked ? 'bg-primary' : 'bg-outline-variant'
      } disabled:cursor-not-allowed disabled:opacity-50`}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
          checked ? 'translate-x-6' : 'translate-x-1'
        }`}
      />
    </button>
  );
}
