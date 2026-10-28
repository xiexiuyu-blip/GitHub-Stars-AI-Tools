import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TaskProgressEvent } from '@/types';

const mocks = vi.hoisted(() => ({
  effects: [] as (() => (() => void) | void)[],
  listen: vi.fn(),
  invoke: vi.fn(),
  invalidate: vi.fn(),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('@/hooks/use-repo-readme', () => ({ invalidateRepoReadmesForProgress: mocks.invalidate }));
vi.mock('react', async () => ({
  ...await vi.importActual('react'),
  useState: (initial: unknown) => [initial, vi.fn()],
  useRef: (initial: unknown) => ({ current: initial }),
  useMemo: (factory: () => unknown) => factory(),
  useEffect: (effect: () => (() => void) | void) => { mocks.effects.push(effect); },
}));
import { useStarsWorkspace } from './use-stars-workspace';

beforeEach(() => {
  mocks.effects = [];
  mocks.listen.mockReset(); mocks.invoke.mockReset(); mocks.invalidate.mockReset();
  // 初始状态读取停在替身 Promise；本用例只验证事件订阅，不读取任何真实数据。
  mocks.invoke.mockImplementation(() => new Promise(() => {}));
});

describe('工作区现有 task-progress 事件接入', () => {
  it('把真实监听到的完成载荷传给 README 失效处理器，卸载后取消监听', async () => {
    const callbacks = new Map<string, (event: { payload: TaskProgressEvent }) => void>();
    const unlistenProgress = vi.fn(); const unlistenStream = vi.fn();
    mocks.listen.mockImplementation((name, callback) => {
      callbacks.set(name, callback);
      return Promise.resolve(name === 'task-progress' ? unlistenProgress : unlistenStream);
    });
    useStarsWorkspace();
    const cleanup = mocks.effects[0]();
    await Promise.resolve();
    const payload: TaskProgressEvent = {
      taskId: 'readme-follow', taskType: 'readme', status: 'succeeded', stage: 'done',
      current: 2, total: 2, message: '项目说明已更新', repositoryName: null,
    };
    callbacks.get('task-progress')!({ payload });
    expect(mocks.invalidate).toHaveBeenCalledExactlyOnceWith(payload);
    cleanup?.();
    expect(unlistenProgress).toHaveBeenCalledOnce();
    expect(unlistenStream).toHaveBeenCalledOnce();
    expect(mocks.invoke.mock.calls.map((args) => args[0])).toEqual(['get_backend_status', 'get_github_auth_state']);
  });
});
