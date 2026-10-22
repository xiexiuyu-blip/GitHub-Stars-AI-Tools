import { afterEach, describe, expect, it } from 'vitest';
import {
  buildReferencePack,
  classifyUpdaterError,
  commitCaptureBatch,
  confirmLibraryCard,
  copyItemForCodex,
  describeInvokeError,
  describeUpdateStatus,
  describeUpdaterError,
  plainTechnicalError,
  getAiAutomation,
  getAiAutomationWithRecent,
  getAiQueueStatus,
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
  UPDATE_FAILED_FALLBACK,
  UPDATE_INSTALL_FAILED_FALLBACK,
  UPDATE_RELAUNCH_FAILED,
  UPDATE_WINDOW_STILL_OPEN,
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

  it('regenerateLibraryCard 传 { id, confirmed: true }', async () => {
    const calls = installInvoke();
    await regenerateLibraryCard('repo-1', true);
    expect(calls).toEqual([{ command: 'regenerate_library_card', args: { id: 'repo-1', confirmed: true } }]);
  });

  it('regenerateLibraryCard 没确认时传 confirmed: false', async () => {
    const calls = installInvoke();
    await regenerateLibraryCard('repo-1', false);
    expect(calls).toEqual([{ command: 'regenerate_library_card', args: { id: 'repo-1', confirmed: false } }]);
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

  it('识别没有这个命令、命令不存在、未知命令', () => {
    expect(describeInvokeError('没有这个命令：list_inbox')).toBe('这个功能还没接上。');
    expect(describeInvokeError({ message: '命令不存在' })).toBe('这个功能还没接上。');
    expect(describeInvokeError('未知命令 solve_v2')).toBe('这个功能还没接上。');
  });

  it('裸 not found 不是命令缺失，原样留下', () => {
    expect(describeInvokeError('not found')).toBe('not found');
  });

  it('业务 404 和 GitHub 找不到，不说功能还没接上', () => {
    expect(describeInvokeError('404 Not Found')).toBe('404 Not Found');
    expect(describeInvokeError('Repository not found')).toBe('Repository not found');
    expect(describeInvokeError('GitHub API 404: Not Found')).not.toContain('这个功能还没接上');
  });

  it('网络错误仍说网络没连上', () => {
    expect(describeInvokeError('Failed to fetch')).toBe('网络没连上，稍后再试一次。');
    expect(describeInvokeError('TypeError: NetworkError when attempting to fetch resource.')).toBe('网络没连上，稍后再试一次。');
  });

  it('普通中文错误原样给 Fox，空错误给一句兜底', () => {
    expect(describeInvokeError('找不到这个工具')).toBe('找不到这个工具');
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
    expect(plainTechnicalError('GitHub 连接无效或权限不足')).toBeNull();
    expect(describeInvokeError('同步没有完成：网络连不上')).toBe('同步没有完成：网络连不上');
  });

  it('describeInvokeError 对技术错误也走翻译，缺命令的优先', () => {
    expect(describeInvokeError(new TypeError("Cannot read properties of undefined (reading 'x')"))).toContain('页面出了点小问题');
    expect(describeInvokeError("Failed to execute 'writeText' on 'Clipboard'")).toBe('复制没成功，请再点一次。');
    expect(describeInvokeError('Command foo not found')).toBe('这个功能还没接上。');
  });
});

describe('最近 24 小时整理了几个：读现成命令 get_ai_queue_status', () => {
  const automation = {
    enabled: true,
    dailyBudgetTokens: 100_000,
    paused: false,
    pauseReason: null,
    tokens24h: 40_000,
    budgetRemaining: 60_000,
    queued: 2,
    running: 1,
    failed: 0,
    providerConfigured: true,
    model: 'gpt-x',
  };
  const queue = {
    state: 'running',
    pauseReason: null,
    queued: 2,
    running: 1,
    succeeded24h: 513,
    failed: 0,
    tokens24h: 40_000,
    dailyBudget: 100_000,
    budgetRemaining: 60_000,
    unlimited: false,
  };

  it('getAiQueueStatus 调 get_ai_queue_status，不带参数', async () => {
    const calls = installInvoke();
    await getAiQueueStatus();
    expect(calls).toEqual([{ command: 'get_ai_queue_status', args: undefined }]);
  });

  it('getAiAutomation 仍然只调 get_ai_automation（别的页面还在用它）', async () => {
    const calls = installInvoke();
    await getAiAutomation();
    expect(calls).toEqual([{ command: 'get_ai_automation', args: undefined }]);
  });

  it('getAiAutomationWithRecent 两个现成命令各调一次，把 succeeded24h 合进状态', async () => {
    const calls: string[] = [];
    setInvokeForTests(async (command) => {
      calls.push(command);
      return command === 'get_ai_queue_status' ? queue : automation;
    });
    const merged = await getAiAutomationWithRecent();
    expect(calls.sort()).toEqual(['get_ai_automation', 'get_ai_queue_status']);
    expect(merged.succeeded24h).toBe(513);
    expect(merged.tokens24h).toBe(40_000);
    expect(merged.queued).toBe(2);
  });

  it('队列状态读不到（命令不存在、出错）：状态照常返回，个数是 null，不编数字', async () => {
    setInvokeForTests(async (command) => {
      if (command === 'get_ai_queue_status') throw new Error('Command get_ai_queue_status not found');
      return automation;
    });
    const merged = await getAiAutomationWithRecent();
    expect(merged.succeeded24h).toBeNull();
    expect(merged.providerConfigured).toBe(true);
    expect(merged.budgetRemaining).toBe(60_000);
  });

  it('自动整理状态本身读不到仍然抛错，和 getAiAutomation 一样', async () => {
    setInvokeForTests(async (command) => {
      if (command === 'get_ai_automation') throw new Error('没读出来');
      return queue;
    });
    await expect(getAiAutomationWithRecent()).rejects.toThrow('没读出来');
  });
});

describe('describeUpdaterError：更新失败按原因分开说，每句写清下一步', () => {
  it.each([
    'error sending request for url (https://github.com/x/releases/latest/download/latest.json)',
    'Failed to fetch',
    'request timed out',
    'dns error: failed to lookup address information: nodename nor servname provided',
    'connection refused',
  ])('断网 / 连不上：%s', (raw) => {
    // 第一条的报错里带着 latest.json 的网址，但它是网络失败，不是「文件没找到」
    expect(classifyUpdaterError(new Error(raw))).toBe('network');
  });

  it('网络问题说「没连上更新服务器」，不说文件没找到、也不说校验没通过', () => {
    const message = describeUpdaterError(new Error('request timed out'));
    expect(message).toBe('没连上更新服务器。检查一下网络，稍后再试一次。');
    expect(message).not.toContain('文件没找到');
    expect(message).not.toContain('校验');
  });

  it('断网时即使报错里碰巧带 not found（域名解析失败），也算网络问题', () => {
    expect(classifyUpdaterError('dns error: Name or service not found')).toBe('network');
  });

  it.each([
    'Could not fetch a valid release JSON from the remote',
    'latest.json is missing',
    'HTTP status client error (404 Not Found) for url (https://example.com/latest.json)',
    '404',
  ])('发布文件没找到：%s', (raw) => {
    expect(classifyUpdaterError(raw)).toBe('not-found');
    const message = describeUpdaterError(raw);
    expect(message).toContain('更新文件没找到');
    expect(message).toContain('稍后再试');
    expect(message).toContain('手动下载');
    expect(message).not.toContain('latest.json');
  });

  it.each([
    'signature verification failed',
    'The signature was invalid',
    'failed to decode pubkey',
  ])('校验没通过：%s', (raw) => {
    expect(classifyUpdaterError(raw)).toBe('signature');
    const message = describeUpdaterError(raw);
    expect(message).toContain('更新文件校验没通过');
    expect(message).toContain('没有安装');
    expect(message).toContain('手动下载');
    expect(message).not.toContain('文件没找到');
  });

  it('没权限写进应用所在的位置：说清放进「应用程序」文件夹', () => {
    expect(classifyUpdaterError('Permission denied (os error 13)')).toBe('permission');
    expect(describeUpdaterError('Operation not permitted')).toContain('「应用程序」');
  });

  it('认不出原因时不乱说，只说这次没成、不影响资料、下一步', () => {
    expect(classifyUpdaterError(new Error('something odd'))).toBe('unknown');
    expect(describeUpdaterError(new Error('something odd'))).toBe(UPDATE_FAILED_FALLBACK);
    expect(describeUpdaterError('something odd', 'install')).toBe(UPDATE_INSTALL_FAILED_FALLBACK);
    expect(describeUpdaterError(null)).toBe(UPDATE_FAILED_FALLBACK);
    for (const text of [UPDATE_FAILED_FALLBACK, UPDATE_INSTALL_FAILED_FALLBACK]) {
      expect(text).toContain('不影响你的资料');
      expect(text).toContain('把这条截图发给开发者');
      expect(text).not.toContain('文件没找到');
    }
  });

  it('同一种原因，检查和安装说的一样（只有认不出原因时才分「检查」「更新」）', () => {
    expect(describeUpdaterError('request timed out', 'check')).toBe(describeUpdaterError('request timed out', 'install'));
  });

  it('重启失败是另一句话：更新已经装好了，下一步是重启，不是重试更新', () => {
    expect(UPDATE_RELAUNCH_FAILED).toContain('更新已经装好了');
    expect(UPDATE_RELAUNCH_FAILED).toContain('重启');
    expect(UPDATE_RELAUNCH_FAILED).not.toContain('文件没找到');
    expect(UPDATE_RELAUNCH_FAILED).not.toContain('校验');
    expect(UPDATE_WINDOW_STILL_OPEN).toContain('更新已安装');
    expect(UPDATE_RELAUNCH_FAILED).not.toBe(UPDATE_WINDOW_STILL_OPEN);
  });
});

describe('describeUpdateStatus：应用更新面板里状态那一句', () => {
  const base = { status: 'idle', availableVersion: null, currentVersion: '1.5.3', errorMessage: null };

  it('已是最新版本只在读到当前版本时说；没读到版本不下这个结论', () => {
    expect(describeUpdateStatus({ ...base, status: 'not-available' })).toBe('已是最新版本。');
    const unknown = describeUpdateStatus({ ...base, status: 'not-available', currentVersion: '' });
    expect(unknown).toBe('还没读到当前版本，先不判断是不是最新。');
    expect(unknown).not.toContain('已是最新');
  });

  it('出错时用具体原因那一句；没有具体原因才用兜底', () => {
    expect(describeUpdateStatus({ ...base, status: 'error', errorMessage: '没连上更新服务器。检查一下网络，稍后再试一次。' })).toBe(
      '没连上更新服务器。检查一下网络，稍后再试一次。',
    );
    expect(describeUpdateStatus({ ...base, status: 'error' })).toBe(UPDATE_FAILED_FALLBACK);
  });

  it('装好但没能重启：显示重启失败那一句，不显示更新失败', () => {
    expect(describeUpdateStatus({ ...base, status: 'installed', errorMessage: UPDATE_RELAUNCH_FAILED })).toBe(UPDATE_RELAUNCH_FAILED);
    expect(describeUpdateStatus({ ...base, status: 'installed' })).toBe('更新已安装，正在准备重启应用。');
  });

  it('其余状态各有一句', () => {
    expect(describeUpdateStatus({ ...base, status: 'checking' })).toBe('正在检查更新…');
    expect(describeUpdateStatus({ ...base, status: 'available', availableVersion: '1.6.0' })).toBe('发现新版本 1.6.0，可查看说明后立即安装。');
    expect(describeUpdateStatus({ ...base, status: 'downloading' })).toContain('请保持应用打开');
    expect(describeUpdateStatus({ ...base, status: 'relaunching' })).toContain('正在重启');
    expect(describeUpdateStatus(base)).toContain('静默检查更新');
  });
});
