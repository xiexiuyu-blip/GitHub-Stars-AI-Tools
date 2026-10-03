import { useEffect, useMemo, useRef, useState } from 'react';
import type { LibraryCounts, LibraryDetail, LibraryItem, LibraryPage as LibraryPageResult, ReadingGroup, ScenarioSummary } from '@/lib/contracts';
import { codexFallbackText, readingGroupLabel } from '@/lib/item-format';
import {
  confirmLibraryCard,
  copyItemForCodex,
  describeInvokeError,
  getLibraryDetail,
  listLibraryView,
  listScenarios,
  setLibraryReading,
} from '@/lib/api';
import {
  applyOptimisticReading,
  cardStateOptions,
  clearFilters,
  formOptions,
  hasActiveFilters,
  nextPageOffset,
  removeIfFilteredOut,
  shouldFallbackToAll,
  toFilters,
  toggleForm,
  toggleReading,
  toggleScenario,
  visibleScenarios,
  withInitial,
  type LibraryUiState,
} from '@/lib/library-filters';
import { automationHint } from '@/lib/automation-view';
import { useAiAutomation } from '@/hooks/use-ai-automation';
import { useRegenerate } from '@/hooks/use-regenerate';
import { shouldIgnoreKey, useListKeys } from '@/hooks/use-list-keys';
import { Button, Card, EmptyState, Kbd, PageHeader, Segmented, useToast } from '@/components/kit';
import { AutomationBanner, ItemDetail, ItemRow } from '@/components/library';
import { FilterPanel } from '@/components/library/filter-panel';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

const LIBRARY_PAGE_SIZE = 80;
const KEYWORD_DEBOUNCE_MS = 200;

type LibraryPageProps = {
  initialKeyword?: string;
  /** 预留：从别处带着场景进来。现在没有调用方。 */
  initialScenario?: string | null;
  onOpenSolve?: (query: string) => void;
  /** 去工具箱（开启 / 调整 AI 自动整理） */
  onOpenToolbox?: () => void;
};

type ScenarioStatus = 'loading' | 'ready' | 'error';
type ListPhase = 'loading' | 'idle' | 'error';

