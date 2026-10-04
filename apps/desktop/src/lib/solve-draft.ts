import type { PackFormat } from '@/lib/api';
import type { RerankResult, SolveResult } from '@/lib/contracts';

/**
 * 「找方案」的草稿：问了什么、找到了什么、勾了哪些、选了哪种格式、是不是直接交给 Codex。
 * 切到别的页面（资料库、发现……）再回来时接着用，不要让人白做一遍。只放在内存里，关掉应用就没了。
 */
export type SolveRerankPhase = 'idle' | 'loading' | 'done';

export type SolveDraft = {
  query: string;
  activeQuery: string;
  result: SolveResult;
  picked: string[];
  format: PackFormat;
  /** 没勾工具、直接交给 Codex。旧草稿没传这个字段时当作 false。 */
  direct?: boolean;
  /** AI 精选结果。旧草稿没传就当作没有。 */
  rerank?: RerankResult | null;
  /** 进行中的精选恢复时当作 idle：请求已经没人等了。 */
  rerankPhase?: SolveRerankPhase;
};

let current: SolveDraft | null = null;

export function saveSolveDraft(draft: SolveDraft | null): void {
  current = draft;
}

export function loadSolveDraft(): SolveDraft | null {
  return current;
}
