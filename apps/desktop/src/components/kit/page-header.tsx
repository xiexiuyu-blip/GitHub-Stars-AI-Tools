import { ReactNode } from 'react';

export function PageHeader(props: { title: string; subtitle?: string; actions?: ReactNode }) {
  return (
    <header className="flex shrink-0 flex-wrap items-end gap-3">
      <div className="min-w-0">
        <h1 className="m-0 text-[24px] leading-[1.3] font-bold tracking-[-0.01em] text-foreground">{props.title}</h1>
        {props.subtitle ? (
          <p className="mt-1 mb-0.5 text-sm text-on-surface-variant">{props.subtitle}</p>
        ) : null}
      </div>
      {props.actions ? <div className="ml-auto flex items-center gap-2">{props.actions}</div> : null}
    </header>
  );
}
