import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Block = { slug: string; nameZh: string; count: number };

/** 读取中写「读取中…」；读完了没有分类写「还没有分类。同步并整理之后这里会出现。」；读出错只显示错误，不当成「没有分类」。 */
const MAP_LOADING_TEXT = '读取中…';
const MAP_EMPTY_TEXT = '还没有分类。同步并整理之后这里会出现。';

export function MapBlocks(props: { loading: boolean; error: string | null; blocks: readonly Block[] }) {
  if (props.error) return <p className="text-sm text-error">{props.error}</p>;
  if (props.loading) return <p className="text-sm text-on-surface-variant">{MAP_LOADING_TEXT}</p>;
  if (props.blocks.length === 0) return <p className="text-sm text-on-surface-variant">{MAP_EMPTY_TEXT}</p>;
  return (
    <div className="grid gap-3 sm:grid-cols-3">
      {props.blocks.map((block) => (
        <div key={block.slug} className="rounded-2xl border p-4">
          <p className="text-3xl font-semibold">{block.count}</p>
          <p className="mt-1 text-sm">{block.nameZh}</p>
        </div>
      ))}
    </div>
  );
}

export function MapPage() {
  const [blocks, setBlocks] = useState<Block[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    invoke<Block[] | null>('get_graph_overview')
      .then((result) => {
        if (cancelled) return;
        setBlocks(Array.isArray(result) ? result : []);
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setError(String(reason));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);
  return (
    <section className="flex h-full flex-col gap-4 overflow-auto p-6">
      <h2 className="text-2xl font-semibold">按类型看</h2>
      <p className="text-sm text-on-surface-variant">按「它是什么」给你的收藏分的类（和「场景」不同：场景是「你拿它做什么」）。目前只能看，不能改。</p>
      <MapBlocks loading={loading} error={error} blocks={blocks} />
    </section>
  );
}
