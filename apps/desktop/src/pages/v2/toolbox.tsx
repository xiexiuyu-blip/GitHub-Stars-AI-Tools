import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { relaunch } from '@tauri-apps/plugin-process';

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
type PauseReason = { kind: string; message: string; at: string };
type AutomationStatus = {
  enabled: boolean;
  dailyBudgetTokens: number;
  paused: boolean;
  pauseReason: PauseReason | null;
  tokens24h: number;
  budgetRemaining: number;
  queued: number;
  running: number;
  failed: number;
  providerConfigured: boolean;
  model: string | null;
};

function parseDailyBudget(raw: string): number | string {
  const text = raw.trim();
  if (!text) return '请先填写每天额度';
  if (text.startsWith('-')) return '每天额度不能小于 0';
  if (!/^\d+$/.test(text)) return '每天额度要填整数';
  const value = Number(text);
  if (!Number.isSafeInteger(value)) return '每天额度要填整数';
  return value;
}

const KIND_LABEL: Record<BackupKind, string> = {
  auto: '自动',
  manual: '手动',
  preRestore: '恢复前',
  preMigrate: '迁移前',
};

function errorText(reason: unknown): string {
  if (typeof reason === 'string' && reason.trim()) return reason;
  if (reason instanceof Error && reason.message.trim()) return reason.message;
  return String(reason);
}

