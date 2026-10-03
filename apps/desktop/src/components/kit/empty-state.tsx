import { ReactNode } from 'react';
import { Icon } from '@/components/ui/icon';

export function EmptyState(props: { icon: string; title: string; description?: string; action?: ReactNode }) {
  return (
    <div className="m-auto flex flex-col items-center gap-1.5 px-4 py-10 text-center text-sm text-on-surface-variant">
      <Icon name={props.icon} size={28} />
      <b className="font-medium text-on-surface-variant">{props.title}</b>
      {props.description ? <span>{props.description}</span> : null}
      {props.action}
    </div>
  );
}
