import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Block = { slug: string; nameZh: string; count: number };

export function MapPage() {
  const [blocks, setBlocks] = useState<Block[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    invoke<Block[] | null>('get_graph_overview')
      .then((result) => setBlocks(Array.isArray(result) ? result : []))
      .catch((reason: unknown) => setError(String(reason)));
  }, []);
  return (
    <section className="flex h-full flex-col gap-4 overflow-auto p-6">
      <h2 className="text-2xl font-semibold">按类型看</h2>
      <p className="text-sm text-on-surface-variant">按「它是什么」给你的收藏分的类（和「场景」不同：场景是「你拿它做什么」）。目前只能看，不能改。</p>
      {error ? <p className="text-sm text-error">{error}</p> : null}
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
