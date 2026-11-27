import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_SETTINGS } from '@/types-settings';

const invokeMock = vi.hoisted(() => vi.fn());
const harness = vi.hoisted(() => ({
  active: null as null | {
    cursor: number;
    slots: { value?: unknown; ref?: { current: unknown }; effects: (() => void)[]; cleanups: (() => void)[] };
  },
}));

const slotState = vi.hoisted(() => ({
  states: [] as unknown[],
  refs: [] as { current: unknown }[],
  didMount: false,
  cursor: 0,
  effects: [] as (() => void)[],
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('react', () => ({
  useState: <T,>(initial: T) => {
    const idx = slotState.cursor++;
    if (!(idx in slotState.states)) {
      slotState.states[idx] = initial;
    }
    return [
      slotState.states[idx] as T,
      (next: T | ((prev: T) => T)) => {
        slotState.states[idx] = typeof next === 'function' ? (next as (prev: T) => T)(slotState.states[idx] as T) : next;
      },
    ];
  },
  useRef: <T,>(initial: T) => {
    const idx = slotState.cursor++;
    if (!(idx in slotState.refs)) {
      slotState.refs[idx] = { current: initial };
    }
    return slotState.refs[idx] as { current: T };
  },
  useEffect: (effect: () => void | (() => void), deps?: unknown[]) => {
    if (!slotState.didMount && Array.isArray(deps) && deps.length === 0) {
      slotState.effects.push(() => {
        effect();
      });
    }
  },
}));

import { useSettings } from './use-settings';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function mountSettings() {
  slotState.states = [];
  slotState.refs = [];
  slotState.didMount = false;
  slotState.cursor = 0;
  slotState.effects = [];

  const render = () => {
    slotState.cursor = 0;
    const api = useSettings();
    if (!slotState.didMount) {
      slotState.didMount = true;
      slotState.effects.splice(0).forEach((fn) => fn());
    }
    return api;
  };
  return { render };
}

describe('useSettings 并发持久化顺序保护', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => null),
      setItem: vi.fn(),
      removeItem: vi.fn(),
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('连续并发修改 theme、ai 与 general 时按顺序合并最新状态，不互相覆盖', async () => {
    const savedSnapshots: Array<Record<string, unknown>> = [];
    const firstLookup = deferred<boolean>();

    invokeMock.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'get_app_settings') return DEFAULT_SETTINGS;
      if (command === 'has_ai_api_key') return firstLookup.promise;
      if (command === 'has_embedding_api_key') return false;
      if (command === 'save_app_settings') {
        savedSnapshots.push(JSON.parse(JSON.stringify(args?.settings ?? {})));
        return null;
      }
      return null;
    });

    const host = mountSettings();
    const firstRender = host.render();
    // Wait for initial loadSettings (resolve first has_ai_api_key check)
    firstLookup.resolve(false);
    await new Promise((r) => setTimeout(r, 10));

    // Prepare slow key lookup only for subsequent provider switch
    const slowAiLookup = deferred<boolean>();
    invokeMock.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'has_ai_api_key') return slowAiLookup.promise;
      if (command === 'save_app_settings') {
        savedSnapshots.push(JSON.parse(JSON.stringify(args?.settings ?? {})));
        return null;
      }
      return null;
    });

    const api = host.render();
    const p1 = api.updateAI({ provider: 'openai-compatible', model: 'deepseek-chat' });
    const p2 = api.updateTheme({ mode: 'dark' });
    const p3 = api.updateGeneral({ showWelcomeOnStartup: false });

    slowAiLookup.resolve(true);
    await Promise.all([p1, p2, p3]);

    const latest = host.render().settings;
    expect(latest.ai.provider).toBe('openai-compatible');
    expect(latest.ai.model).toBe('deepseek-chat');
    expect(latest.theme.mode).toBe('dark');
    expect(latest.general.showWelcomeOnStartup).toBe(false);

    const lastPersisted = savedSnapshots.at(-1) as typeof DEFAULT_SETTINGS;
    expect(lastPersisted.ai.provider).toBe('openai-compatible');
    expect(lastPersisted.theme.mode).toBe('dark');
    expect(lastPersisted.general.showWelcomeOnStartup).toBe(false);
  });
});
