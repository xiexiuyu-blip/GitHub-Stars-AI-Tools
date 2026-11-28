import { useState } from 'react';
import type { CSSProperties, KeyboardEvent } from 'react';
import type { InboxItem, InboxPage } from '@/lib/contracts';
import { Button, Card, CardHeader, EmptyState, RepoAvatar, Segmented, Spinner, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { relativeDays } from '@/lib/item-format';
import { cn } from '@/lib/utils';
import {
  archiveAction,
  canOpenUrl,
  emptyInboxCopy,
  fetchLine,
  githubFullName,
  inboxByline,
  inboxContext,
  inboxExcerpt,
  inboxFilterOptions,
  inboxHeading,
  inboxMeta,
  inboxQuote,
  mentionLine,
  mentionSummary,
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
  /** 后台正在联网读取收集箱里的链接（标题和摘要） */
  fetching: boolean;
  /** 正在把某条文章里提到的新 GitHub 项目放进收集箱 */
  collectingId: string | null;
  onCollectRepos: (item: InboxItem) => void;
  onFilter: (value: InboxFilter) => void;
  onLoadMore: () => void;
  onRetry: () => void;
  onStartNote: (item: InboxItem) => void;
  onDraft: (value: string) => void;
  onSaveNote: () => void;
  onCancelNote: () => void;
  onArchive: (item: InboxItem) => void;
  onOpenSource?: (id: string, editOnOpen: boolean) => void;
  onRetryFetch: (item: InboxItem) => void;
}) {
  const counts = props.page?.counts ?? { pending: 0, ready: 0, failed: 0, archived: 0 };
  const options = inboxFilterOptions(counts);
  const hasMore = Boolean(props.page && props.page.items.length < props.page.totalCount);

  return (
    <Card>
      {/* 标题上不放数字：它只是当前页签里有几条，放在「收集箱」后面像是总数。各页签的个数在页签上。 */}
      <CardHeader
        title="收集箱"
        actions={(
          <Segmented
            ariaLabel="收集箱筛选"
            options={options}
            value={props.filter}
            onChange={props.onFilter}
          />
        )}
      />
      {props.filter === 'pending' && props.page && props.page.items.length > 0 ? (
        <div className="m-0 border-b border-muted bg-surface-container-low px-4 py-2 text-xs leading-[1.6] text-on-surface-variant">
          <p className="m-0">收集箱：链接先放这里，还没有进资料库。</p>
          <p className="m-0">GitHub 项目：想留的，去 GitHub 上点 Star（收藏），再点顶部「同步」，就会进资料库。是否自动写中文说明，看「设置 → 自动整理」里自动整理开没开。</p>
          <p className="m-0">文章 / 帖子：里面提到的 GitHub 项目会列出来，可以一键放进收集箱；没用的点「归档」。</p>
        </div>
      ) : null}
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
            fetching={props.fetching}
            collecting={props.collectingId === item.id}
            onCollectRepos={props.onCollectRepos}
            onRetryFetch={props.onRetryFetch}
            onStartNote={props.onStartNote}
            onDraft={props.onDraft}
            onSaveNote={props.onSaveNote}
            onCancelNote={props.onCancelNote}
            onArchive={props.onArchive}
            onOpenSource={props.onOpenSource}
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
  fetching: boolean;
  collecting: boolean;
  onCollectRepos: (item: InboxItem) => void;
  onRetryFetch: (item: InboxItem) => void;
  onStartNote: (item: InboxItem) => void;
  onDraft: (value: string) => void;
  onSaveNote: () => void;
  onCancelNote: () => void;
  onArchive: (item: InboxItem) => void;
  onOpenSource?: (id: string, editOnOpen: boolean) => void;
}) {
  const title = inboxHeading(props.item.title, props.item.normalizedUrl);
  const repoName = githubFullName(props.item);
  const meta = inboxByline(
    props.item.pageAuthor,
    inboxMeta(
      inboxContext(props.item.sourceContext, props.item.platform),
      relativeDays(props.item.createdAt, new Date()),
    ),
  );
  const excerpt = inboxExcerpt(props.item);
  const quote = inboxQuote(props.item);
  const mentions = mentionSummary(props.item.githubRepos);
  const [mentionsOpen, setMentionsOpen] = useState(false);
  const fetchNote = fetchLine(props.item, props.fetching);
  const tag = inboxStatusTag(props.item.status);
  const action = archiveAction(props.item.status);
  const errorId = `inbox-note-error-${props.item.id}`;
  const needsBody = props.item.fetchStatus === 'failed' && !props.item.hasSourceBody;

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
        {excerpt ? <p className="mt-0.5 mb-0 line-clamp-2 text-xs leading-[1.6] text-on-surface-variant">{excerpt}</p> : null}
        {quote ? (
          <p className="mt-0.5 mb-0 line-clamp-2 border-l-2 border-muted pl-2 text-xs leading-[1.6] text-on-surface-variant" title={quote}>
            {quote}
          </p>
        ) : null}
        {mentions ? (
          <div className="mt-1">
            <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-on-surface-variant">
              <button
                type="button"
                className="inline-flex items-center gap-0.5 text-accent-foreground hover:underline hover:underline-offset-[3px]"
                aria-expanded={mentionsOpen}
                onClick={() => setMentionsOpen((open) => !open)}
              >
                <Icon name={mentionsOpen ? 'expand_more' : 'chevron_right'} size={16} />
                {mentionLine(mentions)}
              </button>
              {mentions.fresh > 0 ? (
                <Button size="sm" loading={props.collecting} onClick={() => props.onCollectRepos(props.item)}>
                  把 {mentions.fresh} 个新的放进收集箱
                </Button>
              ) : null}
            </div>
            {mentionsOpen ? (
              <ul className="mt-1.5 mb-0 flex list-none flex-wrap gap-1.5 p-0">
                {(props.item.githubRepos ?? []).map((mention) => (
                  <li key={mention.fullName} className="inline-flex items-center gap-1.5 rounded-md border border-border bg-card px-2 py-0.5 text-xs">
                    <a
                      className="text-foreground hover:underline hover:underline-offset-[3px]"
                      href={`https://github.com/${mention.fullName}`}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      {mention.fullName}
                    </a>
                    {mention.starred ? (
                      <Tag tone="blue">已收藏</Tag>
                    ) : mention.inInbox ? (
                      <Tag>已在收集箱</Tag>
                    ) : (
                      <span className="scn-chip" style={{ '--h': 150 } as CSSProperties}>新的</span>
                    )}
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        ) : null}
        {meta ? <div className="truncate text-xs text-subtle">{meta}</div> : null}
        {fetchNote ? (
          <div className={cn('mt-0.5 flex items-center gap-1.5 text-xs', fetchNote.tone === 'red' ? 'text-error' : 'text-on-surface-variant')}>
            {fetchNote.spinning ? <Spinner /> : null}
            <span>{fetchNote.text}</span>
            {fetchNote.retry ? (
              <button
                type="button"
                className="text-accent-foreground hover:underline hover:underline-offset-[3px]"
                disabled={props.fetching}
                onClick={() => props.onRetryFetch(props.item)}
              >
                重试
              </button>
            ) : null}
          </div>
        ) : null}
        {props.onOpenSource && props.item.canOpenSource ? (
          <Button
            size="sm"
            variant="text"
            onClick={() => props.onOpenSource?.(props.item.id, needsBody)}
          >
            {needsBody ? '补贴正文' : '查看原文'}
          </Button>
        ) : null}
        {props.onOpenSource && props.item.sources?.length ? (
          <details className="mt-1 text-xs text-on-surface-variant">
            <summary className="cursor-pointer">出处 {props.item.sources.length} 条</summary>
            <ul className="m-0 mt-1 flex list-none flex-col items-start p-0">
              {props.item.sources.map((entry) => (
                <li key={entry.id} className="max-w-full">
                  <Button
                    variant="text"
                    className="max-w-full"
                    onClick={() => props.onOpenSource?.(entry.id, false)}
                  >
                    <span className="truncate">{entry.title || '查看原文'}</span>
                  </Button>
                </li>
              ))}
            </ul>
          </details>
        ) : null}
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
