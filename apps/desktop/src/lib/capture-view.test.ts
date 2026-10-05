import { describe, expect, it } from 'vitest';
import type { CaptureCommit } from '@/lib/api';
import type { CapturePreview, CapturePreviewRow, InboxItem, InboxPage } from '@/lib/contracts';
import {
  appendInboxItems,
  archiveAction,
  buildSourceContext,
  canCommit,
  canOpenUrl,
  commitButtonLabel,
  describeCommit,
  emptyInboxCopy,
  fetchLine,
  hasUnfetched,
  inboxByline,
  inboxContext,
  inboxExcerpt,
  inboxQuote,
  mentionLine,
  mentionSummary,
  mentionText,
  inboxFilterOptions,
  inboxHeading,
  inboxMeta,
  inboxStatusTag,
  isTooLong,
  githubFullName,
  kindIcon,
  plainBlockedReason,
  noteError,
  plainFetchError,
  previewDecision,
  previewSummary,
  removeInboxItem,
  rowLabel,
  rowStatusTag,
  shouldPreview,
  SOURCE_CONTEXT_LIMIT,
  TEXT_LIMIT_BYTES,
  undoTarget,
  utf8ByteLength,
} from './capture-view';

function commit(partial: Partial<CaptureCommit>): CaptureCommit {
  return {
    created: 0,
    skipped: 0,
    duplicateBatch: false,
    alreadyStarred: 0,
    alreadyInInbox: 0,
    blocked: 0,
    linksFound: 0,
    ...partial,
  };
}

function preview(counts: CapturePreview['counts'], rows: CapturePreviewRow[] = []): CapturePreview {
  return { rows, counts };
}

function row(partial: Partial<CapturePreviewRow>): CapturePreviewRow {
  return {
    url: 'https://example.com/docs/',
    normalizedUrl: 'https://example.com/docs/',
    kind: 'web',
    platform: 'web',
    owner: null,
    name: null,
    blockedReason: null,
    status: 'new',
    ...partial,
  };
}

function item(partial: Partial<InboxItem>): InboxItem {
  return {
    id: 'in-1',
    originalUrl: 'https://example.com/a',
    normalizedUrl: 'https://example.com/a',
    title: '示例',
    itemType: 'webpage',
    platform: 'web',
    sourceContext: '微信群',
    userNote: '',
    status: 'pending',
    repositoryId: null,
    createdAt: '2026-10-03T00:00:00.000Z',
    ...partial,
  };
}

function page(partial: Partial<InboxPage> = {}): InboxPage {
  return {
    items: [item({})],
    totalCount: 1,
    counts: { pending: 1, ready: 0, failed: 0, archived: 0 },
    ...partial,
  };
}

describe('describeCommit', () => {
  it('没有链接时直接说明，即使同时标了整段重复', () => {
    expect(describeCommit(commit({ linksFound: 0 }))).toBe('没有找到链接。');
    expect(describeCommit(commit({ linksFound: 0, duplicateBatch: true, created: 3 }))).toBe('没有找到链接。');
  });

  it('整段重复时不报条数，哪怕 created 大于 0', () => {
    expect(describeCommit(commit({
      linksFound: 4,
      duplicateBatch: true,
      created: 4,
      alreadyStarred: 1,
    }))).toBe('这段已经放进过收集箱，没有重复写入。');
  });

  it('只有新的时一句说完', () => {
    expect(describeCommit(commit({ linksFound: 4, created: 4 }))).toBe('新的 4 条。');
  });

  it('为 0 的部分省略，新的 0 仍保留', () => {
    expect(describeCommit(commit({
      linksFound: 7,
      created: 4,
      alreadyStarred: 2,
      alreadyInInbox: 0,
      blocked: 1,
    }))).toBe('新的 4 条，2 条你已经收藏过了，1 条是本机或内网地址，没放进收集箱。');
    expect(describeCommit(commit({ linksFound: 2, created: 0, alreadyInInbox: 2 }))).toBe('新的 0 条，2 条已在收集箱。');
  });

  it('四项都有时按新的、已收藏、已在收集箱、拦下的顺序', () => {
    expect(describeCommit(commit({
      linksFound: 10,
      created: 4,
      alreadyStarred: 2,
      alreadyInInbox: 1,
      blocked: 3,
    }))).toBe('新的 4 条，2 条你已经收藏过了，1 条已在收集箱，3 条是本机或内网地址，没放进收集箱。');
  });
});

