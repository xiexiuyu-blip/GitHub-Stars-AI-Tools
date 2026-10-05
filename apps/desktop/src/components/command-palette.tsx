import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { downloadDir, join } from '@tauri-apps/api/path';
import { exportSavedText } from '@/lib/automation-view';

type Hit = { kind: string; id: string; title: string; subtitle: string };

const COMMANDS = [
  { id: 'sync', label: '同步我的收藏', hint: '>同步' },
  { id: 'export', label: '导出成文档（存到「下载」文件夹）', hint: '>导出' },
  { id: 'pause', label: '暂停自动整理', hint: '>暂停' },
  { id: 'resume', label: '继续自动整理', hint: '>继续' },
  { id: 'retry', label: '重新整理失败的', hint: '>重新整理失败的' },
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
      void invoke<Hit[] | null>('search_command_palette', { query })
        .then((result) => setHits(Array.isArray(result) ? result : []))
        .catch(() => setHits([]));
    }, 120);
    return () => window.clearTimeout(handle);
  }, [props.open, query]);

  if (!props.open) return null;

  async function runCommand() {
    const text = query.slice(1).trim();
    if (text.startsWith('重新整理') || text.startsWith('重试')) {
      const count = await invoke<number>('retry_failed_ai_jobs');
      setMessage(`已把 ${count} 个失败的放回自动整理`);
      return;
    }
    if (text.startsWith('同步')) {
      props.onSync();
      props.onClose();
      return;
    }
    if (text.startsWith('导出')) {
      const fileName = 'fox-stars-lab-library.md';
      const destination = await join(await downloadDir(), fileName);
      const count = await invoke<number>('export_library_markdown', { destination });
      setMessage(exportSavedText(count, fileName));
      return;
    }
    if (text.startsWith('暂停')) {
      await invoke('pause_ai_queue');
      setMessage('自动整理已暂停');
      return;
    }
    if (text.startsWith('继续')) {
      await invoke('resume_ai_queue');
      setMessage('自动整理已继续');
      return;
    }
    setMessage('可用命令：同步我的收藏、导出成文档、暂停自动整理、继续自动整理、重新整理失败的');
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/30 pt-24" onClick={props.onClose}>
      <div className="w-[560px] rounded-2xl bg-surface p-4 shadow-xl" onClick={(event) => event.stopPropagation()}>
        <input
          autoFocus
          className="w-full rounded-xl border px-3 py-2"
          placeholder="直接搜工具；输入 ? 加一句话去「找方案」；输入 > 执行操作"
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
          {hits.filter((hit) => hit.kind !== 'category').map((hit) => (
            <li key={`${hit.kind}-${hit.id}`}>
              <button className="w-full rounded-lg px-2 py-2 text-left hover:bg-surface-container" type="button" onClick={() => { props.onOpenLibrary(hit.title); props.onClose(); }}>
                {hit.title}
                <span className="ml-2 text-xs text-on-surface-variant">工具 · {hit.subtitle}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
