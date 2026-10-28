import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactElement, ReactNode } from 'react';

// 无 DOM 测试依赖：执行组件自身的状态、effect 和事件处理器，焦点对象用替身。
const hooks = vi.hoisted(() => ({
  slots: [] as { value?: unknown; deps?: unknown[]; cleanup?: () => void }[],
  cursor: 0,
  effects: [] as (() => void)[],
}));
const invokeMock = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/path', () => ({ downloadDir: vi.fn(), join: vi.fn() }));
vi.mock('react', async () => ({
  ...await vi.importActual('react'),
  useId: () => 'palette-options',
  useState: (initial: unknown) => {
    const index = hooks.cursor++;
    const slot = hooks.slots[index] ??= { value: typeof initial === 'function' ? initial() : initial };
    return [slot.value, (next: unknown) => { slot.value = typeof next === 'function' ? next(slot.value) : next; }];
  },
  useRef: (initial: unknown) => (hooks.slots[hooks.cursor++] ??= { value: { current: initial } }).value,
  useEffect: (effect: () => (() => void) | void, deps: unknown[]) => {
    const slot = hooks.slots[hooks.cursor++] ??= {};
    if (!slot.deps || deps.some((value, index) => !Object.is(value, slot.deps?.[index]))) {
      hooks.effects.push(() => { slot.cleanup?.(); slot.cleanup = effect() || undefined; });
      slot.deps = deps;
    }
  },
}));
import { CommandPalette } from './command-palette';

type Element = ReactElement<Record<string, any>>;
function elements(node: ReactNode): Element[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!node || typeof node !== 'object' || !('props' in node)) return [];
  const element = node as Element;
  return [element, ...elements(element.props.children)];
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const hit = (id: string, kind = 'repository') => ({ id, kind, title: `fox/${id}`, subtitle: id });

function mount() {
  const documentStub = { activeElement: null as unknown, getElementById: vi.fn(() => ({ scrollIntoView: vi.fn() })) };
  const element = () => ({ isConnected: true, focus: vi.fn(function (this: unknown) { documentStub.activeElement = this; }) });
  const opener = element();
  const input = element();
  const close = element();
  documentStub.activeElement = opener;
  vi.stubGlobal('document', documentStub);
  let tree: ReactNode;
  const props = {
    open: true,
    onClose: vi.fn(() => { props.open = false; render(); }),
    onOpenLibrary: vi.fn(), onOpenSolve: vi.fn(), onSync: vi.fn(),
  };
  function render() {
    hooks.cursor = 0;
    tree = CommandPalette(props);
    for (const node of elements(tree)) {
      if (node.type === 'input') node.props.ref.current = input;
      if (node.props.role === 'dialog') node.props.ref.current = { querySelectorAll: () => [input, close] };
    }
    hooks.effects.splice(0).forEach((effect) => effect());
  }
  const find = (predicate: (node: Element) => boolean) => elements(tree).find(predicate)!;
  function key(key: string, extra = {}, dialog = false) {
    const event = { key, preventDefault: vi.fn(), stopPropagation: vi.fn(), nativeEvent: { isComposing: false }, keyCode: 0, shiftKey: false, ...extra };
    find((node) => dialog ? node.props.role === 'dialog' : node.type === 'input').props.onKeyDown(event);
    render();
    return event;
  }
  function query(value: string) {
    find((node) => node.type === 'input').props.onChange({ target: { value } });
    render();
  }
  render();
  return { props, render, query, key, find, input, close, opener, documentStub,
    options: () => elements(tree).filter((node) => node.props.role === 'option') };
}

beforeEach(() => {
  hooks.slots = []; hooks.cursor = 0; hooks.effects = [];
  invokeMock.mockReset().mockResolvedValue([]);
  vi.useFakeTimers();
  vi.stubGlobal('window', { setTimeout, clearTimeout });
});
afterEach(() => {
  hooks.slots.forEach((slot) => slot.cleanup?.());
  vi.unstubAllGlobals(); vi.useRealTimers();
});

