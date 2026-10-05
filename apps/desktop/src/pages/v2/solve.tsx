import { FormEvent, Suspense, lazy, useCallback, useEffect, useRef, useState } from 'react';
import type { LocalCapabilityInput, PackFormat, PackOutput } from '@/lib/api';
import { buildReferencePack, describeInvokeError, listScenarios, scanLocalCapabilities, solveRerankV2, solveV2 } from '@/lib/api';
import type { RerankFit, RerankResult, ScenarioSummary, SolveResult } from '@/lib/contracts';
import {
  applyRerank,
  promotedHits,
  BACK_TO_PICK_LABEL,
  buildPackRequest,
  COPY_FOR_CODEX_TOAST,
  crossNote,
  DIRECT_FROM_PICKS_LABEL,
  DIRECT_LABEL,
  emptyResultHint,
  formatOptions,
  gapNote,
  initialPicks,
  makeMain,
  missingToolsNote,
  noIntentNote,
  pickListNote,
  pickLocalCapabilities,
  orderedPicks,
  packEmptyNote,
  packPlaceholder,
  packSizeText,
  pendingNote,
  RERANK_TOGGLE_LABEL,
  rerankCandidateIds,
  rerankPicks,
  rerankStatusText,
  todayLocalDate,
  togglePick,
} from '@/lib/solve-view';
import { loadSolveDraft, saveSolveDraft, type SolveRerankPhase } from '@/lib/solve-draft';
import { useCopy } from '@/hooks/use-copy';
import { Button, Card, CardHeader, EmptyState, PageHeader, Spinner } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';
import { FormatPicker } from '@/components/solve/format-picker';
import { HitDrawer } from '@/components/solve/hit-drawer';
import { SolveHitRow } from '@/components/solve/solve-hit-row';

// README 渲染器比较大，只有用到交接单预览时才加载
const ReadmeRenderer = lazy(() => import('@/components/readme-renderer').then((module) => ({ default: module.ReadmeRenderer })));

const EXAMPLES = ['做小红书封面', '给公众号文章配图', '短视频加字幕', '做 PPT 和文档', '做电商详情页图片'];
const AI_STORAGE_KEY = 'fsl.solve.ai';

function readAiEnabled(): boolean {
  try {
    const raw = window.localStorage.getItem(AI_STORAGE_KEY);
    if (raw == null) return true;
    return raw !== '0' && raw !== 'false' && raw !== 'off';
  } catch {
    return true;
  }
}

function writeAiEnabled(on: boolean): void {
  try {
    window.localStorage.setItem(AI_STORAGE_KEY, on ? '1' : '0');
  } catch {
    // 写不进去就只在这一次里记住，下次再读默认开。
  }
}

function failedRerank(): RerankResult {
  return {
    status: 'failed',
    items: [],
    noneFit: false,
    note: null,
    message: 'AI 精选没成功，这次按本地排序',
    tokens: null,
  };
}

function readRerank(value: RerankResult | null | undefined): RerankResult {
  if (!value || !Array.isArray(value.items) || typeof value.status !== 'string') return failedRerank();
  return value;
}

function restoredRerank(draft: ReturnType<typeof loadSolveDraft>): { rerank: RerankResult | null; phase: SolveRerankPhase } {
  if (!draft || draft.rerankPhase === 'loading' || draft.rerankPhase !== 'done') {
    return { rerank: null, phase: 'idle' };
  }
  return { rerank: draft.rerank ?? null, phase: 'done' };
}

function isFit(value: string): value is RerankFit {
  return value === 'best' || value === 'good' || value === 'maybe' || value === 'poor';
}

const PREVIEW_CLASS = 'm-0 min-h-[180px] flex-1 overflow-auto bg-background px-4 py-3.5 font-mono text-xs leading-[1.7] break-words whitespace-pre-wrap text-foreground [overflow-wrap:anywhere] max-[899px]:max-h-[70vh] min-[900px]:max-h-none';

type PackState = {
  key: string;
  output: PackOutput | null;
  error: string | null;
};

