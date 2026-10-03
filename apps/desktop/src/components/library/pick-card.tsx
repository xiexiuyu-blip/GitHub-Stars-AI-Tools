import type { LibraryItem, ScenarioSummary } from '@/lib/contracts';
import { splitRepoName } from '@/lib/item-format';
import { useRepoCover } from '@/hooks/use-repo-cover';
import { CoverImage, LangDot, ScenarioChip, StarCount } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';
import { scenarioChipsOf, StatusTag } from './item-row';

/**
 * 「今天值得先看」的卡片：左边是 README 里的截图（没有就是彩色渐变），右边是名字、一句话、标签。
 * 比一行文字更像「一个工具」，一眼能看出它长什么样。
 */
export function PickCard(props: {
  item: LibraryItem;
  selected?: boolean;
  why?: string | null;
  scenarios?: ScenarioSummary[];
  onSelect: () => void;
  onWant: () => void;
  onDrop: () => void;
}) {
  const { owner, repo } = splitRepoName(props.item.fullName);
  const cover = useRepoCover(props.item.id, props.item.fullName);
  const chips = scenarioChipsOf(props.item, props.scenarios);
  const text = props.item.oneLiner?.trim() || props.item.description?.trim() || '';

  return (
    <div
      className={cn(
        'group relative mx-3 my-2 flex cursor-pointer gap-3 rounded-xl border bg-card p-2.5 transition-all',
        props.selected
          ? 'border-primary shadow-[inset_0_0_0_1px_var(--color-primary),0_4px_14px_rgba(46,107,240,0.12)]'
          : 'border-border hover:border-input hover:shadow-[0_4px_12px_rgba(31,35,41,0.08)]',
      )}
      onClick={props.onSelect}
      aria-current={props.selected ? 'true' : undefined}
    >
      <CoverImage src={cover} seed={props.item.fullName} className="h-[96px] w-[136px] shrink-0 rounded-lg border border-border" />
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex min-w-0 items-baseline gap-2">
          <b className="min-w-0 shrink truncate text-[15px] font-semibold text-foreground">{repo || props.item.fullName}</b>
          {owner ? <span className="min-w-0 truncate text-xs text-on-surface-variant">{owner}</span> : null}
          <span className="ml-auto shrink-0 group-hover:invisible">
            <StatusTag group={props.item.readingGroup} />
          </span>
        </div>
        {text ? <div className="mt-0.5 line-clamp-2 text-sm leading-[1.5] text-foreground">{text}</div> : null}
        <div className="mt-auto flex flex-wrap items-center gap-x-2.5 gap-y-1 pt-1.5">
          <StarCount value={props.item.starsCount} />
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
      <div className="absolute top-2 right-2 z-10 hidden items-center gap-0.5 rounded-lg border border-border bg-card p-0.5 shadow-sm group-hover:flex">
        <HoverIcon label="想试（T）" icon="bookmark_add" onClick={props.onWant} />
        <HoverIcon label="不要了（X）" icon="close" onClick={props.onDrop} />
      </div>
    </div>
  );
}

function HoverIcon(props: { label: string; icon: string; onClick: () => void }) {
  return (
    <button
      type="button"
      className="inline-grid size-7 place-items-center rounded-md text-on-surface-variant hover:bg-surface-container-low hover:text-foreground"
      title={props.label}
      aria-label={props.label}
      onClick={(event) => {
        event.stopPropagation();
        props.onClick();
      }}
    >
      <Icon name={props.icon} size={16} />
    </button>
  );
}