describe('命令面板键盘与焦点', () => {
  it('普通关键词即使没有候选，Enter 也搜索资料库并关闭、还原焦点', () => {
    const page = mount();
    expect(page.documentStub.activeElement).toBe(page.input);
    page.query(' 视频工具 ');
    page.key('Enter');
    expect(page.props.onOpenLibrary).toHaveBeenCalledWith('视频工具');
    expect(page.props.onClose).toHaveBeenCalledOnce();
    expect(page.documentStub.activeElement).toBe(page.opener);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('上下键只选可见工具，Enter 打开选中项；改词后清除旧选择', async () => {
    invokeMock.mockResolvedValue([hit('分类', 'category'), hit('a'), hit('b')]);
    const page = mount(); page.query('工具');
    await vi.advanceTimersByTimeAsync(120); page.render();
    page.key('ArrowDown'); page.key('ArrowDown'); page.key('ArrowUp');
    expect(page.find((node) => node.type === 'input').props['aria-activedescendant']).toBe('palette-options-0');
    expect(page.options()[0].props['aria-selected']).toBe(true);
    page.key('Enter');
    expect(page.props.onOpenLibrary).toHaveBeenCalledWith('fox/a');
  });

  it('中文输入法正在确认候选时 Enter 不提交；? 问题仍交给找方案', () => {
    const page = mount(); page.query('? 做封面');
    page.key('Enter', { nativeEvent: { isComposing: true } });
    page.key('Enter', { keyCode: 229 });
    expect(page.props.onOpenSolve).not.toHaveBeenCalled();
    page.key('Enter');
    expect(page.props.onOpenSolve).toHaveBeenCalledWith('做封面');
    expect(page.props.onOpenLibrary).not.toHaveBeenCalled();
  });

  it('> 命令可用方向键选中并执行；同步只调用替身回调', () => {
    const page = mount(); page.query('>'); page.key('ArrowDown'); page.key('Enter');
    expect(page.props.onSync).toHaveBeenCalledOnce();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('Tab 留在面板里，Esc 在面板任何控件上关闭并恢复焦点', () => {
    const page = mount();
    const previousTab = page.key('Tab', { shiftKey: true }, true);
    expect(previousTab.preventDefault).toHaveBeenCalled();
    expect(page.documentStub.activeElement).toBe(page.close);
    page.key('Tab', {}, true);
    expect(page.documentStub.activeElement).toBe(page.input);
    page.key('Escape', {}, true);
    expect(page.props.onClose).toHaveBeenCalledOnce();
    expect(page.documentStub.activeElement).toBe(page.opener);
  });
});

describe('命令面板搜索请求', () => {
  it.each(['resolve', 'reject'] as const)('迟到旧请求的 %s 不能覆盖新结果，改词后立即隐藏旧项', async (outcome) => {
    const old = deferred<ReturnType<typeof hit>[]>();
    invokeMock.mockReturnValueOnce(old.promise).mockResolvedValueOnce([hit('new')]);
    const page = mount(); page.query('old'); await vi.advanceTimersByTimeAsync(120);
    page.query('new');
    expect(page.options()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(120); page.render();
    if (outcome === 'resolve') old.resolve([hit('old')]); else old.reject(new Error('旧查询失败'));
    await Promise.resolve(); await Promise.resolve(); page.render();
    expect(page.options()).toHaveLength(1);
    expect(page.options()[0].props.children[0]).toBe('fox/new');
    page.key('ArrowDown'); page.query('another'); page.key('Enter');
    expect(page.props.onOpenLibrary).toHaveBeenCalledWith('another');
  });

  it('关闭后返回的结果不能污染重新打开的面板', async () => {
    const old = deferred<ReturnType<typeof hit>[]>();
    invokeMock.mockReturnValueOnce(old.promise);
    const page = mount(); page.query('old'); await vi.advanceTimersByTimeAsync(120);
    page.key('Escape', {}, true);
    old.resolve([hit('old')]); await Promise.resolve(); page.render();
    page.props.open = true; page.render();
    expect(page.options()).toHaveLength(0);
  });
});
