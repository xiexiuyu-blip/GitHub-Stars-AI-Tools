import { useEffect, useMemo, useRef, useState } from 'react';
import type { CSSProperties, FormEvent } from 'react';
import type {
  DiscoverFeedId,
  DiscoverFeedView,
  DiscoverSearchResult,
  DiscoverSearchSort,
  DiscoverSearchTimeRange,
  DiscoverView,
} from '@/lib/contracts';
import {
  commitCaptureBatch,
  describeInvokeError,
  isMissingCommandError,
  listDiscover,
  listDiscoverFeed,
  refreshDiscover,
  refreshDiscoverFeed,
  searchDiscoverGithub,
} from '@/lib/api';
import {
  DISCOVER_FEEDS,
  DISCOVER_LANGUAGES,
  DISCOVER_SEARCH_PRESETS,
  DISCOVER_SEARCH_SORTS,
  DISCOVER_SEARCH_TIME_RANGES,
  addedToInboxToast,
  buildEvaluatePrompt,
  candidateStatus,
  discoverEmptyCopy,
  feedEmptyCopy,
  feedLead,
  feedMeta,
  feedRefreshNotice,
  filterDiscoverItems,
  refreshNotice,
  scenarioLead,
  sortCandidates,
  sortScenarios,
  type RefreshNotice,
} from '@/lib/discover-view';
import { COPY_FAILED_NOTICE, copiedNotice } from '@/lib/item-format';
import { scenarioHue } from '@/lib/repo-assets';
import { Button, Card, EmptyState, PageHeader, Segmented, Spinner, useToast } from '@/components/kit';
import { CandidateCard, ScenarioRow } from '@/components/discover';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

type DiscoverMode = 'trending' | 'search' | 'scenarios';
type Phase = 'loading' | 'ready' | 'error';

const MODE_OPTIONS: ReadonlyArray<{ value: DiscoverMode; label: string }> = [
  { value: 'trending', label: '热门趋势' },
  { value: 'search', label: '自由搜索' },
  { value: 'scenarios', label: '按场景补缺' },
];

/**
 * 发现工作台：
 * 1. 热门趋势：今日飙升 / 本周热门 / 本周新锐 / 本月黑马 / HN Show 热门（平时读缓存，刷新联网）
 * 2. 自由搜索：按关键词、时间范围、排序和语言实时在 GitHub 找新工具
 * 3. 按场景补缺：围绕自身收藏缺口分批找候选（经典高星 + 近 90 天新锐）
 */
