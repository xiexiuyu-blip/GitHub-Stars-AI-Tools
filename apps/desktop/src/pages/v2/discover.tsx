import { useEffect, useMemo, useState } from 'react';
import type { CSSProperties } from 'react';
import type { DiscoverView } from '@/lib/contracts';
import { commitCaptureBatch, describeInvokeError, isMissingCommandError, listDiscover, refreshDiscover } from '@/lib/api';
import {
  addedToInboxToast,
  buildEvaluatePrompt,
  candidateStatus,
  discoverEmptyCopy,
  sortCandidates,
  refreshNotice,
  scenarioLead,
  sortScenarios,
  type RefreshNotice,
} from '@/lib/discover-view';
import { COPY_FAILED_NOTICE, copiedNotice } from '@/lib/item-format';
import { scenarioHue } from '@/lib/repo-assets';
import { Button, Card, EmptyState, PageHeader, Spinner, useToast } from '@/components/kit';
import { CandidateCard, ScenarioRow } from '@/components/discover';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

type Phase = 'loading' | 'ready' | 'error';

/**
 * 发现：按场景去 GitHub 找还没收藏的工具。
 * 左边是场景（偏少的排前面），右边是候选。联网只发生在点「去 GitHub 找找」的时候；平时看的是本地缓存。
 */
export function DiscoverPage(props: { initialScenarioId?: string | null; onOpenCapture?: () => void }) {
  const toast = useToast();
  const [view, setView] = useState<DiscoverView | null>(null);
  const [phase, setPhase] = useState<Phase>('loading');
  const [error, setError] = useState<string | null>(null);
  const [unavailable, setUnavailable] = useState(false);
  const [selected, setSelected] = useState<string | null>(props.initialScenarioId ?? null);
  const [switching, setSwitching] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const [notice, setNotice] = useState<RefreshNotice | null>(null);
  const [adding, setAdding] = useState<string | null>(null);
  const [added, setAdded] = useState<ReadonlySet<string>>(() => new Set());
  const now = new Date();

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
  const candidates = useMemo(() => sortCandidates(view?.candidates ?? [], added), [view, added]);
  const lead = scenarioLead(current, now);

  function choose(id: string) {
    if (id === (view?.selected ?? selected)) return;
    setNotice(null);
    setSelected(id);
  }

  async function refresh() {
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
    const text = buildEvaluatePrompt({ fullName, htmlUrl }, current?.nameZh ?? '');
    try {
      if (!navigator.clipboard?.writeText) throw new Error('clipboard');
      await navigator.clipboard.writeText(text);
      toast.show({ title: copiedNotice('evaluate') });
    } catch {
      toast.show({ title: COPY_FAILED_NOTICE });
    }
  }

  return (
    <section className="flex h-full min-h-0 flex-col gap-3 overflow-hidden px-5 pt-4 pb-5 max-[899px]:px-4 max-[899px]:pt-3 max-[899px]:pb-4">
      <PageHeader title="发现" subtitle="按场景去 GitHub 找还没收藏的好工具。" />
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
                onClick={() => choose(scenario.id)}
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
                <Button variant="primary" loading={refreshing} onClick={() => void refresh()}>
                  <Icon name="search" size={16} />
                  {current.lastFetchedAt ? '重新找找' : '去 GitHub 找找'}
                </Button>
              </Card>

              {refreshing ? (
                <div className="flex items-center gap-2 rounded-lg bg-accent px-3.5 py-2.5 text-sm text-foreground">
                  <Spinner />
                  正在 GitHub 上找（要几秒到十几秒，不用登录）…
                </div>
              ) : null}
              {notice ? (
                <div className={cn('rounded-lg px-3.5 py-2.5 text-sm', notice.tone === 'warn' ? 'bg-error-container text-on-error-container' : 'bg-accent text-foreground')}>
                  {notice.text}
                </div>
              ) : null}

              {candidates.length > 0 ? (
                <div className={cn('grid grid-cols-1 gap-3 min-[1100px]:grid-cols-2 transition-opacity', switching && 'opacity-60')}>
                  {candidates.map((candidate) => (
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
              ) : refreshing || switching ? null : (
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
    </section>
  );
}
