import { afterEach, describe, expect, it } from 'vitest';
import {
  buildReferencePack,
  commitCaptureBatch,
  confirmLibraryCard,
  copyItemForCodex,
  describeInvokeError,
  plainTechnicalError,
  getGithubAuthState,
  getLibraryDetail,
  getTodayView,
  listInbox,
  listLibraryView,
  listScenarios,
  previewCapture,
  regenerateLibraryCard,
  setInboxNote,
  setInboxStatus,
  setInvokeForTests,
  setLibraryReading,
  solveV2,
  type PackRequest,
} from './api';

type Call = { command: string; args: unknown };

function installInvoke() {
  const calls: Call[] = [];
  setInvokeForTests(async (command, args) => {
    calls.push({ command, args });
    return null;
  });
  return calls;
}

afterEach(() => {
  setInvokeForTests(null);
});

const packRequest: PackRequest = {
  title: '参考包',
  problem: '做封面',
  items: [{ repoId: 'repo-1', role: 'main', why: '对得上' }],
  localCapabilities: [{ name: '画板', oneLiner: '本地画板', path: '/tmp/draw' }],
};

describe('api 命令名和参数', () => {
  it('listScenarios 不带参数', async () => {
    const calls = installInvoke();
    await listScenarios();
    expect(calls).toEqual([{ command: 'list_scenarios', args: undefined }]);
  });

  it('listLibraryView 传 { filters }', async () => {
    const calls = installInvoke();
    const filters = { cardState: 'all' as const, limit: 1 };
    await listLibraryView(filters);
    expect(calls).toEqual([{ command: 'list_library_view', args: { filters } }]);
  });

  it('getLibraryDetail 传 { id }', async () => {
    const calls = installInvoke();
    await getLibraryDetail('repo-1');
    expect(calls).toEqual([{ command: 'get_library_detail', args: { id: 'repo-1' } }]);
  });

  it('setLibraryReading 传 { ids, group }', async () => {
    const calls = installInvoke();
    await setLibraryReading(['repo-1', 'repo-2'], 'want');
    expect(calls).toEqual([{
      command: 'set_library_reading',
      args: { ids: ['repo-1', 'repo-2'], group: 'want' },
    }]);
  });

  it('getTodayView 不带参数', async () => {
    const calls = installInvoke();
    await getTodayView();
    expect(calls).toEqual([{ command: 'get_today_view', args: undefined }]);
  });

  it('solveV2 传 { query }', async () => {
    const calls = installInvoke();
    await solveV2('做小红书封面');
    expect(calls).toEqual([{ command: 'solve_v2', args: { query: '做小红书封面' } }]);
  });

  it('buildReferencePack 传 { request, format, generatedOn }', async () => {
    const calls = installInvoke();
    await buildReferencePack(packRequest, 'markdown', '2026-10-03');
    expect(calls).toEqual([{
      command: 'build_reference_pack_v2',
      args: { request: packRequest, format: 'markdown', generatedOn: '2026-10-03' },
    }]);
  });

  it('buildReferencePack 没写日期时 generatedOn 为 null', async () => {
    const calls = installInvoke();
    await buildReferencePack(packRequest, 'json');
    expect(calls).toEqual([{
      command: 'build_reference_pack_v2',
      args: { request: packRequest, format: 'json', generatedOn: null },
    }]);
  });

  it('copyItemForCodex 传 { repoId }', async () => {
    const calls = installInvoke();
    await copyItemForCodex('repo-9');
    expect(calls).toEqual([{ command: 'copy_item_for_codex', args: { repoId: 'repo-9' } }]);
  });

  it('previewCapture 传 { text }', async () => {
    const calls = installInvoke();
    await previewCapture('https://github.com/fox/tool');
    expect(calls).toEqual([{
      command: 'preview_capture',
      args: { text: 'https://github.com/fox/tool' },
    }]);
  });

  it('listInbox 传 status、limit、offset', async () => {
    const calls = installInvoke();
    await listInbox('pending', 20, 0);
    expect(calls).toEqual([{
      command: 'list_inbox',
      args: { status: 'pending', limit: 20, offset: 0 },
    }]);
  });

  it('listInbox 不传可选项时参数是空对象', async () => {
    const calls = installInvoke();
    await listInbox();
    expect(calls).toEqual([{ command: 'list_inbox', args: {} }]);
  });

  it('setInboxStatus 传 { id, status }', async () => {
    const calls = installInvoke();
    await setInboxStatus('in-1', 'archived');
    expect(calls).toEqual([{ command: 'set_inbox_status', args: { id: 'in-1', status: 'archived' } }]);
  });

  it('setInboxNote 传 { id, note }', async () => {
    const calls = installInvoke();
    await setInboxNote('in-1', '群里看到的');
    expect(calls).toEqual([{ command: 'set_inbox_note', args: { id: 'in-1', note: '群里看到的' } }]);
  });

  it('confirmLibraryCard 传 { id }', async () => {
    const calls = installInvoke();
    await confirmLibraryCard('repo-1');
    expect(calls).toEqual([{ command: 'confirm_library_card', args: { id: 'repo-1' } }]);
  });

  it('regenerateLibraryCard 传 { id }', async () => {
    const calls = installInvoke();
    await regenerateLibraryCard('repo-1');
    expect(calls).toEqual([{ command: 'regenerate_library_card', args: { id: 'repo-1' } }]);
  });

  it('commitCaptureBatch 传 { text, sourceContext }', async () => {
    const calls = installInvoke();
    await commitCaptureBatch('一段链接', '微信群');
    expect(calls).toEqual([{
      command: 'commit_capture_batch',
      args: { text: '一段链接', sourceContext: '微信群' },
    }]);
  });

  it('getGithubAuthState 调用现有命令', async () => {
    const calls = installInvoke();
    await getGithubAuthState();
    expect(calls).toEqual([{ command: 'get_github_auth_state', args: undefined }]);
  });
});