describe('buildSourceContext', () => {
  it('来源加备注，并去掉首尾空白', () => {
    expect(buildSourceContext('  微信群  ', '  某某群 10/03  ')).toBe('微信群 · 某某群 10/03');
    expect(buildSourceContext('X', '帖子')).toBe('X · 帖子');
    expect(buildSourceContext('论坛', '版务')).toBe('论坛 · 版务');
  });

  it('空备注不加分隔；非法来源当成其他', () => {
    expect(buildSourceContext('微信群', '   ')).toBe('微信群');
    expect(buildSourceContext('微信群', '')).toBe('微信群');
    expect(buildSourceContext('discord', '群')).toBe('其他 · 群');
    expect(buildSourceContext('微 信群', '')).toBe('其他');
    expect(buildSourceContext('', '备注')).toBe('其他 · 备注');
  });

  it('恰好 200 字保留，超出截断且不报错', () => {
    const prefix = '微信群 · ';
    expect(Array.from(prefix).length).toBe(6);
    const exact = '注'.repeat(SOURCE_CONTEXT_LIMIT - 6);
    expect(buildSourceContext('微信群', exact)).toBe(prefix + exact);
    expect(Array.from(buildSourceContext('微信群', exact)).length).toBe(200);

    const longer = '注'.repeat(SOURCE_CONTEXT_LIMIT);
    const clipped = buildSourceContext('微信群', longer);
    expect(Array.from(clipped).length).toBe(200);
    expect(clipped.startsWith(prefix)).toBe(true);
    expect(clipped).not.toBe(prefix + longer);
  });
});

describe('previewSummary 与 canCommit', () => {
  it('按四类数字汇总，0 也写出来；一个都没有时返回 null', () => {
    expect(previewSummary(preview({ new: 0, starred: 0, inbox: 0, blocked: 0 }))).toBeNull();
    expect(previewSummary(preview({ new: 4, starred: 2, inbox: 0, blocked: 1 }))).toBe(
      '识别到 7 个链接：新的 4 · 已收藏 2 · 已在收集箱 0 · 不会放进收集箱 1',
    );
    expect(previewSummary(preview({ new: 1, starred: 0, inbox: 0, blocked: 0 }))).toBe(
      '识别到 1 个链接：新的 1 · 已收藏 0 · 已在收集箱 0 · 不会放进收集箱 0',
    );
  });

  it('只有新链接数大于 0 才能加入', () => {
    expect(canCommit(preview({ new: 4, starred: 2, inbox: 0, blocked: 1 }))).toBe(true);
    expect(canCommit(preview({ new: 0, starred: 3, inbox: 1, blocked: 1 }))).toBe(false);
    expect(canCommit(preview({ new: 0, starred: 0, inbox: 0, blocked: 0 }))).toBe(false);
    expect(commitButtonLabel(4)).toBe('加入收集箱（4）');
    expect(commitButtonLabel(0)).toBe('加入收集箱');
  });
});

describe('rowLabel 与 rowStatusTag', () => {
  it('仓库显示 owner/name，网页去掉协议和结尾斜杠', () => {
    expect(rowLabel(row({
      kind: 'github_repo',
      owner: 'fox',
      name: 'tool',
      normalizedUrl: 'https://github.com/fox/tool/',
    }))).toBe('fox/tool');
    expect(rowLabel(row({
      kind: 'github',
      owner: 'fox',
      name: 'tool',
      normalizedUrl: 'https://github.com/fox/tool/issues/1/',
    }))).toBe('github.com/fox/tool/issues/1');
    expect(rowLabel(row({ normalizedUrl: 'http://example.com/docs/' }))).toBe('example.com/docs');
    expect(rowLabel(row({
      kind: 'github_repo',
      owner: ' ',
      name: 'tool',
      normalizedUrl: 'https://github.com/tool/',
    }))).toBe('github.com/tool');
  });

  it('四种状态对应颜色和文案，拦下时带上原因', () => {
    expect(rowStatusTag(row({ status: 'new' }))).toEqual({ tone: 'blue', text: '新' });
    expect(rowStatusTag(row({ status: 'starred' }))).toEqual({ tone: 'gray', text: '你已经收藏了' });
    expect(rowStatusTag(row({ status: 'inbox' }))).toEqual({ tone: 'gray', text: '已在收集箱' });
    expect(rowStatusTag(row({ status: 'blocked', blockedReason: ' 内网地址 ' }))).toEqual({
      tone: 'red',
      text: '不会放进收集箱',
      reason: '内网地址',
    });
    expect(rowStatusTag(row({ status: 'blocked', blockedReason: '   ' }))).toEqual({
      tone: 'red',
      text: '不会放进收集箱',
    });
  });
});

