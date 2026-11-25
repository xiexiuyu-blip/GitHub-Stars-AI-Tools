import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button, Spinner, useToast } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import {
  approxRepos,
  AUTO_ORGANIZE_DEFINITION,
  AUTO_ORGANIZE_DETAIL,
  automationState,
  BUDGET_PRESETS,
  DAILY_ORGANIZE_DESCRIPTION,
  DAILY_ORGANIZE_DURING_SCAN,
  DAILY_ORGANIZE_TITLE,
  dailyBudgetConfirmLead,
  dailyBudgetMetric,
  formatMinute,
  formatTokens,
  FULL_SCAN_STARTED,
  FULL_SCAN_STOPPED,
  fullScanConfirmLines,
  fullScanPanel,
  organizeCountsLine,
  parseDailyBudget,
  readmeCheckText,
  recentUsageMetric,
  type AutomationStatus,
  type FullScanPanel,
} from '@/lib/automation-view';
import { getAiAutomationWithRecent } from '@/lib/api';
import { cn } from '@/lib/utils';
import { errorText, Metric, Section } from './shared';

/** 「整理现有收藏」：按 fullScanPanel 的 kind 画。确认面板不调用命令，只有「确认开始」才开始。 */
export function FullScanPanelView(props: {
  panel: FullScanPanel;
  confirming: boolean;
  startPrimary: boolean;
  starting: boolean;
  stopping: boolean;
  onAsk: () => void;
  onCancel: () => void;
  onStart: () => void;
  onStop: () => void;
}) {
  const { panel } = props;
  const confirmationRef = useRef<HTMLDivElement>(null);
  const showingConfirmation = props.confirming && panel.kind === 'ready';
  useEffect(() => {
    if (!showingConfirmation) return;
    const confirmation = confirmationRef.current;
    confirmation?.scrollIntoView({ block: 'nearest' });
    confirmation?.querySelector<HTMLButtonElement>('[data-confirm-full-scan]')?.focus({ preventScroll: true });
  }, [showingConfirmation]);

  return (
    <div className="mb-4 rounded-lg border-2 border-primary bg-card p-4">
      <div className="mb-3 flex items-start gap-3">
        <span className="hue-tile grid size-9 shrink-0 place-items-center rounded-[10px]" style={{ '--h': 148 } as CSSProperties}>
          <Icon name="autorenew" size={20} fill />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h4 className="m-0 text-base font-semibold text-foreground">{panel.title}</h4>
            {panel.kind === 'running' ? (
              <span className="hue-tile ml-auto inline-flex h-5 shrink-0 items-center rounded-full px-2 text-[11px] font-medium" style={{ '--h': panel.paused ? 4 : 148 } as CSSProperties}>
                {panel.paused ? '已暂停' : '进行中'}
              </span>
            ) : null}
          </div>
          {panel.description ? <p className="mt-1 mb-0 text-[13px] leading-[1.6] text-on-surface-variant">{panel.description}</p> : null}
        </div>
      </div>

      {panel.kind === 'loading' ? (
        <div className="flex items-center gap-2 text-sm text-on-surface-variant"><Spinner /> 读取中</div>
      ) : null}

      {panel.kind === 'unavailable' ? (
        <p className="m-0 text-sm leading-[1.7] text-on-surface-variant">{panel.reason}</p>
      ) : null}

      {panel.kind === 'nothing' ? (
        panel.last ? <p className="mt-2 mb-0 text-xs leading-[1.6] text-subtle">{panel.last}</p> : null
      ) : null}

      {panel.kind === 'ready' ? (
        <>
          <p className="m-0 text-sm leading-[1.7] text-foreground">{panel.body}</p>
          {panel.last ? <p className="mt-2 mb-0 text-xs leading-[1.6] text-subtle">{panel.last}</p> : null}
          {props.confirming ? (
            <div ref={confirmationRef} className="mt-3 rounded-lg border border-primary/40 bg-accent p-4 text-sm leading-[1.7] text-foreground">
              {fullScanConfirmLines(panel.pending, panel.estimate).map((line) => (
                <p key={line} className="mt-2 mb-0 first:mt-0">{line}</p>
              ))}
              <div className="mt-3 flex flex-wrap gap-2">
                <Button data-confirm-full-scan variant="primary" loading={props.starting} onClick={props.onStart}>确认开始</Button>
                <Button disabled={props.starting} onClick={props.onCancel}>取消</Button>
              </div>
            </div>
          ) : (
            <div className="mt-3">
              <Button variant={props.startPrimary ? 'primary' : 'default'} onClick={props.onAsk}>{panel.buttonLabel}</Button>
            </div>
          )}
        </>
      ) : null}

      {panel.kind === 'running' ? (
        <>
          <div className="h-2 overflow-hidden rounded-full bg-surface-container-low">
            <div
              className="h-full rounded-full bg-primary"
              role="progressbar"
              aria-valuenow={panel.percent}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label="现在整理进度"
              aria-valuetext={`已整理 ${panel.done} / ${panel.total} 个`}
              style={{ width: `${panel.percent}%` }}
            />
          </div>
          <p className="mt-2.5 mb-0 text-sm leading-[1.6] text-foreground tabular-nums">
            已整理 {Math.max(0, panel.done - panel.failed)} / {panel.total} 个{panel.failed > 0 ? `，失败 ${panel.failed} 个` : ''} · 已用约 {formatTokens(panel.tokensUsed)} token · 开始于 {formatMinute(panel.startedAt)}
          </p>
          <p className="mt-1 mb-0 text-xs leading-[1.6] text-subtle">
            最长到 {formatMinute(panel.until)} 自动结束；随时可以停止。
          </p>
          {panel.paused ? (
            <p className="mt-1.5 mb-0 text-[13px] leading-[1.6] text-error">
              现在是暂停状态{panel.pausedReason ? `（${panel.pausedReason}）` : ''}，点上面的「继续整理」才会接着做。
            </p>
          ) : null}
          <div className="mt-3">
            <Button loading={props.stopping} onClick={props.onStop}>停止</Button>
          </div>
        </>
      ) : null}
    </div>
  );
}

