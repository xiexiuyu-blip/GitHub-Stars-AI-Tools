import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Row = { sourceId: string; sourceName: string; enabled: boolean; lastError: string; fullName: string; description: string; alreadyStarred: boolean; relevance: number };

export function RankingsPage() {
  const [rows, setRows] = useState<Row[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    invoke<Row[]>('list_ranking_board').then(setRows).catch((reason: unknown) => setError(String(reason)));
  }, []);
  const sources = [...new Map(rows.map((row) => [row.sourceId, row])).values()];
  return (
    <section className="flex h-full flex-col gap-4 overflow-auto p-6">
      <h2 className="text-2xl font-semibold">发现与榜单</h2>
      <p className="text-sm text-on-surface-variant">GitHub Trending 网页抓取保持关闭。断网时这里显示上次快照；现在如果还没刷新，只会看到来源名称。</p>
      {error ? <p className="text-sm text-red-700">{error}</p> : null}
      {sources.map((source) => (
        <article key={source.sourceId} className="rounded-2xl border p-4">
          <h3 className="font-medium">{source.sourceName}</h3>
          <p className="text-xs text-on-surface-variant">{source.enabled ? '已启用' : '默认关闭'} {source.lastError}</p>
          <ul className="mt-2 text-sm">
            {rows.filter((row) => row.sourceId === source.sourceId && row.fullName).map((row) => (
              <li key={`${row.sourceId}-${row.fullName}`}>{row.fullName} · 相关度 {row.relevance.toFixed(1)} {row.alreadyStarred ? '· 已收藏' : ''}</li>
            ))}
          </ul>
        </article>
      ))}
    </section>
  );
}