describe('kindIcon', () => {
  it('仓库、X、网页用对应图标，其余用 link', () => {
    expect(kindIcon('github_repo')).toBe('inventory_2');
    expect(kindIcon('github_repository')).toBe('inventory_2');
    expect(kindIcon('repo')).toBe('inventory_2');
    expect(kindIcon('x')).toBe('forum');
    expect(kindIcon('x_post')).toBe('forum');
    expect(kindIcon('web')).toBe('language');
    expect(kindIcon('webpage')).toBe('language');
    expect(kindIcon('github')).toBe('link');
    expect(kindIcon('video')).toBe('link');
    expect(kindIcon('document')).toBe('link');
  });
});

describe('收集箱筛选、状态和归档', () => {
  it('三个筛选项带上各自的个数，没有已整理页签', () => {
    expect(inboxFilterOptions({ pending: 4, ready: 2, failed: 1, archived: 0 })).toEqual([
      { value: 'pending', label: '待处理 4' },
      { value: 'failed', label: '失败 1' },
      { value: 'archived', label: '已归档 0' },
    ]);
  });

  it('待处理不转圈，失败为红，已归档为灰，内部 ready 也不再说已整理', () => {
    expect(inboxStatusTag('pending')).toEqual({ tone: 'gray', text: '待处理', spinning: false });
    expect(inboxStatusTag('ready')).toEqual({ tone: 'gray', text: '待处理', spinning: false });
    expect(inboxStatusTag('failed')).toEqual({ tone: 'red', text: '失败', spinning: false });
    expect(inboxStatusTag('archived')).toEqual({ tone: 'gray', text: '已归档', spinning: false });
  });

  it('归档与恢复两种动作', () => {
    expect(archiveAction('pending')).toEqual({ label: '归档', target: 'archived' });
    expect(archiveAction('ready')).toEqual({ label: '归档', target: 'archived' });
    expect(archiveAction('failed')).toEqual({ label: '归档', target: 'archived' });
    expect(archiveAction('archived')).toEqual({ label: '恢复', target: 'pending' });
    expect(undoTarget('pending')).toBe('pending');
    expect(undoTarget('ready')).toBe('ready');
    expect(undoTarget('archived')).toBe('archived');
    expect(undoTarget('failed')).toBeNull();
  });

  it('空状态：待处理和其他筛选项文案不同，按页签说（待处理 / 失败 / 已归档各一句）', () => {
    expect(emptyInboxCopy('pending')).toEqual({
      title: '没有待处理的链接。',
      description: '上面粘贴后点「加入收集箱」，新链接会进来',
    });
    // 内部的 ready 界面上也叫「待处理」，没有单独的页签
    expect(emptyInboxCopy('ready').title).toBe('没有待处理的链接。');
    expect(emptyInboxCopy('failed')).toEqual({ title: '没有失败的链接。' });
    expect(emptyInboxCopy('failed').description).toBeUndefined();
    expect(emptyInboxCopy('archived')).toEqual({ title: '还没有归档的链接。' });
  });

  it('空状态不再说含糊的「这里没有内容」，也不说「收集箱是空的」（别的页签里可能还有）', () => {
    for (const filter of ['pending', 'ready', 'failed', 'archived'] as const) {
      const { title } = emptyInboxCopy(filter);
      expect(title, filter).not.toContain('这里没有内容');
      expect(title, filter).not.toContain('收集箱是空的');
    }
  });

  it('页签里没有 ready，文字里也没有已整理', () => {
    const options = inboxFilterOptions({ pending: 1, ready: 9, failed: 0, archived: 0 });
    expect(options.map((item) => item.value)).toEqual(['pending', 'failed', 'archived']);
    expect(options.map((item) => item.label).join(' ')).not.toContain('已整理');
    expect(inboxStatusTag('pending').text).toBe('待处理');
    expect(inboxStatusTag('failed').text).toBe('失败');
    expect(inboxStatusTag('archived').text).toBe('已归档');
    expect(inboxStatusTag('ready').text).not.toBe('已整理');
    expect(rowStatusTag({ status: 'starred', blockedReason: null }).text).toBe('你已经收藏了');
  });
});

