import { useEffect, useRef, useState } from 'react';
import type { LibraryDetail, LibraryFilters, LibraryItem, ReadingGroup, ScenarioSummary, TodayView } from '@/lib/contracts';
import {
  applyReadingOverrides,
  assembleWeekGroups,
  buildTodayLead,
  codexFallbackText,
  greetingFor,
  itemsForReadingFilter,
  pickWhyLine,
  readingGroupLabel,
} from '@/lib/item-format';
import {
  confirmLibraryCard,
  copyItemForCodex,
  describeInvokeError,
  getGithubAuthState,
  getLibraryDetail,
  getTodayView,
  listLibraryView,
  listScenarios,
  regenerateLibraryCard,
  setLibraryReading,
} from '@/lib/api';
import { useListKeys } from '@/hooks/use-list-keys';
import { Button, Card, EmptyState, Kbd, PageHeader, StatCard, useToast } from '@/components/kit';
import { ItemDetail, ItemRow } from '@/components/library';
import { cn } from '@/lib/utils';

type TodayPageProps = {
  onOpenLibrary: () => void;
  onOpenCapture: () => void;
  onOpenSolve: () => void;
};

type TodayFilter = 'week' | 'pending' | 'want' | 'in_use';

const FILTER_LABEL: Record<TodayFilter, string> = {
  week: '本周新收藏',
  pending: '待整理',
  want: '想试',
  in_use: '在用',
};

const FILTER_QUERY: Record<Exclude<TodayFilter, 'week'>, LibraryFilters> = {
  pending: { cardState: 'pending', sort: 'starred', limit: 80 },
  want: { reading: 'want', cardState: 'all' },
  in_use: { reading: 'in_use', cardState: 'all' },
};

