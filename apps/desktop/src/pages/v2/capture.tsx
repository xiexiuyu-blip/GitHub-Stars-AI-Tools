import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type ExtractedLink = {
  url: string;
  kind: string;
  owner: string | null;
  name: string | null;
};

export function CapturePage() {
  const [text, setText] = useState('');
  const [links, setLinks] = useState<ExtractedLink[]>([]);
  const [error, setError] = useState<string | null>(null);

  async function extract() {
    try {
      const result = await invoke<ExtractedLink[]>('extract_links_from_text', { text });
      setLinks(result);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }

  return (
    <section className="flex h-full min-h-0 flex-col gap-4 overflow-auto p-6">
      <header>
        <h2 className="text-2xl font-semibold">收集</h2>
        <p className="mt-1 max-w-2xl text-sm text-on-surface-variant">
          把群聊或帖子整段粘进来。现在只抽出链接并去重，还不会自动抓 README。
        </p>
      </header>
      <textarea
        className="min-h-40 rounded-2xl border border-outline-variant bg-surface p-4 text-sm"
        placeholder="粘贴一段聊天记录"
        value={text}
        onChange={(event) => setText(event.target.value)}
      />
      <button className="w-fit rounded-full bg-primary px-4 py-2 text-sm text-on-primary" onClick={() => void extract()} type="button">
        抽出链接
      </button>
      {error ? <p className="text-sm text-error">{error}</p> : null}
      <ul className="space-y-2">
        {links.map((link) => (
          <li key={link.url} className="rounded-xl border border-outline-variant/40 px-3 py-2 text-sm">
            <span className="mr-2 rounded-full bg-surface-container px-2 py-0.5 text-xs">{link.kind}</span>
            {link.owner && link.name ? `${link.owner}/${link.name}` : link.url}
          </li>
        ))}
      </ul>
    </section>
  );
}
