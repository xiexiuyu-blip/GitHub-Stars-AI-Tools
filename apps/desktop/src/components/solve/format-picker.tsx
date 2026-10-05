import type { PackFormat } from '@/lib/api';
import type { FormatOption } from '@/lib/solve-view';
import { cn } from '@/lib/utils';

/**
 * 交接单的格式：竖着排的单选列表，每项一行「名字 + 一句说明」，选中的有底色。
 * 完整一句最长 27 个字，面板只有 340–460 px 宽，所以不用横排的分段按钮（按钮不换行，会撑破面板）：
 * 这里的文字会自己换行。
 *
 * 用的是真正的单选框（看不见，但键盘能用：Tab 进来，方向键换，空格选），点整行也行。
 */
export function FormatPicker(props: {
  options: readonly FormatOption[];
  value: PackFormat;
  onChange: (value: PackFormat) => void;
  /** 同一组单选框共用的 name，浏览器才会把它们当成一组（方向键才能在三项之间换） */
  name?: string;
}) {
  const name = props.name ?? 'pack-format';
  return (
    <div role="radiogroup" aria-label="格式" className="flex shrink-0 flex-col gap-0.5 border-b border-muted px-2 py-1.5">
      {props.options.map((option) => {
        const selected = option.value === props.value;
        return (
          <label key={option.value} className="block cursor-pointer">
            <input
              type="radio"
              className="peer sr-only"
              name={name}
              value={option.value}
              aria-label={option.label}
              checked={selected}
              onChange={() => props.onChange(option.value)}
            />
            <span
              className={cn(
                'grid grid-cols-[14px_minmax(0,1fr)] items-start gap-x-2 rounded-md px-2.5 py-1.5',
                'peer-focus-visible:outline peer-focus-visible:outline-2 peer-focus-visible:outline-offset-1 peer-focus-visible:outline-primary',
                selected ? 'bg-accent' : 'hover:bg-surface-container-low',
              )}
            >
              <span
                aria-hidden="true"
                className={cn(
                  'mt-[3px] size-3.5 rounded-full border-[1.5px]',
                  selected ? 'border-primary bg-primary shadow-[inset_0_0_0_2.5px_var(--color-accent)]' : 'border-input bg-card',
                )}
              />
              <span className="min-w-0 text-sm leading-5 break-words [overflow-wrap:anywhere]">
                <span className="font-medium text-foreground">{option.name}</span>
                <span className="ml-2 text-xs text-on-surface-variant">{option.note}</span>
              </span>
            </span>
          </label>
        );
      })}
    </div>
  );
}
