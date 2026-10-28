import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TaskProgressEvent } from '@/types';
import type { RepositoryReadmeText } from '@/lib/api';

const harness = vi.hoisted(() => ({
  active: null as null | {
    cursor: number;
    slots: { value?: unknown; deps?: unknown[]; cleanup?: () => void }[];
    effects: (() => void)[];
  },
}));
const getReadme = vi.hoisted(() => vi.fn());
vi.mock('@/lib/api', () => ({ getRepositoryReadme: getReadme }));
vi.mock('react', () => ({
  useState: (initial: unknown) => {
    const active = harness.active!;
    const slot = active.slots[active.cursor++] ??= { value: initial };
    return [slot.value, (value: unknown) => { slot.value = value; }];
  },
  useEffect: (effect: () => (() => void) | void, deps: unknown[]) => {
    const active = harness.active!;
    const slot = active.slots[active.cursor++] ??= {};
    if (!slot.deps || deps.some((value, index) => !Object.is(value, slot.deps?.[index]))) {
      active.effects.push(() => { slot.cleanup?.(); slot.cleanup = effect() || undefined; });
      slot.deps = deps;
    }
  },
}));
import { invalidateRepoReadmes, invalidateRepoReadmesForProgress, useRepoReadme } from './use-repo-readme';

const cleanups: (() => void)[] = [];
function mount(id: string | null) {
  const instance: NonNullable<typeof harness.active> = { cursor: 0, slots: [], effects: [] };
  function render(nextId = id) {
    id = nextId; instance.cursor = 0; harness.active = instance;
    useRepoReadme(id);
    instance.effects.splice(0).forEach((effect) => effect());
  }
  const unmount = () => { instance.slots.forEach((slot) => { slot.cleanup?.(); slot.cleanup = undefined; }); };
  cleanups.push(unmount);
  render();
  return { render, unmount, entry: () => instance.slots[0].value as ReturnType<typeof useRepoReadme> };
}
const readme = (rawMarkdown: string): RepositoryReadmeText => ({ rawMarkdown, sourcePath: 'README.md' });
function deferred() {
  let resolve!: (value: RepositoryReadmeText | null) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<RepositoryReadmeText | null>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const settle = async () => { await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); };
function progress(taskId: string, status = 'succeeded'): TaskProgressEvent {
  return { taskId, status, taskType: 'readme', stage: 'done', current: 1, total: 1, message: '', repositoryName: null };
}
beforeEach(() => { invalidateRepoReadmes(); getReadme.mockReset(); });
afterEach(() => { cleanups.splice(0).forEach((cleanup) => cleanup()); harness.active = null; });

describe('README 缓存与打开中的详情', () => {
  it('同仓库共享读取，重复渲染不发请求，空 id 不读取', async () => {
    getReadme.mockResolvedValue(readme('旧正文'));
    const first = mount('a'); const second = mount('a'); mount(null);
    await settle(); first.render(); second.render();
    expect(getReadme).toHaveBeenCalledTimes(1);
    expect(first.entry()).toEqual({ state: 'ready', readme: readme('旧正文') });
    expect(second.entry()).toEqual(first.entry());
  });

  it.each([null, readme('旧正文')])('缓存为 %s 时，README更新自动刷新已打开详情', async (old) => {
    getReadme.mockResolvedValueOnce(old).mockResolvedValueOnce(readme('新正文'));
    const page = mount('a'); await settle();
    expect(page.entry()).toEqual({ state: 'ready', readme: old });
    invalidateRepoReadmesForProgress(progress('readme-follow'));
    expect(page.entry()).toEqual({ state: 'loading' });
    await settle();
    expect(page.entry()).toEqual({ state: 'ready', readme: readme('新正文') });
    expect(getReadme).toHaveBeenCalledTimes(2);
    page.render(); expect(getReadme).toHaveBeenCalledTimes(2);
  });

  it('只失效指定仓库，另一详情保持缓存；关闭后不自动读取，重开时读新值', async () => {
    getReadme.mockResolvedValue(readme('正文'));
    const a = mount('a'); const b = mount('b'); await settle();
    a.unmount(); invalidateRepoReadmes('a'); await settle();
    expect(getReadme).toHaveBeenCalledTimes(2);
    b.render(); mount('a'); await settle();
    expect(getReadme.mock.calls.map((args) => args[0])).toEqual(['a', 'b', 'a']);
  });

  it('失败可重试，旧失败不能删新缓存或覆盖新正文', async () => {
    const old = deferred();
    getReadme.mockReturnValueOnce(old.promise).mockResolvedValueOnce(readme('新正文'));
    const page = mount('a');
    invalidateRepoReadmes('a'); await settle();
    old.reject(new Error('旧请求失败')); await settle();
    expect(page.entry()).toEqual({ state: 'ready', readme: readme('新正文') });
    mount('a'); await settle(); expect(getReadme).toHaveBeenCalledTimes(2);
    getReadme.mockRejectedValueOnce(new Error('临时失败'));
    invalidateRepoReadmes('a'); await settle();
    expect(page.entry()).toEqual({ state: 'error', message: '临时失败' });
    getReadme.mockResolvedValueOnce(readme('重试成功'));
    const retry = mount('a'); await settle();
    expect(retry.entry()).toEqual({ state: 'ready', readme: readme('重试成功') });
  });

  it('失效之前或切换仓库之前的旧响应都不能覆盖当前正文', async () => {
    const old = deferred();
    getReadme.mockReturnValueOnce(old.promise).mockResolvedValueOnce(readme('新正文'));
    const page = mount('a'); invalidateRepoReadmes('a'); await settle();
    old.resolve(readme('旧正文')); await settle();
    expect(page.entry()).toEqual({ state: 'ready', readme: readme('新正文') });
    const pending = deferred(); getReadme.mockReturnValueOnce(pending.promise);
    page.render('b'); page.render('a'); await settle();
    pending.resolve(readme('另一个仓库')); await settle();
    expect(page.entry()).toEqual({ state: 'ready', readme: readme('新正文') });
  });

  it('只响应相关任务收尾；同步之后的README跟进和部分成功也分别刷新', async () => {
    getReadme.mockResolvedValue(readme('正文'));
    mount('a'); await settle();
    for (const event of [progress('sync-stars', 'running'), progress('readme-follow', 'running'), progress('generate-ai-document'), progress('sync-stars', 'failed')]) {
      invalidateRepoReadmesForProgress(event);
    }
    expect(getReadme).toHaveBeenCalledTimes(1);
    for (const event of [progress('sync-stars'), progress('readme-follow'), progress('fetch-readmes', 'partial'), progress('fetch-repository-readme'), progress('readme-follow', 'failed')]) {
      const before = getReadme.mock.calls.length;
      invalidateRepoReadmesForProgress(event); await settle();
      expect(getReadme).toHaveBeenCalledTimes(before + 1);
    }
  });
});
