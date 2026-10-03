import { FormEvent, useCallback, useEffect, useRef, useState } from 'react';
import type { PackFormat, PackOutput } from '@/lib/api';
import { buildReferencePack, describeInvokeError, listScenarios, solveV2 } from '@/lib/api';
import type { ScenarioSummary, SolveResult } from '@/lib/contracts';
import {
  buildPackRequest,
  crossNote,
  formatOptions,
  gapNote,
  initialPicks,
  noIntentNote,
  orderedPicks,
  packPlaceholder,
  pendingNote,
  todayLocalDate,
  togglePick,
} from '@/lib/solve-view';
import { Button, Card, CardHeader, EmptyState, PageHeader, Segmented, Spinner, useToast } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { SolveHitRow } from '@/components/solve/solve-hit-row';

const EXAMPLES = ['做小红书封面', '给公众号文章配图', '短视频加字幕', '做 PPT 和文档', '做电商详情页图片'];

const PREVIEW_CLASS = 'm-0 min-h-[180px] flex-1 overflow-auto bg-background px-4 py-3.5 font-mono text-xs leading-[1.7] break-words whitespace-pre-wrap text-foreground [overflow-wrap:anywhere] max-[899px]:max-h-[70vh] min-[900px]:max-h-none';

type PackState = {
  key: string;
  output: PackOutput | null;
  error: string | null;
};

