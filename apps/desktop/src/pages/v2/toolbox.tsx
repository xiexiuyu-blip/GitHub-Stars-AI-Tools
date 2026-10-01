import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Job = { id: string; jobType: string; status: string; errorKind: string | null };
type ApiStatus = { enabled: boolean; host: string; port: number; listening: boolean };

export function ToolboxPage() {
  const [health, setHealth] = useState('');
  const [backup, setBackup] = useState('');
  const [jobs, setJobs] = useState<Job[]>([]);
  const [api, setApi] = useState<ApiStatus | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  function load() {
    invoke<string>('get_health_summary').then(setHealth).catch((reason: unknown) => setHealth(String(reason)));
    invoke<Job[]>('list_ai_jobs').then(setJobs).catch(() => setJobs([]));
    invoke<ApiStatus>('get_local_api_status').then(setApi).catch((reason: unknown) => setMessage(String(reason)));
  }

  useEffect(load, []);

  return (
    <section className="flex h-full flex-col gap-4 overflow-auto p-6">
      <h2 className="text-2xl font-semibold">工具箱与连接</h2>
      <p className="text-sm leading-6">{health}</p>
      <p className="max-w-2xl text-sm text-on-surface-variant">MCP 只读工具仍读原来的表。log_usage 只有在 FOX_STARS_LAB_ALLOW_WRITE=1 时才写 usage_log。本地 API 默认关闭，只监听 127.0.0.1:{api?.port ?? 39217}。web-search 正在读的表名和列名没有改。</p>
      <p className="text-sm">本地 API：{api?.enabled ? '已开启' : '关闭'} {api?.listening ? '· 正在监听' : ''}</p>
      <div className="flex flex-wrap gap-2">
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke<string>('create_dev_backup').then(setBackup)}>备份 dev 库</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('export_library_markdown', { destination: '/tmp/fox-stars-lab-library.md' }).then(() => setMessage('Markdown 已导出到 /tmp/fox-stars-lab-library.md'))}>导出 Markdown</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('export_library_csv', { destination: '/tmp/fox-stars-lab-library.csv' }).then(() => setMessage('CSV 已导出到 /tmp/fox-stars-lab-library.csv'))}>导出 CSV</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke<ApiStatus>('set_local_api_enabled', { enabled: !api?.enabled }).then((status) => setApi(status))}> {api?.enabled ? '关闭本地 API' : '开启本地 API'}</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('pause_ai_queue').then(() => setMessage('队列已暂停'))}>暂停队列</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('resume_ai_queue').then(() => setMessage('队列已继续'))}>继续队列</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('set_ai_daily_budget', { tokens: 200000 }).then(() => setMessage('每日预算设为 200000 token'))}>每日预算 20 万</button>
      </div>
      {backup ? <p className="text-sm">备份在 {backup}</p> : null}
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
