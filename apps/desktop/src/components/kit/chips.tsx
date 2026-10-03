import type { CSSProperties } from 'react';
import { Icon } from '@/components/ui/icon';
import { languageColor, scenarioHue } from '@/lib/repo-assets';
import { formatStars } from '@/lib/item-format';
import { cn } from '@/lib/utils';

/** 业务场景标签：每个场景一个固定的颜色，列表里看颜色就知道是哪个方向。 */
export function ScenarioChip(props: { name: string; scenarioKey?: string | null; className?: string }) {
  const style = { '--h': scenarioHue(props.scenarioKey ?? props.name) } as CSSProperties;
  return (
    <span className={cn('scn-chip', props.className)} style={style}>
      {props.name}
    </span>
  );
}

/** 编程语言：一个小圆点（GitHub 上同一种颜色）加语言名。不认识的语言用灰点。 */
export function LangDot(props: { language: string | null | undefined; className?: string }) {
  const language = props.language?.trim();
  if (!language) return null;
  return (
    <span className={cn('inline-flex items-center gap-1 text-xs text-on-surface-variant', props.className)}>
      <span className="size-2 rounded-full" style={{ background: languageColor(language) ?? 'var(--color-outline)' }} aria-hidden="true" />
      {language}
    </span>
  );
}

/** 星数：黄色实心星 + 数字，一眼找到「大家都在用的」。 */
export function StarCount(props: { value: number; className?: string }) {
  return (
    <span className={cn('inline-flex items-center gap-0.5 text-xs font-medium text-on-surface-variant tabular-nums', props.className)}>
      <Icon name="star" size={14} fill className="text-star" />
      {formatStars(props.value)}
    </span>
  );
}
