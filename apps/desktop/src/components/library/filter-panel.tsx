import type { ReactNode } from 'react';
import type { MaintenanceTier, ReadingGroup } from '@/lib/contracts';
import type { FormFilterRow, MaintenanceFilterRow, ScenarioFilterRow } from '@/lib/library-filters';
import { readingGroupLabel } from '@/lib/item-format';
import { scenarioHue } from '@/lib/repo-assets';
import { Button } from '@/components/kit';
import { cn } from '@/lib/utils';
import { ThinTag } from './thin-tag';

const READING_GROUPS: ReadingGroup[] = ['unseen', 'want', 'tried', 'in_use', 'dropped'];

export function FilterPanel(props: {
  scenarios: ScenarioFilterRow[];
  scenarioStatus: 'loading' | 'ready' | 'error';
  scenarioError?: string | null;
  onRetryScenarios?: () => void;
  readingCounts: Record<ReadingGroup, number>;
  forms: FormFilterRow[];
  maintenance: MaintenanceFilterRow[];
  countsKnown: boolean;
  scenario: string | null;
  reading: ReadingGroup | null;
  form: string | null;
  maintenanceTier: MaintenanceTier | null;
  onScenario: (id: string) => void;
  onReading: (group: ReadingGroup) => void;
  onForm: (id: string) => void;
  onMaintenance: (tier: MaintenanceTier) => void;
}) {
  return (
    <div className="min-h-0 flex-1 overflow-auto py-2">
      <GroupTitle>场景</GroupTitle>
      {props.scenarioStatus === 'loading' ? <Hint>正在读取…</Hint> : null}
      {props.scenarioStatus === 'error' ? (
        <div className="flex items-center gap-2 px-4 py-1.5">
          <p className="min-w-0 flex-1 text-xs text-on-surface-variant">{props.scenarioError || '场景没有读出来'}</p>
          {props.onRetryScenarios ? <Button variant="text" onClick={props.onRetryScenarios}>重试</Button> : null}
        </div>
      ) : null}
      {props.scenarios.map((row) => (
        <FilterButton
          key={row.id}
          label={row.nameZh}
          dotHue={scenarioHue(row.id)}
          count={row.count}
          countsKnown={props.countsKnown}
          selected={props.scenario === row.id}
          extra={row.thin ? <ThinTag /> : null}
          onClick={() => props.onScenario(row.id)}
        />
      ))}

      <GroupTitle>我的状态</GroupTitle>
      {READING_GROUPS.map((group) => (
        <FilterButton
          key={group}
          label={readingGroupLabel(group)}
          count={props.readingCounts[group]}
          countsKnown={props.countsKnown}
          selected={props.reading === group}
          onClick={() => props.onReading(group)}
        />
      ))}

      <GroupTitle>类型</GroupTitle>
      {props.countsKnown ? null : <Hint>…</Hint>}
      {props.forms.map((row) => (
        <FilterButton
          key={row.id}
          label={row.label}
          title={row.hint ?? undefined}
          count={row.count}
          countsKnown={props.countsKnown}
          selected={props.form === row.id}
          onClick={() => props.onForm(row.id)}
        />
      ))}

      <GroupTitle>维护状态</GroupTitle>
      {props.maintenance.map((row) => (
        <FilterButton
          key={row.tier}
          label={row.label}
          title={row.hint}
          count={row.count}
          countsKnown={props.countsKnown}
          selected={props.maintenanceTier === row.tier}
          onClick={() => props.onMaintenance(row.tier)}
        />
      ))}
    </div>
  );
}

function GroupTitle(props: { children: string }) {
  return <div className="px-4 pt-3 pb-1 text-xs text-on-surface-variant">{props.children}</div>;
}

function Hint(props: { children: string }) {
  return <p className="px-4 py-1.5 text-xs text-on-surface-variant">{props.children}</p>;
}

function FilterButton(props: {
  label: string;
  /** 鼠标悬停时的长解释（类型的长写法放这里，行里只显示短名字） */
  title?: string;
  /** 场景前面的小圆点（色相），和列表里的场景标签同一种颜色 */
  dotHue?: number;
  count: number;
  countsKnown: boolean;
  selected: boolean;
  extra?: ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={props.selected}
      title={props.title}
      onClick={props.onClick}
      className={cn(
        'mx-2 flex w-[calc(100%-16px)] items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-on-surface-variant',
        'hover:bg-surface-container-low hover:text-foreground',
        props.selected && 'bg-accent text-accent-foreground hover:bg-accent hover:text-accent-foreground',
        // 当前筛选下一个都没有的选项淡一点：能点，但一眼看出点了也是空的
        props.countsKnown && props.count === 0 && !props.selected && 'opacity-50',
      )}
    >
      {props.dotHue !== undefined ? (
        <span className="size-2 shrink-0 rounded-full" style={{ background: `hsl(${props.dotHue} 70% 52%)` }} aria-hidden="true" />
      ) : null}
      <span className="min-w-0 flex-1 truncate">{props.label}</span>
      {props.extra}
      <span className={cn('shrink-0 text-xs font-medium', props.selected ? 'text-accent-foreground' : 'text-on-surface-variant')}>
        {props.countsKnown ? props.count : '…'}
      </span>
    </button>
  );
}
