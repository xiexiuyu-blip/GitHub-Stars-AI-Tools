import { describe, expect, it, vi } from 'vitest';
import type { PackFormat, PackOutput, PackRequest } from '@/lib/api';
import { currentSolvePack, generateSolvePack, solvePackInput, type SolvePackState } from '@/lib/solve-pack-state';

function request(): PackRequest {
  return {
    title: '交接单：做封面', problem: '做封面',
    items: [{ repoId: 'a', role: 'main', why: '能做封面' }, { repoId: 'b', role: 'alternative' }],
    localCapabilities: [{ name: 'ai-draw', oneLiner: '做封面', path: '/fixture/ai-draw' }],
  };
}

function output(content = '当前交接单', format: PackFormat = 'markdown'): PackOutput {
  return { format, content, itemCount: 2, missing: [], warnings: [] };
}

function input(packRequest = request(), format: PackFormat = 'markdown', version = 1, attempt = 0) {
  return solvePackInput(packRequest, format, '2026-10-28', version, attempt);
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

const describeError = (reason: unknown) => reason instanceof Error ? reason.message : String(reason);
const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

describe('交接单版本与真实生成生命周期', () => {
  it('当前输出可取用；没有当前任务时不能取用', async () => {
    const current = input();
    const build = vi.fn().mockResolvedValue(output());
    const publish = vi.fn();
    generateSolvePack(current, build, publish, describeError);
    await settle();
    expect(build).toHaveBeenCalledExactlyOnceWith(current.request, 'markdown', '2026-10-28');
    const state = publish.mock.calls[0][0] as SolvePackState;
    expect(currentSolvePack(current, state)?.output?.content).toBe('当前交接单');
    expect(currentSolvePack(null, state)).toBeNull();
  });

  it.each([
    ['同一批工具、换问题', () => input({ ...request(), problem: '给公众号配图' })],
    ['换格式', () => input(request(), 'json')],
    ['改选工具', () => input({ ...request(), items: [{ repoId: 'c', role: 'main' }] })],
    ['换主参考与顺序', () => input({ ...request(), items: [{ repoId: 'b', role: 'main' }, { repoId: 'a', role: 'alternative' }] })],
    ['换直接模式', () => input({ ...request(), mode: 'direct', items: [] })],
    ['本机能力扫描完成', () => input({ ...request(), localCapabilities: [] })],
    ['本机能力说明或路径变化', () => input({ ...request(), localCapabilities: [{ name: 'ai-draw', oneLiner: '画图', path: '/fixture/new' }] })],
    ['同一句话明确重新搜索', () => input(request(), 'markdown', 2)],
    ['当前版本重新生成', () => input(request(), 'markdown', 1, 1)],
  ] as const)('%s：新请求尚未开始就不能取用旧输出', async (_label, nextInput) => {
    const old = input();
    let state: SolvePackState | null = null;
    generateSolvePack(old, async () => output('旧交接单'), (next) => { state = next; }, describeError);
    await settle();
    expect(currentSolvePack(old, state)?.output?.content).toBe('旧交接单');
    expect(currentSolvePack(nextInput(), state)).toBeNull();
  });

  it('新一轮生成失败后没有可复制内容，重试也不会重新放行旧版', async () => {
    const old = input();
    let state: SolvePackState | null = { key: old.key, output: output('旧版'), error: null };
    const current = input({ ...request(), problem: '新问题' });
    const failure = deferred<PackOutput>();
    generateSolvePack(current, () => failure.promise, (next) => { state = next; }, describeError);
    expect(currentSolvePack(current, state)).toBeNull();
    failure.reject(new Error('读取失败'));
    await settle();
    expect(currentSolvePack(current, state)).toEqual({ key: current.key, output: null, error: '读取失败' });
    const retry = input(current.request, 'markdown', 1, 1);
    expect(currentSolvePack(retry, state)).toBeNull();
    generateSolvePack(retry, async () => output('重试的新内容'), (next) => { state = next; }, describeError);
    await settle();
    expect(currentSolvePack(retry, state)?.output?.content).toBe('重试的新内容');
  });

  it.each(['成功', '失败'] as const)('旧请求最后才%s，不会覆盖新结果', async (ending) => {
    const oldRequest = deferred<PackOutput>();
    const newRequest = deferred<PackOutput>();
    const publish = vi.fn();
    const old = input();
    const current = input({ ...request(), problem: '新问题' });
    const cancel = generateSolvePack(old, () => oldRequest.promise, publish, describeError);
    await settle();
    cancel();
    generateSolvePack(current, () => newRequest.promise, publish, describeError);
    newRequest.resolve(output('新交接单'));
    await settle();
    if (ending === '成功') oldRequest.resolve(output('旧交接单'));
    else oldRequest.reject(new Error('旧错误'));
    await settle();
    expect(publish).toHaveBeenCalledTimes(1);
    expect(publish.mock.calls[0][0]).toMatchObject({ key: current.key, output: { content: '新交接单' }, error: null });
  });

  it('输入 A → B → A，第一次 A 的迟到结果也不得冒充第三轮 A', async () => {
    const first = input();
    const third = input(request(), 'markdown', 3);
    expect(currentSolvePack(third, { key: first.key, output: output('第一轮'), error: null })).toBeNull();
    const pending = deferred<PackOutput>();
    const publish = vi.fn();
    const cancel = generateSolvePack(first, () => pending.promise, publish, describeError);
    await settle();
    cancel();
    pending.resolve(output('第一轮'));
    await settle();
    expect(publish).not.toHaveBeenCalled();
  });

  it('离开页面后不回写结果，未发出的构建也不再执行', async () => {
    const build = vi.fn().mockResolvedValue(output());
    const publish = vi.fn();
    generateSolvePack(input(), build, publish, describeError)();
    await settle();
    expect(build).not.toHaveBeenCalled();
    expect(publish).not.toHaveBeenCalled();
  });

  it.each([
    ['空内容', output('   ')],
    ['格式不符', output('旧 JSON', 'json')],
    ['缺少内容', { format: 'markdown' } as PackOutput],
  ] as const)('%s会变成错误，不能当作可复制版本', async (_label, invalid) => {
    const publish = vi.fn();
    generateSolvePack(input(), async () => invalid, publish, describeError);
    await settle();
    expect(publish.mock.calls[0][0]).toMatchObject({ output: null, error: '交接单的结果不完整，请重试' });
  });
});
