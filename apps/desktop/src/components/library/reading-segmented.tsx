import type { ReadingGroup } from '@/lib/contracts';
import { readingGroupLabel } from '@/lib/item-format';
import { Segmented } from '@/components/kit';
import { cn } from '@/lib/utils';

const GROUPS: ReadingGroup[] = ['unseen', 'want', 'tried', 'in_use', 'dropped'];

export function ReadingSegmented(props: {
  value: ReadingGroup;
  onChange: (group: ReadingGroup) => void;
  className?: string;
}) {
  return (
    <div className={cn('flex flex-wrap items-center gap-2.5', props.className)}>
      <span className="text-xs text-on-surface-variant">我的状态</span>
      <Segmented
        ariaLabel="我的状态"
        options={GROUPS.map((value) => ({ value, label: readingGroupLabel(value) }))}
        value={props.value}
        onChange={props.onChange}
      />
    </div>
  );
}
