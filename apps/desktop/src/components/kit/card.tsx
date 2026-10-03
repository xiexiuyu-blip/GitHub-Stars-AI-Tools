import { ReactNode } from 'react';
import { cn } from '@/lib/utils';

export function Card(props: { children: ReactNode; className?: string }) {
  return (
    <section className={cn('min-w-0 rounded-lg border border-border bg-card', props.className)}>
      {props.children}
    </section>
  );
}

export function CardHeader(props: { title: string; count?: number | string; actions?: ReactNode }) {
  return (
    <div className="flex h-12 shrink-0 items-center gap-2.5 border-b border-muted px-4">
      <h3 className="m-0 text-base font-semibold whitespace-nowrap text-foreground">{props.title}</h3>
      {props.count != null ? (
        <span className="text-xs font-medium text-on-surface-variant">{props.count}</span>
      ) : null}
      <div className="flex-1" />
      {props.actions}
    </div>
  );
}