describe('noteError', () => {
  it('恰好 500 字通过，501 字报错，按字而不是按字节', () => {
    expect(noteError('')).toBeNull();
    expect(noteError('字'.repeat(500))).toBeNull();
    expect(noteError('字'.repeat(501))).toBe('备注最多 500 个字');
    expect(utf8ByteLength('字'.repeat(500))).toBe(1500);
    expect(noteError('a'.repeat(500))).toBeNull();
    expect(noteError('a'.repeat(501))).toBe('备注最多 500 个字');
    expect(noteError('😀'.repeat(500))).toBeNull();
    expect(noteError('😀'.repeat(501))).toBe('备注最多 500 个字');
  });
});

describe('shouldPreview 与 isTooLong', () => {
  it('去掉首尾空白后满 8 个字才预览，7 个字不请求', () => {
    expect(shouldPreview('1234567')).toBe(false);
    expect(shouldPreview('12345678')).toBe(true);
    expect(shouldPreview('   1234567   ')).toBe(false);
    expect(shouldPreview('  12345678  ')).toBe(true);
    expect(shouldPreview('')).toBe(false);
    expect(shouldPreview('       ')).toBe(false);
    expect(shouldPreview('测'.repeat(7))).toBe(false);
    expect(shouldPreview('测'.repeat(8))).toBe(true);
    expect(shouldPreview('abc defgh')).toBe(true);
  });

  it('超过 64KiB 才算太长，中文按 3 字节计', () => {
    expect(utf8ByteLength('测')).toBe(3);
    const underCount = Math.floor((TEXT_LIMIT_BYTES - 1) / 3);
    expect(underCount).toBe(21845);
    const under = '测'.repeat(underCount);
    expect(utf8ByteLength(under)).toBe(65535);
    expect(isTooLong(under)).toBe(false);
    expect(utf8ByteLength(`${under}a`)).toBe(65536);
    expect(isTooLong(`${under}a`)).toBe(false);
    expect(utf8ByteLength(`${under}aa`)).toBe(65537);
    expect(isTooLong(`${under}aa`)).toBe(true);
    expect(utf8ByteLength('测'.repeat(underCount + 1))).toBe(65538);
    expect(isTooLong('测'.repeat(underCount + 1))).toBe(true);
    expect(isTooLong('a'.repeat(TEXT_LIMIT_BYTES))).toBe(false);
    expect(isTooLong('a'.repeat(TEXT_LIMIT_BYTES + 1))).toBe(true);
  });

  it('太长优先于「够 8 个字」，短文本不请求', () => {
    expect(previewDecision('1234567')).toBe('skip');
    expect(previewDecision('https://example.com')).toBe('fetch');
    expect(previewDecision('a'.repeat(TEXT_LIMIT_BYTES + 1))).toBe('too-long');
  });
});

describe('收集箱条目文案和列表拼接', () => {
  it('有标题用标题，没有就用规范化链接；来源优先于平台', () => {
    expect(inboxHeading('  一个工具  ', 'https://example.com/a/')).toBe('一个工具');
    expect(inboxHeading('  ', 'https://example.com/a/')).toBe('example.com/a');
    expect(inboxContext('  微信群 · 某某群  ', 'web')).toBe('微信群 · 某某群');
    expect(inboxContext('   ', ' github ')).toBe('github');
    expect(inboxMeta('微信群', '今天')).toBe('微信群 · 今天');
    expect(inboxMeta('', '今天')).toBe('今天');
  });

  it('加载更多按 id 去重，并采用新一页的总数', () => {
    const current = page({
      items: [item({ id: 'a' })],
      totalCount: 3,
    });
    const next = page({
      items: [item({ id: 'a' }), item({ id: 'b' })],
      totalCount: 3,
      counts: { pending: 2, ready: 1, failed: 0, archived: 0 },
    });
    const merged = appendInboxItems(current, next);
    expect(merged.items.map((entry) => entry.id)).toEqual(['a', 'b']);
    expect(merged.totalCount).toBe(3);
    expect(merged.counts.ready).toBe(1);
  });

  it('归档后从当前列表移走，并改两侧计数', () => {
    const current = page({
      items: [item({ id: 'a', status: 'pending' }), item({ id: 'b', status: 'pending' })],
      totalCount: 2,
      counts: { pending: 2, ready: 0, failed: 1, archived: 4 },
    });
    const next = removeInboxItem(current, 'a', 'pending', 'archived');
    expect(next.items.map((entry) => entry.id)).toEqual(['b']);
    expect(next.totalCount).toBe(1);
    expect(next.counts).toEqual({ pending: 1, ready: 0, failed: 1, archived: 5 });
  });

  it('只打开 http 链接', () => {
    expect(canOpenUrl(' https://example.com/a ')).toBe(true);
    expect(canOpenUrl('http://10.0.0.1/x')).toBe(true);
    expect(canOpenUrl('javascript:alert(1)')).toBe(false);
    expect(canOpenUrl('')).toBe(false);
  });
});

