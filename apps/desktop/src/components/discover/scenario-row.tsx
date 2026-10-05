import type { DiscoverScenario } from '@/lib/contracts';
import { candidateCountHint, candidateCountText } from '@/lib/discover-view';
import { scenarioHue } from '@/lib/repo-assets';
import { ThinTag } from '@/components/library/thin-tag';
import { cn } from '@/lib/utils';

/**
 * 「发现」左栏里的一行场景：颜色点 + 场景名（太长就截断，不让它把别的挤没）+ 偏少标签 + 候选个数。
 * 窄屏时左栏变成横排的一排标签，这一行跟着变成不折行的小按钮。
 */
export function ScenarioRow(props: { scenario: DiscoverScenario; active: boolean; onClick: () => void }) {
  const { scenario, active } = props;
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={props.onClick}
      className={cn(
        'mx-2 flex w-[calc(100%-16px)] items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-on-surface-variant',
        'max-[899px]:mx-0 max-[899px]:w-auto max-[899px]:shrink-0 max-[899px]:whitespace-nowrap',
        'hover:bg-surface-container-low hover:text-foreground',
        active && 'bg-accent text-accent-foreground hover:bg-accent hover:text-accent-foreground',
      )}
    >
      <span className="size-2 shrink-0 rounded-full" style={{ background: `hsl(${scenarioHue(scenario.id)} 70% 52%)` }} aria-hidden="true" />
      <span className="min-w-0 flex-1 truncate">{scenario.nameZh}</span>
      {scenario.thin ? <ThinTag tone="blue" /> : null}
      <span
        className="shrink-0 text-xs font-medium tabular-nums"
        title={candidateCountHint(scenario.candidateCount, scenario.lastFetchedAt)}
      >
        {candidateCountText(scenario.candidateCount, scenario.lastFetchedAt)}
      </span>
    </button>
  );
}
