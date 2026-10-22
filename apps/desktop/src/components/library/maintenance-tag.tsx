import { Tag } from '@/components/kit';
import type { MaintenanceTier } from '@/lib/contracts';
import { MAINTENANCE_HINTS, MAINTENANCE_LABELS } from '@/lib/maintenance';

const TONE: Record<MaintenanceTier, 'gray' | 'blue' | 'red'> = {
  active: 'blue',
  slow: 'gray',
  stale: 'red',
  archived: 'red',
};

/** 维护状态小标签。列表行不画活跃（`showActive` 为假）；没有状态不画。 */
export function MaintenanceTag(props: { tier: MaintenanceTier | null; showActive?: boolean }) {
  if (props.tier == null) return null;
  if (!props.showActive && props.tier === 'active') return null;
  return (
    <span className="inline-flex shrink-0" title={MAINTENANCE_HINTS[props.tier]}>
      <Tag tone={TONE[props.tier]}>{MAINTENANCE_LABELS[props.tier]}</Tag>
    </span>
  );
}