describe('describeInvokeError', () => {
  it('识别 Command not found', () => {
    expect(describeInvokeError('Command list_library_view not found')).toBe('这个功能还没接上。');
    expect(describeInvokeError(new Error('Command get_today_view not found'))).toBe('这个功能还没接上。');
  });

  it('识别 not found、没有这个命令、命令不存在', () => {
    expect(describeInvokeError('not found')).toBe('这个功能还没接上。');
    expect(describeInvokeError('没有这个命令：list_inbox')).toBe('这个功能还没接上。');
    expect(describeInvokeError({ message: '命令不存在' })).toBe('这个功能还没接上。');
    expect(describeInvokeError('未知命令 solve_v2')).toBe('这个功能还没接上。');
  });

  it('普通中文错误原样给 Fox，空错误给一句兜底', () => {
    expect(describeInvokeError('找不到这个仓库')).toBe('找不到这个仓库');
    expect(describeInvokeError(new Error('Error: 这段文字太长，请分几次粘贴'))).toBe('这段文字太长，请分几次粘贴');
    expect(describeInvokeError('')).toBe('操作没有完成，请稍后再试。');
    expect(describeInvokeError(null)).toBe('操作没有完成，请稍后再试。');
  });
});

describe('plainTechnicalError / describeInvokeError：页面代码的英文异常翻成人话', () => {
  it('复制失败、网络失败、页面代码出错各有一句人话', () => {
    expect(plainTechnicalError("Failed to execute 'writeText' on 'Clipboard': Document is not focused.")).toBe('复制没成功，请再点一次。');
    expect(plainTechnicalError('TypeError: Load failed')).toBe('网络没连上，稍后再试一次。');
    expect(plainTechnicalError("TypeError: Cannot read properties of null (reading 'activeCount')")).toContain('页面出了点小问题');
    expect(plainTechnicalError('x is not a function')).toContain('页面出了点小问题');
  });

  it('后台给的中文提示和不认识的话原样保留', () => {
    expect(plainTechnicalError('GitHub Token 无效或权限不足')).toBeNull();
    expect(describeInvokeError('同步没有完成：网络连不上')).toBe('同步没有完成：网络连不上');
  });

  it('describeInvokeError 对技术错误也走翻译，缺命令的优先', () => {
    expect(describeInvokeError(new TypeError("Cannot read properties of undefined (reading 'x')"))).toContain('页面出了点小问题');
    expect(describeInvokeError("Failed to execute 'writeText' on 'Clipboard'")).toBe('复制没成功，请再点一次。');
    expect(describeInvokeError('Command foo not found')).toBe('这个功能还没接上。');
  });
});
