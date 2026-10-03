import { cn } from '@/lib/utils';
import { splitRepoName } from '@/lib/item-format';

export function RepoAvatar(props: { name: string; size?: 'sm' | 'lg' }) {
  const large = props.size === 'lg';
  return (
    <span
      className={cn(
        'inline-grid shrink-0 place-items-center rounded-lg bg-muted font-semibold text-on-surface-variant',
        large ? 'size-10 text-base' : 'size-7 text-xs',
      )}
      aria-hidden="true"
    >
      {splitRepoName(props.name).initial}
    </span>
  );
}
