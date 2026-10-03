import type { CaptureCommit } from '@/lib/api';
import type { CapturePreview, CapturePreviewRow, InboxItem, InboxPage } from '@/lib/contracts';

/**
 * 收集页的展示规则。不依赖 React。
 * 字数按 Unicode 码位计（与后端 `chars()` 一致），粘贴长度按 UTF-8 字节计（64KiB，与入库前预览一致）。
 */

export const CAPTURE_SOURCES = ['微信群', 'X', '论坛', '其他'] as const;
export type CaptureSource = (typeof CAPTURE_SOURCES)[number];

export const SOURCE_CONTEXT_LIMIT = 200;
export const NOTE_LIMIT = 500;
export const TEXT_LIMIT_BYTES = 64 * 1024;
export const PREVIEW_MIN_CHARS = 8;
export const TOO_LONG_MESSAGE = '这段文字太长，请分几次粘贴';

export type InboxFilter = 'pending' | 'ready' | 'failed' | 'archived';
export type PreviewDecision = 'too-long' | 'skip' | 'fetch';
export type StatusTone = 'blue' | 'gray' | 'red';

export type RowStatusTag = {
  tone: StatusTone;
  text: string;
  reason?: string;
};

export type InboxStatusTag = {
  tone: StatusTone;
  text: string;
  spinning: boolean;
};

const REPO_KINDS = new Set(['github_repo', 'github_repository', 'repo']);
const X_KINDS = new Set(['x', 'x_post', 'xpost']);
const WEB_KINDS = new Set(['web', 'webpage']);

function charLength(value: string): number {
  return Array.from(value).length;
}

function clipChars(value: string, limit: number): string {
  const chars = Array.from(value);
  if (chars.length <= limit) return value;
  return chars.slice(0, limit).join('');
}