export function SolvePage(props: {
  initialQuestion?: string;
  onOpenDiscover?: (scenarioId: string | null) => void;
  /** 去工具箱（开启 / 调整 AI 自动整理） */
  onOpenToolbox?: () => void;
}) {
  const copy = useCopy();
  const searchSeq = useRef(0);
  const rerankSeq = useRef(0);
  const openingQuery = props.initialQuestion?.trim() ?? '';
  // 切到别的页面再回来：接着上次的问题、结果和勾选，不要白做一遍
  const draft = openingQuery ? null : loadSolveDraft();
  const restored = restoredRerank(draft);
  // 用户自己改过勾选：AI 精选之后不再替他重勾。ref 给异步回调读最新值，state 给列表顶部那一行用（见 pickListNote）。
  // 从草稿恢复的，分不清是默认的还是改过的，当作改过（只说「已勾 N 个」，不会说错成「替你勾」）。
  const picksTouched = useRef(draft !== null);
  const [picksEdited, setPicksEdited] = useState(draft !== null);
  const aiEnabledRef = useRef(readAiEnabled());
  const rerankUsefulRef = useRef(restored.phase === 'done' && (restored.rerank?.status === 'ok' || restored.rerank?.status === 'cached'));
  const [query, setQuery] = useState(props.initialQuestion || draft?.query || '');
  const [activeQuery, setActiveQuery] = useState(draft?.activeQuery ?? '');
  const [phase, setPhase] = useState<'idle' | 'loading' | 'error' | 'ready'>(openingQuery ? 'loading' : draft ? 'ready' : 'idle');
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SolveResult | null>(draft?.result ?? null);
  const [picked, setPicked] = useState<string[]>(draft?.picked ?? []);
  const [direct, setDirect] = useState(draft?.direct ?? false);
  const [aiEnabled, setAiEnabled] = useState(aiEnabledRef.current);
  const [rerank, setRerank] = useState<RerankResult | null>(restored.rerank);
  const [rerankPhase, setRerankPhase] = useState<SolveRerankPhase>(restored.phase);
  const [scenarios, setScenarios] = useState<ScenarioSummary[]>([]);
  const [format, setFormat] = useState<PackFormat>(draft?.format ?? 'markdown');
  const [openedId, setOpenedId] = useState<string | null>(null);
  const [localCapabilities, setLocalCapabilities] = useState<LocalCapabilityInput[]>([]);
  const [showRaw, setShowRaw] = useState(false);
  const lastPack = useRef<{ mode: 'direct' | 'pick'; output: PackOutput } | null>(null);
  const [packState, setPackState] = useState<PackState | null>(null);
  const [packAttempt, setPackAttempt] = useState(0);
  const [copied, setCopied] = useState(false);

  const requestRerank = useCallback((ready: SolveResult, text: string, searchToken: number) => {
    if (!aiEnabledRef.current || ready.hits.length < 2) return;
    const token = ++rerankSeq.current;
    rerankUsefulRef.current = false;
    setRerank(null);
    setRerankPhase('loading');
    const queryText = ready.query.trim() ? ready.query : text;
    void solveRerankV2({
      query: queryText,
      candidateIds: rerankCandidateIds(ready),
    }).then((next) => {
      if (searchToken !== searchSeq.current || token !== rerankSeq.current) return;
      const parsed = readRerank(next);
      rerankUsefulRef.current = parsed.status === 'ok' || parsed.status === 'cached';
      // AI 从后备候选（本地 9–16 名）里点名的，拿出来放进列表
      const extras = promotedHits(ready, parsed);
      if (extras.length > 0) {
        setResult((current) => (current && current.query === ready.query ? { ...current, hits: [...current.hits, ...extras] } : current));
      }
      setRerank(parsed);
      setRerankPhase('done');
      if (!picksTouched.current) {
        const picks = rerankPicks(parsed, [...ready.hits, ...extras]);
        if (picks !== null) setPicked(picks);
      }
    }).catch(() => {
      if (searchToken !== searchSeq.current || token !== rerankSeq.current) return;
      rerankUsefulRef.current = false;
      setRerank(failedRerank());
      setRerankPhase('done');
    });
  }, []);

  const runSearch = useCallback((raw: string) => {
    const text = raw.trim();
    if (!text) return;
    const seq = ++searchSeq.current;
    rerankSeq.current += 1;
    picksTouched.current = false;
    setPicksEdited(false);
    rerankUsefulRef.current = false;
    setActiveQuery(text);
    setPhase('loading');
    setError(null);
    setResult(null);
    setDirect(false);
    setPicked([]);
    setRerank(null);
    setRerankPhase('idle');
    void solveV2(text).then((next) => {
      if (seq !== searchSeq.current) return;
      const ready = readSolveResult(next);
      setResult(ready);
      setPicked(initialPicks(ready));
      setPhase('ready');
      requestRerank(ready, text, seq);
    }).catch((reason) => {
      if (seq !== searchSeq.current) return;
      setError(describeInvokeError(reason));
      setPhase('error');
    });
  }, [requestRerank]);

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

  // 本机已经装了哪些 Skill / MCP（只读名字）：交接单里要标「本机已装」，Codex 才不会重复装
  useEffect(() => {
    let cancelled = false;
    void scanLocalCapabilities().then((items) => {
      if (!cancelled) setLocalCapabilities(items);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (result) saveSolveDraft({ query, activeQuery, result, picked, format, direct, rerank, rerankPhase });
  }, [query, activeQuery, result, picked, format, direct, rerank, rerankPhase]);

  const selectedHits = result ? orderedPicks(result.hits, picked) : [];
  // direct 写进 key，免得「一个都没勾」和「直接交给 Codex」共用同一份预览
  const packKey = `${format}\u0001${direct ? 'direct' : 'pick'}\u0001${selectedHits.map((hit) => hit.id).join('\u0001')}`;
  const packView = packState && packState.key === packKey ? packState : null;
  const packMode = direct ? 'direct' : 'pick';
  if (packView?.output) lastPack.current = { mode: packMode, output: packView.output };
  const awaitingPack = direct || selectedHits.length > 0;
  const loadingPack = awaitingPack && !packView?.output && !packView?.error;
  // 重新生成时先留着同一模式下的上一份，别把预览整块清空；换模式不拿另一份来充数
  const pack = packView?.output ?? (awaitingPack && lastPack.current?.mode === packMode ? lastPack.current.output : null);
  const hasResult = result !== null;
  const rerankStatus = rerankStatusText({ phase: rerankPhase, result: rerank, enabled: aiEnabled });
  const showNoneFit = Boolean(rerank && (rerank.status === 'ok' || rerank.status === 'cached') && rerank.noneFit);
  const canCopy = direct ? Boolean(pack?.content) : selectedHits.length > 0 && Boolean(pack?.content);

  useEffect(() => {
    if (!result) return;
    const hits = orderedPicks(result.hits, picked);
    if (!direct && hits.length === 0) return;
    const key = packKey;
    const queryText = result.query.trim() || activeQuery;
    const request = direct
      ? buildPackRequest(queryText, [], pickLocalCapabilities(localCapabilities, [], result.intents), true)
      : buildPackRequest(queryText, hits, pickLocalCapabilities(localCapabilities, hits, result.intents));
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
  }, [packKey, result, picked, format, packAttempt, activeQuery, localCapabilities, direct]);

  // 复制成功后按钮上显示一会儿「已复制」
  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1600);
    return () => window.clearTimeout(timer);
  }, [copied]);

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    runSearch(query);
  }

  function openDiscover(scenarioId: string | null) {
    props.onOpenDiscover?.(scenarioId);
  }

  async function copyToCodex() {
    if (!pack?.content || (!direct && selectedHits.length === 0)) return;
    const detail = direct
      ? '直接交给 Codex'
      : `${Number.isFinite(pack.itemCount) ? pack.itemCount : selectedHits.length} 个工具`;
    if (await copy(pack.content, COPY_FOR_CODEX_TOAST, detail)) setCopied(true);
  }

  function updatePicked(update: (current: string[]) => string[]) {
    picksTouched.current = true;
    setPicksEdited(true);
    setDirect(false);
    setPicked(update);
  }

  function toggleAi() {
    const next = !aiEnabledRef.current;
    aiEnabledRef.current = next;
    setAiEnabled(next);
    writeAiEnabled(next);
    if (!next) {
      rerankSeq.current += 1;
      rerankUsefulRef.current = false;
      setRerank(null);
      setRerankPhase('idle');
      return;
    }
    if (!result || result.hits.length < 2 || rerankUsefulRef.current) return;
    requestRerank(result, result.query.trim() || activeQuery || query, searchSeq.current);
  }

  function retryPack() {
    setPackState(null);
    setPackAttempt((current) => current + 1);
  }

  // 没有结果时「找」是唯一的主按钮；有结果后改成次要，主按钮只留给「复制交接单给 Codex」。
  const findButton = hasResult ? (
    <Button type="submit" variant="default" className="h-12 rounded-xl px-6 text-base" disabled={!query.trim()}>找</Button>
  ) : (
    <Button type="submit" variant="primary" className="h-12 rounded-xl px-6 text-base" disabled={!query.trim()}>找</Button>
  );

  // N 数的是界面上此刻真勾着的（和右边「已选 N 个」同一个数）：AI 精选改勾之后、用户自己改过之后都跟着变。
  const pickNote = result ? pickListNote(result.hits, picked, direct, picksEdited) : null;
  // 一个都没勾、又不是用户自己取消的（后端没默认勾，或 AI 精选说都不合适）：右边说一句为什么没替你勾
  const emptyNote = packEmptyNote(result, picksEdited);
  const missingNote = pack ? missingToolsNote(pack.missing.length) : null;

  const copyButton = (
    <Button variant={hasResult ? 'primary' : 'default'} disabled={!canCopy} onClick={() => void copyToCodex()}>
      <Icon name={copied ? 'check' : 'content_copy'} size={16} />
      {copied ? '已复制' : '复制交接单给 Codex'}
    </Button>
  );

  return (
    <section className="flex h-full min-h-0 flex-col gap-3 overflow-auto px-5 pt-4 pb-5 max-[899px]:px-4 max-[899px]:pt-3 max-[899px]:pb-4">
      <PageHeader
        title="找方案"
        subtitle="说一句你想做的事，从你收藏的里面挑出能用的，再整理成给 Codex 的交接单。"
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
              className={cn(
                'inline-flex h-8 items-center rounded-full border px-3.5 text-[13px] whitespace-nowrap',
                activeQuery === example
                  ? 'border-primary bg-accent text-accent-foreground'
                  : 'border-input bg-card text-on-surface-variant hover:bg-surface-container-low hover:text-foreground',
              )}
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
      <div className="grid min-h-[420px] flex-1 grid-cols-1 grid-rows-[minmax(240px,1fr)_minmax(460px,1fr)] gap-3 min-[900px]:grid-cols-[minmax(0,1fr)_minmax(340px,460px)] min-[900px]:grid-rows-[minmax(0,1fr)]">
        <Card className="flex min-h-0 flex-col overflow-hidden">
          <CardHeader
            title="你收藏里最相关的"
            count={result ? result.hits.length : undefined}
            actions={result && result.hits.length > 0 ? (
              <span className="text-xs text-on-surface-variant">勾选的会写进交接单；点一行看详情</span>
            ) : undefined}
          />
          {pickNote ? (
            <p className="m-0 shrink-0 border-b border-muted px-4 py-2 text-xs leading-5 text-on-surface-variant">{pickNote}</p>
          ) : null}
          <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b border-muted px-4 py-1.5">
            {rerankStatus ? <span className="text-xs text-on-surface-variant">{rerankStatus}</span> : null}
            <Button variant="text" size="sm" aria-pressed={aiEnabled} onClick={toggleAi}>{RERANK_TOGGLE_LABEL(aiEnabled)}</Button>
          </div>
          {result ? <GapBanner result={result} onOpenDiscover={openDiscover} /> : null}
          {result ? <NoIntentBanner result={result} /> : null}
          {showNoneFit ? (
            <div className="flex shrink-0 flex-wrap items-center gap-2.5 border-b border-muted px-4 py-2.5 text-sm text-foreground">
              {rerank?.note ? <span className="min-w-0 flex-1">{rerank.note}</span> : <span className="flex-1" />}
              <Button variant="text" onClick={() => setDirect(true)}>{DIRECT_LABEL}</Button>
            </div>
          ) : null}
          <div className="min-h-0 flex-1 overflow-auto">
            <ResultBody
              scenarios={scenarios}
              phase={phase}
              error={error}
              result={result}
              rerank={rerank}
              picked={picked}
              onToggle={(id) => updatePicked((current) => togglePick(current, id))}
              onOpen={(id) => setOpenedId(id)}
              onMakeMain={(id) => updatePicked((current) => makeMain(current, id))}
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
            <h3 className="m-0 text-base font-semibold whitespace-nowrap text-foreground">交接单</h3>
            <span className="text-xs font-medium text-on-surface-variant">
              {direct ? '不选工具 · 直接交给 Codex' : `已选 ${selectedHits.length} 个`}
              {(direct || selectedHits.length > 0) && pack?.content ? ` · ${packSizeText(pack.content.length)}` : ''}
            </span>
            <div className="min-w-2 flex-1" />
            {format !== 'json' && (direct || selectedHits.length > 0) ? (
              <Button variant="text" size="sm" onClick={() => setShowRaw((current) => !current)}>
                {showRaw ? '看排版' : '看原文'}
              </Button>
            ) : null}
            {direct ? (
              <Button variant="text" size="sm" onClick={() => setDirect(false)}>{BACK_TO_PICK_LABEL}</Button>
            ) : null}
          </div>
          <FormatPicker options={formatOptions} value={format} onChange={setFormat} />
          {loadingPack && pack ? <div className="h-0.5 shrink-0 animate-pulse bg-primary/60" role="status" aria-label="正在更新交接单" /> : null}
          {pack && pack.warnings.length > 0 ? (
            <div className="shrink-0 space-y-1 px-4 pt-2.5">
              {pack.warnings.map((warning, index) => (
                <p key={`${index}-${warning}`} className="m-0 text-xs text-on-surface-variant">{warning}</p>
              ))}
            </div>
          ) : null}
          {missingNote ? (
            <p className="m-0 shrink-0 px-4 pt-2.5 text-xs text-on-surface-variant">{missingNote}</p>
          ) : null}
          {!direct && selectedHits.length === 0 ? (
            result ? (
              <div className="flex min-h-[180px] flex-1 flex-col overflow-auto bg-background px-4 py-3.5 max-[899px]:max-h-[70vh] min-[900px]:max-h-none">
                <pre className="m-0 font-mono text-xs leading-[1.7] break-words whitespace-pre-wrap text-foreground [overflow-wrap:anywhere]">{packPlaceholder(format)}</pre>
                {emptyNote ? <p className="m-0 pt-2 text-xs leading-[1.7] text-on-surface-variant">{emptyNote}</p> : null}
                <div className="pt-2">
                  <Button variant="text" onClick={() => setDirect(true)}>{DIRECT_LABEL}</Button>
                </div>
              </div>
            ) : (
              <pre className={PREVIEW_CLASS}>{packPlaceholder(format)}</pre>
            )
          ) : packView?.error ? (
            <div className="flex flex-1 flex-col items-start justify-center gap-2 px-4 py-6">
              <p className="m-0 text-sm text-on-surface-variant">{packView.error}</p>
              <Button variant="text" onClick={retryPack}>重试</Button>
            </div>
          ) : pack ? (
            <PackPreview content={pack.content} format={format} raw={showRaw} />
          ) : (
            <div className="flex flex-1 items-center justify-center" role="status" aria-label="正在整理交接单">
              <Spinner />
            </div>
          )}
          <div className="flex shrink-0 items-center gap-2 border-t border-muted px-4 py-3">
            {!direct && selectedHits.length > 0 ? (
              <Button variant="text" size="sm" onClick={() => setDirect(true)}>{DIRECT_FROM_PICKS_LABEL}</Button>
            ) : null}
            <span className="flex-1" />
            {copyButton}
          </div>
        </Card>
      </div>
      {openedId && result ? (() => {
        const hit = result.hits.find((item) => item.id === openedId);
        if (!hit) return null;
        return (
          <HitDrawer
            hit={hit}
            problem={result.query}
            scenarios={scenarios}
            picked={picked.includes(hit.id)}
            isMain={picked[0] === hit.id}
            onTogglePick={() => updatePicked((current) => togglePick(current, hit.id))}
            onMakeMain={() => updatePicked((current) => makeMain(current, hit.id))}
            onClose={() => setOpenedId(null)}
            onOpenToolbox={props.onOpenToolbox}
          />
        );
      })() : null}
    </section>
  );
}

