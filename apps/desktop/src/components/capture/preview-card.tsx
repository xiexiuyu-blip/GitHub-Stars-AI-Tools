import type { CapturePreview, CapturePreviewRow } from '@/lib/contracts';
import { Button, Card, EmptyState, Spinner, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import {
  commitButtonLabel,
  kindIcon,
  previewSummary,
  rowLabel,
  rowStatusTag,
  TOO_LONG_MESSAGE,
  plainBlockedReason,
} from '@/lib/capture-view';
import { cn } from '@/lib/utils';

export function PreviewCard(props: {
  phase: 'idle' | 'loading' | 'error' | 'too-long';
  error: string | null;
  preview: CapturePreview | null;
  canCommit: boolean;
  submitting: boolean;
  onCommit: () => void;
  onSaveSource?: () => void;
  onRetry: () => void;
}) {
  const saveSource = props.onSaveSource ? (
    <Button variant="primary" loading={props.submitting} onClick={props.onSaveSource}>
      保存正文
    </Button>
  ) : null;

  if (props.phase === 'too-long') {
    return (
      <Card>
        <p className="px-4 py-6 text-sm text-error">{TOO_LONG_MESSAGE}</p>
      </Card>
    );
  }

  if (props.phase === 'error') {
    return (
      <Card>
        <div className="flex flex-col items-center gap-2 px-4 py-8 text-center">
          <p className="text-sm text-error">{props.error ?? '预览没有完成，请稍后再试。'}</p>
          <div className="flex items-center gap-2">
            <Button onClick={props.onRetry}>重试</Button>
            {saveSource}
          </div>
        </div>
      </Card>
    );
  }

  if (props.phase === 'loading') {
    return (
      <Card>
        <div className="flex items-center justify-center gap-2 px-4 py-8 text-sm text-on-surface-variant">
          <Spinner />
          正在识别链接…
        </div>
      </Card>
    );
  }

  if (!props.preview || props.preview.rows.length === 0) {
    if (saveSource) {
      return (
        <Card className="flex flex-wrap items-center justify-between gap-3 px-4 py-3">
          <p className="m-0 text-sm text-on-surface-variant">可以把这段文字保存到收集箱。</p>
          {saveSource}
        </Card>
      );
    }
    return (
      <Card>
        <EmptyState
          icon="link"
          title="还没有链接"
          description="把群聊或帖子粘贴到上面，链接会在这里列出来"
        />
      </Card>
    );
  }

  const summary = previewSummary(props.preview);
  const newCount = props.canCommit ? props.preview.counts.new : 0;

  return (
    <Card>
      <div className="flex flex-wrap items-center gap-3 px-4 py-3">
        <p className="m-0 min-w-0 flex-1 text-sm text-foreground" aria-live="polite">{summary}</p>
        <Button
          variant={saveSource ? 'default' : 'primary'}
          loading={props.submitting}
          disabled={!props.canCommit}
          onClick={props.onCommit}
        >
          {saveSource ? `只放链接（${newCount}）` : commitButtonLabel(newCount)}
        </Button>
        {saveSource}
      </div>
      <div>
        {props.preview.rows.map((row, index) => (
          <PreviewRow key={`${row.normalizedUrl}-${index}`} row={row} />
        ))}
      </div>
    </Card>
  );
}

function PreviewRow(props: { row: CapturePreviewRow }) {
  const label = rowLabel(props.row);
  const tag = rowStatusTag(props.row);
  return (
    <div className="grid grid-cols-[28px_minmax(0,1fr)_auto] items-center gap-2.5 border-t border-muted px-4 py-2.5">
      <span className={cn('grid size-7 place-items-center rounded-md', iconTone(props.row.status))}>
        <Icon name={kindIcon(props.row.kind)} size={16} />
      </span>
      <div className="min-w-0">
        <b className="block truncate text-sm font-medium text-foreground" title={label}>{label}</b>
        {tag.reason ? <div className="truncate text-xs text-error" title={tag.reason}>{plainBlockedReason(tag.reason)}</div> : null}
      </div>
      <Tag tone={tag.tone}>{tag.text}</Tag>
    </div>
  );
}

function iconTone(status: CapturePreviewRow['status']): string {
  if (status === 'new') return 'bg-accent text-accent-foreground';
  if (status === 'blocked') return 'bg-error-container text-on-error-container';
  return 'bg-muted text-on-surface-variant';
}
