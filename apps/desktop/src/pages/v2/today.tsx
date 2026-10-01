import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type TodayOverview = {
  activeRepositories: number;
  readmes: number;
  aiDocuments: number;
  citations: number;
  inboxItems: number;
  tools: number;
  schemaVersion: string;
};

export function TodayPage(props: { onOpenLibrary: () => void; onOpenCapture: () => void }) {
  const [overview, setOverview] = useState<TodayOverview | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<TodayOverview>('get_today_overview')
      .then((value) => {
        if (!cancelled) {
          setOverview(value);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <section className="flex h-full min-h-0 flex-col gap-4 overflow-auto p-6">
      <header>
        <p className="text-sm text-on-surface-variant">Fox Stars Lab 2.0</p>
        <h2 className="text-2xl font-semibold text-on-surface">今日</h2>
        <p className="mt-1 max-w-2xl text-sm text-on-surface-variant">
          这是重建后的第一版可用骨架。收藏还在本机，理解卡和找方案还没接上模型。
        </p>
      </header>
      {error ? <p className="text-sm text-error">{error}</p> : null}
      <div className="grid gap-3 sm:grid-cols-3">
        <Stat label="有效仓库" value={overview?.activeRepositories} />
        <Stat label="README" value={overview?.readmes} />
        <Stat label="已有 AI 摘要" value={overview?.aiDocuments} />
      </div>
      <p className="text-xs text-on-surface-variant">
        迁移版本 {overview?.schemaVersion ?? '…'} · 引用 {overview?.citations ?? '…'} · 投喂 {overview?.inboxItems ?? '…'} · 工具 {overview?.tools ?? '…'}
      </p>
      <div className="flex gap-2">
        <button className="rounded-full bg-primary px-4 py-2 text-sm text-on-primary" onClick={props.onOpenLibrary} type="button">
          打开资料库
        </button>
        <button className="rounded-full border border-outline-variant px-4 py-2 text-sm" onClick={props.onOpenCapture} type="button">
          去收集
        </button>
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
