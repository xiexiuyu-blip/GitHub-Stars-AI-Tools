import type { RerankFit, ScenarioSummary, SolveHit } from '@/lib/contracts';
import { formLabel } from '@/lib/library-filters';
import { matchBadge, RERANK_FIT_LABEL, whyLine } from '@/lib/solve-view';
import { CoverImage, RepoAvatar, ScenarioChip, StarCount, Tag } from '@/components/kit';
import { useRepoCover } from '@/hooks/use-repo-cover';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';
import { maintenanceOf } from '@/lib/maintenance';
import { MaintenanceTag } from '@/components/library/maintenance-tag';

/**
 * 找方案结果里的一行。点这一行（名字、简介任意位置）在右边滑出详情；左边的圆圈只管「放不放进交接单」。
 * 已勾选的里排第一的是「主参考」，其余可以点「设为主参考」。
 */
export function SolveHitRow(props: {
  hit: SolveHit;
  intents: readonly string[];
  pressed: boolean;
  isMain?: boolean;
  scenarios?: ScenarioSummary[];
  onToggle: () => void;
  onOpen?: () => void;
  onMakeMain?: () => void;
  /** 有 AI 精选时才传。不传就和原来一样。 */
  ai?: { fit: RerankFit; reason: string };
}) {
  const { hit } = props;
  const badge = matchBadge(hit, props.intents);
  const oneLiner = hit.oneLiner?.trim() ?? '';
  const related = whyLine(hit);
  const form = formLabel(hit.form);
  const chips = (hit.scenarioIds ?? [])
    .map((id) => props.scenarios?.find((scenario) => scenario.id === id))
    .filter((scenario): scenario is ScenarioSummary => Boolean(scenario))
    .slice(0, 2);

  return (
    <div
      className={cn(
        'group grid grid-cols-[32px_40px_minmax(0,1fr)] items-start gap-2 border-t border-muted px-3 py-3 transition-colors first:border-t-0 min-[1000px]:grid-cols-[32px_40px_minmax(0,1fr)_88px]',
        props.onOpen && 'cursor-pointer hover:bg-surface-container-low',
        props.ai?.fit === 'poor' && 'opacity-60',
      )}
      onClick={props.onOpen}
    >
      <button
        type="button"
        className="grid size-8 shrink-0 place-items-center rounded-full focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary"
        aria-pressed={props.pressed}
        aria-label={props.pressed ? `从交接单移除 ${hit.fullName}` : `加入交接单 ${hit.fullName}`}
        title={props.pressed ? '从交接单移除' : '加入交接单'}
        onClick={(event) => {
          event.stopPropagation();
          props.onToggle();
        }}
      >
        <span
          className={cn(
            'grid size-[18px] place-items-center rounded-full border-[1.5px] transition-colors',
            props.pressed ? 'border-primary bg-primary text-on-primary' : 'border-input bg-card text-transparent group-hover:border-primary',
          )}
        >
          <Icon name="check" size={11} />
        </span>
      </button>
      <RepoAvatar name={hit.fullName} size="md" />
      <div className="min-w-0">
        <div className="flex min-w-0 flex-wrap items-baseline gap-2">
          <span className="text-left text-sm font-semibold break-words text-foreground [overflow-wrap:anywhere]">{hit.fullName}</span>
          <StarCount value={hit.starsCount} />
          {props.pressed && props.isMain ? <Tag tone="blue">主参考</Tag> : null}
          {props.pressed && props.isMain ? <span className="text-xs text-on-surface-variant">Codex 会优先用它，其余是备选</span> : null}
          {form ? <Tag>{form}</Tag> : null}
          {hit.hasCard === false ? <Tag>待 AI 整理</Tag> : null}
          <MaintenanceTag tier={maintenanceOf(hit.pushedAt, new Date(), hit.archived === true)} />
          {badge ? <Tag tone={badge.tone}>{badge.text}</Tag> : null}
          {props.ai ? <Tag tone={props.ai.fit === 'best' ? 'blue' : 'gray'}>{RERANK_FIT_LABEL[props.ai.fit]}</Tag> : null}
          {chips.map((scenario) => (
            <ScenarioChip key={scenario.id} name={scenario.nameZh} scenarioKey={scenario.slug || scenario.id} />
          ))}
        </div>
        {oneLiner ? (
          <p className={cn('mt-0.5 mb-0 line-clamp-3 text-sm leading-relaxed', hit.hasCard === false ? 'text-on-surface-variant' : 'text-foreground')}>
            {hit.hasCard === false ? <span className="mr-1 text-xs text-subtle">GitHub 原文</span> : null}
            {oneLiner}
          </p>
        ) : null}
        {props.ai?.reason ? (
          <div className="mt-0.5 text-xs leading-5 text-on-surface-variant">{props.ai.reason}</div>
        ) : null}
        {related ? (
          <div className="mt-0.5 text-xs leading-5 text-on-surface-variant">{related}</div>
        ) : null}
        {props.pressed && !props.isMain && props.onMakeMain ? (
          <button
            type="button"
            className="mt-1 text-xs text-accent-foreground hover:underline hover:underline-offset-[3px]"
            onClick={(event) => {
              event.stopPropagation();
              props.onMakeMain?.();
            }}
          >
            设为主参考
          </button>
        ) : null}
      </div>
      <RowCover id={hit.id} fullName={hit.fullName} />
    </div>
  );
}

/** 右边的一小张封面（README 里的第一张截图，没有就是渐变底），宽窗口才显示：一眼认出「哪个是做什么的」。 */
function RowCover(props: { id: string; fullName: string }) {
  const cover = useRepoCover(props.id, props.fullName);
  return (
    <CoverImage
      src={cover}
      seed={props.fullName}
      className="hidden h-[60px] w-[88px] shrink-0 rounded-md border border-border min-[1000px]:block"
    />
  );
}
