import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type ExtractedLink = { url: string; kind: string; owner: string | null; name: string | null };
type Commit = { created: number; skipped: number; duplicateBatch: boolean };

export function CapturePage() {
  const [text, setText] = useState('');
  const [links, setLinks] = useState<ExtractedLink[]>([]);
  const [message, setMessage] = useState<string | null>(null);

  return (
    <section className="flex h-full flex-col gap-4 overflow-auto p-6">
      <h2 className="text-2xl font-semibold">收集</h2>
      <p className="max-w-2xl text-sm text-on-surface-variant">粘贴群聊或帖子。同一段再粘一次不会重复入库。X 和内网地址会被拦住。</p>
      <textarea className="min-h-40 rounded-2xl border p-4 text-sm" value={text} onChange={(event) => setText(event.target.value)} placeholder="粘贴一段聊天记录" />
      <div className="flex gap-2">
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke<ExtractedLink[]>('extract_links_from_text', { text }).then(setLinks)}>预览链接</button>
        <button className="rounded-full bg-primary px-4 py-2 text-sm text-on-primary" type="button" onClick={() => void invoke<Commit>('commit_capture_batch', { text, sourceContext: '手动粘贴' }).then((result) => setMessage(result.duplicateBatch ? '这段已经收过，没有重复写入' : `新收 ${result.created} 条，跳过 ${result.skipped} 条`))}>加入收集箱</button>
      </div>
      {message ? <p className="text-sm">{message}</p> : null}
      <ul className="space-y-2 text-sm">
        {links.map((link) => <li key={link.url} className="rounded-xl border px-3 py-2">{link.kind} · {link.owner && link.name ? `${link.owner}/${link.name}` : link.url}</li>)}
      </ul>
    </section>
  );
}
