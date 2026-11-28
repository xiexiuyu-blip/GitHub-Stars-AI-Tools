import { useEffect, useId, useState } from 'react';
import { Dialog } from 'radix-ui';
import { Button, Spinner, Tag } from '@/components/kit';
import {
  collectCaptureCandidates,
  describeInvokeError,
  getCaptureSource,
  saveCaptureSourceBody,
} from '@/lib/api';
import {
  canOpenUrl,
  isTooLong,
  plainBlockedReason,
  rowLabel,
  rowStatusTag,
  TOO_LONG_MESSAGE,
} from '@/lib/capture-view';
import type { CaptureSourceView } from '@/lib/contracts';

export function CaptureSourceDialog(props: {
  id: string;
  editOnOpen: boolean;
  onClose: () => void;
  onChanged: (created: number) => void;
}) {
  const { id, onClose, onChanged } = props;
  const textId = useId();
  const [view, setView] = useState<CaptureSourceView | null>(null);
  const [edit, setEdit] = useState(props.editOnOpen);
  const [draft, setDraft] = useState('');
  const [picked, setPicked] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const [retry, setRetry] = useState(0);
  const [busy, setBusy] = useState<'body' | 'candidates' | null>(null);

  useEffect(() => {
    let active = true;
    setError(null);
    getCaptureSource(id).then((next) => {
      if (!active) return;
      setView(next);
      setDraft(next.body);
    }).catch((reason: unknown) => {
      if (active) setError(describeInvokeError(reason));
    });
    return () => { active = false; };
  }, [id, retry]);

  const available = (view?.candidates ?? []).filter((candidate) => !candidate.linked && candidate.status !== 'blocked');
  const draftTooLong = isTooLong(draft);

  async function saveBody() {
    if (!draft.trim() || draftTooLong || busy) return;
    setError(null);
    setNotice('');
    setBusy('body');
    try {
      setView(await saveCaptureSourceBody(id, draft));
      setEdit(false);
      setPicked([]);
      onChanged(0);
    } catch (reason) {
      setError(describeInvokeError(reason));
    } finally {
      setBusy(null);
    }
  }

  async function confirmCandidates() {
    if (!picked.length || busy) return;
    setError(null);
    setNotice('');
    setBusy('candidates');
    try {
      const result = await collectCaptureCandidates(id, picked);
      setView(result.source);
      setPicked([]);
      setNotice(result.created
        ? `新收 ${result.created} 条，记录了 ${result.linked} 个出处。`
        : result.linked
          ? `已为现有条目新增 ${result.linked} 个出处。`
          : '所选链接已关联到这篇原文。');
      onChanged(result.created);
    } catch (reason) {
      setError(describeInvokeError(reason));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Dialog.Root open onOpenChange={(open) => { if (!open && !busy) onClose(); }}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/45" />
        <Dialog.Content className="fixed top-1/2 left-1/2 z-50 flex max-h-[min(90vh,760px)] w-[min(calc(100%-2rem),42rem)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-card-border bg-card shadow-xl">
          <div className="flex shrink-0 items-start justify-between gap-3 border-b border-muted px-5 py-4">
            <div className="min-w-0">
              <Dialog.Title className="m-0 text-lg font-semibold">原文与候选</Dialog.Title>
              <Dialog.Description className="mt-1 mb-0 line-clamp-2 text-sm break-words text-on-surface-variant">
                {view?.title || '查看正文和其中提到的链接。'}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <Button variant="text" disabled={Boolean(busy)}>关闭</Button>
            </Dialog.Close>
          </div>
          <div className="min-h-0 space-y-4 overflow-y-auto px-5 py-4">
            {error ? <p role="alert" className="m-0 text-sm text-error">{error}</p> : null}
            {notice ? <p role="status" className="m-0 rounded-md bg-accent px-3 py-2 text-sm">{notice}</p> : null}
            {!view ? (
              error
                ? <Button onClick={() => setRetry((value) => value + 1)}>重试</Button>
                : <div className="flex items-center gap-2 text-sm"><Spinner />正在读取原文…</div>
            ) : (
              <>
                {canOpenUrl(view.originalUrl) ? (
                  <a href={view.originalUrl} target="_blank" rel="noopener noreferrer" className="text-sm text-accent-foreground hover:underline">
                    打开原链接
                  </a>
                ) : null}
                {edit ? (
                  <div className="space-y-2">
                    <label htmlFor={textId} className="block text-sm font-medium">正文</label>
                    <textarea
                      id={textId}
                      value={draft}
                      onChange={(event) => setDraft(event.target.value)}
                      placeholder="粘贴帖子正文，保留链接和说明"
                      disabled={Boolean(busy)}
                      className="min-h-40 w-full resize-y rounded-md border border-input bg-card p-3 text-sm leading-relaxed"
                    />
                    {draftTooLong ? <p className="text-sm text-error">{TOO_LONG_MESSAGE}</p> : null}
                    <div className="flex items-center gap-2">
                      <Button variant="primary" loading={busy === 'body'} disabled={!draft.trim() || draftTooLong} onClick={() => void saveBody()}>
                        保存正文
                      </Button>
                      <Button variant="text" disabled={Boolean(busy)} onClick={() => { setDraft(view.body); setEdit(false); }}>取消</Button>
                    </div>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {view.body || view.fetchedExcerpt ? (
                      <p className="m-0 max-h-52 overflow-auto rounded-md bg-muted px-3 py-2.5 text-sm leading-relaxed break-words whitespace-pre-wrap">
                        {view.body || view.fetchedExcerpt}
                      </p>
                    ) : null}
                    <Button size="sm" disabled={Boolean(busy)} onClick={() => { setDraft(view.body); setEdit(true); }}>
                      {view.body ? '编辑正文' : '补贴正文'}
                    </Button>
                  </div>
                )}
                {!edit && view.candidates.length > 0 ? (
                  <div>
                    <div className="mb-2 flex items-center justify-between gap-2">
                      <h3 className="m-0 text-sm font-semibold">候选链接 · {view.candidates.length}</h3>
                      {available.length > 1 ? (
                        <Button
                          variant="text"
                          disabled={Boolean(busy)}
                          onClick={() => setPicked(picked.length === available.length ? [] : available.map((row) => row.normalizedUrl))}
                        >
                          {picked.length === available.length ? '取消全选' : '全选'}
                        </Button>
                      ) : null}
                    </div>
                    <ul className="m-0 list-none divide-y divide-muted rounded-md border border-muted p-0">
                      {view.candidates.map((row) => {
                        const label = rowLabel(row);
                        const tag = rowStatusTag(row);
                        const selectable = !row.linked && row.status !== 'blocked';
                        return (
                          <li key={row.normalizedUrl} className="flex items-start gap-2.5 px-3 py-2.5">
                            <input
                              type="checkbox"
                              aria-label={`选择 ${label}`}
                              checked={row.linked || picked.includes(row.normalizedUrl)}
                              disabled={!selectable || Boolean(busy)}
                              onChange={(event) => {
                                const checked = event.target.checked;
                                setPicked((current) => checked
                                  ? [...current, row.normalizedUrl]
                                  : current.filter((url) => url !== row.normalizedUrl));
                              }}
                              className="mt-0.5 size-4 shrink-0 accent-primary"
                            />
                            <div className="min-w-0 flex-1 text-sm break-all">
                              {row.status !== 'blocked' && canOpenUrl(row.url) ? (
                                <a href={row.url} target="_blank" rel="noopener noreferrer" className="hover:underline">{label}</a>
                              ) : <span>{label}</span>}
                              {tag.reason ? <p className="mt-1 text-xs text-error">{plainBlockedReason(tag.reason)}</p> : null}
                            </div>
                            <div className="flex shrink-0 flex-wrap justify-end gap-1">
                              {row.kind === 'github_skill' ? <Tag>Skill</Tag> : null}
                              <Tag tone={row.linked ? 'gray' : tag.tone}>{row.linked ? '已关联' : tag.text}</Tag>
                            </div>
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                ) : !edit && (view.body || view.fetchedExcerpt) ? (
                  <p className="m-0 text-sm text-on-surface-variant">这段正文没有可提取的链接。</p>
                ) : null}
              </>
            )}
          </div>
          {!edit && available.length > 0 ? (
            <div className="flex shrink-0 items-center justify-between gap-2 border-t border-muted px-5 py-3">
              <span className="text-sm text-on-surface-variant">已选 {picked.length} 项</span>
              <Button variant="primary" loading={busy === 'candidates'} disabled={!picked.length} onClick={() => void confirmCandidates()}>
                确认所选
              </Button>
            </div>
          ) : null}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