export function OrganizeSection({ onOpenAiSettings }: { onOpenAiSettings?: () => void }) {
  const toast = useToast();
  const [automation, setAutomation] = useState<AutomationStatus | null>(null);
  const [budgetInput, setBudgetInput] = useState('');
  const [budgetLoaded, setBudgetLoaded] = useState(false);
  const [pendingBudget, setPendingBudget] = useState<number | null>(null);
  const [pendingFullScan, setPendingFullScan] = useState(false);
  const [fullScanBusy, setFullScanBusy] = useState<'start' | 'stop' | null>(null);
  const [customOpen, setCustomOpen] = useState(false);
  const budgetTouched = useRef(false);
  const fullScanLock = useRef(false);

  function say(title: string) {
    toast.show({ title });
  }

  function applyAutomation(next: AutomationStatus) {
    setAutomation((previous) => ({ ...next, succeeded24h: previous?.succeeded24h ?? null }));
    budgetTouched.current = false;
    setBudgetInput(String(next.dailyBudgetTokens));
    setBudgetLoaded(true);
  }

  function refreshAutomation() {
    return getAiAutomationWithRecent().then((next) => {
      setAutomation(next);
      setBudgetLoaded(true);
      setBudgetInput((value) => (budgetTouched.current ? value : String(next.dailyBudgetTokens)));
    });
  }

  useEffect(() => {
    refreshAutomation().catch((reason: unknown) => say(errorText(reason)));
  }, []);

  useEffect(() => {
    if (fullScanPanel(automation).kind !== 'ready') setPendingFullScan(false);
  }, [automation]);

  const scanning = automation?.unlimited === true;
  useEffect(() => {
    if (!scanning) return undefined;
    const timer = window.setInterval(() => {
      void refreshAutomation().catch(() => undefined);
    }, 5000);
    return () => window.clearInterval(timer);
  }, [scanning]);

  function saveBudget(tokens: number) {
    setPendingBudget(null);
    void invoke<AutomationStatus>('set_ai_automation', { dailyBudgetTokens: tokens })
      .then((next) => {
        applyAutomation(next);
        say(next.dailyBudgetTokens === 0 ? '已关闭自动整理' : '已保存每天额度');
      })
      .catch((reason: unknown) => say(errorText(reason)));
  }

  function chooseBudget(tokens: number) {
    setPendingFullScan(false);
    const current = automation?.dailyBudgetTokens ?? 0;
    if (tokens === current) {
      setPendingBudget(null);
      return;
    }
    if (tokens > 0) {
      setPendingBudget(tokens);
      return;
    }
    saveBudget(0);
  }

  function requestSaveCustom() {
    const parsed = parseDailyBudget(budgetInput);
    if (typeof parsed === 'string') {
      say(parsed);
      return;
    }
    chooseBudget(parsed);
  }

  function askFullScan() {
    setPendingBudget(null);
    setPendingFullScan(true);
  }

  function startFullScan() {
    if (fullScanLock.current) return;
    fullScanLock.current = true;
    setFullScanBusy('start');
    void invoke<AutomationStatus>('start_full_scan', { confirmed: true })
      .then((next) => {
        applyAutomation(next);
        setPendingFullScan(false);
        say(FULL_SCAN_STARTED);
      })
      .catch((reason: unknown) => say(errorText(reason)))
      .finally(() => {
        fullScanLock.current = false;
        setFullScanBusy((current) => (current === 'start' ? null : current));
      });
  }

  function stopFullScan() {
    if (fullScanLock.current) return;
    fullScanLock.current = true;
    setFullScanBusy('stop');
    void invoke<AutomationStatus>('stop_full_scan')
      .then((next) => {
        applyAutomation(next);
        say(FULL_SCAN_STOPPED);
      })
      .catch((reason: unknown) => say(errorText(reason)))
      .finally(() => {
        fullScanLock.current = false;
        setFullScanBusy((current) => (current === 'stop' ? null : current));
      });
  }

  function pauseAutomation() {
    void invoke('pause_ai_queue')
      .then(refreshAutomation)
      .then(() => say('已暂停'))
      .catch((reason: unknown) => say(errorText(reason)));
  }

  function resumeAutomation() {
    void invoke('resume_ai_queue')
      .then(refreshAutomation)
      .then(() => say('已继续'))
      .catch((reason: unknown) => say(errorText(reason)));
  }

  const state = automationState(automation);
  const current = automation?.dailyBudgetTokens ?? 0;
  const scanPanel = fullScanPanel(automation);
  const countsLine = organizeCountsLine(automation);
  const readmeLine = readmeCheckText(automation?.lastReadmeCheck);
  const confirmingFullScan = pendingFullScan && scanPanel.kind === 'ready';

  return (
    <Section
      icon="auto_awesome"
      hue={222}
      title="自动整理"
      hint={AUTO_ORGANIZE_DEFINITION}
      actions={
        <span className="hue-tile inline-flex h-6 shrink-0 items-center gap-1.5 rounded-full px-2.5 text-xs font-medium" style={{ '--h': state.hue } as CSSProperties}>
          <span className="size-1.5 rounded-full" style={{ background: `hsl(${state.hue} 70% 45%)` }} />
          {state.label}
        </span>
      }
    >
      <details className="mb-3 text-xs text-on-surface-variant">
        <summary className="cursor-pointer hover:text-foreground">查看详细说明</summary>
        <p className="mt-1.5 mb-0 leading-[1.6]">{AUTO_ORGANIZE_DETAIL}</p>
      </details>

      {state.detail ? (
        <div className="mb-3 flex flex-wrap items-center gap-2">
          <p className={cn('m-0 flex-1 text-[13px] leading-[1.6]', automation?.paused ? 'text-error' : 'text-on-surface-variant')}>
            {state.detail}
          </p>
          {automation && !automation.providerConfigured && onOpenAiSettings ? (
            <Button size="sm" onClick={onOpenAiSettings}>去设置 AI</Button>
          ) : null}
        </div>
      ) : null}
      {countsLine ? <p className="mt-0 mb-3 text-[13px] leading-[1.6] text-on-surface-variant">{countsLine}</p> : null}

      {automation?.paused ? (
        <div className="mb-3">
          <Button onClick={resumeAutomation}>继续整理</Button>
        </div>
      ) : null}

      <FullScanPanelView
        panel={scanPanel}
        confirming={confirmingFullScan}
        startPrimary={pendingBudget === null}
        starting={fullScanBusy === 'start'}
        stopping={fullScanBusy === 'stop'}
        onAsk={askFullScan}
        onCancel={() => setPendingFullScan(false)}
        onStart={startFullScan}
        onStop={stopFullScan}
      />
      {readmeLine ? <p className="mt-0 mb-3 text-xs leading-[1.6] text-subtle">{readmeLine}</p> : null}

      <b className="mb-1 block text-[13px] font-medium text-foreground">{DAILY_ORGANIZE_TITLE}</b>
      <p className="mt-0 mb-2 text-xs leading-[1.6] text-on-surface-variant">{DAILY_ORGANIZE_DESCRIPTION}</p>
      {automation?.unlimited === true ? (
        <p className="mt-0 mb-2 text-xs leading-[1.6] text-on-surface-variant">{DAILY_ORGANIZE_DURING_SCAN}</p>
      ) : null}
      <div className="grid grid-cols-2 gap-2 min-[1000px]:grid-cols-4">
        {BUDGET_PRESETS.map((preset) => {
          const selected = preset.tokens === current;
          const pending = preset.tokens === pendingBudget;
          return (
            <button
              key={preset.tokens}
              type="button"
              disabled={!budgetLoaded}
              aria-pressed={selected}
              onClick={() => chooseBudget(preset.tokens)}
              className={cn(
                'rounded-lg border px-3.5 py-3 text-left transition-all disabled:opacity-60',
                selected ? 'border-primary bg-accent shadow-[inset_0_0_0_1px_var(--color-primary)]' : pending ? 'border-primary border-dashed bg-card' : 'border-border bg-card hover:border-input hover:shadow-sm',
              )}
            >
              <b className={cn('block text-sm font-semibold', selected ? 'text-accent-foreground' : 'text-foreground')}>{preset.title}</b>
              <span className="mt-0.5 block text-xs leading-[1.5] text-on-surface-variant">{preset.hint}</span>
            </button>
          );
        })}
      </div>

      {pendingBudget !== null ? (
        <div className="mt-3 rounded-lg border border-primary/40 bg-accent p-4 text-sm leading-[1.7] text-foreground">
          <p className="m-0">
            {dailyBudgetConfirmLead(pendingBudget)}（约 {formatTokens(pendingBudget)} token，具体花多少钱以你服务商的账单为准）。用到额度就停。随时可以暂停或关闭。
          </p>
          <div className="mt-3 flex gap-2">
            <Button variant="primary" onClick={() => saveBudget(pendingBudget)}>确认开启</Button>
            <Button onClick={() => setPendingBudget(null)}>取消</Button>
          </div>
        </div>
      ) : null}

      <div className="mt-2.5 flex flex-wrap items-center justify-between gap-2">
        <div>
          {customOpen ? (
            <div className="flex flex-wrap items-center gap-2">
              <label className="text-[13px] text-on-surface-variant" htmlFor="daily-budget">自定义每天额度（token）</label>
              <input
                id="daily-budget"
                className="h-8 w-36 rounded-md border border-input bg-card px-2.5 text-sm outline-none focus:border-primary"
                type="number"
                min={0}
                step={1000}
                inputMode="numeric"
                value={budgetInput}
                onChange={(event) => {
                  budgetTouched.current = true;
                  setBudgetInput(event.target.value);
                }}
              />
              <Button disabled={!budgetLoaded} onClick={requestSaveCustom}>保存</Button>
              <span className="text-xs text-subtle">约 {approxRepos(Number(budgetInput) || 0)} 个工具 / 天</span>
            </div>
          ) : (
            <Button variant="text" onClick={() => setCustomOpen(true)}>自定义额度</Button>
          )}
        </div>
        {automation && !automation.paused && automation.enabled ? (
          <Button variant="text" onClick={pauseAutomation}>暂停</Button>
        ) : null}
      </div>

      {automation ? (
        <div className="mt-4 grid grid-cols-3 gap-2.5 max-[699px]:grid-cols-1">
          <Metric {...recentUsageMetric(automation)} />
          <Metric {...dailyBudgetMetric(automation)} />
          <Metric
            label="等待整理"
            value={`${automation.queued} 个`}
            sub={`进行中 ${automation.running} · 失败 ${automation.failed}`}
            title={automation.model ? `当前模型：${automation.model}` : undefined}
          />
        </div>
      ) : (
        <div className="mt-4 flex items-center gap-2 text-sm text-on-surface-variant"><Spinner /> 读取中</div>
      )}
    </Section>
  );
}
