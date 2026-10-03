import type { MouseEvent } from 'react';
import type { LibraryItem, ReadingGroup } from '@/lib/contracts';
import { formatStars, readingGroupLabel, splitRepoName } from '@/lib/item-format';
import { RepoAvatar, Spinner, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

export function ItemRow(props: {
  item: LibraryItem;
  selected?: boolean;
  why?: string | null;
  onSelect: () => void;
  onWant: () => void;
  onDrop: () => void;
}) {
  const { owner, repo } = splitRepoName(props.item.fullName);
  const meta = [owner, `★ ${formatStars(props.item.starsCount)}`, props.item.language]
    .filter((part) => part && part.trim())
    .join(' · ');
  const pendingText = props.item.description?.trim()
    ? `AI 正在整理 · ${props.item.description.trim()}`
    : 'AI 正在整理';

  return (
    <div
      className={cn(
        'group relative grid cursor-pointer grid-cols-[28px_minmax(0,1fr)_auto] items-start gap-2.5 px-4 py-2.5',
        props.selected
          ? 'bg-primary/10 before:absolute before:top-2 before:bottom-2 before:left-0 before:w-[3px] before:rounded-r-[3px] before:bg-primary'
          : 'hover:bg-surface-container-low',
      )}
      onClick={props.onSelect}
      aria-current={props.selected ? 'true' : undefined}
    >
      <RepoAvatar name={props.item.fullName} />
      <div className="min-w-0">
        <div className="flex min-w-0 items-baseline gap-2 text-xs text-on-surface-variant">
          <b className="min-w-0 shrink truncate text-sm font-medium text-foreground">{repo || props.item.fullName}</b>
          <span className="min-w-0 truncate">{meta}</span>
        </div>
        {props.item.oneLiner ? (
          <div className="mt-0.5 truncate text-sm text-foreground">{props.item.oneLiner}</div>
        ) : (
          <div className="mt-0.5 flex min-w-0 items-center gap-1.5">
            <Spinner />
            <span className="truncate text-sm text-on-surface-variant">{pendingText}</span>
          </div>
        )}
        {props.why ? <div className="mt-0.5 truncate text-xs text-on-surface-variant">{props.why}</div> : null}
      </div>
      <div className="flex flex-col items-end gap-1 pt-0.5 group-hover:invisible">
        <StatusTag group={props.item.readingGroup} />
      </div>
      <div className="absolute top-1/2 right-3 z-10 hidden -translate-y-1/2 items-center gap-0.5 rounded-md border border-border bg-card p-0.5 shadow-sm group-hover:flex">
        <HoverButton label="想试（T）" icon="bookmark_add" onClick={props.onWant} />
        <HoverButton label="不要了（X）" icon="close" onClick={props.onDrop} />
      </div>
    </div>
  );
}

function StatusTag(props: { group: ReadingGroup }) {
  if (props.group === 'unseen') return null;
  if (props.group === 'want') return <Tag tone="blue">想试</Tag>;
  if (props.group === 'in_use') {
    return (
      <Tag tone="blue">
        <Icon name="check" size={12} />
        在用
      </Tag>
    );
  }
  return <Tag>{readingGroupLabel(props.group)}</Tag>;
}

function HoverButton(props: { label: string; icon: string; onClick: () => void }) {
  function handleClick(event: MouseEvent<HTMLButtonElement>) {
    event.stopPropagation();
    props.onClick();
  }

  return (
    <button
      type="button"
      className="inline-grid size-7 place-items-center rounded-md text-on-surface-variant hover:bg-surface-container-low hover:text-foreground"
      title={props.label}
      aria-label={props.label}
      onClick={handleClick}
    >
      <Icon name={props.icon} size={16} />
    </button>
  );
}