export function SolvePage(props: {
  initialQuestion?: string;
  onOpenDiscover?: (scenarioId: string | null) => void;
}) {
  const toast = useToast();
  const searchSeq = useRef(0);
  const openingQuery = props.initialQuestion?.trim() ?? '';
  const [query, setQuery] = useState(props.initialQuestion ?? '');
  const [activeQuery, setActiveQuery] = useState('');
  const [phase, setPhase] = useState<'idle' | 'loading' | 'error' | 'ready'>(openingQuery ? 'loading' : 'idle');
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SolveResult | null>(null);
  const [picked, setPicked] = useState<string[]>([]);
  const [scenarios, setScenarios] = useState<ScenarioSummary[]>([]);
  const [format, setFormat] = useState<PackFormat>('markdown');
  const [packState, setPackState] = useState<PackState | null>(null);
  const [packAttempt, setPackAttempt] = useState(0);

  const runSearch = useCallback((raw: string) => {
    const text = raw.trim();
    if (!text) return;
    const seq = ++searchSeq.current;
    setActiveQuery(text);
    setPhase('loading');
    setError(null);
    setResult(null);
    setPicked([]);
    void solveV2(text).then((next) => {
      if (seq !== searchSeq.current) return;
      const ready = readSolveResult(next);
      setResult(ready);
      setPicked(initialPicks(ready));
      setPhase('ready');
    }).catch((reason) => {
      if (seq !== searchSeq.current) return;
      setError(describeInvokeError(reason));
      setPhase('error');
    });
  }, []);

  useEffect(() => {
    return () => {
      searchSeq.current += 1;
    };
  }, []);

  useEffect(() => {
    const text = props.initialQuestion?.trim() ?? '';
    if (!text) return;
    setQuery(text);
    runSearch(text);
  }, [props.initialQuestion, runSearch]);

  useEffect(() => {
    let cancelled = false;
    void listScenarios().then((items) => {
      if (!cancelled) setScenarios(Array.isArray(items) ? items : []);
    }).catch(() => {
      if (!cancelled) setScenarios([]);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const selectedHits = result ? orderedPicks(result.hits, picked) : [];
  const packKey = `${format}\u0001${selectedHits.map((hit) => hit.id).join('\u0001')}`;
  const packView = packState && packState.key === packKey ? packState : null;
  const pack = packView?.output ?? null;
  const hasResult = result !== null;
  const canCopy = selectedHits.length > 0 && Boolean(pack?.content);

  useEffect(() => {
    if (!result) return;
    const hits = orderedPicks(result.hits, picked);
    if (hits.length === 0) return;
    const key = packKey;
    const request = buildPackRequest(result.query.trim() || activeQuery, hits);
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (cancelled) return;
      void buildReferencePack(request, format, todayLocalDate(new Date())).then((output) => {
        if (cancelled) return;
        setPackState({ key, output: readPack(output), error: null });
      }).catch((reason) => {
        if (cancelled) return;
        setPackState({ key, output: null, error: describeInvokeError(reason) });
      });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [packKey, result, picked, format, packAttempt, activeQuery]);

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    runSearch(query);
  }

  function openDiscover(scenarioId: string | null) {
    props.onOpenDiscover?.(scenarioId);
  }

  async function copyToCodex() {
    if (!pack?.content || selectedHits.length === 0) return;
    const content = pack.content;
    const count = Number.isFinite(pack.itemCount) ? pack.itemCount : selectedHits.length;
    try {
      if (!navigator.clipboard?.writeText) throw new Error('clipboard');
      await navigator.clipboard.writeText(content);
      toast.show({ title: '已复制，去 Codex 里粘贴', sub: `${count} 个参考项目` });
    } catch {
      toast.show({ title: '浏览器不允许自动复制', sub: '请手动选中后按 ⌘C' });
    }
  }

  function retryPack() {
    setPackState(null);
    setPackAttempt((current) => current + 1);
  }

  // 没有结果时「找」是唯一的主按钮；有结果后改成次要，主按钮只留给「复制给 Codex」。
  const findButton = hasResult ? (
    <Button type="submit" variant="default" className="h-12 rounded-xl px-6 text-base" disabled={!query.trim()}>找</Button>
  ) : (
    <Button type="submit" variant="primary" className="h-12 rounded-xl px-6 text-base" disabled={!query.trim()}>找</Button>
  );

  const copyButton = hasResult ? (
    <Button variant="primary" disabled={!canCopy} onClick={() => void copyToCodex()}>
      <Icon name="content_copy" size={16} />
      复制给 Codex
    </Button>
  ) : (
    <Button variant="default" disabled={!canCopy} onClick={() => void copyToCodex()}>
      <Icon name="content_copy" size={16} />
      复制给 Codex
    </Button>
  );

  return (
    <section className="flex h-full min-h-0 flex-col gap-3 overflow-auto px-5 pt-4 pb-5 min-[900px]:overflow-hidden max-[899px]:px-4 max-[899px]:pt-3 max-[899px]:pb-4">
      <PageHeader
        title="找方案"
        subtitle="说一句你想做的事，从你收藏的里面挑出能用的，再整理成给 Codex 的参考包。"
      />
      <Card className="flex shrink-0 flex-col gap-3 p-4">
        <form className="flex gap-2" onSubmit={onSubmit}>
          <div className="relative min-w-0 flex-1">
            <Icon name="search" size={22} className="pointer-events-none absolute top-1/2 left-3.5 -translate-y-1/2 text-on-surface-variant" />
            <input
              className="h-12 w-full min-w-0 rounded-xl border border-input bg-card pr-3 pl-11 text-base text-foreground shadow-[0_1px_2px_rgba(31,35,41,0.05)] outline-none placeholder:text-on-surface-variant focus:border-primary focus:shadow-[0_0_0_3px_rgba(46,107,240,0.14)]"
              value={query}
              placeholder="你想解决什么？比如：做小红书封面"
              aria-label="你想解决什么"
              autoComplete="off"
              autoFocus={!openingQuery}
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
          {findButton}
        </form>
        <div className="flex flex-wrap gap-2">
          {EXAMPLES.map((example) => (
            <button
              key={example}
              type="button"
              className="inline-flex h-7 items-center rounded-full border border-input bg-card px-3 text-sm whitespace-nowrap text-on-surface-variant hover:bg-surface-container-low hover:text-foreground"
              onClick={() => {
                setQuery(example);
                runSearch(example);
              }}
            >
              {example}
            </button>
          ))}
        </div>
      </Card>
      <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(240px,1fr)_minmax(280px,1fr)] gap-3 min-[900px]:grid-cols-[minmax(0,1fr)_minmax(340px,460px)] min-[900px]:grid-rows-[minmax(0,1fr)]">
        <Card className="flex min-h-0 flex-col overflow-hidden">
          <CardHeader
            title="你已经有的"
            count={result ? result.hits.length : undefined}
            actions={result && result.hits.length > 0 ? (
              <span className="text-xs text-on-surface-variant">勾选的会进右边的参考包</span>
            ) : undefined}
          />
          {result ? <GapBanner result={result} onOpenDiscover={openDiscover} /> : null}
          {result ? <NoIntentBanner result={result} /> : null}
          <div className="min-h-0 flex-1 overflow-auto">
            <ResultBody
              scenarios={scenarios}
              phase={phase}
              error={error}
              result={result}
              picked={picked}
              onToggle={(id) => setPicked((current) => togglePick(current, id))}
              onRetry={() => runSearch(query)}
              onOpenDiscover={openDiscover}
            />
          </div>
          {result && result.hits.length > 0 ? (
            <ResultNotes result={result} scenarios={scenarios} onOpenDiscover={openDiscover} />
          ) : null}
        </Card>
        <Card className="flex min-h-0 flex-col overflow-hidden">
          <div className="flex min-h-12 shrink-0 flex-wrap items-center gap-2.5 border-b border-muted px-4 py-2">
            <h3 className="m-0 text-base font-semibold whitespace-nowrap text-foreground">参考包</h3>
            <span className="text-xs font-medium text-on-surface-variant">已选 {selectedHits.length} 个</span>
            <div className="min-w-2 flex-1" />
            <Segmented
              options={formatOptions}
              value={format}
              onChange={setFormat}
              ariaLabel="格式"
            />
          </div>
          {pack && pack.warnings.length > 0 ? (
            <div className="shrink-0 space-y-1 px-4 pt-2.5">
              {pack.warnings.map((warning, index) => (
                <p key={`${index}-${warning}`} className="m-0 text-xs text-on-surface-variant">{warning}</p>
              ))}
            </div>
          ) : null}
          {pack && pack.missing.length > 0 ? (
            <p className="m-0 shrink-0 px-4 pt-2.5 text-xs text-on-surface-variant">有 {pack.missing.length} 个项目已不在库里，已跳过</p>
          ) : null}
          {selectedHits.length === 0 ? (
            <pre className={PREVIEW_CLASS}>{packPlaceholder(format)}</pre>
          ) : packView?.error ? (
            <div className="flex flex-1 flex-col items-start justify-center gap-2 px-4 py-6">
              <p className="m-0 text-sm text-on-surface-variant">{packView.error}</p>
              <Button variant="text" onClick={retryPack}>重试</Button>
            </div>
          ) : pack ? (
            <pre className={PREVIEW_CLASS}>{pack.content}</pre>
          ) : (
            <div className="flex flex-1 items-center justify-center" role="status" aria-label="正在整理参考包">
              <Spinner />
            </div>
          )}
          <div className="flex shrink-0 items-center gap-2 border-t border-muted px-4 py-3">
            <span className="flex-1" />
            {copyButton}
          </div>
        </Card>
      </div>
    </section>
  );
}

function NoIntentBanner(props: { result: SolveResult }) {
  const note = noIntentNote(props.result);
  if (!note) return null;
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-2.5 border-b border-muted bg-accent px-4 py-2.5 text-sm text-foreground">
      <Icon name="lightbulb" size={16} className="shrink-0 text-accent-foreground" />
      <span className="min-w-0 flex-1">{note}</span>
    </div>
  );
}

function GapBanner(props: { result: SolveResult; onOpenDiscover: (scenarioId: string | null) => void }) {
  const note = gapNote(props.result);
  if (!note) return null;
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-2.5 border-b border-muted bg-accent px-4 py-2.5 text-sm text-foreground">
      <Icon name="lightbulb" size={16} className="shrink-0 text-accent-foreground" />
      <span className="min-w-0 flex-1">{note}</span>
      <Button variant="text" onClick={() => props.onOpenDiscover(props.result.suggestedScenarioId)}>去「发现」看看</Button>
    </div>
  );
}

function ResultNotes(props: {
  result: SolveResult;
  scenarios: ScenarioSummary[];
  onOpenDiscover: (scenarioId: string | null) => void;
}) {
  const cross = crossNote(props.result, props.scenarios);
  const pending = pendingNote(props.result);
  if (!cross && !pending) return null;
  return (
    <>
      {cross ? (
        <div className="flex shrink-0 flex-wrap items-center gap-2.5 border-t border-muted px-4 py-2.5 text-sm text-on-surface-variant">
          <Icon name="lightbulb" size={16} className="shrink-0 text-accent-foreground" />
          <span className="min-w-0 flex-1">{cross}</span>
          <Button variant="text" onClick={() => props.onOpenDiscover(props.result.suggestedScenarioId)}>去「发现」看看</Button>
        </div>
      ) : null}
      {pending ? <p className="m-0 shrink-0 border-t border-muted px-4 py-2.5 text-sm text-on-surface-variant">{pending}</p> : null}
    </>
  );
}

function ResultBody(props: {
  scenarios: ScenarioSummary[];
  phase: 'idle' | 'loading' | 'error' | 'ready';
  error: string | null;
  result: SolveResult | null;
  picked: readonly string[];
  onToggle: (id: string) => void;
  onRetry: () => void;
  onOpenDiscover: (scenarioId: string | null) => void;
}) {
  if (props.phase === 'loading') {
    return (
      <div className="flex h-full min-h-24 items-center justify-center" role="status" aria-label="正在找">
        <Spinner />
      </div>
    );
  }
  if (props.phase === 'error') {
    return (
      <div className="flex flex-col items-start gap-2 px-4 py-4">
        <p className="m-0 text-sm text-on-surface-variant" role="alert">{props.error ?? '操作没有完成，请稍后再试。'}</p>
        <Button variant="default" onClick={props.onRetry}>重试</Button>
      </div>
    );
  }
  if (!props.result) {
    return <p className="m-0 px-4 py-8 text-center text-sm text-on-surface-variant">输入想做的事，回车开始找。</p>;
  }
  if (props.result.hits.length === 0) {
    const pending = pendingNote(props.result);
    const description = pending
      ? `去「发现」按这个方向找找看。${pending}`
      : '去「发现」按这个方向找找看';
    return (
      <div className="flex h-full items-center">
        <EmptyState
          icon="search_off"
          title="你的收藏里没有直接对得上的"
          description={description}
          action={(
            <Button variant="default" onClick={() => props.onOpenDiscover(props.result?.suggestedScenarioId ?? null)}>去发现</Button>
          )}
        />
      </div>
    );
  }
  const picked = new Set(props.picked);
  return (
    <>
      {props.result.hits.map((hit) => (
        <SolveHitRow
          key={hit.id}
          hit={hit}
          intents={props.result?.intents ?? []}
          scenarios={props.scenarios}
          pressed={picked.has(hit.id)}
          onToggle={() => props.onToggle(hit.id)}
        />
      ))}
    </>
  );
}

function readSolveResult(value: SolveResult): SolveResult {
  if (!value || !Array.isArray(value.hits) || !Array.isArray(value.intents) || !Array.isArray(value.preselected)) {
    throw new Error('找方案的结果不完整');
  }
  return {
    ...value,
    extras: Array.isArray(value.extras) ? value.extras : [],
    fullMatchCount: Number.isFinite(value.fullMatchCount) ? value.fullMatchCount : 0,
    gapIntents: Array.isArray(value.gapIntents) ? value.gapIntents : [],
    suggestedScenarioId: value.suggestedScenarioId ?? null,
    suggestedScenarioCount: typeof value.suggestedScenarioCount === 'number' ? value.suggestedScenarioCount : null,
    pendingMatches: Number.isFinite(value.pendingMatches) ? value.pendingMatches : 0,
    hits: value.hits.filter((hit) => hit && typeof hit.id === 'string' && hit.id).map((hit) => ({
      ...hit,
      matched: Array.isArray(hit.matched) ? hit.matched : [],
      missing: Array.isArray(hit.missing) ? hit.missing : [],
      why: hit.why ?? '',
      fullName: hit.fullName ?? hit.id,
      htmlUrl: hit.htmlUrl ?? '',
      starsCount: Number.isFinite(hit.starsCount) ? hit.starsCount : 0,
      fullMatch: Boolean(hit.fullMatch),
    })),
  };
}

function readPack(output: PackOutput): PackOutput {
  if (!output || typeof output.content !== 'string') {
    throw new Error('参考包的结果不完整');
  }
  return {
    format: output.format,
    content: output.content,
    itemCount: Number.isFinite(output.itemCount) ? output.itemCount : 0,
    missing: Array.isArray(output.missing) ? output.missing.filter((item) => typeof item === 'string') : [],
    warnings: Array.isArray(output.warnings) ? output.warnings.filter((item) => typeof item === 'string') : [],
  };
}