export function TodayPage(props: TodayPageProps) {
  void props;
  const toast = useToast();
  const [name, setName] = useState('Fox');
  const [phase, setPhase] = useState<'loading' | 'ready' | 'error'>('loading');
  const [loadError, setLoadError] = useState<string | null>(null);
  const [today, setToday] = useState<TodayView | null>(null);
  const [readingCounts, setReadingCounts] = useState<{ want: number; inUse: number } | null>(null);
  const [scenarios, setScenarios] = useState<ScenarioSummary[]>([]);
  const [filter, setFilter] = useState<TodayFilter>('week');
  const [filteredItems, setFilteredItems] = useState<LibraryItem[]>([]);
  const [listPhase, setListPhase] = useState<'idle' | 'loading' | 'error'>('idle');
  const [listError, setListError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<LibraryDetail | null>(null);
  const [detailPhase, setDetailPhase] = useState<'idle' | 'loading' | 'error'>('idle');
  const [detailError, setDetailError] = useState<string | null>(null);
  const [overrides, setOverrides] = useState<Record<string, ReadingGroup>>({});
  const [narrow, setNarrow] = useState(false);
  const [detailOpen, setDetailOpen] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const [listReloadKey, setListReloadKey] = useState(0);
  const [detailReloadKey, setDetailReloadKey] = useState(0);
  const overridesRef = useRef(overrides);
  overridesRef.current = overrides;
  const filterRef = useRef(filter);

  useEffect(() => {
    const media = window.matchMedia('(max-width: 899px)');
    const apply = () => setNarrow(media.matches);
    apply();
    media.addEventListener('change', apply);
    return () => media.removeEventListener('change', apply);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setPhase('loading');
    setLoadError(null);
    void (async () => {
      try {
        const [todayView, countsPage] = await Promise.all([
          getTodayView(),
          listLibraryView({ cardState: 'all', limit: 1 }),
        ]);
        if (cancelled) return;
        setToday(todayView);
        setReadingCounts({
          want: countsPage.counts?.reading?.want ?? 0,
          inUse: countsPage.counts?.reading?.in_use ?? 0,
        });
        setPhase('ready');
      } catch (reason) {
        if (cancelled) return;
        setPhase('error');
        setLoadError(describeInvokeError(reason));
      }
    })();
    void listScenarios().then((items) => {
      if (!cancelled) setScenarios(items);
    }).catch(() => {
      if (!cancelled) setScenarios([]);
    });
    void getGithubAuthState().then((state) => {
      if (cancelled) return;
      const display = state.user?.name?.trim();
      if (display) setName(display);
    }).catch(() => {
      if (!cancelled) setName('Fox');
    });
    return () => {
      cancelled = true;
    };
  }, [reloadKey]);

  useEffect(() => {
    const filterChanged = filterRef.current !== filter;
    filterRef.current = filter;
    if (filter === 'week') {
      setListPhase('idle');
      setListError(null);
      return;
    }
    let cancelled = false;
    if (filterChanged) setFilteredItems([]);
    setListPhase('loading');
    setListError(null);
    void listLibraryView(FILTER_QUERY[filter]).then((page) => {
      if (cancelled) return;
      setFilteredItems(page.items ?? []);
      setListPhase('idle');
    }).catch((reason) => {
      if (cancelled) return;
      setListPhase('error');
      setListError(describeInvokeError(reason));
    });
    return () => {
      cancelled = true;
    };
  }, [filter, listReloadKey]);

  const weekGroups = today
    ? assembleWeekGroups(
      applyReadingOverrides(today.picks ?? [], overrides),
      applyReadingOverrides(today.weekNew ?? [], overrides),
    )
    : { picks: [] as LibraryItem[], rest: [] as LibraryItem[] };
  const patchedFiltered = applyReadingOverrides(filteredItems, overrides);
  const plainItems = filter === 'want'
    ? itemsForReadingFilter(patchedFiltered, 'want')
    : filter === 'in_use'
      ? itemsForReadingFilter(patchedFiltered, 'in_use')
      : patchedFiltered;
  const picks = filter === 'week' ? weekGroups.picks : [];
  const rest = filter === 'week' ? weekGroups.rest : plainItems;
  const ordered = [...picks, ...rest];
  const orderedKey = ordered.map((item) => item.id).join('\u0001');

  useEffect(() => {
    const ids = orderedKey ? orderedKey.split('\u0001') : [];
    setSelectedId((current) => (current && ids.includes(current) ? current : ids[0] ?? null));
  }, [orderedKey]);

  useEffect(() => {
    if (!selectedId) {
      setDetail(null);
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
      setDetail(group ? { ...result, readingGroup: group } : result);
      setDetailPhase('idle');
    }).catch((reason) => {
      if (cancelled) return;
      setDetail(null);
      setDetailPhase('error');
      setDetailError(describeInvokeError(reason));
    });
    return () => {
      cancelled = true;
    };
  }, [selectedId, detailReloadKey]);

  function findRaw(id: string): LibraryItem | null {
    const pools = [...(today?.picks ?? []), ...(today?.weekNew ?? []), ...filteredItems];
    return pools.find((item) => item.id === id) ?? null;
  }

  function displayedGroup(id: string): ReadingGroup | null {
    if (overrides[id]) return overrides[id];
    const raw = findRaw(id);
    if (raw) return raw.readingGroup;
    if (detail?.id === id) return detail.readingGroup;
    return null;
  }

  function labelOf(id: string): string {
    const raw = findRaw(id);
    if (raw) return raw.fullName;
    if (detail?.id === id) return detail.fullName;
    return '';
  }

  async function refreshCounts() {
    const [todayView, countsPage] = await Promise.all([
      getTodayView(),
      listLibraryView({ cardState: 'all', limit: 1 }),
    ]);
    setToday(todayView);
    setReadingCounts({
      want: countsPage.counts?.reading?.want ?? 0,
      inUse: countsPage.counts?.reading?.in_use ?? 0,
    });
  }

  async function changeReading(id: string, group: ReadingGroup) {
    const previous = displayedGroup(id);
    const server = findRaw(id)?.readingGroup ?? (detail?.id === id ? detail.readingGroup : null);
    if (!previous || previous === group) return;
    const projectName = labelOf(id);
    setOverrides((current) => ({ ...current, [id]: group }));
    setDetail((current) => (current && current.id === id ? { ...current, readingGroup: group } : current));
    try {
      await setLibraryReading([id], group);
      toast.show({
        title: `已标为「${readingGroupLabel(group)}」`,
        sub: projectName || undefined,
        actions: [{ label: '撤销', onClick: () => { void undoReading(id, previous, group); } }],
      });
      try {
        await refreshCounts();
        setListReloadKey((key) => key + 1);
      } catch (reason) {
        toast.show({ title: describeInvokeError(reason), sub: '状态已经改了，数字可能还没刷新' });
      }
    } catch (reason) {
      setOverrides((current) => {
        if (current[id] !== group) return current;
        const next = { ...current };
        if (server && previous === server) delete next[id];
        else next[id] = previous;
        return next;
      });
      setDetail((current) => (current && current.id === id ? { ...current, readingGroup: previous } : current));
      toast.show({ title: describeInvokeError(reason) });
    }
  }

  async function undoReading(id: string, previous: ReadingGroup, written: ReadingGroup) {
    setOverrides((current) => ({ ...current, [id]: previous }));
    setDetail((current) => (current && current.id === id ? { ...current, readingGroup: previous } : current));
    try {
      await setLibraryReading([id], previous);
      try {
        await refreshCounts();
        setListReloadKey((key) => key + 1);
      } catch (reason) {
        toast.show({ title: describeInvokeError(reason), sub: '已撤销，数字可能还没刷新' });
      }
    } catch (reason) {
      setOverrides((current) => (current[id] === previous ? { ...current, [id]: written } : current));
      setDetail((current) => (current && current.id === id ? { ...current, readingGroup: written } : current));
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
      if (message.includes('这个功能还没接上') && detail?.id === id) {
        text = codexFallbackText(detail);
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
      setDetail((current) => (current && current.id === id ? { ...current, cardReview: 'confirmed' } : current));
    } catch (reason) {
      toast.show({ title: describeInvokeError(reason) });
    }
  }

  async function regenerateCard(id: string) {
    try {
      await regenerateLibraryCard(id);
      toast.show({ title: '已排队，整理好会更新' });
    } catch (reason) {
      toast.show({ title: describeInvokeError(reason) });
    }
  }

  useListKeys({
    ids: ordered.map((item) => item.id),
    currentId: selectedId,
    onMove: (id) => setSelectedId(id),
    onStatus: (group) => {
      if (selectedId) void changeReading(selectedId, group);
    },
    onCopy: () => {
      if (selectedId) void copyForCodex(selectedId);
    },
  });

  function choose(next: TodayFilter) {
    setFilter(next);
    if (narrow) setDetailOpen(false);
  }

  function pickRow(id: string) {
    setSelectedId(id);
    if (narrow) setDetailOpen(true);
  }

  const subtitle = !today && phase === 'loading'
    ? '正在整理今天的清单…'
    : !today && phase === 'error'
      ? '今天的清单没有读出来。'
      : today
        ? buildTodayLead(today.counts?.weekNew ?? 0, weekGroups.picks.length)
        : '正在整理今天的清单…';
  const matchedDetail = detail && detail.id === selectedId ? detail : null;
  const shownDetail = matchedDetail
    ? { ...matchedDetail, readingGroup: overrides[matchedDetail.id] ?? matchedDetail.readingGroup }
    : null;
  const detailLoading = Boolean(selectedId) && !shownDetail && detailPhase !== 'error';
  const hideList = narrow && detailOpen;
  const hideDetail = narrow && !detailOpen;
  const empty = emptyCopy(filter);

  return (
    <section className="flex h-full min-h-0 flex-col gap-3 overflow-hidden px-5 pt-4 pb-5 max-[899px]:px-4 max-[899px]:pt-3 max-[899px]:pb-4">
      <PageHeader title={`${greetingFor(new Date().getHours())}，${name}`} subtitle={subtitle} />
      <div className="grid shrink-0 grid-cols-2 gap-3 min-[900px]:grid-cols-4">
        <StatCard label="本周新收藏" value={today ? (today.counts?.weekNew ?? 0) : '…'} active={filter === 'week'} onClick={() => choose('week')} />
        <StatCard label="待整理" value={today ? (today.counts?.cardsPending ?? 0) : '…'} active={filter === 'pending'} onClick={() => choose('pending')} />
        <StatCard label="想试" value={readingCounts ? readingCounts.want : '…'} active={filter === 'want'} onClick={() => choose('want')} />
        <StatCard label="在用" value={readingCounts ? readingCounts.inUse : '…'} active={filter === 'in_use'} onClick={() => choose('in_use')} />
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 min-[900px]:grid-cols-[minmax(340px,460px)_minmax(0,1fr)]">
        <Card className={cn('flex min-h-0 flex-col overflow-hidden', hideList && 'hidden')}>
          <div className="min-h-0 flex-1 overflow-auto py-1">
            {renderListBody()}
          </div>
          <div className="flex shrink-0 flex-wrap gap-2.5 border-t border-muted px-4 py-2.5 text-xs text-on-surface-variant">
            <span className="inline-flex items-center gap-1"><Kbd>J</Kbd><Kbd>K</Kbd> 上下</span>
            <span className="inline-flex items-center gap-1"><Kbd>T</Kbd> 想试</span>
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
            onBack={narrow ? () => setDetailOpen(false) : undefined}
            onRetry={() => setDetailReloadKey((key) => key + 1)}
            onReadingChange={(group) => { if (selectedId) void changeReading(selectedId, group); }}
            onConfirm={() => { if (selectedId) void confirmCard(selectedId); }}
            onRegenerate={() => { if (selectedId) void regenerateCard(selectedId); }}
            onCopyCodex={() => { if (selectedId) void copyForCodex(selectedId); }}
            onCopyHow={(text) => {
              void writeClipboard(text).then((copied) => {
                if (copied) toast.show({ title: '已复制' });
              });
            }}
          />
        </Card>
      </div>
    </section>
  );

  function renderListBody() {
    if (filter === 'week' && !today && phase === 'loading') return <ListSkeleton />;
    if (filter === 'week' && !today && phase === 'error') {
      return <ListMessage message={loadError ?? '今天的清单没有读出来。'} onRetry={() => setReloadKey((key) => key + 1)} />;
    }
    if (filter !== 'week' && listPhase === 'loading' && filteredItems.length === 0) return <ListSkeleton />;
    if (filter !== 'week' && listPhase === 'error' && ordered.length === 0) {
      return <ListMessage message={listError ?? '这个列表没有读出来。'} onRetry={() => setListReloadKey((key) => key + 1)} />;
    }
    if (ordered.length === 0) {
      return (
        <div className="flex h-full items-center justify-center">
          <EmptyState icon="inbox" title={empty.title} description={empty.description} />
        </div>
      );
    }
    return (
      <>
        {filter !== 'week' && listPhase === 'error' && listError ? (
          <ListMessage message={listError} onRetry={() => setListReloadKey((key) => key + 1)} />
        ) : null}
        {picks.length > 0 ? (
          <>
            <GroupHeader title="今天值得先看" count={picks.length} hint="从你还没看过的里，按你常做的事挑的" />
            {picks.map((item) => (
              <ItemRow
                key={item.id}
                item={item}
                selected={item.id === selectedId}
                why={pickWhyLine(item)}
                onSelect={() => pickRow(item.id)}
                onWant={() => void changeReading(item.id, 'want')}
                onDrop={() => void changeReading(item.id, 'dropped')}
              />
            ))}
          </>
        ) : null}
        {rest.length > 0 ? (
          <>
            <GroupHeader title={filter === 'week' ? '本周新收藏' : FILTER_LABEL[filter]} count={rest.length} />
            {rest.map((item) => (
              <ItemRow
                key={item.id}
                item={item}
                selected={item.id === selectedId}
                onSelect={() => pickRow(item.id)}
                onWant={() => void changeReading(item.id, 'want')}
                onDrop={() => void changeReading(item.id, 'dropped')}
              />
            ))}
          </>
        ) : null}
      </>
    );
  }
}

function emptyCopy(filter: TodayFilter): { title: string; description: string } {
  if (filter === 'want') {
    return {
      title: '还没有标成「想试」的工具',
      description: '在右边点「想试」，它就会出现在这里',
    };
  }
  return {
    title: '这里没有内容',
    description: '在右边点「想试」，它就会出现在这里',
  };
}

function GroupHeader(props: { title: string; count: number; hint?: string }) {
  return (
    <div className="flex flex-wrap items-center gap-1.5 px-4 pt-3 pb-1 text-xs text-on-surface-variant">
      <span>{props.title}</span>
      <span className="font-medium">{props.count}</span>
      {props.hint ? <span>· {props.hint}</span> : null}
    </div>
  );
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
