import { ReactNode } from 'react';
import { cn } from '@/lib/utils';

export function Tag(props: { tone?: 'gray' | 'blue' | 'red'; children: ReactNode; className?: string }) {
  const tone = props.tone ?? 'gray';
  return (
    <span
      className={cn(
        'inline-flex h-5 shrink-0 items-center gap-0.5 rounded-sm px-1.5 text-xs whitespace-nowrap',
        tone === 'blue' && 'bg-accent text-accent-foreground',
        tone === 'red' && 'bg-error-container text-on-error-container',
        tone === 'gray' && 'bg-muted text-on-surface-variant',
        props.className,
      )}
    >
      {props.children}
    </span>
  );
}
