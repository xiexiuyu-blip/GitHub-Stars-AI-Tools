import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

export function PacksPage(props: { question: string }) {
  const [content, setContent] = useState('');
  const [format, setFormat] = useState<'markdown' | 'json' | 'agents'>('markdown');
  const [destination, setDestination] = useState('/tmp/fox-stars-lab-pack.md');
  const [message, setMessage] = useState<string | null>(null);

  function generate() {
    void invoke<string>('create_reference_pack', {
      title: '给 Codex 的参考包',
      problem: props.question,
      question: props.question,
      format,
    }).then(setContent).catch((reason: unknown) => setMessage(String(reason)));
  }

  return (
    <section className="flex h-full flex-col gap-4 overflow-auto p-6">
      <h2 className="text-2xl font-semibold">参考包</h2>
      <p className="text-sm text-on-surface-variant">当前问题：{props.question || '还没从找方案带过来'}</p>
      <div className="flex flex-wrap gap-2">
        {(['markdown', 'agents', 'json'] as const).map((item) => (
          <button key={item} className={`rounded-full px-3 py-1 text-xs ${format === item ? 'bg-primary text-on-primary' : 'border'}`} type="button" onClick={() => setFormat(item)}>{item === 'agents' ? 'AGENTS.md' : item}</button>
        ))}
      </div>
      <button className="w-fit rounded-full bg-primary px-4 py-2 text-sm text-on-primary" type="button" onClick={generate}>生成</button>
      <pre className="whitespace-pre-wrap rounded-2xl border p-4 text-sm">{content}</pre>
      <div className="flex flex-wrap gap-2">
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void navigator.clipboard.writeText(content)}>复制</button>
        <input className="min-w-72 rounded-full border px-4 py-2 text-sm" value={destination} onChange={(event) => setDestination(event.target.value)} />
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => {
          void invoke('export_library_markdown', { destination }).then(() => setMessage(`资料库 Markdown 已写到 ${destination}`)).catch((reason: unknown) => setMessage(String(reason)));
        }}>导出资料库到这个路径</button>
      </div>
      {message ? <p className="text-sm text-on-surface-variant">{message}</p> : null}
    </section>
  );
}
