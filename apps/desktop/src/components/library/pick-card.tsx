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
        'group relative mx-3 my-2.5 flex cursor-pointer gap-3.5 rounded-lg border bg-card p-3 transition-all',
        props.selected
          ? 'border-primary shadow-[inset_0_0_0_1px_var(--color-primary),0_4px_14px_rgba(46,107,240,0.14)]'
          : 'border-border shadow-xs hover:border-input hover:shadow-md',
      )}
      onClick={props.onSelect}
      aria-current={props.selected ? 'true' : undefined}
    >
      <CoverImage src={cover} seed={props.item.fullName} className="h-[100px] w-[140px] shrink-0 rounded-md border border-border" />
      <div className="flex min-w-0 flex-1 flex-col">
        {/* 身份：名字、所有者、语言、星数 */}
        <div className="flex min-w-0 items-baseline gap-2">
          <b className="min-w-0 shrink truncate text-[15px] leading-[1.35] font-semibold text-foreground">{repo || props.item.fullName}</b>
          <span className={cn('ml-auto shrink-0 group-hover:invisible', props.selected && 'invisible')}>
            <StatusTag group={props.item.readingGroup} />
          </span>
        </div>
        <div className="mt-0.5 flex min-w-0 items-center gap-2 text-xs text-subtle">
          {owner ? <span className="min-w-0 truncate">{owner}</span> : null}
          <LangDot language={props.item.language} />
          <StarCount value={props.item.starsCount} className="ml-auto" />
        </div>
        {/* 内容：一句话 */}
        {text ? <div className="mt-1.5 line-clamp-2 text-sm leading-[1.55] text-foreground">{text}</div> : null}
        {/* 标签 */}
        {chips.length > 0 ? (
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            {chips.map((scenario) => (
              <ScenarioChip key={scenario.id} name={scenario.nameZh} scenarioKey={scenario.slug || scenario.id} />
            ))}
          </div>
        ) : null}
        {props.why ? (
          <div className="why-line mt-auto flex items-start gap-1 pt-2 text-xs leading-[1.5]">
            <Icon name="lightbulb" size={14} fill className="mt-px shrink-0" />
            <span className="min-w-0">{props.why}</span>
          </div>
        ) : null}
      </div>
      <div className={cn('absolute top-2.5 right-2.5 z-10 items-center gap-0.5 rounded-md border border-border bg-card p-0.5 shadow-sm group-hover:flex', props.selected ? 'flex' : 'hidden')}>
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