describe('githubFullName', () => {
  it('从 GitHub 仓库链接取出 owner/repo，忽略后面的路径、参数和 .git', () => {
    const repo = (url: string) => githubFullName({ itemType: 'github_repository', normalizedUrl: url });
    expect(repo('https://github.com/openai/codex')).toBe('openai/codex');
    expect(repo('https://github.com/openai/codex/issues/12?x=1')).toBe('openai/codex');
    expect(repo('https://www.github.com/a-b/c.d.git')).toBe('a-b/c.d');
    expect(repo('http://github.com/a/b#readme')).toBe('a/b');
  });

  it('不是仓库条目、或链接不是 GitHub 仓库，返回 null', () => {
    expect(githubFullName({ itemType: 'webpage', normalizedUrl: 'https://github.com/a/b' })).toBeNull();
    expect(githubFullName({ itemType: 'github_repository', normalizedUrl: 'https://example.com/a/b' })).toBeNull();
    expect(githubFullName({ itemType: 'github_repository', normalizedUrl: 'https://github.com/a' })).toBeNull();
    expect(githubFullName({ itemType: 'github_repository', normalizedUrl: '' })).toBeNull();
  });
});

describe('plainBlockedReason', () => {
  it('本机、内网、云元数据地址换成人话', () => {
    expect(plainBlockedReason('本机、内网或云元数据地址，不收')).toContain('不是公开网页');
    expect(plainBlockedReason(' 内网地址 ')).toContain('不是公开网页');
    expect(plainBlockedReason(' 内网地址 ')).toContain('不会放进收集箱');
  });

  it('别的原因原样给', () => {
    expect(plainBlockedReason(' 只收 http / https 链接 ')).toBe('只收 http / https 链接');
  });
});