export function LibraryPage(props: LibraryPageProps) {
  const toast = useToast();
  const { status: automation } = useAiAutomation();
  const regenerateCard = useRegenerate(props.onOpenToolbox);
  const [ui, setUi] = useState(() => withInitial(props.initialKeyword, props.initialScenario));
  const [keywordDraft, setKeywordDraft] = useState(() => withInitial(props.initialKeyword, props.initialScenario).keyword);
  const [scenarios, setScenarios] = useState<ScenarioSummary[]>([]);
  const [scenarioStatus, setScenarioStatus] = useState<ScenarioStatus>('loading');
  const [scenarioError, setScenarioError] = useState<string | null>(null);
  const [scenarioReloadKey, setScenarioReloadKey] = useState(0);
  const [serverItems, setServerItems] = useState<LibraryItem[]>([]);
  const [resultCount, setResultCount] = useState<number | null>(null);
  const [counts, setCounts] = useState<LibraryCounts | null>(null);
  const [listPhase, setListPhase] = useState<ListPhase>('loading');
  const [listError, setListError] = useState<string | null>(null);
  const [listReloadKey, setListReloadKey] = useState(0);
  const [noMore, setNoMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<LibraryDetail | null>(null);
  const [detailPhase, setDetailPhase] = useState<'idle' | 'loading' | 'error'>('idle');
  const [detailError, setDetailError] = useState<string | null>(null);
  const [detailReloadKey, setDetailReloadKey] = useState(0);
  const [overrides, setOverrides] = useState<Record<string, ReadingGroup>>({});
  const [cardStateTouched, setCardStateTouched] = useState(false);
  const [fallbackHint, setFallbackHint] = useState(false);
  const [filterOpen, setFilterOpen] = useState(false);
  const [detailOpen, setDetailOpen] = useState(false);

  const searchRef = useRef<HTMLInputElement>(null);
  const filterRootRef = useRef<HTMLDivElement>(null);
  const uiRef = useRef(ui);
  const overridesRef = useRef(overrides);
  const serverItemsRef = useRef(serverItems);
  const detailRef = useRef(detail);
  const resultCountRef = useRef(resultCount);
  const selectedIdRef = useRef(selectedId);
  const touchedRef = useRef(cardStateTouched);
  const noMoreRef = useRef(noMore);
  const loadingMoreRef = useRef(false);
  const listGeneration = useRef(0);
  const fallbackChecked = useRef(false);
  const changeReadingRef = useRef<(id: string, group: ReadingGroup) => void>(() => {});
  const copyForCodexRef = useRef<(id: string) => void>(() => {});
  uiRef.current = ui;
  overridesRef.current = overrides;
  serverItemsRef.current = serverItems;
  detailRef.current = detail;
  resultCountRef.current = resultCount;
  selectedIdRef.current = selectedId;
  touchedRef.current = cardStateTouched;
  noMoreRef.current = noMore;

  const belowFilter = useMaxWidth(1099);
  const narrow = useMaxWidth(899);
  const layout = narrow ? 'narrow' : belowFilter ? 'medium' : 'wide';
  const queryKey = uiKey(ui);
  const items = useMemo(() => projectItems(serverItems, overrides, ui.reading), [serverItems, overrides, ui.reading]);
  const orderedKey = items.map((item) => item.id).join('\u0001');
  const previousQueryKey = useRef(queryKey);
  const initialStamp = `${props.initialKeyword ?? ''}\u0001${props.initialScenario ?? ''}`;
  const seenInitial = useRef(initialStamp);

  useEffect(() => {
    if (seenInitial.current === initialStamp) return;
    seenInitial.current = initialStamp;
    const keyword = props.initialKeyword?.trim() ?? '';
    const scenario = props.initialScenario?.trim() || null;
    if (keyword) setKeywordDraft(keyword);
    if (!keyword && !scenario) return;
    setUi((current) => ({
      ...current,
      keyword: keyword || current.keyword,
      scenario: scenario ?? current.scenario,
    }));
  }, [initialStamp, props.initialKeyword, props.initialScenario]);

  useEffect(() => {
    const handle = window.setTimeout(() => {
      setUi((current) => (current.keyword === keywordDraft ? current : { ...current, keyword: keywordDraft }));
    }, KEYWORD_DEBOUNCE_MS);
    return () => window.clearTimeout(handle);
  }, [keywordDraft]);

  useEffect(() => {
    let cancelled = false;
    setScenarioStatus('loading');
    setScenarioError(null);
    void listScenarios().then((list) => {
      if (cancelled) return;
      setScenarios(Array.isArray(list) ? list : []);
      setScenarioStatus('ready');
    }).catch((reason) => {
      if (cancelled) return;
      setScenarios([]);
      setScenarioStatus('error');
      setScenarioError(describeInvokeError(reason));
    });
    return () => {
      cancelled = true;
    };
  }, [scenarioReloadKey]);

  useEffect(() => {
    const generation = ++listGeneration.current;
    loadingMoreRef.current = false;
    setLoadingMore(false);
    const filterChanged = previousQueryKey.current !== queryKey;
    previousQueryKey.current = queryKey;
    if (filterChanged) {
      setServerItems([]);
      setResultCount(null);
    }
    setNoMore(false);
    noMoreRef.current = false;
    setListPhase('loading');
    setListError(null);
    const state = uiRef.current;
    let cancelled = false;
    void listLibraryView(toFilters(state, 0, LIBRARY_PAGE_SIZE)).then((page) => {
      // 筛选已经变了，或这次请求已经被下一次盖掉：丢掉，避免旧结果盖住新结果。
      if (cancelled || generation !== listGeneration.current) return;
      const read = readLibraryPage(page);
      if (!fallbackChecked.current && read.counts) {
        fallbackChecked.current = true;
        if (shouldFallbackToAll(read.counts, state, touchedRef.current) && state.cardState === 'done') {
          setFallbackHint(true);
          setUi((current) => (current.cardState === 'done' ? { ...current, cardState: 'all' } : current));
          return;
        }
      }
      setServerItems(read.items);
      resultCountRef.current = read.totalCount;
      setResultCount(read.totalCount);
      if (read.counts) setCounts(read.counts);
      setListPhase('idle');
    }).catch((reason) => {
      if (cancelled || generation !== listGeneration.current) return;
      setListPhase('error');
      setListError(describeInvokeError(reason));
    });
    return () => {
      cancelled = true;
    };
  }, [queryKey, listReloadKey]);

  useEffect(() => {
    const ids = orderedKey ? orderedKey.split('\u0001') : [];
    setSelectedId((current) => (current && ids.includes(current) ? current : ids[0] ?? null));
  }, [orderedKey]);

  useEffect(() => {
    if (narrow) setDetailOpen(false);
  }, [queryKey, narrow]);

  useEffect(() => {
    if (layout === 'wide') setFilterOpen(false);
  }, [layout]);

  useEffect(() => {
    if (!filterOpen) return undefined;
    function onPointer(event: MouseEvent) {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (filterRootRef.current?.contains(target)) return;
      setFilterOpen(false);
    }
    document.addEventListener('mousedown', onPointer);
    return () => document.removeEventListener('mousedown', onPointer);
  }, [filterOpen]);

  useEffect(() => {
    if (!selectedId) {
      setDetail(null);
      detailRef.current = null;
      setDetailPhase('idle');
      setDetailError(null);
      return;
    }
    let cancelled = false;
    setDetailPhase('loading');
    setDetailError(null);
    void getLibraryDetail(selectedId).then((result) => {
      if (cancelled) return;
      const group = overridesRef.current[result.id];
      const next = group ? { ...result, readingGroup: group } : result;
      detailRef.current = next;
      setDetail(next);
      setDetailPhase('idle');
    }).catch((reason) => {
      if (cancelled) return;
      detailRef.current = null;
      setDetail(null);
      setDetailPhase('error');
      setDetailError(describeInvokeError(reason));
    });
    return () => {
      cancelled = true;
    };
  }, [selectedId, detailReloadKey]);

  useEffect(() => {
    // use-list-keys 只处理 J/K、T、X、C。`/` 和 U 在这里补上。
    function onKeyDown(event: KeyboardEvent) {
      if (event.isComposing) return;
      const element = event.target instanceof Element ? event.target : null;
      const typing = shouldIgnoreKey({
        tagName: element?.tagName,
        isContentEditable: element instanceof HTMLElement ? element.isContentEditable : false,
        metaKey: event.metaKey,
        ctrlKey: event.ctrlKey,
        altKey: event.altKey,
      });
      if (event.key === '/') {
        if (typing) return;
        event.preventDefault();
        searchRef.current?.focus();
        return;
      }
      if (event.key.toLowerCase() !== 'u' || typing) return;
      const id = selectedIdRef.current;
      if (!id) return;
      event.preventDefault();
      changeReadingRef.current(id, 'in_use');
    }
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  useListKeys({
    ids: items.map((item) => item.id),
    currentId: selectedId,
    onMove: (id) => setSelectedId(id),
    onStatus: (group) => {
      if (selectedId) void changeReading(selectedId, group);
    },
    onCopy: () => {
      if (selectedId) void copyForCodex(selectedId);
    },
  });

  function displayedGroup(id: string): ReadingGroup | null {
    if (overridesRef.current[id]) return overridesRef.current[id];
    const raw = serverItemsRef.current.find((item) => item.id === id);
    if (raw) return raw.readingGroup;
    if (detailRef.current?.id === id) return detailRef.current.readingGroup;
    return null;
  }

  function serverGroup(id: string): ReadingGroup | null {
    const raw = serverItemsRef.current.find((item) => item.id === id);
    if (raw) return raw.readingGroup;
    if (detailRef.current?.id === id) return detailRef.current.readingGroup;
    return null;
  }

  function labelOf(id: string): string {
    const raw = serverItemsRef.current.find((item) => item.id === id);
    if (raw) return raw.fullName;
    if (detailRef.current?.id === id) return detailRef.current.fullName;
    return '';
  }

  function writeOverride(id: string, group: ReadingGroup | null) {
    const next = { ...overridesRef.current };
    if (group) next[id] = group;
    else delete next[id];
    overridesRef.current = next;
    setOverrides(next);
  }

  function paintDetail(id: string, group: ReadingGroup) {
    if (detailRef.current?.id !== id) return;
    const next = { ...detailRef.current, readingGroup: group };
    detailRef.current = next;
    setDetail(next);
  }

  async function refreshCounts() {
    const generation = listGeneration.current;
    const page = await listLibraryView(toFilters(uiRef.current, 0, LIBRARY_PAGE_SIZE));
    if (generation !== listGeneration.current) return;
    const read = readLibraryPage(page);
    if (read.counts) setCounts(read.counts);
    resultCountRef.current = read.totalCount;
    setResultCount(read.totalCount);
  }

  async function changeReading(id: string, group: ReadingGroup) {
    const previous = displayedGroup(id);
    const server = serverGroup(id);
    if (!previous || previous === group) return;
    const projectName = labelOf(id);
    writeOverride(id, group);
    paintDetail(id, group);
    try {
      await setLibraryReading([id], group);
      toast.show({
        title: `已标为「${readingGroupLabel(group)}」`,
        sub: projectName || undefined,
        actions: [{ label: '撤销', onClick: () => { void undoReading(id, previous, group); } }],
      });
      try {
        await refreshCounts();
      } catch (reason) {
        toast.show({ title: describeInvokeError(reason), sub: '状态已经改了，数字可能还没刷新' });
      }
    } catch (reason) {
      if (overridesRef.current[id] === group) {
        writeOverride(id, server && previous === server ? null : previous);
      }
      paintDetail(id, previous);
      toast.show({ title: describeInvokeError(reason) });
    }
  }

  async function undoReading(id: string, previous: ReadingGroup, written: ReadingGroup) {
    writeOverride(id, previous);
    paintDetail(id, previous);
    try {
      await setLibraryReading([id], previous);
      try {
        await refreshCounts();
      } catch (reason) {
        toast.show({ title: describeInvokeError(reason), sub: '已撤销，数字可能还没刷新' });
      }
    } catch (reason) {
      if (overridesRef.current[id] === previous) writeOverride(id, written);
      paintDetail(id, written);
      toast.show({ title: describeInvokeError(reason) });
    }
  }

  async function writeClipboard(text: string): Promise<boolean> {
    try {
      if (!navigator.clipboard?.writeText) throw new Error('clipboard');
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      toast.show({ title: '浏览器不允许自动复制', sub: '请手动选中后按 ⌘C' });
      return false;
    }
  }

  async function copyForCodex(id: string) {
    const projectName = labelOf(id);
    let text = '';
    try {
      text = await copyItemForCodex(id);
    } catch (reason) {
      const message = describeInvokeError(reason);
      if (message.includes('这个功能还没接上') && detailRef.current?.id === id) {
        text = codexFallbackText(detailRef.current);
      } else {
        toast.show({ title: message });
        return;
      }
    }
    const copied = await writeClipboard(text);
    if (copied) toast.show({ title: '已复制，去 Codex 里粘贴', sub: projectName || undefined });
  }

  async function confirmCard(id: string) {
    try {
      await confirmLibraryCard(id);
      if (detailRef.current?.id === id) {
        const next = { ...detailRef.current, cardReview: 'confirmed' as const };
        detailRef.current = next;
        setDetail(next);
      }
    } catch (reason) {
      toast.show({ title: describeInvokeError(reason) });
    }
  }


  changeReadingRef.current = (id, group) => { void changeReading(id, group); };
  copyForCodexRef.current = (id) => { void copyForCodex(id); };

  async function loadMore() {
    if (loadingMoreRef.current || noMoreRef.current) return;
    const offset = nextPageOffset(serverItemsRef.current.length, resultCountRef.current ?? 0, LIBRARY_PAGE_SIZE);
    if (offset == null) return;
    const generation = listGeneration.current;
    const pageIndex = offset / LIBRARY_PAGE_SIZE;
    loadingMoreRef.current = true;
    setLoadingMore(true);
    try {
      const page = await listLibraryView(toFilters(uiRef.current, pageIndex, LIBRARY_PAGE_SIZE));
      if (generation !== listGeneration.current) return;
      const read = readLibraryPage(page);
      const seen = new Set(serverItemsRef.current.map((item) => item.id));
      const extra = read.items.filter((item) => !seen.has(item.id));
      if (extra.length === 0) {
        noMoreRef.current = true;
        setNoMore(true);
      } else {
        setServerItems((current) => {
          const latest = new Set(current.map((item) => item.id));
          return [...current, ...extra.filter((item) => !latest.has(item.id))];
        });
      }
      resultCountRef.current = read.totalCount;
      setResultCount(read.totalCount);
      if (read.counts) setCounts(read.counts);
    } catch (reason) {
      if (generation !== listGeneration.current) return;
      toast.show({ title: describeInvokeError(reason) });
    } finally {
      if (generation === listGeneration.current) {
        loadingMoreRef.current = false;
        setLoadingMore(false);
      }
    }
  }

  function chooseCardState(value: LibraryUiState['cardState']) {
    touchedRef.current = true;
    setCardStateTouched(true);
    setFallbackHint(false);
    setUi((current) => (current.cardState === value ? current : { ...current, cardState: value }));
  }

  function clearAll() {
    setKeywordDraft('');
    setUi((current) => clearFilters(current));
  }

  function pickRow(id: string) {
    setSelectedId(id);
    if (narrow) setDetailOpen(true);
  }

  const scenarioRows = visibleScenarios(scenarios, counts);
  const formRows = formOptions(counts?.forms);
  const readingCounts = readingCountsOf(counts);
  const cardOptions = counts
    ? cardStateOptions(counts.cardState)
    : [
      { value: 'done' as const, label: '已整理 …' },
      { value: 'pending' as const, label: '待整理 …' },
      { value: 'all' as const, label: '全部 …' },
    ];
  const totalText = counts && typeof counts.total === 'number' && Number.isFinite(counts.total)
    ? String(counts.total)
    : '…';
  const subtitle = `你收藏的 ${totalText} 个工具，按你能拿它做什么来找。已整理的有中文说明，其余还没整理。`;
  const showFallbackHint = fallbackHint && !cardStateTouched && ui.cardState === 'all';
  const filtersActive = hasActiveFilters(ui);
  // 提示说的是整个资料库还有多少没整理；已经按场景 / 关键词筛窄了，数字对不上，就不提示
  const narrowedDown = Boolean(ui.scenario || ui.reading || ui.form || ui.keyword.trim());
  const hint = narrowedDown ? null : automationHint(automation, counts?.cardState?.pending ?? 0);
  const moreOffset = nextPageOffset(serverItems.length, resultCount ?? 0, LIBRARY_PAGE_SIZE);
  const hasMore = moreOffset != null && !noMore;
  const matchedDetail = detail && detail.id === selectedId ? detail : null;
  const shownDetail = matchedDetail
    ? { ...matchedDetail, readingGroup: overrides[matchedDetail.id] ?? matchedDetail.readingGroup }
    : null;
  const detailLoading = Boolean(selectedId) && !shownDetail && detailPhase !== 'error';
  const hideList = layout === 'narrow' && detailOpen;
  const hideDetail = layout === 'narrow' && !detailOpen;
  const gridClass = layout === 'wide'
    ? 'grid-cols-[200px_minmax(0,1fr)_minmax(340px,440px)]'
    : layout === 'medium'
      ? 'grid-cols-[minmax(0,1fr)_minmax(340px,440px)]'
      : 'grid-cols-1';

  return (
    <section className="flex h-full min-h-0 flex-col gap-3 overflow-hidden px-5 pt-4 pb-5 max-[899px]:px-4 max-[899px]:pt-3 max-[899px]:pb-4">
      <PageHeader title="资料库" subtitle={subtitle} />
      <div className="relative z-20 flex shrink-0 flex-wrap items-center gap-3">
        <div className="relative w-full min-[900px]:min-w-[240px] min-[900px]:flex-1">
          <Icon name="search" size={16} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-on-surface-variant" />
          <input
            ref={searchRef}
            type="text"
            value={keywordDraft}
            onChange={(event) => setKeywordDraft(event.target.value)}
            placeholder="搜工具、说明、关键词"
            aria-label="搜索资料库"
            autoComplete="off"
            className="h-9 w-full rounded-md border border-input bg-card pr-3 pl-9 text-sm text-foreground outline-none placeholder:text-on-surface-variant focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary"
          />
        </div>
        {layout !== 'wide' ? (
          <div ref={filterRootRef} className="relative">
            <Button
              variant="default"
              aria-expanded={filterOpen}
              aria-controls="library-filters"
              className={cn((ui.scenario || ui.reading || ui.form) && 'text-accent-foreground')}
              onClick={() => setFilterOpen((open) => !open)}
            >
              <Icon name="filter_list" size={16} />
              筛选
            </Button>
            {filterOpen ? (
              <div id="library-filters" className="absolute top-full left-0 z-30 mt-2 w-[240px]">
                <Card className="flex max-h-[70vh] flex-col overflow-hidden shadow-lg">
                  {renderFilters()}
                </Card>
              </div>
            ) : null}
          </div>
        ) : null}
        <Segmented ariaLabel="整理状态" options={cardOptions} value={ui.cardState} onChange={chooseCardState} />
        {filtersActive ? <Button variant="text" onClick={clearAll}>清除筛选</Button> : null}
        <span className="ml-auto text-xs font-medium text-on-surface-variant">{resultCount == null ? '…' : resultCount} 个</span>
      </div>
      <AutomationBanner hint={hint} onAction={props.onOpenToolbox} />
      <div className={cn('grid min-h-0 flex-1 gap-3', gridClass)}>
        {layout === 'wide' ? (
          <Card className="flex min-h-0 flex-col overflow-hidden">
            {renderFilters()}
          </Card>
        ) : null}
        <Card className={cn('flex min-h-0 flex-col overflow-hidden', hideList && 'hidden')}>
          {showFallbackHint ? (
            <p className="shrink-0 border-b border-muted px-4 py-2 text-xs text-on-surface-variant">
              AI 还没整理出理解卡，先显示全部收藏。
            </p>
          ) : null}
          <div className="min-h-0 flex-1 overflow-auto">
            {renderListBody()}
            {hasMore ? (
              <div className="flex justify-center px-4 py-3">
                <Button variant="default" loading={loadingMore} onClick={() => { void loadMore(); }}>加载更多</Button>
              </div>
            ) : null}
          </div>
          <div className="flex shrink-0 flex-wrap gap-2.5 border-t border-muted px-4 py-2.5 text-xs text-on-surface-variant">
            <span className="inline-flex items-center gap-1"><Kbd>/</Kbd> 搜索</span>
            <span className="inline-flex items-center gap-1"><Kbd>J</Kbd><Kbd>K</Kbd> 上下</span>
            <span className="inline-flex items-center gap-1"><Kbd>T</Kbd> 想试</span>
            <span className="inline-flex items-center gap-1"><Kbd>U</Kbd> 在用</span>
            <span className="inline-flex items-center gap-1"><Kbd>X</Kbd> 不要了</span>
            <span className="inline-flex items-center gap-1"><Kbd>C</Kbd> 复制给 Codex</span>
          </div>
        </Card>
        <Card className={cn('flex min-h-0 flex-col overflow-hidden', hideDetail && 'hidden')}>
          <ItemDetail
            detail={shownDetail}
            loading={detailLoading}
            error={detailPhase === 'error' ? detailError : null}
            scenarios={scenarios}
            onBack={layout === 'narrow' ? () => setDetailOpen(false) : undefined}
            onRetry={() => setDetailReloadKey((key) => key + 1)}
            onReadingChange={(group) => { if (selectedId) void changeReading(selectedId, group); }}
            onConfirm={() => { if (selectedId) void confirmCard(selectedId); }}
            onRegenerate={() => { if (selectedId) void regenerateCard(selectedId); }}
            onOpenToolbox={props.onOpenToolbox}
            onCopyCodex={() => { if (selectedId) void copyForCodex(selectedId); }}
            onCopyHow={(text) => {
              void writeClipboard(text).then((copied) => {
                if (copied) toast.show({ title: '已复制' });
              });
            }}
            onCopyPrompt={(text) => {
              void writeClipboard(text).then((copied) => {
                if (copied) toast.show({ title: '已复制，去 Codex 里粘贴' });
              });
            }}
          />
        </Card>
      </div>
    </section>
  );

  function renderFilters() {
    return (
      <FilterPanel
        scenarios={scenarioRows}
        scenarioStatus={scenarioStatus}
        scenarioError={scenarioError}
        onRetryScenarios={() => setScenarioReloadKey((key) => key + 1)}
        readingCounts={readingCounts}
        forms={formRows}
        countsKnown={counts != null}
        scenario={ui.scenario}
        reading={ui.reading}
        form={ui.form}
        onScenario={(id) => setUi((current) => toggleScenario(current, id))}
        onReading={(group) => setUi((current) => toggleReading(current, group))}
        onForm={(id) => setUi((current) => toggleForm(current, id))}
      />
    );
  }

  function renderListBody() {
    if (listPhase === 'loading' && serverItems.length === 0) return <ListSkeleton />;
    if (listPhase === 'error' && serverItems.length === 0) {
      return <ListMessage message={listError ?? '资料库没有读出来。'} onRetry={() => setListReloadKey((key) => key + 1)} />;
    }
    if (items.length === 0) {
      return (
        <div className="flex h-full items-center justify-center">
          <EmptyState
            icon="search_off"
            title="没有找到"
            description="换个说法试试，或去「找方案」说说你想做什么"
            action={(
              <Button variant="default" onClick={() => props.onOpenSolve?.(ui.keyword.trim())}>
                去找方案
              </Button>
            )}
          />
        </div>
      );
    }
    return (
      <>
        {listPhase === 'error' && listError ? (
          <ListMessage message={listError} onRetry={() => setListReloadKey((key) => key + 1)} />
        ) : null}
        {items.map((item) => (
          <ItemRow
            key={item.id}
            item={item}
            selected={item.id === selectedId}
            scenarios={scenarios}
            onSelect={() => pickRow(item.id)}
            onWant={() => void changeReading(item.id, 'want')}
            onDrop={() => void changeReading(item.id, 'dropped')}
          />
        ))}
      </>
    );
  }
}

/** 列表上看到的条目：先套上还没落库的状态，再按当前状态筛选拿掉对不上的。 */
function projectItems(
  items: readonly LibraryItem[],
  overrides: Readonly<Record<string, ReadingGroup>>,
  reading: ReadingGroup | null,
): LibraryItem[] {
  let next = items.slice();
  for (const id of Object.keys(overrides)) {
    const group = overrides[id];
    if (group) next = applyOptimisticReading(next, id, group);
  }
  if (!reading) return next;
  for (const item of [...next]) {
    next = removeIfFilteredOut(next, item.id, reading);
  }
  return next;
}

function readLibraryPage(page: Partial<LibraryPageResult> | null | undefined): {
  items: LibraryItem[];
  totalCount: number;
  counts: LibraryCounts | null;
} {
  const items = Array.isArray(page?.items) ? page.items : [];
  const totalCount = typeof page?.totalCount === 'number' && Number.isFinite(page.totalCount)
    ? page.totalCount
    : items.length;
  const counts = page?.counts && typeof page.counts === 'object' ? page.counts : null;
  return { items, totalCount, counts };
}

function readingCountsOf(counts: LibraryCounts | null): Record<ReadingGroup, number> {
  return {
    unseen: countOf(counts?.reading?.unseen),
    want: countOf(counts?.reading?.want),
    tried: countOf(counts?.reading?.tried),
    in_use: countOf(counts?.reading?.in_use),
    dropped: countOf(counts?.reading?.dropped),
  };
}

function countOf(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : 0;
}

function uiKey(state: LibraryUiState): string {
  return [
    state.scenario ?? '',
    state.reading ?? '',
    state.form ?? '',
    state.cardState,
    state.keyword,
    state.sort,
  ].join('\u0001');
}

function useMaxWidth(px: number): boolean {
  const query = `(max-width: ${px}px)`;
  const [matches, setMatches] = useState(() => (
    typeof window !== 'undefined' && typeof window.matchMedia === 'function' && window.matchMedia(query).matches
  ));
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return undefined;
    const media = window.matchMedia(query);
    const apply = () => setMatches(media.matches);
    apply();
    media.addEventListener('change', apply);
    return () => media.removeEventListener('change', apply);
  }, [query]);
  return matches;
}

function ListSkeleton() {
  return (
    <div className="flex flex-col gap-3 px-4 py-4" aria-hidden="true">
      {Array.from({ length: 4 }, (_, index) => (
        <div key={index} className="h-12 animate-pulse rounded-md bg-muted" />
      ))}
    </div>
  );
}

function ListMessage(props: { message: string; onRetry: () => void }) {
  return (
    <div className="flex items-center gap-2 px-4 py-3 text-sm text-destructive">
      <span className="min-w-0 flex-1">{props.message}</span>
      <Button variant="text" onClick={props.onRetry}>重试</Button>
    </div>
  );
}
