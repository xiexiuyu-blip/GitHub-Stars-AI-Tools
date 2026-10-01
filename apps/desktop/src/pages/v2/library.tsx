import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type LibraryCard = {
  id: string;
  fullName: string;
  description: string | null;
  language: string | null;
  htmlUrl: string;
  starsCount: number;
  starredAt: string;
  oneLiner: string | null;
};

type LibraryPage = {
  items: LibraryCard[];
  totalCount: number;
};

type LibraryItem = LibraryCard & {
  summaryZh: string | null;
  readmeZh: string | null;
  topics: string[];
};

export function LibraryPage() {
  const [keyword, setKeyword] = useState('');
  const [page, setPage] = useState<LibraryPage | null>(null);
  const [selected, setSelected] = useState<LibraryItem | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const handle = window.setTimeout(() => {
      invoke<LibraryPage>('list_library_cards', { keyword, limit: 80, offset: 0 })
        .then(setPage)
        .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : String(reason)));
    }, 200);
    return () => window.clearTimeout(handle);
  }, [keyword]);

  async function openItem(id: string) {
    const item = await invoke<LibraryItem>('get_library_item', { id });
    setSelected(item);
  }

  return (
    <section className="flex h-full min-h-0">
      <div className="flex min-w-0 flex-1 flex-col gap-3 overflow-hidden p-6">
        <header className="flex items-end justify-between gap-3">
          <div>
            <h2 className="text-2xl font-semibold">资料库</h2>
            <p className="text-sm text-on-surface-variant">{page ? `${page.totalCount} 个有效仓库` : '正在读取本机库'}</p>
          </div>
          <input
            className="w-64 rounded-full border border-outline-variant bg-surface px-4 py-2 text-sm"
            placeholder="搜名称、说明或摘要"
            value={keyword}
            onChange={(event) => setKeyword(event.target.value)}
          />
        </header>
        {error ? <p className="text-sm text-error">{error}</p> : null}
        <div className="min-h-0 flex-1 space-y-2 overflow-auto pr-1">
          {page?.items.map((item) => (
            <button
              key={item.id}
              className="block w-full rounded-2xl border border-outline-variant/40 bg-surface-container px-4 py-3 text-left"
              onClick={() => void openItem(item.id)}
              type="button"
            >
              <div className="flex items-center justify-between gap-3">
                <span className="font-medium">{item.fullName}</span>
                <span className="text-xs text-on-surface-variant">{item.language ?? '未标语言'} · {item.starsCount} stars</span>
              </div>
              <p className="mt-1 line-clamp-2 text-sm text-on-surface-variant">{item.oneLiner || item.description || '还没有中文说明'}</p>
            </button>
          ))}
        </div>
      </div>
      {selected ? (
        <aside className="hidden w-[420px] shrink-0 overflow-auto border-l border-outline-variant/40 p-5 lg:block">
          <p className="text-xs text-on-surface-variant">条目详情</p>
          <h3 className="mt-1 text-lg font-semibold">{selected.fullName}</h3>
          <a className="text-sm text-primary" href={selected.htmlUrl} target="_blank" rel="noreferrer">
            打开 GitHub
          </a>
          <p className="mt-4 text-sm leading-6">{selected.summaryZh || selected.description || '还没有 AI 摘要'}</p>
          {selected.topics.length > 0 ? (
            <p className="mt-3 text-xs text-on-surface-variant">{selected.topics.join(' · ')}</p>
          ) : null}
        </aside>
      ) : null}
    </section>
  );
}
