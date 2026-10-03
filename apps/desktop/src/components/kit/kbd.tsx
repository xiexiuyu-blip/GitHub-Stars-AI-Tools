import { ReactNode } from 'react';

export function Kbd(props: { children: ReactNode }) {
  return (
    <kbd className="rounded-sm border border-input bg-card px-1 font-label-sm text-[11px] leading-4 font-medium whitespace-nowrap text-on-surface-variant">
      {props.children}
    </kbd>
  );
}