function formatMinute(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function formatSize(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function restoreBanner(report: RestoreReport): string {
  if (report.ok) {
    const when = report.backupCreatedAt ? formatMinute(report.backupCreatedAt) : report.fileName;
    return `已恢复到 ${when} 的备份`;
  }
  return `恢复失败：${report.error ?? '未知原因'}（资料库没有被改动）`;
}

export function ToolboxPage() {
  const [health, setHealth] = useState('');
  const [jobs, setJobs] = useState<Job[]>([]);
  const [api, setApi] = useState<ApiStatus | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [confirmName, setConfirmName] = useState<string | null>(null);
  const [restartReady, setRestartReady] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  const [automation, setAutomation] = useState<AutomationStatus | null>(null);
  const [budgetInput, setBudgetInput] = useState('');
  const [budgetLoaded, setBudgetLoaded] = useState(false);
  const [pendingBudget, setPendingBudget] = useState<number | null>(null);
  const [enqueueNote, setEnqueueNote] = useState<string | null>(null);
  const budgetTouched = useRef(false);

  function load() {
    invoke<string>('get_health_summary').then(setHealth).catch((reason: unknown) => setHealth(String(reason)));
    invoke<Job[]>('list_ai_jobs').then(setJobs).catch(() => setJobs([]));
    invoke<ApiStatus>('get_local_api_status').then(setApi).catch((reason: unknown) => setMessage(errorText(reason)));
    invoke<BackupStatus>('get_backup_status').then(setStatus).catch((reason: unknown) => setMessage(errorText(reason)));
    invoke<BackupInfo[]>('list_backups').then(setBackups).catch((reason: unknown) => setMessage(errorText(reason)));
    invoke<AutomationStatus>('get_ai_automation')
      .then((next) => {
        setAutomation(next);
        setBudgetInput((current) => (budgetTouched.current ? current : String(next.dailyBudgetTokens)));
        setBudgetLoaded(true);
      })
      .catch((reason: unknown) => setMessage(errorText(reason)));
  }

  function applyAutomation(next: AutomationStatus) {
    setAutomation(next);
    budgetTouched.current = false;
    setBudgetInput(String(next.dailyBudgetTokens));
    setBudgetLoaded(true);
  }

  function saveBudget(tokens: number) {
    setPendingBudget(null);
    void invoke<AutomationStatus>('set_ai_automation', { dailyBudgetTokens: tokens })
      .then((next) => {
        applyAutomation(next);
        setMessage(next.dailyBudgetTokens === 0 ? '已关闭自动整理' : '已保存每天额度');
      })
      .catch((reason: unknown) => setMessage(errorText(reason)));
  }

  function requestSaveBudget() {
    const parsed = parseDailyBudget(budgetInput);
    if (typeof parsed === 'string') {
      setMessage(parsed);
      return;
    }
    const current = automation?.dailyBudgetTokens ?? 0;
    if (parsed > 0 && parsed !== current) {
      setPendingBudget(parsed);
      return;
    }
    if (parsed === current) return;
    saveBudget(parsed);
  }

  function enqueueCards(scope: 'missing' | 'outdated') {
    void invoke<number>('enqueue_cards_now', { scope })
      .then((count) => {
        setEnqueueNote(`已排队 ${count} 个；是否开始整理取决于上面的每天额度`);
        return invoke<AutomationStatus>('get_ai_automation').then((next) => {
          setAutomation(next);
          setBudgetLoaded(true);
        });
      })
      .catch((reason: unknown) => setMessage(errorText(reason)));
  }

  function resumeAutomation() {
    void invoke('resume_ai_queue')
      .then(() => invoke<AutomationStatus>('get_ai_automation'))
      .then((next) => {
        setAutomation(next);
        setMessage('队列已继续');
      })
      .catch((reason: unknown) => setMessage(errorText(reason)));
  }

  useEffect(load, []);

  function backupNow() {
    setBackingUp(true);
    void invoke<BackupInfo>('create_backup_now')
      .then(() => {
        setMessage('已备份');
        return invoke<BackupInfo[]>('list_backups').then(setBackups);
      })
      .then(() => invoke<BackupStatus>('get_backup_status').then(setStatus))
      .catch((reason: unknown) => setMessage(errorText(reason)))
      .finally(() => setBackingUp(false));
  }

  function confirmRestore() {
    if (!confirmName) return;
    const fileName = confirmName;
    void invoke('request_restore_backup', { fileName })
      .then(() => {
        setConfirmName(null);
        setRestartReady(true);
        setMessage('已记录，重启后生效');
      })
      .catch((reason: unknown) => setMessage(errorText(reason)));
  }

  return (
    <section className="flex h-full flex-col gap-4 overflow-auto p-6">
      {status?.lastRestore ? <p className="text-sm">{restoreBanner(status.lastRestore)}</p> : null}
      <h2 className="text-2xl font-semibold">工具箱与连接</h2>
      <p className="text-sm leading-6">{health}</p>
      <p className="max-w-2xl text-sm text-on-surface-variant">MCP 只读工具仍读原来的表。log_usage 只有在 FOX_STARS_LAB_ALLOW_WRITE=1 时才写 usage_log。本地 API 默认关闭，只监听 127.0.0.1:{api?.port ?? 39217}，开启后需要令牌，网页里的脚本调用不了。web-search 正在读的表名和列名没有改。</p>
      <p className="text-sm">本地 API：{api?.enabled ? '已开启' : '关闭'} {api?.listening ? '· 正在监听' : ''}</p>
      <p className="text-sm">自动备份：上次 {status ? (status.lastAutoAt ? formatMinute(status.lastAutoAt) : '还没有') : '读取中'}，下次 {status ? formatMinute(status.nextAutoAt) : '读取中'}</p>
      <div className="flex flex-wrap gap-2">
        <button className="rounded-full bg-primary px-4 py-2 text-sm text-on-primary disabled:opacity-60" type="button" disabled={backingUp} onClick={backupNow}>现在备份</button>
      </div>
      {restartReady ? (
        <div className="flex flex-wrap items-center gap-2 rounded-2xl border bg-surface-container-low p-4 text-sm">
          <p>已记录，重启后生效</p>
          <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void relaunch().catch((reason: unknown) => setMessage(errorText(reason)))}>立即重启</button>
          <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => setRestartReady(false)}>稍后</button>
        </div>
      ) : null}
      {confirmName ? (
        <div className="rounded-2xl border bg-surface-container-low p-4 text-sm">
          <p>恢复会把当前资料库换成这一份。当前的库会先另存一份「恢复前」备份。需要重启应用才会生效。继续吗？</p>
          <div className="mt-3 flex gap-2">
            <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={confirmRestore}>继续</button>
            <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => setConfirmName(null)}>取消</button>
          </div>
        </div>
      ) : null}
      <ul className="space-y-2 text-sm">
        {backups.map((item) => (
          <li key={item.fileName} className="flex items-center justify-between gap-3 rounded-xl border px-3 py-2">
            <span>{formatMinute(item.createdAt)} · {KIND_LABEL[item.kind]} · {formatSize(item.bytes)} · {item.repositoryCount === null ? '仓库数未知' : `${item.repositoryCount} 个仓库`}</span>
            <button className="shrink-0 rounded-full border px-3 py-1 text-xs" type="button" onClick={() => setConfirmName(item.fileName)}>恢复到这一份…</button>
          </li>
        ))}
      </ul>
      <div className="flex flex-wrap gap-2">
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('export_library_markdown', { destination: '/tmp/fox-stars-lab-library.md' }).then(() => setMessage('Markdown 已导出到 /tmp/fox-stars-lab-library.md'))}>导出 Markdown</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('export_library_csv', { destination: '/tmp/fox-stars-lab-library.csv' }).then(() => setMessage('CSV 已导出到 /tmp/fox-stars-lab-library.csv'))}>导出 CSV</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke<ApiStatus>('set_local_api_enabled', { enabled: !api?.enabled }).then((status) => setApi(status)).catch((reason: unknown) => setMessage(String(reason)))}> {api?.enabled ? '关闭本地 API' : '开启本地 API'}</button>
        {api?.enabled ? (
          <>
            <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke<string>('get_local_api_token').then((token) => navigator.clipboard.writeText(token)).then(() => setMessage('令牌已复制。调用时加请求头 Authorization: Bearer <令牌>')).catch((reason: unknown) => setMessage(String(reason)))}>复制令牌</button>
            <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke<string>('rotate_local_api_token').then(() => { setMessage('令牌已重置，旧令牌立即失效。'); load(); }).catch((reason: unknown) => setMessage(String(reason)))}>重置令牌</button>
          </>
        ) : null}
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('pause_ai_queue').then(() => invoke<AutomationStatus>('get_ai_automation')).then((next) => { setAutomation(next); setMessage('队列已暂停'); }).catch((reason: unknown) => setMessage(errorText(reason)))}>暂停队列</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('resume_ai_queue').then(() => invoke<AutomationStatus>('get_ai_automation')).then((next) => { setAutomation(next); setMessage('队列已继续'); }).catch((reason: unknown) => setMessage(errorText(reason)))}>继续队列</button>
      </div>
      <section className="flex flex-col gap-3">
        <h3 className="text-lg font-semibold">AI 自动整理</h3>
        {automation ? (
          <p className="text-sm">{automation.enabled ? '已开启' : '未开启'} · 今天已用 {automation.tokens24h}，还剩 {automation.budgetRemaining} · 排队 {automation.queued}，进行中 {automation.running}，失败 {automation.failed}</p>
        ) : (
          <p className="text-sm">读取中</p>
        )}
        {automation && !automation.providerConfigured ? <p className="text-sm">还没配置 AI 服务，先去设置里配置</p> : null}
        {automation?.pauseReason ? (
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-sm text-error">{automation.pauseReason.message}</p>
            <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={resumeAutomation}>继续队列</button>
          </div>
        ) : null}
        <div className="flex flex-wrap items-center gap-2">
          <label className="text-sm" htmlFor="daily-budget">每天最多用多少 token</label>
          <input
            id="daily-budget"
            className="w-40 rounded-full border bg-transparent px-4 py-2 text-sm"
            type="number"
            min={0}
            step={1}
            inputMode="numeric"
            value={budgetInput}
            onChange={(event) => {
              budgetTouched.current = true;
              setBudgetInput(event.target.value);
            }}
          />
          <button className="rounded-full bg-primary px-4 py-2 text-sm text-on-primary disabled:opacity-60" type="button" disabled={!budgetLoaded} onClick={requestSaveBudget}>保存</button>
        </div>
        {pendingBudget !== null ? (
          <div className="rounded-2xl border bg-surface-container-low p-4 text-sm">
            <p>这会让应用在后台调用你配置的 AI 服务整理仓库，并按你的服务商计费。每天最多使用 {pendingBudget} token（约相当于整理 {Math.floor(pendingBudget / 4000)} 个仓库）。确认开启吗？</p>
            <div className="mt-3 flex gap-2">
              <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => saveBudget(pendingBudget)}>确认</button>
              <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => setPendingBudget(null)}>取消</button>
            </div>
          </div>
        ) : null}
        <div className="flex flex-wrap gap-2">
          <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => enqueueCards('missing')}>给没有理解卡的仓库排队</button>
          <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => enqueueCards('outdated')}>把旧版理解卡排队重做</button>
        </div>
        {enqueueNote ? <p className="text-sm">{enqueueNote}</p> : null}
      </section>
      {message ? <p className="text-sm text-on-surface-variant">{message}</p> : null}
      <ul className="space-y-2 text-sm">
        {jobs.slice(0, 8).map((job) => (
          <li key={job.id} className="flex items-center justify-between rounded-xl border px-3 py-2">
            <span>{job.jobType} · {job.status} {job.errorKind ? `· ${job.errorKind}` : ''}</span>
            <button className="text-xs" type="button" onClick={() => void invoke('cancel_ai_job', { id: job.id }).then(load)}>取消</button>
          </li>
        ))}
      </ul>
    </section>
  );
}
