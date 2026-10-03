import { useEffect, useRef, useState } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { relaunch } from '@tauri-apps/plugin-process';
import { Button, Card, PageHeader, Spinner, useToast } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import {
  approxRepos,
  automationState,
  BUDGET_PRESETS,
  formatMinute,
  formatSize,
  formatTokens,
  parseDailyBudget,
  type AutomationStatus,
} from '@/lib/automation-view';
import { cn } from '@/lib/utils';

type Job = { id: string; jobType: string; status: string; errorKind: string | null };
type ApiStatus = { enabled: boolean; host: string; port: number; listening: boolean; hasToken: boolean };
type BackupKind = 'auto' | 'manual' | 'preRestore' | 'preMigrate';
type BackupInfo = {
  fileName: string;
  kind: BackupKind;
  createdAt: string;
  bytes: number;
  repositoryCount: number | null;
  schemaVersion: string | null;
};
type RestoreReport = {
  ok: boolean;
  fileName: string;
  backupCreatedAt: string | null;
  preRestoreFileName: string | null;
  error: string | null;
};
type BackupStatus = {
  lastAutoAt: string | null;
  nextAutoAt: string;
  lastRestore: RestoreReport | null;
};
const KIND_LABEL: Record<BackupKind, string> = {
  auto: '自动',
  manual: '手动',
  preRestore: '恢复前',
  preMigrate: '升级前',
};

function errorText(reason: unknown): string {
  if (typeof reason === 'string' && reason.trim()) return reason;
  if (reason instanceof Error && reason.message.trim()) return reason.message;
  return String(reason);
}

function restoreBanner(report: RestoreReport): string {
  if (report.ok) {
    const when = report.backupCreatedAt ? formatMinute(report.backupCreatedAt) : report.fileName;
    return `已恢复到 ${when} 的备份`;
  }
  return `恢复失败：${report.error ?? '未知原因'}（资料库没有被改动）`;
}

function Section(props: { icon: string; hue: number; title: string; hint?: string; actions?: ReactNode; children: ReactNode }) {
  return (
    <Card className="p-5">
      <div className="mb-3.5 flex items-start gap-3">
        <span className="hue-tile grid size-9 shrink-0 place-items-center rounded-[10px]" style={{ '--h': props.hue } as CSSProperties}>
          <Icon name={props.icon} size={20} fill />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="m-0 text-base font-semibold text-foreground">{props.title}</h2>
          {props.hint ? <p className="mt-0.5 mb-0 text-[13px] leading-[1.6] text-on-surface-variant">{props.hint}</p> : null}
        </div>
        {props.actions}
      </div>
      {props.children}
    </Card>
  );
}

/** 一行令牌：说明它能做什么，一个「复制」、一个要点两下才生效的「重置」（重置后旧令牌立刻失效）。 */
function TokenRow(props: { title: string; hint: string; onCopy: () => void; onReset: () => void }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return undefined;
    const timer = window.setTimeout(() => setArmed(false), 4000);
    return () => window.clearTimeout(timer);
  }, [armed]);
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 rounded-lg border border-border px-3.5 py-2.5">
      <div className="min-w-[220px] flex-1">
        <b className="block text-sm font-medium text-foreground">{props.title}</b>
        <span className="text-xs leading-[1.6] text-on-surface-variant">{props.hint}</span>
      </div>
      <div className="flex items-center gap-2">
        <Button size="sm" onClick={props.onCopy}>复制令牌</Button>
        <Button
          size="sm"
          variant="text"
          onClick={() => {
            if (!armed) {
              setArmed(true);
              return;
            }
            setArmed(false);
            props.onReset();
          }}
        >
          {armed ? '再点一次确认重置' : '重置'}
        </Button>
      </div>
    </div>
  );
}