/** 交接单预览：Markdown / AGENTS.md 排成正常文档，JSON 和「看原文」显示等宽原文（复制的永远是原文）。 */
function PackPreview(props: { content: string; format: PackFormat; raw: boolean }) {
  if (props.format === 'json' || props.raw) {
    return <pre className={PREVIEW_CLASS}>{props.content}</pre>;
  }
  return (
    <div className="min-h-[180px] flex-1 overflow-auto bg-background px-4 py-3.5 max-[899px]:max-h-[70vh] min-[900px]:max-h-none">
      <Suspense fallback={<div className="flex justify-center py-6"><Spinner /></div>}>
        <ReadmeRenderer
          markdown={props.content}
          repositoryFullName=""
          sourcePath="交接单.md"
          className="readme-rendered min-w-0 text-sm text-foreground"
          singleTilde={false}
        />
      </Suspense>
    </div>
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
  rerank: RerankResult | null;
  picked: readonly string[];
  onToggle: (id: string) => void;
  onOpen: (id: string) => void;
  onMakeMain: (id: string) => void;
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
    return (
      <div className="flex h-full items-center">
        <EmptyState
          icon="search_off"
          title="你的收藏里没有直接对得上的"
          description={emptyResultHint(props.result)}
          action={(
            <Button variant="default" onClick={() => props.onOpenDiscover(props.result?.suggestedScenarioId ?? null)}>去发现</Button>
          )}
        />
      </div>
    );
  }
  const picked = new Set(props.picked);
  const shown = applyRerank(props.result.hits, props.rerank);
  const aiById = new Map<string, { fit: RerankFit; reason: string }>();
  if (props.rerank && (props.rerank.status === 'ok' || props.rerank.status === 'cached')) {
    for (const item of props.rerank.items) {
      if (!isFit(item.fit)) continue;
      aiById.set(item.repoId, { fit: item.fit, reason: item.reason });
    }
  }
  return (
    <>
      {shown.map((hit) => (
        <SolveHitRow
          key={hit.id}
          hit={hit}
          intents={props.result?.intents ?? []}
          scenarios={props.scenarios}
          pressed={picked.has(hit.id)}
          isMain={props.picked[0] === hit.id}
          ai={aiById.get(hit.id)}
          onToggle={() => props.onToggle(hit.id)}
          onOpen={() => props.onOpen(hit.id)}
          onMakeMain={() => props.onMakeMain(hit.id)}
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
    hits: readHits(value.hits),
    // 旧后端没有这一项：当作没有后备候选
    moreHits: readHits(Array.isArray(value.moreHits) ? value.moreHits : []),
  };
}

function readHits(hits: SolveResult['hits']): SolveResult['hits'] {
  return hits.filter((hit) => hit && typeof hit.id === 'string' && hit.id).map((hit) => ({
    ...hit,
    matched: Array.isArray(hit.matched) ? hit.matched : [],
    missing: Array.isArray(hit.missing) ? hit.missing : [],
    why: hit.why ?? '',
    fullName: hit.fullName ?? hit.id,
    htmlUrl: hit.htmlUrl ?? '',
    starsCount: Number.isFinite(hit.starsCount) ? hit.starsCount : 0,
    fullMatch: Boolean(hit.fullMatch),
  }));
}

function readPack(output: PackOutput): PackOutput {
  if (!output || typeof output.content !== 'string') {
    throw new Error('交接单的结果不完整');
  }
  return {
    format: output.format,
    content: output.content,
    itemCount: Number.isFinite(output.itemCount) ? output.itemCount : 0,
    missing: Array.isArray(output.missing) ? output.missing.filter((item) => typeof item === 'string') : [],
    warnings: Array.isArray(output.warnings) ? output.warnings.filter((item) => typeof item === 'string') : [],
  };
}
