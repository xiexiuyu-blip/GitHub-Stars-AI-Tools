import type { ReactNode } from 'react';
import type { ReadingGroup } from '@/lib/contracts';
import type { FormFilterRow, ScenarioFilterRow } from '@/lib/library-filters';
import { readingGroupLabel } from '@/lib/item-format';
import { Button, Tag } from '@/components/kit';
import { cn } from '@/lib/utils';

const READING_GROUPS: ReadingGroup[] = ['unseen', 'want', 'tried', 'in_use', 'dropped'];

export function FilterPanel(props: {
  scenarios: ScenarioFilterRow[];
  scenarioStatus: 'loading' | 'ready' | 'error';
  scenarioError?: string | null;
  onRetryScenarios?: () => void;
  readingCounts: Record<ReadingGroup, number>;
  forms: FormFilterRow[];
  countsKnown: boolean;
  scenario: string | null;
  reading: ReadingGroup | null;
  form: string | null;
  onScenario: (id: string) => void;
  onReading: (group: ReadingGroup) => void;
  onForm: (id: string) => void;
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
          count={row.count}
          countsKnown={props.countsKnown}
          selected={props.scenario === row.id}
          extra={row.thin ? <Tag>偏少</Tag> : null}
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

      <GroupTitle>形态</GroupTitle>
      {props.countsKnown ? null : <Hint>…</Hint>}
      {props.forms.map((row) => (
        <FilterButton
          key={row.id}
          label={row.label}
          count={row.count}
          countsKnown={props.countsKnown}
          selected={props.form === row.id}
          onClick={() => props.onForm(row.id)}
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
      onClick={props.onClick}
      className={cn(
        'mx-2 flex w-[calc(100%-16px)] items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-on-surface-variant',
        'hover:bg-surface-container-low hover:text-foreground',
        props.selected && 'bg-accent text-accent-foreground hover:bg-accent hover:text-accent-foreground',
      )}
    >
      <span className="min-w-0 flex-1 truncate">{props.label}</span>
      {props.extra}
      <span className={cn('shrink-0 text-xs font-medium', props.selected ? 'text-accent-foreground' : 'text-on-surface-variant')}>
        {props.countsKnown ? props.count : '…'}
      </span>
    </button>
  );
}
