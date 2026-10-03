import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type ExtractedLink = {
  url: string;
  normalizedUrl: string;
  kind: string;
  platform: string;
  owner: string | null;
  name: string | null;
  blockedReason: string | null;
};
type Commit = {
  created: number;
  skipped: number;
  duplicateBatch: boolean;
  alreadyStarred: number;
  alreadyInInbox: number;
  blocked: number;
  linksFound: number;
};

const KIND_LABEL: Record<string, string> = {
  github_repo: 'GitHub 仓库',
  github: 'GitHub 页面',
  x_post: 'X 帖子',
  x: 'X 页面',
  web: '网页',
};

function describeCommit(result: Commit): string {
  if (result.linksFound === 0) return '没有找到链接。';
  if (result.duplicateBatch) return '这段已经收过，没有重复写入。';
  const parts = [`新收 ${result.created} 条`];
  if (result.alreadyStarred) parts.push(`${result.alreadyStarred} 条已在你的 Stars 里`);
  if (result.alreadyInInbox) parts.push(`${result.alreadyInInbox} 条已在收集箱`);
  if (result.blocked) parts.push(`${result.blocked} 条是本机或内网地址，已拦下`);
  return `${parts.join('，')}。`;
}

export function CapturePage() {
  const [text, setText] = useState('');
  const [links, setLinks] = useState<ExtractedLink[]>([]);
  const [message, setMessage] = useState<string | null>(null);

  return (
    <section className="flex h-full flex-col gap-4 overflow-auto p-6">
      <h2 className="text-2xl font-semibold">收集</h2>
      <p className="max-w-2xl text-sm text-on-surface-variant">粘贴群聊或帖子，自动找出里面的链接。同一段再粘一次不会重复入库；已在你 Stars 里的仓库会跳过；本机和内网地址会被拦下。</p>
      <textarea className="min-h-40 rounded-2xl border p-4 text-sm" value={text} onChange={(event) => setText(event.target.value)} placeholder="粘贴一段聊天记录" />
      <div className="flex gap-2">
        <button className="rounded-full border px-4 py-2 text-sm" type="button" onClick={() => void invoke<ExtractedLink[]>('extract_links_from_text', { text }).then(setLinks).catch((reason: unknown) => setMessage(String(reason)))}>预览链接</button>
        <button className="rounded-full bg-primary px-4 py-2 text-sm text-on-primary" type="button" onClick={() => void invoke<Commit>('commit_capture_batch', { text, sourceContext: '手动粘贴' }).then((result) => setMessage(describeCommit(result))).catch((reason: unknown) => setMessage(String(reason)))}>加入收集箱</button>
      </div>
      {message ? <p className="text-sm">{message}</p> : null}
      <ul className="space-y-2 text-sm">
        {links.map((link) => (
          <li key={link.normalizedUrl} className="rounded-xl border px-3 py-2">
            {KIND_LABEL[link.kind] ?? link.kind} · {link.owner && link.name ? `${link.owner}/${link.name}` : link.url}
            {link.blockedReason ? <span className="text-red-700"> · 将被拦下：{link.blockedReason}</span> : null}
          </li>
        ))}
      </ul>
    </section>
  );
}
