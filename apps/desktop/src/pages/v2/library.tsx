import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Card = {
  id: string;
  fullName: string;
  description: string | null;
  language: string | null;
  htmlUrl: string;
  starsCount: number;
  readingLabel: string;
  oneLiner: string | null;
};

type Detail = Card & {
  summaryZh: string | null;
  keywords: string[];
  readmeExcerpt: string | null;
};

const FILTERS = [
  { id: '', label: '全部' },
  { id: 'unseen', label: '没看' },
  { id: 'want_to_try', label: '想试' },
  { id: 'tried', label: '试过' },
  { id: 'in_use', label: '在用' },
  { id: 'dropped', label: '不要了' },
];

export function LibraryPage(props: { initialKeyword?: string }) {
  const [keyword, setKeyword] = useState(props.initialKeyword ?? '');
  const [language, setLanguage] = useState('');
  const [reading, setReading] = useState('');
  const [table, setTable] = useState(false);
  const [items, setItems] = useState<Card[]>([]);
  const [total, setTotal] = useState(0);
  const [selected, setSelected] = useState<Detail | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (props.initialKeyword) setKeyword(props.initialKeyword);
  }, [props.initialKeyword]);

  useEffect(() => {
    const handle = window.setTimeout(() => {
      invoke<{ items: Card[]; totalCount: number }>('list_workspace_cards', { keyword, language, reading, limit: 80, offset: 0 })
        .then((page) => {
          setItems(page.items);
          setTotal(page.totalCount);
        })
        .catch((reason: unknown) => setError(String(reason)));
    }, 180);
    return () => window.clearTimeout(handle);
  }, [keyword, language, reading]);

  return (
    <section className="flex h-full min-h-0">
      <div className="flex min-w-0 flex-1 flex-col gap-3 overflow-hidden p-6">
        <header className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h2 className="text-2xl font-semibold">资料库</h2>
            <p className="text-sm text-on-surface-variant">{total} 个有效仓库</p>
          </div>
          <div className="flex gap-2">
            <input className="w-40 rounded-full border px-4 py-2 text-sm" placeholder="语言" value={language} onChange={(event) => setLanguage(event.target.value)} />
            <input className="w-56 rounded-full border px-4 py-2 text-sm" placeholder="搜索" value={keyword} onChange={(event) => setKeyword(event.target.value)} />
            <button className="rounded-full border px-3 text-sm" type="button" onClick={() => setTable((value) => !value)}>{table ? '卡片' : '表格'}</button>
          </div>
        </header>
        <div className="flex flex-wrap gap-2">
          {FILTERS.map((filter) => (
            <button key={filter.id} className={`rounded-full px-3 py-1 text-xs ${reading === filter.id ? 'bg-primary text-on-primary' : 'border'}`} type="button" onClick={() => setReading(filter.id)}>{filter.label}</button>
          ))}
        </div>
        {error ? <p className="text-sm text-red-700">{error}</p> : null}
        <div className="min-h-0 flex-1 overflow-auto">
          {table ? (
            <table className="w-full text-left text-sm">
              <tbody>
                {items.map((item) => (
                  <tr key={item.id} className="cursor-pointer border-b" onClick={() => void invoke<Detail>('get_workspace_detail', { id: item.id }).then(setSelected)}>
                    <td className="py-2">{item.fullName}</td>
                    <td>{item.readingLabel}</td>
                    <td>{item.language}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <div className="space-y-2">
              {items.map((item) => (
                <button key={item.id} className="block w-full rounded-2xl border px-4 py-3 text-left" type="button" onClick={() => void invoke<Detail>('get_workspace_detail', { id: item.id }).then(setSelected)}>
                  <div className="flex justify-between gap-3"><span className="font-medium">{item.fullName}</span><span className="text-xs">{item.readingLabel} · {item.starsCount}</span></div>
                  <p className="mt-1 line-clamp-2 text-sm text-on-surface-variant">{item.oneLiner || item.description || '还没有中文说明'}</p>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
      {selected ? (
        <aside className="hidden w-[440px] shrink-0 overflow-auto border-l p-5 lg:block">
          <h3 className="text-lg font-semibold">{selected.fullName}</h3>
          <p className="text-xs text-on-surface-variant">{selected.readingLabel} · {selected.language}</p>
          <a className="text-sm text-primary" href={selected.htmlUrl}>打开 GitHub</a>
          <p className="mt-4 text-sm leading-6">{selected.summaryZh || '还没有 AI 摘要'}</p>
          {selected.keywords.length > 0 ? <p className="mt-3 text-xs">{selected.keywords.join(' · ')}</p> : null}
          {selected.readmeExcerpt ? <pre className="mt-4 whitespace-pre-wrap text-xs text-on-surface-variant">{selected.readmeExcerpt}</pre> : null}
          <button className="mt-4 rounded-full border px-3 py-1 text-xs" type="button" onClick={() => void invoke('log_repository_usage', { entityId: selected.id, verdict: 'useful', note: '资料库里点过' })}>记为好用</button>
        </aside>
      ) : null}
    </section>
  );
}
