import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type TodayActions = {
  activeRepositories: number;
  suggestedCards: number;
  failedJobs: number;
  starredThisWeek: number;
  topLanguages: { language: string; count: number }[];
  schemaVersion: string;
};

export function TodayPage(props: { onOpenLibrary: () => void; onOpenCapture: () => void; onOpenSolve: () => void }) {
  const [overview, setOverview] = useState<TodayActions | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  function load() {
    invoke<TodayActions>('get_today_actions').then(setOverview).catch((reason: unknown) => setError(String(reason)));
  }

  useEffect(load, []);

  return (
    <section className="flex h-full min-h-0 flex-col gap-4 overflow-auto p-6">
      <header>
        <p className="text-sm text-on-surface-variant">按 ⌘K 搜索，输入 ? 去找方案</p>
        <h2 className="text-2xl font-semibold">今日</h2>
      </header>
      {error ? <p className="text-sm text-red-700">{error}</p> : null}
      {message ? <p className="text-sm text-on-surface-variant">{message}</p> : null}
      <div className="grid gap-3 sm:grid-cols-4">
        <Stat label="有效仓库" value={overview?.activeRepositories} />
        <Stat label="待确认理解卡" value={overview?.suggestedCards} />
        <Stat label="失败可重试" value={overview?.failedJobs} />
        <Stat label="近 7 天收藏" value={overview?.starredThisWeek} />
      </div>
      <p className="text-sm text-on-surface-variant">
        收藏画像：{overview?.topLanguages.map((item) => `${item.language} ${item.count}`).join(' · ') || '读取中'}
      </p>
      <p className="text-xs text-on-surface-variant">迁移版本 {overview?.schemaVersion ?? '…'}</p>
      <div className="flex flex-wrap gap-2">
        <button className="rounded-full bg-primary px-4 py-2 text-sm text-on-primary" type="button" onClick={props.onOpenLibrary}>资料库</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={props.onOpenCapture}>收集</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={props.onOpenSolve}>找方案</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke<string>('prepare_knowledge').then(setMessage).catch((reason: unknown) => setError(String(reason)))}>生成建议卡和分类</button>
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke('retry_failed_ai_jobs').then(load)}>重试失败任务</button>
      </div>
    </section>
  );
}

function Stat(props: { label: string; value: number | undefined }) {
  return (
    <div className="rounded-2xl border border-outline-variant/40 bg-surface-container p-4">
      <p className="text-sm text-on-surface-variant">{props.label}</p>
      <p className="mt-1 text-3xl font-semibold">{props.value ?? '…'}</p>
    </div>
  );
}
