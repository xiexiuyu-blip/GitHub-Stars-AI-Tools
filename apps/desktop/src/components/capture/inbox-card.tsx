import type { KeyboardEvent } from 'react';
import type { InboxItem, InboxPage } from '@/lib/contracts';
import { Button, Card, CardHeader, EmptyState, RepoAvatar, Segmented, Spinner, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { relativeDays } from '@/lib/item-format';
import {
  archiveAction,
  canOpenUrl,
  emptyInboxCopy,
  githubFullName,
  inboxContext,
  inboxFilterOptions,
  inboxHeading,
  inboxMeta,
  inboxStatusTag,
  kindIcon,
  type InboxFilter,
} from '@/lib/capture-view';

export function InboxCard(props: {
  filter: InboxFilter;
  page: InboxPage | null;
  phase: 'loading' | 'ready' | 'error';
  error: string | null;
  loadingMore: boolean;
  busyId: string | null;
  editingId: string | null;
  draft: string;
  noteMessage: string | null;
  savingNote: boolean;
  onFilter: (value: InboxFilter) => void;
  onLoadMore: () => void;
  onRetry: () => void;
  onStartNote: (item: InboxItem) => void;
  onDraft: (value: string) => void;
  onSaveNote: () => void;
  onCancelNote: () => void;
  onArchive: (item: InboxItem) => void;
}) {
  const counts = props.page?.counts ?? { pending: 0, ready: 0, failed: 0, archived: 0 };
  const options = inboxFilterOptions(counts);
  const headerCount = props.phase === 'loading' ? '…' : props.page ? props.page.totalCount : undefined;
  const hasMore = Boolean(props.page && props.page.items.length < props.page.totalCount);

  return (
    <Card>
      <CardHeader
        title="收集箱"
        count={headerCount}
        actions={(
          <Segmented
            ariaLabel="收集箱筛选"
            options={options}
            value={props.filter}
            onChange={props.onFilter}
          />
        )}
      />
      {renderBody()}
      {hasMore ? (
        <div className="flex justify-center border-t border-muted px-4 py-3">
          <Button loading={props.loadingMore} onClick={props.onLoadMore}>加载更多</Button>
        </div>
      ) : null}
    </Card>
  );

  function renderBody() {
    if (props.phase === 'error' && (!props.page || props.page.items.length === 0)) {
      return (
        <div className="flex flex-col items-center gap-2 px-4 py-8 text-center">
          <p className="text-sm text-error">{props.error ?? '收集箱没有读出来。'}</p>
          <Button onClick={props.onRetry}>重试</Button>
        </div>
      );
    }
    if (props.phase === 'loading' && (!props.page || props.page.items.length === 0)) {
      return (
        <div className="flex items-center justify-center gap-2 px-4 py-8 text-sm text-on-surface-variant">
          <Spinner />
          正在读取收集箱…
        </div>
      );
    }
    if (!props.page || props.page.items.length === 0) {
      const empty = emptyInboxCopy(props.filter);
      return <EmptyState icon="inbox" title={empty.title} description={empty.description} />;
    }
    return (
      <div>
        {props.page.items.map((item) => (
          <InboxRow
            key={item.id}
            item={item}
            busy={props.busyId === item.id}
            editing={props.editingId === item.id}
            draft={props.draft}
            noteMessage={props.editingId === item.id ? props.noteMessage : null}
            savingNote={props.savingNote && props.editingId === item.id}
            onStartNote={props.onStartNote}
            onDraft={props.onDraft}
            onSaveNote={props.onSaveNote}
            onCancelNote={props.onCancelNote}
            onArchive={props.onArchive}
          />
        ))}
      </div>
    );
  }
}

function InboxRow(props: {
  item: InboxItem;
  busy: boolean;
  editing: boolean;
  draft: string;
  noteMessage: string | null;
  savingNote: boolean;
  onStartNote: (item: InboxItem) => void;
  onDraft: (value: string) => void;
  onSaveNote: () => void;
  onCancelNote: () => void;
  onArchive: (item: InboxItem) => void;
}) {
  const title = inboxHeading(props.item.title, props.item.normalizedUrl);
  const repoName = githubFullName(props.item);
  const meta = inboxMeta(
    inboxContext(props.item.sourceContext, props.item.platform),
    relativeDays(props.item.createdAt, new Date()),
  );
  const tag = inboxStatusTag(props.item.status);
  const action = archiveAction(props.item.status);
  const errorId = `inbox-note-error-${props.item.id}`;

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.nativeEvent.isComposing || event.key === 'Process') return;
    if (event.key === 'Enter') {
      event.preventDefault();
      props.onSaveNote();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      props.onCancelNote();
    }
  }

  return (
    <div className="group grid grid-cols-[32px_minmax(0,1fr)_auto] items-start gap-2.5 border-t border-muted px-4 py-2.5 first:border-t-0">
      {repoName ? (
        <RepoAvatar name={repoName} size="sm" />
      ) : (
        <span className="grid size-8 place-items-center rounded-md bg-accent text-accent-foreground">
          <Icon name={kindIcon(props.item.itemType)} size={16} />
        </span>
      )}
      <div className="min-w-0">
        <b className="block truncate text-sm font-medium text-foreground" title={title}>{title}</b>
        {meta ? <div className="truncate text-xs text-on-surface-variant">{meta}</div> : null}
        {props.editing ? (
          <div className="mt-1.5">
            <div className="flex flex-wrap items-center gap-2">
              <input
                aria-label="编辑备注"
                aria-invalid={props.noteMessage ? true : undefined}
                aria-describedby={props.noteMessage ? errorId : undefined}
                autoFocus
                className="h-8 min-w-0 flex-1 rounded-md border border-input bg-card px-2 text-sm text-foreground"
                value={props.draft}
                onChange={(event) => props.onDraft(event.target.value)}
                onKeyDown={onKeyDown}
              />
              <Button size="sm" loading={props.savingNote} onClick={props.onSaveNote}>保存</Button>
              <Button size="sm" variant="text" onClick={props.onCancelNote}>取消</Button>
            </div>
            {props.noteMessage ? (
              <p id={errorId} className="mt-1 text-xs text-error">{props.noteMessage}</p>
            ) : null}
          </div>
        ) : props.item.userNote.trim() ? (
          <div className="truncate text-xs text-on-surface-variant" title={props.item.userNote}>{props.item.userNote}</div>
        ) : null}
      </div>
      <div className="relative flex items-center justify-end">
        {props.editing ? (
          <Tag tone={tag.tone}>
            {tag.spinning ? <Spinner /> : null}
            {tag.text}
          </Tag>
        ) : (
          <>
            <div className="group-hover:invisible group-focus-within:invisible">
              <Tag tone={tag.tone}>
                {tag.spinning ? <Spinner /> : null}
                {tag.text}
              </Tag>
            </div>
            <div className="pointer-events-none absolute top-1/2 right-0 flex -translate-y-1/2 items-center gap-1 opacity-0 group-hover:pointer-events-auto group-hover:opacity-100 group-focus-within:pointer-events-auto group-focus-within:opacity-100">
              <Button size="sm" onClick={() => props.onStartNote(props.item)}>备注</Button>
              <Button size="sm" disabled={props.busy} onClick={() => props.onArchive(props.item)}>{action.label}</Button>
              {canOpenUrl(props.item.originalUrl) ? (
                <a
                  className="inline-flex h-7 items-center rounded-md border border-input bg-card px-2.5 text-sm whitespace-nowrap text-foreground hover:bg-surface-container-low"
                  href={props.item.originalUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  打开
                </a>
              ) : null}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
