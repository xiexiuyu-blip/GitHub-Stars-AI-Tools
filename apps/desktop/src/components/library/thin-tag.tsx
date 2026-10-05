import { Tag } from '@/components/kit';
import { THIN_TAG, THIN_TAG_HINT } from '@/lib/item-format';

/**
 * 场景「偏少」的小标签。资料库左栏和发现页的左栏只有 200–220px 宽，标签只写「偏少」，
 * 「14 个以内」放在鼠标悬停提示里；外面包一层 `inline-flex shrink-0`（不撑出多余的行高，也不让标签被挤扁），
 * 窄的时候被挤的是场景名（场景名那段有 `truncate`），不是标签。
 */
export function ThinTag(props: { tone?: 'gray' | 'blue' }) {
  return (
    <span className="inline-flex shrink-0" title={THIN_TAG_HINT}>
      <Tag tone={props.tone}>{THIN_TAG}</Tag>
    </span>
  );
}
