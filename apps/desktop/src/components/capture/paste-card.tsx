import { Card } from '@/components/kit';
import { CAPTURE_SOURCES, type CaptureSource } from '@/lib/capture-view';

export function PasteCard(props: {
  text: string;
  source: CaptureSource;
  note: string;
  disabled?: boolean;
  onText: (value: string) => void;
  onSource: (value: CaptureSource) => void;
  onNote: (value: string) => void;
}) {
  return (
    <Card className="flex flex-col gap-3 p-4">
      <textarea
        aria-label="粘贴内容"
        className="min-h-[150px] w-full resize-y rounded-lg border border-input bg-card px-3.5 py-3 text-sm leading-relaxed text-foreground placeholder:text-on-surface-variant disabled:opacity-60"
        disabled={props.disabled}
        placeholder="粘贴一段聊天记录或帖子"
        value={props.text}
        onChange={(event) => props.onText(event.target.value)}
      />
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-xs text-on-surface-variant">来源</span>
        <select
          aria-label="来源"
          className="h-8 rounded-md border border-input bg-card px-2 text-sm text-foreground disabled:opacity-60"
          disabled={props.disabled}
          value={props.source}
          onChange={(event) => {
            const value = event.target.value;
            if ((CAPTURE_SOURCES as readonly string[]).includes(value)) {
              props.onSource(value as CaptureSource);
            }
          }}
        >
          {CAPTURE_SOURCES.map((option) => (
            <option key={option} value={option}>{option}</option>
          ))}
        </select>
        <input
          aria-label="备注"
          className="h-8 w-full max-w-[280px] rounded-md border border-input bg-card px-2.5 text-sm text-foreground placeholder:text-on-surface-variant disabled:opacity-60"
          disabled={props.disabled}
          placeholder="备注，比如：某某群 10 月 3 日"
          value={props.note}
          onChange={(event) => props.onNote(event.target.value)}
        />
      </div>
    </Card>
  );
}
