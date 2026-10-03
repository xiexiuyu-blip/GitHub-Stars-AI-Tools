import { useEffect, useRef, useState } from 'react';
import type { CapturePreview, InboxItem, InboxPage } from '@/lib/contracts';
import {
  commitCaptureBatch,
  describeInvokeError,
  fetchInboxPending,
  isMissingCommandError,
  listInbox,
  previewCapture,
  setInboxNote,
  setInboxStatus,
} from '@/lib/api';
import {
  appendInboxItems,
  archiveAction,
  buildSourceContext,
  canCommit,
  type CaptureSource,
  describeCommit,
  hasUnfetched,
  inboxHeading,
  mentionSummary,
  mentionText,
  type InboxFilter,
  isTooLong,
  noteError,
  previewDecision,
  removeInboxItem,
  undoTarget,
} from '@/lib/capture-view';
import { PageHeader, useToast } from '@/components/kit';
import { InboxCard, PasteCard, PreviewCard } from '@/components/capture';

const PREVIEW_DELAY_MS = 300;
const INBOX_PAGE_SIZE = 100;

export function CapturePage() {
  const toast = useToast();
  const [text, setText] = useState('');
  const [source, setSource] = useState<CaptureSource>('微信群');
  const [note, setNote] = useState('');
  const [preview, setPreview] = useState<CapturePreview | null>(null);
  const [previewFor, setPreviewFor] = useState('');
  const [previewPhase, setPreviewPhase] = useState<'idle' | 'loading' | 'error' | 'too-long'>('idle');
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewRetry, setPreviewRetry] = useState(0);
  const [submitting, setSubmitting] = useState(false);
  const [filter, setFilter] = useState<InboxFilter>('pending');
  const [inbox, setInbox] = useState<InboxPage | null>(null);
  const [inboxPhase, setInboxPhase] = useState<'loading' | 'ready' | 'error'>('loading');
  const [inboxError, setInboxError] = useState<string | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState('');
  const [noteMessage, setNoteMessage] = useState<string | null>(null);
  const [savingNote, setSavingNote] = useState(false);
  const [fetching, setFetching] = useState(false);
  const [collectingId, setCollectingId] = useState<string | null>(null);

  const previewGen = useRef(0);
  const inboxGen = useRef(0);
  const submittingRef = useRef(false);
  const loadingMoreRef = useRef(false);
  const savingNoteRef = useRef(false);
  const busyRef = useRef<string | null>(null);
  const filterRef = useRef(filter);
  filterRef.current = filter;
  const inboxRef = useRef(inbox);
  inboxRef.current = inbox;
  const fetchingRef = useRef(false);
  const mountedRef = useRef(true);
  const runFetchRef = useRef<(id?: string) => Promise<void>>(async () => {});

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    const decision = previewDecision(text);
    const gen = ++previewGen.current;
    if (decision === 'too-long') {
      setPreviewPhase('too-long');
      setPreview(null);
      setPreviewFor('');
      setPreviewError(null);
      return;
    }
    if (decision === 'skip') {
      setPreviewPhase('idle');
      setPreview(null);
      setPreviewFor('');
      setPreviewError(null);
      return;
    }

    setPreviewPhase('loading');
    setPreview(null);
    setPreviewFor('');
    setPreviewError(null);
    let active = true;
    const timer = window.setTimeout(() => {
      previewCapture(text)
        .then((result) => {
          if (!active || gen !== previewGen.current) return;
          setPreview(result);
          setPreviewFor(text);
          setPreviewError(null);
          setPreviewPhase('idle');
        })
        .catch((reason: unknown) => {
          if (!active || gen !== previewGen.current) return;
          setPreview(null);
          setPreviewFor('');
          setPreviewError(describeInvokeError(reason));
          setPreviewPhase('error');
        });
    }, PREVIEW_DELAY_MS);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [text, previewRetry]);

  useEffect(() => {
    const gen = ++inboxGen.current;
    let active = true;
    setInboxPhase('loading');
    setInboxError(null);
    setLoadingMore(false);
    loadingMoreRef.current = false;
    setInbox((current) => (current ? { ...current, items: [], totalCount: 0 } : current));
    listInbox(filter, INBOX_PAGE_SIZE, 0)
      .then((page) => {
        if (!active || gen !== inboxGen.current) return;
        setInbox(page);
        setInboxPhase('ready');
        // 收进来的链接（X、公众号、视频、网页）只有网址：后台去读公开的标题和摘要，读完静默刷新
        if (hasUnfetched(page.items)) void runFetchRef.current();
      })
      .catch((reason: unknown) => {
        if (!active || gen !== inboxGen.current) return;
        setInboxPhase('error');
        setInboxError(describeInvokeError(reason));
      });
    return () => {
      active = false;
    };
  }, [filter, reloadKey]);

  /** 不清空列表地换一份新数据，用在后台读完一轮之后（正常刷新会先清空再转圈，会闪）。 */
  async function refreshSilently() {
    const status = filterRef.current;
    const gen = inboxGen.current;
    const size = Math.max(INBOX_PAGE_SIZE, inboxRef.current?.items.length ?? 0);
    try {
      const page = await listInbox(status, size, 0);
      if (!mountedRef.current || gen !== inboxGen.current || status !== filterRef.current) return;
      setInbox(page);
    } catch {
      // 静默刷新失败不打扰；下次正常刷新会再读
    }
  }

  /** 让后台联网读还没读过的链接。一次最多 20 条，还有剩的就再来一轮（最多 5 轮）。带 id 只读这一条（读不到的重试）。 */
  async function runFetch(id?: string) {
    if (fetchingRef.current) return;
    fetchingRef.current = true;
    setFetching(true);
    try {
      for (let round = 0; round < 5; round += 1) {
        const summary = await fetchInboxPending(id);
        await refreshSilently();
        if (id || summary.remaining <= 0 || summary.done + summary.failed + summary.skipped === 0) break;
      }
    } catch (reason: unknown) {
      if (!isMissingCommandError(reason) && mountedRef.current) toast.show({ title: describeInvokeError(reason) });
    } finally {
      fetchingRef.current = false;
      if (mountedRef.current) setFetching(false);
    }
  }
  runFetchRef.current = runFetch;

  /** 把文章里提到、还没收藏也没在收集箱里的 GitHub 项目收进收集箱。 */
  async function collectRepos(item: InboxItem) {
    const summary = mentionSummary(item.githubRepos);
    if (!summary || summary.fresh === 0 || collectingId) return;
    setCollectingId(item.id);
    try {
      const context = `来自文章：${inboxHeading(item.title, item.normalizedUrl)}`.slice(0, 190);
      const result = await commitCaptureBatch(mentionText(summary), context);
      toast.show({ title: describeCommit(result) });
      await refreshSilently();
    } catch (reason: unknown) {
      toast.show({ title: describeInvokeError(reason) });
    } finally {
      if (mountedRef.current) setCollectingId(null);
    }
  }

  const matchedPreview = preview && previewFor === text ? preview : null;
  const commitReady = Boolean(matchedPreview && canCommit(matchedPreview) && !isTooLong(text));

  async function commit() {
    if (submittingRef.current) return;
    if (!matchedPreview || !canCommit(matchedPreview) || isTooLong(text)) return;
    const payload = text;
    const context = buildSourceContext(source, note);
    submittingRef.current = true;
    setSubmitting(true);
    try {
      const result = await commitCaptureBatch(payload, context);
      toast.show({ title: describeCommit(result) });
      setText('');
      setPreview(null);
      setPreviewFor('');
      setPreviewPhase('idle');
      setPreviewError(null);
      setEditingId(null);
      setDraft('');
      setNoteMessage(null);
      setFilter('pending');
      setReloadKey((key) => key + 1);
    } catch (reason: unknown) {
      toast.show({ title: describeInvokeError(reason) });
    } finally {
      submittingRef.current = false;
      setSubmitting(false);
    }
  }

  function chooseFilter(next: InboxFilter) {
    setEditingId(null);
    setDraft('');
    setNoteMessage(null);
    setFilter(next);
  }

  async function loadMore() {
    if (loadingMoreRef.current || !inbox) return;
    if (inbox.items.length >= inbox.totalCount) return;
    const gen = inboxGen.current;
    const offset = inbox.items.length;
    const status = filterRef.current;
    loadingMoreRef.current = true;
    setLoadingMore(true);
    try {
      const page = await listInbox(status, INBOX_PAGE_SIZE, offset);
      if (gen !== inboxGen.current) return;
      setInbox((current) => (current ? appendInboxItems(current, page) : page));
    } catch (reason: unknown) {
      if (gen !== inboxGen.current) return;
      toast.show({ title: describeInvokeError(reason) });
    } finally {
      if (gen === inboxGen.current) {
        loadingMoreRef.current = false;
        setLoadingMore(false);
      }
    }
  }

  function startNote(item: InboxItem) {
    setEditingId(item.id);
    setDraft(item.userNote);
    setNoteMessage(noteError(item.userNote));
  }

  function changeDraft(value: string) {
    setDraft(value);
    setNoteMessage(noteError(value));
  }

  function cancelNote() {
    setEditingId(null);
    setDraft('');
    setNoteMessage(null);
  }

  async function saveNote() {
    if (!editingId || savingNoteRef.current) return;
    const message = noteError(draft);
    if (message) {
      setNoteMessage(message);
      return;
    }
    const id = editingId;
    const saved = draft.trim();
    savingNoteRef.current = true;
    setSavingNote(true);
    try {
      await setInboxNote(id, saved);
      setInbox((current) => {
        if (!current) return current;
        return {
          ...current,
          items: current.items.map((item) => (item.id === id ? { ...item, userNote: saved } : item)),
        };
      });
      setEditingId(null);
      setDraft('');
      setNoteMessage(null);
    } catch (reason: unknown) {
      toast.show({ title: describeInvokeError(reason) });
    } finally {
      savingNoteRef.current = false;
      setSavingNote(false);
    }
  }

  async function archive(item: InboxItem) {
    if (busyRef.current) return;
    const action = archiveAction(item.status);
    const editingThis = editingId === item.id;
    const itemName = inboxHeading(item.title, item.normalizedUrl);
    busyRef.current = item.id;
    setBusyId(item.id);
    try {
      await setInboxStatus(item.id, action.target);
      setInbox((current) => (current ? removeInboxItem(current, item.id, item.status, action.target) : current));
      if (editingThis) cancelNote();
      const back = undoTarget(item.status);
      if (action.target === 'archived' && back) {
        toast.show({
          title: '已归档',
          sub: itemName,
          actions: [{ label: '撤销', onClick: () => { void undoStatus(item.id, back); } }],
        });
      } else if (action.target === 'archived') {
        toast.show({ title: '已归档', sub: itemName });
      } else {
        toast.show({
          title: '已恢复',
          sub: itemName,
          actions: [{ label: '撤销', onClick: () => { void undoStatus(item.id, 'archived'); } }],
        });
      }
    } catch (reason: unknown) {
      toast.show({ title: describeInvokeError(reason) });
    } finally {
      busyRef.current = null;
      setBusyId(null);
    }
  }

  async function undoStatus(id: string, status: 'pending' | 'ready' | 'archived') {
    try {
      await setInboxStatus(id, status);
      setReloadKey((key) => key + 1);
    } catch (reason: unknown) {
      toast.show({ title: describeInvokeError(reason) });
    }
  }

  return (
    <section className="flex h-full min-h-0 flex-col gap-3 overflow-auto px-5 pt-4 pb-5">
      <PageHeader
        title="收集"
        subtitle="把群聊、帖子整段粘贴进来，我来找出里面的链接，并告诉你哪些是新的。"
      />
      <PasteCard
        text={text}
        source={source}
        note={note}
        disabled={submitting}
        onText={setText}
        onSource={setSource}
        onNote={setNote}
      />
      <PreviewCard
        phase={previewPhase}
        error={previewError}
        preview={matchedPreview}
        canCommit={commitReady}
        submitting={submitting}
        onCommit={() => { void commit(); }}
        onRetry={() => setPreviewRetry((value) => value + 1)}
      />
      <InboxCard
        filter={filter}
        page={inbox}
        phase={inboxPhase}
        error={inboxError}
        loadingMore={loadingMore}
        busyId={busyId}
        editingId={editingId}
        draft={draft}
        noteMessage={noteMessage}
        savingNote={savingNote}
        fetching={fetching}
        collectingId={collectingId}
        onCollectRepos={(item) => { void collectRepos(item); }}
        onFilter={chooseFilter}
        onLoadMore={() => { void loadMore(); }}
        onRetry={() => setReloadKey((key) => key + 1)}
        onStartNote={startNote}
        onDraft={changeDraft}
        onSaveNote={() => { void saveNote(); }}
        onCancelNote={cancelNote}
        onArchive={(item) => { void archive(item); }}
        onRetryFetch={(item) => { void runFetch(item.id); }}
      />
    </section>
  );
}