describe('收集箱读取内容的状态', () => {
  it('读不到：红字原因加重试；原因为空时用兜底话', () => {
    expect(fetchLine({ status: 'pending', fetchStatus: 'failed', fetchError: '帖子不存在、已删除或需要登录才能看' }, false)).toEqual({
      tone: 'red',
      text: '帖子不存在、已删除或需要登录才能看',
      retry: true,
    });
    expect(fetchLine({ status: 'pending', fetchStatus: 'failed', fetchError: '  ' }, false)?.text).toBe('读不到内容');
    expect(fetchLine({ status: 'pending', fetchStatus: 'failed' }, false)?.text).toBe('读不到内容');
  });

  it('还没读：正在读才转圈，没在读不说话', () => {
    expect(fetchLine({ status: 'pending', fetchStatus: 'pending' }, true)).toEqual({ tone: 'gray', text: '正在读取内容…', spinning: true });
    expect(fetchLine({ status: 'pending', fetchStatus: 'pending' }, false)).toBeNull();
  });

  it('读到了、不需要读、旧版后台没有这项、已归档：都不说话', () => {
    expect(fetchLine({ status: 'pending', fetchStatus: 'done' }, true)).toBeNull();
    expect(fetchLine({ status: 'pending', fetchStatus: 'skipped' }, true)).toBeNull();
    expect(fetchLine({ status: 'pending' }, true)).toBeNull();
    expect(fetchLine({ status: 'archived', fetchStatus: 'failed', fetchError: '读不到' }, false)).toBeNull();
  });

  it('失败原因太长就截断', () => {
    const long = '原'.repeat(100);
    const text = plainFetchError(long);
    expect(Array.from(text).length).toBe(61);
    expect(text.endsWith('…')).toBe(true);
    expect(plainFetchError('第一行\n第二行')).toBe('第一行 第二行');
  });

  it('摘要折成一行，空的或和标题相同的不显示', () => {
    expect(inboxExcerpt({ title: '标题', excerpt: '第一行\n\n  第二行' })).toBe('第一行 第二行');
    expect(inboxExcerpt({ title: '标题', excerpt: '' })).toBeNull();
    expect(inboxExcerpt({ title: '标题', excerpt: ' 标题 ' })).toBeNull();
    expect(inboxExcerpt({ title: '标题' })).toBeNull();
  });

  it('有没有还没读的链接：GitHub 仓库和已归档的不算', () => {
    expect(hasUnfetched([{ status: 'pending', itemType: 'webpage', fetchStatus: 'pending' }])).toBe(true);
    expect(hasUnfetched([{ status: 'pending', itemType: 'github_repository', fetchStatus: 'pending' }])).toBe(false);
    expect(hasUnfetched([{ status: 'archived', itemType: 'webpage', fetchStatus: 'pending' }])).toBe(false);
    expect(hasUnfetched([{ status: 'pending', itemType: 'webpage', fetchStatus: 'done' }])).toBe(false);
    expect(hasUnfetched([{ status: 'pending', itemType: 'webpage' }])).toBe(false);
    expect(hasUnfetched([])).toBe(false);
  });

  it('作者放在来源前面', () => {
    expect(inboxByline('张三', '微信群 · 3 天前')).toBe('张三 · 微信群 · 3 天前');
    expect(inboxByline(undefined, '微信群')).toBe('微信群');
    expect(inboxByline(' 张三 ', '')).toBe('张三');
    expect(inboxByline('', '')).toBe('');
  });
});

describe('收集箱条目的原话', () => {
  it('折成一行；没有或全是空白就不显示', () => {
    expect(inboxQuote({ quote: '这个能自动出封面，\n试试  看' })).toBe('这个能自动出封面， 试试 看');
    expect(inboxQuote({ quote: '' })).toBeNull();
    expect(inboxQuote({ quote: '  \n ' })).toBeNull();
    expect(inboxQuote({})).toBeNull();
  });
});

describe('文章里提到的 GitHub 项目', () => {
  const mentions = [
    { fullName: 'a/one', starred: true, inInbox: false },
    { fullName: 'b/two', starred: false, inInbox: true },
    { fullName: 'c/three', starred: false, inInbox: false },
    { fullName: 'd/four', starred: true, inInbox: true },
    { fullName: 'e/five', starred: false, inInbox: false },
  ];

  it('没有提到就是 null（旧版后台没有这一项也一样）', () => {
    expect(mentionSummary(undefined)).toBeNull();
    expect(mentionSummary([])).toBeNull();
  });

  it('先算已收藏，再算已在收集箱，剩下的是新的，并按出现顺序留下名字', () => {
    expect(mentionSummary(mentions)).toEqual({
      total: 5,
      starred: 2,
      inInbox: 1,
      fresh: 2,
      freshNames: ['c/three', 'e/five'],
    });
  });

  it('一行说明只写有数的那几类', () => {
    expect(mentionLine(mentionSummary(mentions)!)).toBe('文中提到 5 个 GitHub 项目：2 个已收藏，1 个已在收集箱，2 个是新的');
    expect(mentionLine(mentionSummary([{ fullName: 'a/b', starred: true, inInbox: false }])!)).toBe('文中提到 1 个 GitHub 项目：1 个已收藏');
    expect(mentionLine(mentionSummary([{ fullName: 'a/b', starred: false, inInbox: false }])!)).toBe('文中提到 1 个 GitHub 项目：1 个是新的');
  });

  it('收进收集箱的文字：每个新项目一行网址', () => {
    expect(mentionText(mentionSummary(mentions)!)).toBe('https://github.com/c/three\nhttps://github.com/e/five');
    expect(mentionText(mentionSummary([{ fullName: 'a/b', starred: true, inInbox: false }])!)).toBe('');
  });
});
