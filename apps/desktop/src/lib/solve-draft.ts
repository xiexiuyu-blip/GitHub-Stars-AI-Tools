import type { PackFormat } from '@/lib/api';
import type { LibraryDetail, RerankResult, SolveHit, SolveResult } from '@/lib/contracts';

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

/** 非空的带入问题代表新任务；父页面消费后清空它，普通返回才恢复草稿。 */
export function loadSolveDraft(initialQuestion = ''): SolveDraft | null {
  return initialQuestion.trim() ? null : current;
}

/** 从工具详情一键加入「找方案」草稿：已有草稿则追加并选中，没有则新建一份单工具草稿。 */
export function addDetailToSolveDraft(detail: LibraryDetail): { mode: 'appended' | 'already' | 'created' } {
  const why = detail.oneLiner?.trim() || detail.whatItIs?.trim() || '从详情手动加入';
  const hit: SolveHit = {
    id: detail.id,
    fullName: detail.fullName,
    htmlUrl: detail.htmlUrl,
    starsCount: detail.starsCount,
    pushedAt: detail.pushedAt,
    archived: detail.archived ?? null,
    oneLiner: detail.oneLiner,
    form: detail.form,
    scenarioIds: detail.scenarioIds,
    matched: [],
    missing: [],
    why,
    score: 1,
    fullMatch: true,
    hasCard: detail.hasCard,
  };

  if (!current) {
    const label = detail.oneLiner?.trim() || detail.fullName;
    saveSolveDraft({
      query: label,
      activeQuery: label,
      result: {
        query: label,
        intents: [],
        extras: [],
        hits: [hit],
        preselected: [detail.id],
        fullMatchCount: 1,
        gapIntents: [],
        suggestedScenarioId: null,
        suggestedScenarioCount: null,
        pendingMatches: 0,
      },
      picked: [detail.id],
      format: 'markdown',
      direct: false,
      rerank: null,
      rerankPhase: 'idle',
    });
    return { mode: 'created' };
  }

  if (current.picked.includes(detail.id)) {
    return { mode: 'already' };
  }

  const hasHit = current.result.hits.some((item) => item.id === detail.id);
  saveSolveDraft({
    ...current,
    result: hasHit ? current.result : { ...current.result, hits: [...current.result.hits, hit] },
    picked: [...current.picked, detail.id],
    direct: false,
  });
  return { mode: 'appended' };
}
