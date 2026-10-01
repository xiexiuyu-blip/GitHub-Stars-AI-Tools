import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Block = { slug: string; nameZh: string; count: number };

export function MapPage() {
  const [blocks, setBlocks] = useState<Block[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    invoke<Block[]>('get_graph_overview').then(setBlocks).catch((reason: unknown) => setError(String(reason)));
  }, []);
  const total = blocks.reduce((sum, block) => sum + block.count, 0);
  return (
    <section className="flex h-full flex-col gap-4 overflow-auto p-6">
      <h2 className="text-2xl font-semibold">地图</h2>
      <p className="text-sm text-on-surface-variant">规则初分合计 {total}。这是建议分类，确认后才会锁定。</p>
      {error ? <p className="text-sm text-red-700">{error}</p> : null}
      <div className="grid gap-3 sm:grid-cols-3">
        {blocks.map((block) => (
          <div key={block.slug} className="rounded-2xl border p-4">
            <p className="text-3xl font-semibold">{block.count}</p>
            <p className="mt-1 text-sm">{block.nameZh}</p>
          </div>
        ))}
      </div>
    </section>
  );
}