function Notice(props: { tone?: 'info' | 'warn'; children: ReactNode }) {
  return (
    <div className={cn('mb-3 flex flex-wrap items-center gap-2 rounded-lg px-3.5 py-2.5 text-sm', props.tone === 'warn' ? 'bg-error-container text-on-error-container' : 'bg-accent text-foreground')}>
      {props.children}
    </div>
  );
}

export function ToolboxPage() {
  const toast = useToast();
  const [health, setHealth] = useState('');
  const [jobs, setJobs] = useState<Job[]>([]);
  const [api, setApi] = useState<ApiStatus | null>(null);
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [confirmName, setConfirmName] = useState<string | null>(null);
  const [restartReady, setRestartReady] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  const [automation, setAutomation] = useState<AutomationStatus | null>(null);
  const [budgetInput, setBudgetInput] = useState('');
  const [budgetLoaded, setBudgetLoaded] = useState(false);
  const [pendingBudget, setPendingBudget] = useState<number | null>(null);
  const [customOpen, setCustomOpen] = useState(false);
  const [enqueueNote, setEnqueueNote] = useState<string | null>(null);
  const budgetTouched = useRef(false);

  function say(title: string) {
    toast.show({ title });
  }

  function load() {
    invoke<string>('get_health_summary').then(setHealth).catch((reason: unknown) => setHealth(String(reason)));
    invoke<Job[]>('list_ai_jobs').then(setJobs).catch(() => setJobs([]));
    invoke<ApiStatus>('get_local_api_status').then(setApi).catch((reason: unknown) => say(errorText(reason)));
    invoke<BackupStatus>('get_backup_status').then(setStatus).catch((reason: unknown) => say(errorText(reason)));
    invoke<BackupInfo[]>('list_backups').then(setBackups).catch((reason: unknown) => say(errorText(reason)));
    invoke<AutomationStatus>('get_ai_automation')
      .then((next) => {
        setAutomation(next);
        setBudgetInput((current) => (budgetTouched.current ? current : String(next.dailyBudgetTokens)));
        setBudgetLoaded(true);
      })
      .catch((reason: unknown) => say(errorText(reason)));
  }

  function applyAutomation(next: AutomationStatus) {
    setAutomation(next);
    budgetTouched.current = false;
    setBudgetInput(String(next.dailyBudgetTokens));
    setBudgetLoaded(true);
  }

  function refreshAutomation() {
    return invoke<AutomationStatus>('get_ai_automation').then((next) => {
      setAutomation(next);
      setBudgetLoaded(true);
    });
  }

  function saveBudget(tokens: number) {
    setPendingBudget(null);
    void invoke<AutomationStatus>('set_ai_automation', { dailyBudgetTokens: tokens })
      .then((next) => {
        applyAutomation(next);
        say(next.dailyBudgetTokens === 0 ? '已关闭自动整理' : '已保存每天额度');
      })
      .catch((reason: unknown) => say(errorText(reason)));
  }

  /** 选一个额度：大于 0 且和现在不同，先弹确认；0 直接关闭。 */
  function chooseBudget(tokens: number) {
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

  function enqueueCards(scope: 'missing' | 'outdated') {
    void invoke<number>('enqueue_cards_now', { scope })
      .then((count) => {
        setEnqueueNote(count > 0 ? `已排队 ${count} 个。是否开始整理，取决于上面的每天额度。` : '没有需要排队的仓库了。');
        return refreshAutomation();
      })
      .catch((reason: unknown) => say(errorText(reason)));
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

  useEffect(load, []);

  function backupNow() {
    setBackingUp(true);
    void invoke<BackupInfo>('create_backup_now')
      .then(() => {
        say('已备份');
        return invoke<BackupInfo[]>('list_backups').then(setBackups);
      })
      .then(() => invoke<BackupStatus>('get_backup_status').then(setStatus))
      .catch((reason: unknown) => say(errorText(reason)))
      .finally(() => setBackingUp(false));
  }

  function confirmRestore() {
    if (!confirmName) return;
    const fileName = confirmName;
    void invoke('request_restore_backup', { fileName })
      .then(() => {
        setConfirmName(null);
        setRestartReady(true);
        say('已记录，重启后生效');
      })
      .catch((reason: unknown) => say(errorText(reason)));
  }

  const state = automationState(automation);
  const current = automation?.dailyBudgetTokens ?? 0;

  return (
    <section className="flex h-full flex-col gap-4 overflow-auto px-5 pt-4 pb-6 max-[899px]:px-4">
      <PageHeader title="设置 · 工具箱" subtitle="AI 自动整理、备份和给其他工具用的接口都在这里。" />
      {status?.lastRestore ? <Notice>{restoreBanner(status.lastRestore)}</Notice> : null}
      {restartReady ? (
        <Notice>
          <span>已记录，重启应用后恢复生效。</span>
          <Button onClick={() => void relaunch().catch((reason: unknown) => say(errorText(reason)))}>立即重启</Button>
          <Button variant="text" onClick={() => setRestartReady(false)}>稍后</Button>
        </Notice>
      ) : null}

      {/* ───────── AI 自动整理 ───────── */}
      <Section
        icon="auto_awesome"
        hue={222}
        title="AI 自动整理"
        hint="让 AI 帮你给收藏写中文说明（它能做什么、适合解决什么、怎么试），整理好的会出现在「今日」和「资料库」里。用的是你自己配置的 AI 服务，按服务商的价格计费。"
        actions={
          <span className="hue-tile inline-flex h-6 shrink-0 items-center gap-1.5 rounded-full px-2.5 text-xs font-medium" style={{ '--h': state.hue } as CSSProperties}>
            <span className="size-1.5 rounded-full" style={{ background: `hsl(${state.hue} 70% 45%)` }} />
            {state.label}
          </span>
        }
      >
        {state.detail ? <p className={cn('mt-0 mb-3 text-[13px] leading-[1.6]', automation?.paused ? 'text-error' : 'text-on-surface-variant')}>{state.detail}</p> : null}

        {automation?.paused ? (
          <div className="mb-3">
            <Button onClick={resumeAutomation}>继续整理</Button>
          </div>
        ) : null}

        <div className="mb-1.5 text-[13px] font-medium text-foreground">每天最多整理多少</div>
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
              这会让应用在后台调用你配置的 AI 服务整理收藏，<b>按服务商的价格计费</b>。每天最多使用 <b>{formatTokens(pendingBudget)} token</b>，大约整理 {approxRepos(pendingBudget)} 个仓库；用完当天就停，不会超过。随时可以暂停或关闭。
            </p>
            <div className="mt-3 flex gap-2">
              <Button variant="primary" onClick={() => saveBudget(pendingBudget)}>确认开启</Button>
              <Button onClick={() => setPendingBudget(null)}>取消</Button>
            </div>
          </div>
        ) : null}

        <div className="mt-2.5">
          {customOpen ? (
            <div className="flex flex-wrap items-center gap-2">
              <label className="text-[13px] text-on-surface-variant" htmlFor="daily-budget">自定义每天 token 数</label>
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
              <span className="text-xs text-subtle">约 {approxRepos(Number(budgetInput) || 0)} 个仓库 / 天</span>
            </div>
          ) : (
            <Button variant="text" onClick={() => setCustomOpen(true)}>自定义额度</Button>
          )}
        </div>

        {automation ? (
          <div className="mt-4 grid grid-cols-3 gap-2.5 max-[699px]:grid-cols-1">
            <Metric label="今天已用" value={`${formatTokens(automation.tokens24h)} token`} sub={`约整理了 ${approxRepos(automation.tokens24h)} 个`} />
            <Metric label="今天还剩" value={current > 0 ? `${formatTokens(automation.budgetRemaining)} token` : '—'} sub={current > 0 ? `约还能整理 ${approxRepos(automation.budgetRemaining)} 个` : '没有开启额度'} />
            <Metric label="排队中" value={`${automation.queued} 个`} sub={`进行中 ${automation.running} · 失败 ${automation.failed}${automation.model ? ` · 模型 ${automation.model}` : ''}`} />
          </div>
        ) : (
          <div className="mt-4 flex items-center gap-2 text-sm text-on-surface-variant"><Spinner /> 读取中</div>
        )}

        <div className="mt-4 flex flex-wrap items-center gap-2 border-t border-border pt-3.5">
          <Button onClick={() => enqueueCards('missing')}>给还没有说明的仓库排队</Button>
          <Button onClick={() => enqueueCards('outdated')}>把旧版说明排队重做</Button>
          {automation && !automation.paused && automation.enabled ? <Button variant="text" onClick={pauseAutomation}>暂停</Button> : null}
          {enqueueNote ? <span className="text-xs text-on-surface-variant">{enqueueNote}</span> : null}
        </div>
      </Section>

      {/* ───────── 备份 ───────── */}
      <Section
        icon="backup"
        hue={148}
        title="资料库备份"
        hint={`每天自动备份一次。上次 ${status ? (status.lastAutoAt ? formatMinute(status.lastAutoAt) : '还没有') : '读取中'}，下次 ${status ? formatMinute(status.nextAutoAt) : '读取中'}。`}
        actions={<Button disabled={backingUp} loading={backingUp} onClick={backupNow}>现在备份</Button>}
      >
        {confirmName ? (
          <Notice tone="warn">
            <span className="min-w-0 flex-1">恢复会把当前资料库换成这一份。当前的库会先另存一份「恢复前」备份。需要重启应用才会生效。继续吗？</span>
            <Button onClick={confirmRestore}>继续恢复</Button>
            <Button variant="text" onClick={() => setConfirmName(null)}>取消</Button>
          </Notice>
        ) : null}
        <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
          {backups.slice(0, 6).map((item) => (
            <li key={item.fileName} className="flex items-center gap-3 rounded-lg border border-border px-3 py-2 text-sm">
              <span className="min-w-0 flex-1 truncate text-foreground">
                {formatMinute(item.createdAt)}
                <span className="ml-2 text-xs text-on-surface-variant">{KIND_LABEL[item.kind]} · {formatSize(item.bytes)} · {item.repositoryCount === null ? '仓库数未知' : `${item.repositoryCount} 个仓库`}</span>
              </span>
              <Button size="sm" onClick={() => setConfirmName(item.fileName)}>恢复到这一份…</Button>
            </li>
          ))}
        </ul>
        {backups.length > 6 ? <p className="mt-2 mb-0 text-xs text-subtle">只显示最近 6 份，更早的在备份文件夹里。</p> : null}
      </Section>

      {/* ───────── 给其他工具用 ───────── */}
      <Section
        icon="hub"
        hue={268}
        title="给其他工具用的接口"
        hint="开启后，微信工作台这类工具可以通过本机把链接送进「收集」。只在这台电脑上可用，需要令牌，默认关闭。"
        actions={
          <Button
            onClick={() => void invoke<ApiStatus>('set_local_api_enabled', { enabled: !api?.enabled }).then((next) => setApi(next)).catch((reason: unknown) => say(errorText(reason)))}
          >
            {api?.enabled ? '关闭' : '开启'}
          </Button>
        }
      >
        <p className="m-0 text-sm text-on-surface-variant">
          现在：{api?.enabled ? '已开启' : '关闭'}{api?.listening ? '，正在监听' : ''}（只监听 127.0.0.1:{api?.port ?? 39217}）
        </p>
        {api?.enabled ? (
          <div className="mt-3 flex flex-col gap-2.5">
            <TokenRow
              title="微信工作台用的令牌"
              hint="只能往「收集」里放链接，读不到你的资料库。复制后粘进微信工作台的设置里就行。"
              onCopy={() => void invoke<string>('get_local_api_capture_token').then((token) => navigator.clipboard.writeText(token)).then(() => say('收集专用令牌已复制')).catch((reason: unknown) => say(errorText(reason)))}
              onReset={() => void invoke<string>('rotate_local_api_capture_token').then(() => say('收集专用令牌已重置，旧的立即失效，要把新的粘回微信工作台')).catch((reason: unknown) => say(errorText(reason)))}
            />
            <TokenRow
              title="完整令牌（高级）"
              hint="能读你的资料库，只给你信得过的本机工具。调用时加请求头 Authorization: Bearer 令牌。"
              onCopy={() => void invoke<string>('get_local_api_token').then((token) => navigator.clipboard.writeText(token)).then(() => say('完整令牌已复制')).catch((reason: unknown) => say(errorText(reason)))}
              onReset={() => void invoke<string>('rotate_local_api_token').then(() => { say('完整令牌已重置，旧的立即失效'); load(); }).catch((reason: unknown) => say(errorText(reason)))}
            />
          </div>
        ) : null}
      </Section>

      {/* ───────── 导出 ───────── */}
      <Section icon="ios_share" hue={28} title="导出资料库" hint="把资料库导成文件，方便存档或给别的工具读。文件会保存在这台电脑的 /tmp 文件夹里。">
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => void invoke('export_library_markdown', { destination: '/tmp/fox-stars-lab-library.md' }).then(() => say('Markdown 已导出到 /tmp/fox-stars-lab-library.md')).catch((reason: unknown) => say(errorText(reason)))}>导出 Markdown</Button>
          <Button onClick={() => void invoke('export_library_csv', { destination: '/tmp/fox-stars-lab-library.csv' }).then(() => say('CSV 已导出到 /tmp/fox-stars-lab-library.csv')).catch((reason: unknown) => say(errorText(reason)))}>导出 CSV</Button>
        </div>
      </Section>

      {/* ───────── 开发者信息（默认收起） ───────── */}
      <details className="group rounded-lg border border-border bg-card px-5 py-3.5">
        <summary className="flex cursor-pointer list-none items-center gap-2 text-sm font-medium text-on-surface-variant [&::-webkit-details-marker]:hidden">
          <Icon name="chevron_right" size={18} className="transition-transform group-open:rotate-90" />
          运行状态（排查问题时用）
        </summary>
        <p className="mt-3 mb-2 text-sm leading-6 text-foreground">{health}</p>
        <p className="m-0 max-w-2xl text-xs leading-[1.7] text-on-surface-variant">
          MCP 只读工具读取原来的表；log_usage 只有在 FOX_STARS_LAB_ALLOW_WRITE=1 时才写 usage_log；本地接口默认关闭，网页里的脚本调用不了；web-search 读取的表名和列名没有改。
        </p>
        <ul className="mt-3 mb-0 flex list-none flex-col gap-1.5 p-0 text-sm">
          {jobs.slice(0, 8).map((job) => (
            <li key={job.id} className="flex items-center justify-between rounded-lg border border-border px-3 py-1.5">
              <span className="text-on-surface-variant">{job.jobType} · {job.status} {job.errorKind ? `· ${job.errorKind}` : ''}</span>
              <Button size="sm" variant="text" onClick={() => void invoke('cancel_ai_job', { id: job.id }).then(load)}>取消</Button>
            </li>
          ))}
        </ul>
      </details>
    </section>
  );
}

function Metric(props: { label: string; value: string; sub: string }) {
  return (
    <div className="rounded-lg bg-surface-container-low px-3.5 py-3">
      <div className="text-xs text-on-surface-variant">{props.label}</div>
      <div className="mt-0.5 text-lg leading-[1.3] font-bold tracking-tight text-foreground tabular-nums">{props.value}</div>
      <div className="mt-0.5 text-xs leading-[1.5] text-subtle">{props.sub}</div>
    </div>
  );
}
