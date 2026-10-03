import { useEffect, useState } from 'react';
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

  function load() {
    invoke<string>('get_health_summary').then(setHealth).catch((reason: unknown) => setHealth(String(reason)));
    invoke<Job[]>('list_ai_jobs').then(setJobs).catch(() => setJobs([]));
    invoke<ApiStatus>('get_local_api_status').then(setApi).catch((reason: unknown) => setMessage(errorText(reason)));
    invoke<BackupStatus>('get_backup_status').then(setStatus).catch((reason: unknown) => setMessage(errorText(reason)));
    invoke<BackupInfo[]>('list_backups').then(setBackups).catch((reason: unknown) => setMessage(errorText(reason)));
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
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('pause_ai_queue').then(() => setMessage('队列已暂停'))}>暂停队列</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('resume_ai_queue').then(() => setMessage('队列已继续'))}>继续队列</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('set_ai_daily_budget', { tokens: 200000 }).then(() => setMessage('每日预算设为 200000 token'))}>每日预算 20 万</button>
      </div>
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
