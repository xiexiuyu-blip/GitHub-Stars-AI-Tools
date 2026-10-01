import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Hit = { kind: string; id: string; title: string; subtitle: string };

const COMMANDS = [
  { id: 'retry', label: '重试失败', hint: '>重试失败' },
  { id: 'sync', label: '同步 Stars', hint: '>同步' },
  { id: 'export', label: '导出 Markdown', hint: '>导出 /tmp/fox-stars-lab-library.md' },
  { id: 'pause', label: '暂停 AI 队列', hint: '>暂停' },
  { id: 'resume', label: '继续 AI 队列', hint: '>继续' },
];

export function CommandPalette(props: {
  open: boolean;
  onClose: () => void;
  onOpenSolve: (query: string) => void;
  onOpenLibrary: (query: string) => void;
  onSync: () => void;
}) {
  const [query, setQuery] = useState('');
  const [hits, setHits] = useState<Hit[]>([]);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!props.open) return;
    const handle = window.setTimeout(() => {
      if (query.startsWith('?') || query.startsWith('>')) {
        setHits([]);
        return;
      }
      void invoke<Hit[]>('search_command_palette', { query }).then(setHits).catch(() => setHits([]));
    }, 120);
    return () => window.clearTimeout(handle);
  }, [props.open, query]);

  if (!props.open) return null;

  async function runCommand() {
    const text = query.slice(1).trim();
    if (text.startsWith('重试')) {
      const count = await invoke<number>('retry_failed_ai_jobs');
      setMessage(`已把 ${count} 个失败任务放回队列`);
      return;
    }
    if (text.startsWith('同步')) {
      props.onSync();
      props.onClose();
      return;
    }
    if (text.startsWith('导出')) {
      const destination = text.replace(/^导出\s*/, '') || '/tmp/fox-stars-lab-library.md';
      const count = await invoke<number>('export_library_markdown', { destination });
      setMessage(`已导出 ${count} 条到 ${destination}`);
      return;
    }
    if (text.startsWith('暂停')) {
      await invoke('pause_ai_queue');
      setMessage('AI 队列已暂停');
      return;
    }
    if (text.startsWith('继续')) {
      await invoke('resume_ai_queue');
      setMessage('AI 队列已继续');
      return;
    }
    setMessage('可用命令：重试失败、同步、导出、暂停、继续');
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/30 pt-24" onClick={props.onClose}>
      <div className="w-[560px] rounded-2xl bg-surface p-4 shadow-xl" onClick={(event) => event.stopPropagation()}>
        <input
          autoFocus
          className="w-full rounded-xl border px-3 py-2"
          placeholder="搜索仓库和分类，? 找方案，> 同步 / 重试失败 / 导出"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setMessage(null);
          }}
          onKeyDown={(event) => {
            if (event.key !== 'Enter') return;
            if (query.startsWith('?')) {
              props.onOpenSolve(query.slice(1).trim());
              props.onClose();
              return;
            }
            if (query.startsWith('>')) {
              void runCommand().catch((reason: unknown) => setMessage(String(reason)));
            }
          }}
        />
        {query.startsWith('>') ? (
          <ul className="mt-3 space-y-1 text-sm">
            {COMMANDS.map((command) => (
              <li key={command.id}>
                <button className="w-full rounded-lg px-2 py-2 text-left hover:bg-surface-container" type="button" onClick={() => setQuery(`>${command.label}`)}>
                  {command.label}
                  <span className="ml-2 text-xs text-on-surface-variant">{command.hint}</span>
                </button>
              </li>
            ))}
          </ul>
        ) : null}
        {message ? <p className="mt-3 text-sm text-on-surface-variant">{message}</p> : null}
        <ul className="mt-3 max-h-80 overflow-auto text-sm">
          {hits.map((hit) => (
            <li key={`${hit.kind}-${hit.id}`}>
              <button className="w-full rounded-lg px-2 py-2 text-left hover:bg-surface-container" type="button" onClick={() => { props.onOpenLibrary(hit.title); props.onClose(); }}>
                {hit.title}
                <span className="ml-2 text-xs text-on-surface-variant">{hit.kind === 'category' ? '分类' : '仓库'} · {hit.subtitle}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
