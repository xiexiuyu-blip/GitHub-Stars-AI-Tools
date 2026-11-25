import type { LibraryUiState } from '@/lib/library-filters';

/**
 * 资料库会话内状态快照：切去发现/找方案/设置再回来时，保持当前筛选、搜索词、选中项和滚动位置。
 * 只保存在内存里，应用关闭即清空。
 */
export type LibrarySessionSnapshot = {
  ui: LibraryUiState;
  keywordDraft: string;
  selectedId: string | null;
  scrollTop: number;
  cardStateTouched: boolean;
};

let currentSession: LibrarySessionSnapshot | null = null;

export function saveLibrarySession(snapshot: LibrarySessionSnapshot | null): void {
  currentSession = snapshot;
}

/** 若带着初始筛选条件从别的页面跳进来，忽略历史会话；常规切回才恢复。 */
export function loadLibrarySession(hasInitialOverride: boolean): LibrarySessionSnapshot | null {
  return hasInitialOverride ? null : currentSession;
}