/** 去掉协议和结尾斜杠，供网页链接和没有标题的收集条目显示。 */
export function displayLink(url: string): string {
  return url.trim().replace(/^https?:\/\//i, '').replace(/\/+$/, '');
}

export function describeCommit(commit: CaptureCommit): string {
  if (commit.linksFound === 0) return '没有找到链接。';
  if (commit.duplicateBatch) return '这段已经收过，没有重复写入。';
  const parts = [`新收 ${commit.created} 条`];
  if (commit.alreadyStarred) parts.push(`${commit.alreadyStarred} 条你已经收藏过了`);
  if (commit.alreadyInInbox) parts.push(`${commit.alreadyInInbox} 条已在收集箱`);
  if (commit.blocked) parts.push(`${commit.blocked} 条是本机或内网地址，没有收`);
  return `${parts.join('，')}。`;
}

export function buildSourceContext(source: string, note: string): string {
  const trimmedSource = source.trim();
  const allowed = (CAPTURE_SOURCES as readonly string[]).includes(trimmedSource) ? trimmedSource : '其他';
  const trimmedNote = note.trim();
  const combined = trimmedNote ? `${allowed} · ${trimmedNote}` : allowed;
  return clipChars(combined, SOURCE_CONTEXT_LIMIT);
}

export function previewSummary(preview: CapturePreview): string | null {
  const fresh = preview.counts.new;
  const starred = preview.counts.starred;
  const inbox = preview.counts.inbox;
  const blocked = preview.counts.blocked;
  const total = fresh + starred + inbox + blocked;
  if (total <= 0) return null;
  return `识别到 ${total} 个链接：新的 ${fresh} · 已收藏 ${starred} · 已在收集箱 ${inbox} · 不会收 ${blocked}`;
}

export function canCommit(preview: CapturePreview): boolean {
  return preview.counts.new > 0;
}

export function commitButtonLabel(newCount: number): string {
  if (newCount > 0) return `加入收集箱（${newCount}）`;
  return '加入收集箱';
}

export function rowLabel(row: Pick<CapturePreviewRow, 'kind' | 'owner' | 'name' | 'normalizedUrl'>): string {
  const owner = row.owner?.trim() ?? '';
  const name = row.name?.trim() ?? '';
  if (REPO_KINDS.has(row.kind) && owner && name) return `${owner}/${name}`;
  return displayLink(row.normalizedUrl);
}

export function rowStatusTag(row: Pick<CapturePreviewRow, 'status' | 'blockedReason'>): RowStatusTag {
  if (row.status === 'new') return { tone: 'blue', text: '新' };
  if (row.status === 'starred') return { tone: 'gray', text: '你已经收藏了' };
  if (row.status === 'inbox') return { tone: 'gray', text: '已在收集箱' };
  const reason = row.blockedReason?.trim();
  return reason ? { tone: 'red', text: '不会收', reason } : { tone: 'red', text: '不会收' };
}

/** 收集箱里的 GitHub 仓库条目 → `owner/repo`（用来显示头像）。不是仓库、或链接里认不出来就返回 null。 */
export function githubFullName(item: { itemType: string; normalizedUrl: string }): string | null {
  if (!REPO_KINDS.has(item.itemType)) return null;
  const match = /^https?:\/\/(?:www\.)?github\.com\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+?)(?:\.git)?(?:[/?#]|$)/i.exec(item.normalizedUrl.trim());
  return match ? `${match[1]}/${match[2]}` : null;
}

export function kindIcon(kind: string): string {
  if (REPO_KINDS.has(kind)) return 'inventory_2';
  if (X_KINDS.has(kind)) return 'forum';
  if (WEB_KINDS.has(kind)) return 'language';
  return 'link';
}

export function inboxFilterOptions(counts: InboxPage['counts']): { value: InboxFilter; label: string }[] {
  return [
    { value: 'pending', label: `待处理 ${counts.pending}` },
    { value: 'ready', label: `已整理 ${counts.ready}` },
    { value: 'failed', label: `失败 ${counts.failed}` },
    { value: 'archived', label: `已归档 ${counts.archived}` },
  ];
}

export function inboxStatusTag(status: InboxItem['status']): InboxStatusTag {
  if (status === 'ready') return { tone: 'blue', text: '已整理', spinning: false };
  if (status === 'failed') return { tone: 'red', text: '失败', spinning: false };
  if (status === 'archived') return { tone: 'gray', text: '已归档', spinning: false };
  // 不转圈：目前没有后台会自动处理收集箱，转圈会让人以为它正在忙。以后接上自动整理再按真实状态转。
  return { tone: 'gray', text: '待处理', spinning: false };
}

export function archiveAction(status: InboxItem['status']): { label: string; target: 'pending' | 'archived' } {
  if (status === 'archived') return { label: '恢复', target: 'pending' };
  return { label: '归档', target: 'archived' };
}

/** 失败状态只能由系统写入，撤销时不能把它设回去。 */
export function undoTarget(status: InboxItem['status']): 'pending' | 'ready' | 'archived' | null {
  if (status === 'pending' || status === 'ready' || status === 'archived') return status;
  return null;
}

export function noteError(note: string): string | null {
  if (charLength(note) > NOTE_LIMIT) return '备注最多 500 个字';
  return null;
}

export function shouldPreview(text: string): boolean {
  return charLength(text.trim()) >= PREVIEW_MIN_CHARS;
}

export function utf8ByteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

export function isTooLong(text: string): boolean {
  return utf8ByteLength(text) > TEXT_LIMIT_BYTES;
}

export function previewDecision(text: string): PreviewDecision {
  if (isTooLong(text)) return 'too-long';
  if (!shouldPreview(text)) return 'skip';
  return 'fetch';
}

export function inboxHeading(title: string, normalizedUrl: string): string {
  const trimmed = title.trim();
  if (trimmed) return trimmed;
  return displayLink(normalizedUrl);
}

export function inboxContext(sourceContext: string, platform: string): string {
  const source = sourceContext.trim();
  if (source) return source;
  return platform.trim();
}

export function inboxMeta(context: string, relativeTime: string): string {
  const when = relativeTime.trim();
  if (context && when) return `${context} · ${when}`;
  return context || when;
}

export function emptyInboxCopy(filter: InboxFilter): { title: string; description?: string } {
  if (filter === 'pending') {
    return {
      title: '收集箱是空的',
      description: '上面粘贴后点「加入收集箱」，新链接会进来',
    };
  }
  return { title: '这个分类下没有内容' };
}

export function canOpenUrl(url: string): boolean {
  return /^https?:\/\//i.test(url.trim());
}

export function appendInboxItems(current: InboxPage, next: InboxPage): InboxPage {
  const seen = new Set(current.items.map((item) => item.id));
  const items = current.items.slice();
  for (const item of next.items) {
    if (seen.has(item.id)) continue;
    seen.add(item.id);
    items.push(item);
  }
  return {
    items,
    totalCount: next.totalCount,
    counts: next.counts,
  };
}

export function shiftInboxCounts(
  counts: InboxPage['counts'],
  from: InboxItem['status'],
  to: InboxItem['status'],
): InboxPage['counts'] {
  if (from === to) return counts;
  const next: InboxPage['counts'] = { ...counts, [from]: Math.max(0, counts[from] - 1) };
  next[to] = counts[to] + 1;
  return next;
}

export function removeInboxItem(
  page: InboxPage,
  id: string,
  from: InboxItem['status'],
  to: InboxItem['status'],
): InboxPage {
  return {
    items: page.items.filter((item) => item.id !== id),
    totalCount: Math.max(0, page.totalCount - 1),
    counts: shiftInboxCounts(page.counts, from, to),
  };
}

/** 后台给的拦截原因里有「云元数据地址」这类词：换成人话。 */
export function plainBlockedReason(reason: string): string {
  const text = reason.trim();
  if (/内网|本机|云元数据|localhost|127\.0\.0\.1|metadata/i.test(text)) return '这是你电脑本机或公司内网的地址，不是公开网页，不收';
  return text;
}

export type FetchLine = { tone: 'gray' | 'red'; text: string; spinning?: boolean; retry?: boolean };

/**
 * 收集箱一条在「读取内容」上要说的话（后台联网读公开页面的标题和摘要）。
 * 读到了、不需要读、没有这项信息（旧版后台）、已归档，都没什么可说的，返回 null。
 * 还没读时：正在读就转圈，没在读（比如命令没法用）也不说话，免得让人以为它一直在忙。
 */
export function fetchLine(
  item: Pick<InboxItem, 'status' | 'fetchStatus' | 'fetchError'>,
  fetching: boolean,
): FetchLine | null {
  if (item.status === 'archived') return null;
  if (item.fetchStatus === 'failed') {
    return { tone: 'red', text: plainFetchError(item.fetchError ?? ''), retry: true };
  }
  if (item.fetchStatus === 'pending' && fetching) {
    return { tone: 'gray', text: '正在读取内容…', spinning: true };
  }
  return null;
}

/** 后台写的失败原因本来就是人话；这里只补空的、压长的。 */
export function plainFetchError(reason: string): string {
  const text = reason.replace(/\s+/g, ' ').trim();
  if (!text) return '读不到内容';
  return charLength(text) > 60 ? `${Array.from(text).slice(0, 60).join('')}…` : text;
}

/** 摘要：折成一行；没有、或和标题一模一样（没有额外信息）就不显示。 */
export function inboxExcerpt(item: Pick<InboxItem, 'excerpt' | 'title'>): string | null {
  const text = (item.excerpt ?? '').replace(/\s+/g, ' ').trim();
  if (!text) return null;
  if (text === item.title.trim()) return null;
  return text;
}

/** 还有没读过的链接吗（GitHub 仓库、已归档的不算）。 */
export function hasUnfetched(items: readonly Pick<InboxItem, 'status' | 'itemType' | 'fetchStatus'>[]): boolean {
  return items.some((item) => item.fetchStatus === 'pending' && item.status !== 'archived' && !REPO_KINDS.has(item.itemType));
}

/** 作者放在来源前面：「张三 · 微信群 · 3 天前」。 */
export function inboxByline(author: string | undefined, context: string): string {
  const name = (author ?? '').trim();
  if (name && context) return `${name} · ${context}`;
  return name || context;
}
