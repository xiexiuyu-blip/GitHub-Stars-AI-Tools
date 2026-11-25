import { useEffect, useId, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { downloadDir, join } from '@tauri-apps/api/path';
import { exportSavedText } from '@/lib/automation-view';

type Hit = { kind: string; id: string; title: string; subtitle: string };

const COMMANDS = [
  { id: 'sync', label: '同步我的收藏', hint: '>同步' },
  { id: 'export', label: '导出成文档（存到「下载」文件夹）', hint: '>导出' },
  { id: 'pause', label: '暂停自动整理', hint: '>暂停' },
  { id: 'resume', label: '继续自动整理', hint: '>继续' },
  { id: 'retry', label: '重试失败的整理（受每天额度限制）', hint: '>重试失败的整理' },
];

export function CommandPalette(props: {
  open: boolean;
  onClose: () => void;
  onOpenSolve: (query: string) => void;
  onOpenLibrary: (query: string) => void;
  onSync: () => void;
}) {
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<{ query: string; hits: Hit[] }>({ query: '', hits: [] });
  const [message, setMessage] = useState<string | null>(null);
  const [selected, setSelected] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const listId = useId();
  const isCommand = query.startsWith('>');
  const isQuestion = query.startsWith('?');
  // 查询刚变时就隐藏上一轮结果，不能在防抖的间隙打开旧工具。
  const hits = !isCommand && !isQuestion && result.query === query ? result.hits.filter((hit) => hit.kind !== 'category') : [];
  const count = isCommand ? COMMANDS.length : hits.length;

  useEffect(() => {
    if (!props.open) return;
    const previous = document.activeElement as HTMLElement | null;
    inputRef.current?.focus();
    setSelected(-1);
    return () => {
      if (previous?.isConnected) previous.focus();
    };
  }, [props.open]);

  useEffect(() => {
    if (!props.open || !query.trim() || isCommand || isQuestion) return;
    let cancelled = false;
    const handle = window.setTimeout(() => {
      void invoke<Hit[] | null>('search_command_palette', { query })
        .then((items) => {
          if (!cancelled) setResult({ query, hits: Array.isArray(items) ? items : [] });
        })
        .catch(() => {
          if (!cancelled) setResult({ query, hits: [] });
        });
    }, 120);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [props.open, query, isCommand, isQuestion]);

  useEffect(() => {
    if (props.open && selected >= 0) {
      document.getElementById(`${listId}-${selected}`)?.scrollIntoView?.({ block: 'nearest' });
    }
  }, [props.open, selected, listId]);

  if (!props.open) return null;

  async function runCommand(text: string) {
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
    setMessage('可用命令：同步我的收藏、导出成文档、暂停自动整理、继续自动整理、重试失败的整理（受每天额度限制）');
  }

  function openLibrary(text: string) {
    if (!text.trim()) return;
    props.onOpenLibrary(text.trim());
    props.onClose();
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/30 pt-24" onClick={props.onClose}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label="搜索与快捷操作"
        className="w-[560px] max-w-[calc(100vw-2rem)] rounded-2xl bg-surface p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing || event.keyCode === 229) return;
          if (event.key === 'Escape') {
            event.preventDefault();
            event.stopPropagation();
            props.onClose();
          }
          if (event.key === 'Tab') {
            const controls = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>('input, button:not([tabindex="-1"])') ?? []);
            const first = controls[0];
            const last = controls[controls.length - 1];
            if (event.shiftKey && document.activeElement === first) {
              event.preventDefault();
              last?.focus();
            } else if (!event.shiftKey && document.activeElement === last) {
              event.preventDefault();
              first?.focus();
            }
          }
        }}
      >
        <div className="flex items-center gap-2">
          <input
            ref={inputRef}
            role="combobox"
            aria-label="搜索工具或输入快捷操作"
            aria-autocomplete="list"
            aria-expanded={count > 0}
            aria-controls={listId}
            aria-activedescendant={selected >= 0 && selected < count ? `${listId}-${selected}` : undefined}
            className="min-w-0 flex-1 rounded-xl border px-3 py-2 focus-visible:outline-2 focus-visible:outline-primary"
            placeholder="搜工具；? 找方案；> 执行操作"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setSelected(-1);
              setMessage(null);
            }}
            onKeyDown={(event) => {
              if (event.nativeEvent.isComposing || event.keyCode === 229) return;
              if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
                event.preventDefault();
                if (count) setSelected((current) => event.key === 'ArrowDown'
                  ? Math.min(current + 1, count - 1)
                  : current < 0 ? count - 1 : Math.max(current - 1, 0));
                return;
              }
              if (event.key !== 'Enter') return;
              event.preventDefault();
              if (isQuestion) {
                props.onOpenSolve(query.slice(1).trim());
                props.onClose();
              } else if (isCommand) {
                const command = selected >= 0 ? COMMANDS[selected]?.label : query.slice(1).trim();
                void runCommand(command ?? '').catch((reason: unknown) => setMessage(String(reason)));
              } else {
                openLibrary(hits[selected]?.title ?? query);
              }
            }}
          />
          <button type="button" className="rounded-lg px-3 py-2 text-sm hover:bg-surface-container focus-visible:outline-2 focus-visible:outline-primary" onClick={props.onClose}>关闭</button>
        </div>
        <p className="mt-2 text-xs text-on-surface-variant">回车搜索 · ↑↓ 选择 · Esc 关闭</p>
        {message ? <p role="status" className="mt-3 text-sm text-on-surface-variant">{message}</p> : null}
        <ul id={listId} role="listbox" aria-label={isCommand ? '快捷操作' : '工具搜索结果'} className="mt-3 max-h-80 space-y-1 overflow-auto text-sm">
          {isCommand ? COMMANDS.map((command, index) => (
            <li key={command.id} role="presentation">
              <button
                id={`${listId}-${index}`}
                role="option"
                aria-selected={selected === index}
                tabIndex={-1}
                className={`w-full rounded-lg px-2 py-2 text-left hover:bg-surface-container ${selected === index ? 'bg-surface-container' : ''}`}
                type="button"
                onClick={() => {
                  setQuery(`>${command.label}`);
                  setSelected(index);
                  setMessage(null);
                  inputRef.current?.focus();
                }}
              >
                {command.label}<span className="ml-2 text-xs text-on-surface-variant">{command.hint}</span>
              </button>
            </li>
          )) : hits.map((hit, index) => (
            <li key={`${hit.kind}-${hit.id}`} role="presentation">
              <button
                id={`${listId}-${index}`}
                role="option"
                aria-selected={selected === index}
                tabIndex={-1}
                className={`w-full rounded-lg px-2 py-2 text-left hover:bg-surface-container ${selected === index ? 'bg-surface-container' : ''}`}
                type="button"
                onClick={() => openLibrary(hit.title)}
              >
                {hit.title}<span className="ml-2 text-xs text-on-surface-variant">工具 · {hit.subtitle}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
