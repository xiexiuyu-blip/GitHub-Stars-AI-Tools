import type { PackFormat, PackOutput, PackRequest } from '@/lib/api';

export type SolvePackInput = {
  key: string;
  request: PackRequest;
  format: PackFormat;
  date: string;
};

export type SolvePackState = {
  key: string;
  output: PackOutput | null;
  error: string | null;
};

/** 标识实际发出的整份请求；新搜索和重试即使内容相同，也不能沿用上一轮输出。 */
export function solvePackInput(
  request: PackRequest,
  format: PackFormat,
  date: string,
  searchVersion: number,
  attempt: number,
): SolvePackInput {
  return { key: JSON.stringify([searchVersion, attempt, format, date, request]), request, format, date };
}

/** 在 effect 开始之前就挡住旧输出，生成失败也不会退回旧交接单。 */
export function currentSolvePack(input: SolvePackInput | null, state: SolvePackState | null): SolvePackState | null {
  return input && state?.key === input.key ? state : null;
}

/** 清理时作废本轮回调：晚到的成功、失败都不能盖掉下一轮。构建函数可替换成离线夹具。 */
export function generateSolvePack(
  input: SolvePackInput,
  build: (request: PackRequest, format: PackFormat, date: string) => Promise<PackOutput>,
  publish: (state: SolvePackState) => void,
  describeError: (reason: unknown) => string,
): () => void {
  let active = true;
  void Promise.resolve().then(() => {
    if (!active) return;
    return build(input.request, input.format, input.date);
  }).then((output) => {
    if (!active) return;
    if (!output || output.format !== input.format || typeof output.content !== 'string' || !output.content.trim()) {
      throw new Error('交接单的结果不完整，请重试');
    }
    publish({
      key: input.key,
      output: {
        format: output.format,
        content: output.content,
        itemCount: Number.isFinite(output.itemCount) ? output.itemCount : 0,
        missing: Array.isArray(output.missing) ? output.missing.filter((item) => typeof item === 'string') : [],
        warnings: Array.isArray(output.warnings) ? output.warnings.filter((item) => typeof item === 'string') : [],
      },
      error: null,
    });
  }).catch((reason) => {
    if (active) publish({ key: input.key, output: null, error: describeError(reason) });
  });
  return () => { active = false; };
}
