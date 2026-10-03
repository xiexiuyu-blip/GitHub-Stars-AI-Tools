import { invoke } from '@tauri-apps/api/core';
import type { ScenarioSummary, SolveHit } from '@/lib/contracts';
import { formLabel } from '@/lib/library-filters';
import { matchBadge, whyLine } from '@/lib/solve-view';
import { RepoAvatar, ScenarioChip, StarCount, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

export function SolveHitRow(props: {
  hit: SolveHit;
  intents: readonly string[];
  pressed: boolean;
  scenarios?: ScenarioSummary[];
  onToggle: () => void;
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
    <div className="grid grid-cols-[18px_40px_minmax(0,1fr)] items-start gap-3 border-t border-muted px-4 py-3 first:border-t-0">
      <button
        type="button"
        className={cn(
          'mt-[5px] grid size-[18px] shrink-0 place-items-center rounded-full border-[1.5px]',
          'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary',
          props.pressed
            ? 'border-primary bg-primary text-on-primary'
            : 'border-input bg-card text-transparent hover:border-primary',
        )}
        aria-pressed={props.pressed}
        aria-label={props.pressed ? `从参考包移除 ${hit.fullName}` : `选入参考包 ${hit.fullName}`}
        onClick={props.onToggle}
      >
        <Icon name="check" size={11} />
      </button>
      <RepoAvatar name={hit.fullName} size="md" />
      <div className="min-w-0">
        <div className="flex min-w-0 flex-wrap items-baseline gap-2">
          <button
            type="button"
            className="text-left text-sm font-medium break-words text-foreground hover:text-accent-foreground [overflow-wrap:anywhere]"
            onClick={() => openOnGitHub(hit)}
          >
            {hit.fullName}
          </button>
          <StarCount value={hit.starsCount} />
          {form ? <Tag>{form}</Tag> : null}
          {badge ? <Tag tone={badge.tone}>{badge.text}</Tag> : null}
          {chips.map((scenario) => (
            <ScenarioChip key={scenario.id} name={scenario.nameZh} scenarioKey={scenario.slug || scenario.id} />
          ))}
        </div>
        {oneLiner ? <p className="mt-0.5 mb-0 text-sm leading-relaxed text-foreground">{oneLiner}</p> : null}
        {related ? (
          <div className="mt-0.5 text-xs leading-5 text-on-surface-variant">
            <b className="font-medium">为什么相关</b>
            {` ${related}`}
          </div>
        ) : null}
      </div>
    </div>
  );
}

/** 用 App 已有的 open_external_url 在系统浏览器打开。命令不可用时退回 window.open。 */
function openOnGitHub(hit: SolveHit) {
  const explicit = hit.htmlUrl?.trim() ?? '';
  const url = /^https?:\/\//i.test(explicit)
    ? explicit
    : `https://github.com/${encodeURI(hit.fullName.trim())}`;
  if (!/^https?:\/\//i.test(url)) return;
  void invoke('open_external_url', { url }).catch(() => {
    window.open(url, '_blank', 'noopener,noreferrer');
  });
}