export function DiscoverPage(props: { initialScenarioId?: string | null; onOpenCapture?: () => void }) {
  const toast = useToast();
  const [mode, setMode] = useState<DiscoverMode>(props.initialScenarioId ? 'scenarios' : 'trending');
  const [hideMine, setHideMine] = useState(false);
  const [adding, setAdding] = useState<string | null>(null);
  const [added, setAdded] = useState<ReadonlySet<string>>(() => new Set());
  const now = new Date();

  // ── 热门趋势 state ──
  const [feedId, setFeedId] = useState<DiscoverFeedId>('trending_daily');
  const [feedLanguage, setFeedLanguage] = useState('');
  const [feedView, setFeedView] = useState<DiscoverFeedView | null>(null);
  const [feedPhase, setFeedPhase] = useState<Phase>('loading');
  const [feedError, setFeedError] = useState<string | null>(null);
  const [feedUnavailable, setFeedUnavailable] = useState(false);
  const [feedReloadKey, setFeedReloadKey] = useState(0);
  const [feedRefreshing, setFeedRefreshing] = useState(false);
  const [feedNotice, setFeedNotice] = useState<RefreshNotice | null>(null);

  // ── 自由搜索 state ──
  const [searchQuery, setSearchQuery] = useState('');
  const [searchTimeRange, setSearchTimeRange] = useState<DiscoverSearchTimeRange>('30d');
  const [searchSort, setSearchSort] = useState<DiscoverSearchSort>('stars');
  const [searchLanguage, setSearchLanguage] = useState('');
  const [searching, setSearching] = useState(false);
  const [searchResult, setSearchResult] = useState<DiscoverSearchResult | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);

  // ── 按场景补缺 state ──
  const [view, setView] = useState<DiscoverView | null>(null);
  const [phase, setPhase] = useState<Phase>('loading');
  const [error, setError] = useState<string | null>(null);
  const [unavailable, setUnavailable] = useState(false);
  const [selected, setSelected] = useState<string | null>(props.initialScenarioId ?? null);
  const [switching, setSwitching] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const [notice, setNotice] = useState<RefreshNotice | null>(null);
  const searchSeqRef = useRef(0);
  const feedRefreshSeqRef = useRef(0);

  useEffect(() => {
    if (props.initialScenarioId) {
      setMode('scenarios');
      setSelected(props.initialScenarioId);
    }
  }, [props.initialScenarioId]);

  const activeFeedMeta = useMemo(() => feedMeta(feedId), [feedId]);
  const effectiveFeedLang = activeFeedMeta.supportsLanguage ? feedLanguage || null : null;
  const feedMatches =
    feedView !== null
    && feedView.feedId === feedId
    && (feedView.language ?? null) === effectiveFeedLang;
  const matchedFeedView = feedMatches ? feedView : null;

  useEffect(() => {
    let cancelled = false;
    setFeedPhase('loading');
    setFeedError(null);
    listDiscoverFeed(feedId, effectiveFeedLang)
      .then((next) => {
        if (cancelled) return;
        setFeedView(next);
        setFeedPhase('ready');
        setFeedError(null);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setFeedError(describeInvokeError(reason));
        setFeedUnavailable(isMissingCommandError(reason));
        setFeedPhase('error');
      });
    return () => {
      cancelled = true;
    };
  }, [feedId, effectiveFeedLang, feedReloadKey]);

  useEffect(() => {
    let cancelled = false;
    setSwitching(true);
    listDiscover(selected)
      .then((next) => {
        if (cancelled) return;
        setView(next);
        setPhase('ready');
        setError(null);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setError(describeInvokeError(reason));
        setUnavailable(isMissingCommandError(reason));
        setPhase((current) => (current === 'ready' ? current : 'error'));
      })
      .finally(() => {
        if (!cancelled) setSwitching(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selected, reloadKey]);

  const scenarios = useMemo(() => sortScenarios(view?.scenarios ?? []), [view]);
  const current = scenarios.find((item) => item.id === (view?.selected ?? selected)) ?? null;
  const scenarioCandidates = useMemo(
    () => filterDiscoverItems(sortCandidates(view?.candidates ?? [], added), hideMine),
    [view, added, hideMine],
  );
  const feedItems = useMemo(() => filterDiscoverItems(matchedFeedView?.items ?? [], hideMine), [matchedFeedView, hideMine]);
  const searchItems = useMemo(() => filterDiscoverItems(searchResult?.items ?? [], hideMine), [searchResult, hideMine]);

  const lead = scenarioLead(current, now);
  const trendingLead = feedLead(matchedFeedView, feedId, now);

  function chooseFeed(nextId: DiscoverFeedId) {
    if (nextId === feedId) return;
    feedRefreshSeqRef.current += 1;
    setFeedRefreshing(false);
    setFeedNotice(null);
    setFeedId(nextId);
  }

  function chooseFeedLanguage(nextLang: string) {
    if (nextLang === feedLanguage) return;
    feedRefreshSeqRef.current += 1;
    setFeedRefreshing(false);
    setFeedNotice(null);
    setFeedLanguage(nextLang);
  }

  async function refreshTrending() {
    if (feedRefreshing) return;
    const seq = ++feedRefreshSeqRef.current;
    const targetFeedId = feedId;
    const targetLang = effectiveFeedLang;
    setFeedRefreshing(true);
    setFeedNotice(null);
    try {
      const report = await refreshDiscoverFeed(targetFeedId, targetLang);
      if (seq !== feedRefreshSeqRef.current) return;
      setFeedNotice(feedRefreshNotice(report));
      setFeedReloadKey((key) => key + 1);
    } catch (reason) {
      if (seq !== feedRefreshSeqRef.current) return;
      setFeedNotice({ tone: 'warn', text: describeInvokeError(reason) });
    } finally {
      if (seq === feedRefreshSeqRef.current) setFeedRefreshing(false);
    }
  }

  async function runSearch(
    nextQuery: string,
    opts?: { timeRange?: DiscoverSearchTimeRange; sort?: DiscoverSearchSort; language?: string },
  ) {
    const seq = ++searchSeqRef.current;
    const trimmed = nextQuery.trim();
    if (!trimmed) {
      setSearching(false);
      setSearchError('请先输入想搜的关键词');
      return;
    }
    const timeRange = opts?.timeRange ?? searchTimeRange;
    const sort = opts?.sort ?? searchSort;
    const language = opts?.language ?? searchLanguage;
    setSearching(true);
    setSearchError(null);
    try {
      const res = await searchDiscoverGithub({
        query: trimmed,
        timeRange,
        sort,
        language: language || null,
      });
      if (seq !== searchSeqRef.current) return;
      setSearchResult(res);
      if (res.message) setSearchError(res.message);
    } catch (reason) {
      if (seq !== searchSeqRef.current) return;
      setSearchError(describeInvokeError(reason));
    } finally {
      if (seq === searchSeqRef.current) setSearching(false);
    }
  }

  function onSearchSubmit(event: FormEvent) {
    event.preventDefault();
    void runSearch(searchQuery);
  }

  function chooseScenario(id: string) {
    if (id === (view?.selected ?? selected)) return;
    setNotice(null);
    setSelected(id);
  }

  async function refreshScenario() {
    if (!current || refreshing) return;
    setRefreshing(true);
    setNotice(null);
    try {
      const report = await refreshDiscover(current.id);
      setNotice(refreshNotice(report));
      setReloadKey((key) => key + 1);
    } catch (reason) {
      setNotice({ tone: 'warn', text: describeInvokeError(reason) });
    } finally {
      setRefreshing(false);
    }
  }

  async function addToInbox(fullName: string, htmlUrl: string) {
    setAdding(fullName);
    try {
      const result = await commitCaptureBatch(htmlUrl || `https://github.com/${fullName}`, '发现');
      setAdded((currentSet) => new Set(currentSet).add(fullName));
      toast.show({
        ...addedToInboxToast(result.created, fullName),
        actions: props.onOpenCapture ? [{ label: '去看看', onClick: props.onOpenCapture }] : undefined,
      });
    } catch (reason) {
      toast.show({ title: describeInvokeError(reason) });
    } finally {
      setAdding(null);
    }
  }

  async function copyEvaluate(fullName: string, htmlUrl: string) {
    const text = buildEvaluatePrompt({ fullName, htmlUrl }, mode === 'scenarios' ? current?.nameZh ?? '' : '');
    try {
      if (!navigator.clipboard?.writeText) throw new Error('clipboard');
      await navigator.clipboard.writeText(text);
      toast.show({ title: copiedNotice('evaluate') });
    } catch {
      toast.show({ title: COPY_FAILED_NOTICE });
    }
  }

  const hideMineToggle = (
    <button
      type="button"
      aria-pressed={hideMine}
      onClick={() => setHideMine((prev) => !prev)}
      className={cn(
        'inline-flex h-8 items-center gap-1.5 rounded-full border px-3 text-xs font-medium transition-colors',
        hideMine
          ? 'border-primary/40 bg-primary/10 text-primary'
          : 'border-border bg-surface text-on-surface-variant hover:text-foreground',
      )}
    >
      <Icon name={hideMine ? 'check_box' : 'check_box_outline_blank'} size={16} />
      只看未收藏
    </button>
  );

  return (
    <section className="flex h-full min-h-0 flex-col gap-3 overflow-hidden px-5 pt-4 pb-5 max-[899px]:px-4 max-[899px]:pt-3 max-[899px]:pb-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <PageHeader title="发现" subtitle="看热门趋势、自由搜索 GitHub 新项目，或按场景补缺还没收藏的好工具。" />
        <Segmented ariaLabel="发现模式" options={MODE_OPTIONS} value={mode} onChange={setMode} />
      </div>

      {mode === 'trending' ? (
        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-auto">
          <Card className="flex flex-col gap-3 p-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex flex-wrap items-center gap-1.5" role="group" aria-label="趋势榜单">
                {DISCOVER_FEEDS.map((feed) => {
                  const active = feed.id === feedId;
                  return (
                    <button
                      key={feed.id}
                      type="button"
                      aria-pressed={active}
                      onClick={() => chooseFeed(feed.id)}
                      className={cn(
                        'h-8 rounded-full px-3.5 text-sm font-medium transition-colors',
                        active
                          ? 'bg-primary text-white shadow-xs'
                          : 'bg-surface-container-low text-on-surface-variant hover:bg-surface-container-high hover:text-foreground',
                      )}
                    >
                      {feed.label}
                    </button>
                  );
                })}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                {hideMineToggle}
                <Button variant="primary" loading={feedRefreshing} onClick={() => void refreshTrending()}>
                  <Icon name="refresh" size={16} />
                  {matchedFeedView?.lastFetchedAt ? '刷新榜单' : '拉取榜单'}
                </Button>
              </div>
            </div>

            <div className="flex flex-wrap items-center justify-between gap-2">
              <p className="m-0 text-[13px] leading-[1.6] text-on-surface-variant">{trendingLead}</p>
              {activeFeedMeta.supportsLanguage ? (
                <div className="flex flex-wrap items-center gap-1" role="group" aria-label="语言筛选">
                  {DISCOVER_LANGUAGES.map((lang) => {
                    const active = lang.id === feedLanguage;
                    return (
                      <button
                        key={lang.id || 'all'}
                        type="button"
                        aria-pressed={active}
                        onClick={() => chooseFeedLanguage(lang.id)}
                        className={cn(
                          'h-7 rounded-md px-2.5 text-xs font-medium transition-colors',
                          active
                            ? 'bg-accent text-accent-foreground'
                            : 'text-on-surface-variant hover:bg-surface-container-low hover:text-foreground',
                        )}
                      >
                        {lang.label}
                      </button>
                    );
                  })}
                </div>
              ) : null}
            </div>
          </Card>

          {feedRefreshing ? (
            <div className="flex items-center gap-2 rounded-lg bg-accent px-3.5 py-2.5 text-sm text-foreground">
              <Spinner />
              正在拉取「{activeFeedMeta.label}」最新榜单（一般几秒完成）…
            </div>
          ) : null}

          {feedNotice ? (
            <div className={cn('rounded-lg px-3.5 py-2.5 text-sm', feedNotice.tone === 'warn' ? 'bg-error-container text-on-error-container' : 'bg-accent text-foreground')}>
              {feedNotice.text}
            </div>
          ) : matchedFeedView?.message ? (
            <div className="rounded-lg bg-surface-container-high px-3.5 py-2.5 text-sm text-on-surface-variant">
              {matchedFeedView.message}
            </div>
          ) : null}

          {feedPhase === 'error' ? (
            <Card className="p-6">
              <EmptyState
                icon={feedUnavailable ? 'construction' : 'error'}
                title={feedUnavailable ? '榜单功能还在接后台' : '榜单缓存没有读出来'}
                description={feedUnavailable ? '界面已经好了，后台下一次更新后就能用。' : feedError ?? undefined}
                action={feedUnavailable ? undefined : <Button variant="text" onClick={() => setFeedReloadKey((k) => k + 1)}>重试</Button>}
              />
            </Card>
          ) : feedItems.length > 0 ? (
            <div className="grid grid-cols-1 gap-3 min-[1100px]:grid-cols-2">
              {feedItems.map((item) => (
                <CandidateCard
                  key={`${matchedFeedView?.feedKey ?? feedId}:${item.fullName}`}
                  candidate={item}
                  status={candidateStatus(item, added)}
                  adding={adding === item.fullName}
                  now={now}
                  onAdd={() => void addToInbox(item.fullName, item.htmlUrl)}
                  onCopyEvaluate={() => void copyEvaluate(item.fullName, item.htmlUrl)}
                  onOpenInbox={props.onOpenCapture}
                />
              ))}
            </div>
          ) : feedPhase === 'loading' || feedRefreshing || !feedMatches ? (
            <div className="flex items-center gap-2 rounded-lg border border-card-border bg-card px-4 py-6 text-sm text-on-surface-variant">
              <Spinner />
              正在读取「{activeFeedMeta.label}」…
            </div>
          ) : (matchedFeedView?.items.length ?? 0) > 0 && hideMine ? (
            <Card className="flex flex-1 p-6">
              <EmptyState
                icon="done_all"
                title="这个榜单里的项目你都收藏过了"
                description="关掉「只看未收藏」，就可以看到完整榜单。"
                action={<Button variant="text" onClick={() => setHideMine(false)}>显示全部</Button>}
              />
            </Card>
          ) : (
            <Card className="flex flex-1 p-6">
              <EmptyState icon="trending_up" {...feedEmptyCopy(feedId, matchedFeedView?.lastFetchedAt)} />
            </Card>
          )}
        </div>
      ) : mode === 'search' ? (
        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-auto">
          <Card className="flex flex-col gap-3 p-4">
            <form onSubmit={onSearchSubmit} className="flex flex-wrap items-center gap-2">
              <div className="relative min-w-[220px] flex-1">
                <Icon name="search" size={18} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-on-surface-variant" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="搜开源项目或关键词，比如 mcp server / claude skill / 浏览器自动化"
                  className="h-9 w-full rounded-lg border border-border bg-surface pr-3 pl-9 text-sm text-foreground focus:border-primary focus:outline-none"
                />
              </div>
              <Button type="submit" variant="primary" loading={searching}>
                <Icon name="travel_explore" size={16} />
                搜索
              </Button>
              {hideMineToggle}
            </form>

            <div className="flex flex-wrap items-center gap-1.5 text-xs text-on-surface-variant">
              <span>试试搜：</span>
              {DISCOVER_SEARCH_PRESETS.map((preset) => (
                <button
                  key={preset}
                  type="button"
                  onClick={() => {
                    setSearchQuery(preset);
                    void runSearch(preset);
                  }}
                  className="rounded-full border border-border bg-surface-container-low px-2.5 py-0.5 text-xs text-foreground transition-colors hover:border-primary/40 hover:bg-accent"
                >
                  {preset}
                </button>
              ))}
            </div>

            <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-border/60 pt-2.5 text-xs">
              <div className="flex flex-wrap items-center gap-1">
                <span className="text-on-surface-variant">时间：</span>
                {DISCOVER_SEARCH_TIME_RANGES.map((range) => (
                  <button
                    key={range.id}
                    type="button"
                    aria-pressed={searchTimeRange === range.id}
                    onClick={() => {
                      setSearchTimeRange(range.id);
                      if (searchQuery.trim()) void runSearch(searchQuery, { timeRange: range.id });
                    }}
                    className={cn(
                      'rounded-md px-2 py-1 font-medium transition-colors',
                      searchTimeRange === range.id
                        ? 'bg-accent text-accent-foreground'
                        : 'text-on-surface-variant hover:bg-surface-container-low hover:text-foreground',
                    )}
                  >
                    {range.label}
                  </button>
                ))}
              </div>

              <div className="flex flex-wrap items-center gap-1">
                <span className="text-on-surface-variant">排序：</span>
                {DISCOVER_SEARCH_SORTS.map((opt) => (
                  <button
                    key={opt.id}
                    type="button"
                    aria-pressed={searchSort === opt.id}
                    onClick={() => {
                      setSearchSort(opt.id);
                      if (searchQuery.trim()) void runSearch(searchQuery, { sort: opt.id });
                    }}
                    className={cn(
                      'rounded-md px-2 py-1 font-medium transition-colors',
                      searchSort === opt.id
                        ? 'bg-accent text-accent-foreground'
                        : 'text-on-surface-variant hover:bg-surface-container-low hover:text-foreground',
                    )}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>

              <div className="flex flex-wrap items-center gap-1">
                <span className="text-on-surface-variant">语言：</span>
                {DISCOVER_LANGUAGES.map((lang) => (
                  <button
                    key={lang.id || 'all'}
                    type="button"
                    aria-pressed={searchLanguage === lang.id}
                    onClick={() => {
                      setSearchLanguage(lang.id);
                      if (searchQuery.trim()) void runSearch(searchQuery, { language: lang.id });
                    }}
                    className={cn(
                      'rounded-md px-2 py-1 font-medium transition-colors',
                      searchLanguage === lang.id
                        ? 'bg-accent text-accent-foreground'
                        : 'text-on-surface-variant hover:bg-surface-container-low hover:text-foreground',
                    )}
                  >
                    {lang.label}
                  </button>
                ))}
              </div>
            </div>
          </Card>

          {searching ? (
            <div className="flex items-center gap-2 rounded-lg bg-accent px-3.5 py-2.5 text-sm text-foreground">
              <Spinner />
              正在 GitHub 搜索「{searchQuery.trim()}」…
            </div>
          ) : null}

          {searchError ? (
            <div className="rounded-lg bg-error-container px-3.5 py-2.5 text-sm text-on-error-container">
              {searchError}
            </div>
          ) : null}

          {searchResult && !searching ? (
            <div className="flex items-center justify-between px-1 text-xs text-on-surface-variant">
              <span>
                「{searchResult.query}」共找到约 {searchResult.totalCount.toLocaleString()} 个项目，展示前 {searchItems.length} 个
              </span>
            </div>
          ) : null}

          {searchItems.length > 0 ? (
            <div className="grid grid-cols-1 gap-3 min-[1100px]:grid-cols-2">
              {searchItems.map((item) => (
                <CandidateCard
                  key={`search:${item.fullName}`}
                  candidate={item}
                  status={candidateStatus(item, added)}
                  adding={adding === item.fullName}
                  now={now}
                  onAdd={() => void addToInbox(item.fullName, item.htmlUrl)}
                  onCopyEvaluate={() => void copyEvaluate(item.fullName, item.htmlUrl)}
                  onOpenInbox={props.onOpenCapture}
                />
              ))}
            </div>
          ) : searching ? null : searchResult ? (
            <Card className="flex flex-1 p-6">
              <EmptyState
                icon="search_off"
                title={hideMine && searchResult.items.length > 0 ? '搜索结果里的项目你都收藏过了' : '没有搜到符合条件的新项目'}
                description={hideMine && searchResult.items.length > 0 ? '关掉「只看未收藏」即可查看全部搜索结果。' : '可以把时间改成「近 3 个月新建」或「不限时间」，或者换个关键词再搜。'}
              />
            </Card>
          ) : (
            <Card className="flex flex-1 p-6">
              <EmptyState
                icon="travel_explore"
                title="想找什么工具，直接搜 GitHub"
                description="默认按近 30 天新建、星数最高排序，帮你优先看到最近冒出来的新项目。如果设置里连了 GitHub 账号，还会自动复用账号额度。"
              />
            </Card>
          )}
        </div>
      ) : (
        <div className="grid min-h-0 flex-1 grid-cols-[220px_minmax(0,1fr)] gap-3 max-[899px]:grid-cols-1 max-[899px]:grid-rows-[auto_minmax(0,1fr)]">
          <Card className="flex min-h-0 flex-col overflow-hidden max-[899px]:overflow-visible">
            <div className="px-4 pt-3 pb-1 text-xs text-on-surface-variant max-[899px]:hidden">场景（偏少的在前面）</div>
            <div className="flex min-h-0 flex-1 flex-col overflow-auto py-1.5 max-[899px]:flex-row max-[899px]:gap-1 max-[899px]:overflow-x-auto max-[899px]:px-2 max-[899px]:py-2">
              {phase === 'loading' ? <p className="px-4 py-2 text-xs text-on-surface-variant">正在读取…</p> : null}
              {scenarios.map((scenario) => (
                <ScenarioRow
                  key={scenario.id}
                  scenario={scenario}
                  active={scenario.id === (view?.selected ?? selected)}
                  onClick={() => chooseScenario(scenario.id)}
                />
              ))}
            </div>
          </Card>

          <div className="flex min-h-0 min-w-0 flex-col gap-3 overflow-auto">
            {phase === 'error' ? (
              <Card className="p-6">
                <EmptyState
                  icon={unavailable ? 'construction' : 'error'}
                  title={unavailable ? '发现功能还在接后台' : '发现页没有读出来'}
                  description={unavailable ? '界面已经好了，后台下一次更新后就能用。' : error ?? undefined}
                  action={unavailable ? undefined : <Button variant="text" onClick={() => { setPhase('loading'); setReloadKey((key) => key + 1); }}>重试</Button>}
                />
              </Card>
            ) : null}

            {phase === 'ready' && current ? (
              <>
                <Card className="flex flex-wrap items-center gap-x-4 gap-y-2 p-4">
                  <span className="hue-tile grid size-10 shrink-0 place-items-center rounded-[10px]" style={{ '--h': scenarioHue(current.id) } as CSSProperties}>
                    <Icon name="travel_explore" size={22} fill />
                  </span>
                  <div className="min-w-0 flex-1">
                    <h2 className="m-0 text-lg font-semibold text-foreground">{current.nameZh}</h2>
                    <p className="mt-0.5 mb-0 text-[13px] leading-[1.6] text-on-surface-variant">{lead}</p>
                  </div>
                  <div className="flex flex-wrap items-center gap-2">
                    {hideMineToggle}
                    <Button variant="primary" loading={refreshing} onClick={() => void refreshScenario()}>
                      <Icon name="search" size={16} />
                      {current.lastFetchedAt ? '重新找找' : '去 GitHub 找找'}
                    </Button>
                  </div>
                </Card>

                {refreshing ? (
                  <div className="flex items-center gap-2 rounded-lg bg-accent px-3.5 py-2.5 text-sm text-foreground">
                    <Spinner />
                    正在 GitHub 上找（一般几秒完成）…
                  </div>
                ) : null}
                {notice ? (
                  <div className={cn('rounded-lg px-3.5 py-2.5 text-sm', notice.tone === 'warn' ? 'bg-error-container text-on-error-container' : 'bg-accent text-foreground')}>
                    {notice.text}
                  </div>
                ) : null}

                {scenarioCandidates.length > 0 ? (
                  <div className={cn('grid grid-cols-1 gap-3 min-[1100px]:grid-cols-2 transition-opacity', switching && 'opacity-60')}>
                    {scenarioCandidates.map((candidate) => (
                      <CandidateCard
                        key={candidate.fullName}
                        candidate={candidate}
                        status={candidateStatus(candidate, added)}
                        adding={adding === candidate.fullName}
                        now={now}
                        onAdd={() => void addToInbox(candidate.fullName, candidate.htmlUrl)}
                        onCopyEvaluate={() => void copyEvaluate(candidate.fullName, candidate.htmlUrl)}
                        onOpenInbox={props.onOpenCapture}
                      />
                    ))}
                  </div>
                ) : refreshing || switching ? null : (view?.candidates.length ?? 0) > 0 && hideMine ? (
                  <Card className="flex flex-1 p-6">
                    <EmptyState
                      icon="done_all"
                      title="这个场景的候选你都已经收藏过了"
                      description="关掉「只看未收藏」，可以重新查看已收藏的条目。"
                      action={<Button variant="text" onClick={() => setHideMine(false)}>显示全部</Button>}
                    />
                  </Card>
                ) : (
                  <Card className="flex flex-1 p-6">
                    <EmptyState icon="travel_explore" {...discoverEmptyCopy(current)} />
                  </Card>
                )}
              </>
            ) : null}

            {phase === 'ready' && !current ? (
              <Card className="p-6">
                <EmptyState icon="category" {...discoverEmptyCopy(null)} />
              </Card>
            ) : null}
          </div>
        </div>
      )}
    </section>
  );
}
