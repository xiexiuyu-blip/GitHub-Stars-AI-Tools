import type { MouseEvent } from 'react';
import type { LibraryItem, ReadingGroup, ScenarioSummary } from '@/lib/contracts';
import { cleanDescription, readingGroupLabel, splitRepoName } from '@/lib/item-format';
import { LangDot, RepoAvatar, ScenarioChip, StarCount, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

/** 一个仓库最多在行上显示几个场景标签。 */
const MAX_CHIPS = 2;

export function scenarioChipsOf(item: Pick<LibraryItem, 'scenarioIds'>, scenarios: ScenarioSummary[] | undefined, max = MAX_CHIPS) {
  if (!scenarios || scenarios.length === 0) return [];
  return item.scenarioIds
    .map((id) => scenarios.find((scenario) => scenario.id === id))
    .filter((scenario): scenario is ScenarioSummary => Boolean(scenario))
    .slice(0, max);
}

export function ItemRow(props: {
  item: LibraryItem;
  selected?: boolean;
  why?: string | null;
  scenarios?: ScenarioSummary[];
  onSelect: () => void;
  onWant: () => void;
  onDrop: () => void;
}) {
  const { owner, repo } = splitRepoName(props.item.fullName);
  const chips = scenarioChipsOf(props.item, props.scenarios);
  const text = props.item.oneLiner?.trim() || cleanDescription(props.item.description);

  return (
    <div
      className={cn(
        'group relative grid cursor-pointer grid-cols-[40px_minmax(0,1fr)_auto] items-start gap-3 border-b border-muted px-4 py-3 transition-colors last:border-b-0',
        props.selected
          ? 'bg-primary/10 before:absolute before:top-2 before:bottom-2 before:left-0 before:w-[3px] before:rounded-r-[3px] before:bg-primary'
          : 'hover:bg-surface-container-low',
      )}
      onClick={props.onSelect}
      aria-current={props.selected ? 'true' : undefined}
    >
      <RepoAvatar name={props.item.fullName} size="md" />
      <div className="min-w-0">
        <div className="flex min-w-0 items-baseline gap-2">
          <b className="min-w-0 shrink truncate text-[15px] leading-[1.35] font-semibold text-foreground">{repo || props.item.fullName}</b>
          {owner ? <span className="min-w-0 truncate text-xs text-subtle">{owner}</span> : null}
        </div>
        {text ? (
          <div className={cn('mt-0.5 line-clamp-2 text-sm leading-[1.55]', props.item.oneLiner ? 'text-foreground' : 'text-on-surface-variant')}>{text}</div>
        ) : null}
        <div className="mt-1.5 flex flex-wrap items-center gap-x-2.5 gap-y-1">
          <LangDot language={props.item.language} />
          {chips.map((scenario) => (
            <ScenarioChip key={scenario.id} name={scenario.nameZh} scenarioKey={scenario.slug || scenario.id} />
          ))}
        </div>
        {props.why ? (
          <div className="why-line mt-1.5 flex items-start gap-1 text-xs leading-[1.5]">
            <Icon name="lightbulb" size={14} fill className="mt-px shrink-0" />
            <span className="min-w-0">{props.why}</span>
          </div>
        ) : null}
      </div>
      <div className="flex flex-col items-end gap-1.5 pt-0.5 group-hover:invisible">
        <StarCount value={props.item.starsCount} />
        <StatusTag group={props.item.readingGroup} />
      </div>
      <div className="absolute top-3 right-3 z-10 hidden items-center gap-0.5 rounded-md border border-border bg-card p-0.5 shadow-sm group-hover:flex">
        <HoverButton label="想试（T）" icon="bookmark_add" onClick={props.onWant} />
        <HoverButton label="不要了（X）" icon="close" onClick={props.onDrop} />
      </div>
    </div>
  );
}

export function StatusTag(props: { group: ReadingGroup }) {
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
