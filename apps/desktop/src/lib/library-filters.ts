import type { LibraryCounts, LibraryFilters, ReadingGroup, ScenarioSummary } from '@/lib/contracts';

/** 资料库页面上的筛选。默认只看已整理，按收藏时间新到旧。 */
export type LibraryUiState = {
  scenario: string | null;
  reading: ReadingGroup | null;
  form: string | null;
  cardState: 'done' | 'pending' | 'all';
  keyword: string;
  sort: 'starred' | 'stars' | 'updated';
};

export type ScenarioFilterRow = {
  id: string;
  nameZh: string;
  count: number;
  thin: boolean;
};

export type FormFilterRow = {
  id: string;
  label: string;
  count: number;
};

export type CardStateOption = {
  value: LibraryUiState['cardState'];
  label: string;
};

const FORM_LABELS: Record<string, string> = {
  skill: '技能',
  mcp: 'MCP',
  cli: '命令行',
  app: '应用',
  library: '代码库',
  api: '接口',
  service: '服务',
  collection: '合集',
  other: '其他',
};

/** 项目的「形态」写成中文标签。没填或者是「其他」（没有信息量）返回 null，界面就不画这个标签；不认识的原样给。 */
export function formLabel(form: string | null | undefined): string | null {
  const id = form?.trim();
  if (!id || id === 'other') return null;
  return FORM_LABELS[id] ?? id;
}

export function createLibraryUiState(): LibraryUiState {
  return {
    scenario: null,
    reading: null,
    form: null,
    cardState: 'done',
    keyword: '',
    sort: 'starred',
  };
}

export function withInitial(initialKeyword?: string | null, initialScenario?: string | null): LibraryUiState {
  const state = createLibraryUiState();
  state.keyword = initialKeyword?.trim() ?? '';
  const scenario = initialScenario?.trim() ?? '';
  state.scenario = scenario ? scenario : null;
  return state;
}

/** 空关键词不传。offset 是页码乘每页条数。场景、状态、形态为空时不传。 */
export function toFilters(state: LibraryUiState, page: number, pageSize: number): LibraryFilters {
  const safePage = Number.isFinite(page) && page > 0 ? Math.floor(page) : 0;
  const safeSize = Number.isFinite(pageSize) && pageSize > 0 ? Math.floor(pageSize) : 0;
  const filters: LibraryFilters = {
    cardState: state.cardState,
    sort: state.sort,
    limit: safeSize,
    offset: safePage * safeSize,
  };
  if (state.scenario) filters.scenario = state.scenario;
  if (state.reading) filters.reading = state.reading;
  if (state.form) filters.form = state.form;
  const keyword = state.keyword.trim();
  if (keyword) filters.keyword = keyword;
  return filters;
}

export function toggleScenario(state: LibraryUiState, value: string): LibraryUiState {
  if (!value) return { ...state, scenario: null };
  return { ...state, scenario: state.scenario === value ? null : value };
}

export function toggleReading(state: LibraryUiState, value: ReadingGroup): LibraryUiState {
  return { ...state, reading: state.reading === value ? null : value };
}

export function toggleForm(state: LibraryUiState, value: string): LibraryUiState {
  if (!value) return { ...state, form: null };
  return { ...state, form: state.form === value ? null : value };
}

/** 清掉场景、状态、形态和关键词，整理状态回到默认。排序保留。 */
export function clearFilters(state: LibraryUiState): LibraryUiState {
  return {
    scenario: null,
    reading: null,
    form: null,
    cardState: 'done',
    keyword: '',
    sort: state.sort,
  };
}

/** 决定是否显示「清除筛选」。整理状态不是默认的「已整理」也算有筛选。 */
export function hasActiveFilters(state: LibraryUiState): boolean {
  return Boolean(state.scenario)
    || Boolean(state.reading)
    || Boolean(state.form)
    || state.keyword.trim() !== ''
    || state.cardState !== 'done';
}

/**
 * 筛选栏的场景行。个数用联动计数 `counts.scenarios[id]`，没有这个键就当 0。
 * 不使用场景自己带的 count（那是没跟其他筛选联动的数）。顺序和「偏少」保持原样。
 */
export function visibleScenarios(
  scenarios: readonly Pick<ScenarioSummary, 'id' | 'nameZh' | 'thin'>[],
  counts: Pick<LibraryCounts, 'scenarios'> | null | undefined,
): ScenarioFilterRow[] {
  const map = counts?.scenarios ?? {};
  return scenarios.map((scenario) => ({
    id: scenario.id,
    nameZh: scenario.nameZh,
    count: Object.prototype.hasOwnProperty.call(map, scenario.id) ? asCount(map[scenario.id]) : 0,
    thin: Boolean(scenario.thin),
  }));
}

/** 形态按个数从多到少；个数相同按名称的字符码排序。未知形态原样显示。 */
export function formOptions(forms: Readonly<Record<string, number>> | null | undefined): FormFilterRow[] {
  const rows = Object.entries(forms ?? {}).map(([id, count]) => ({
    id,
    label: FORM_LABELS[id] ?? id,
    count: asCount(count),
  }));
  rows.sort((a, b) => {
    if (a.count !== b.count) return b.count - a.count;
    if (a.id < b.id) return -1;
    if (a.id > b.id) return 1;
    return 0;
  });
  return rows;
}

/** 「全部」是已整理加待整理：这两个数本身已经按其他筛选联动过，不按卡片状态再筛一次。 */
export function cardStateOptions(
  cardState: { done?: number; pending?: number } | null | undefined,
): CardStateOption[] {
  const done = asCount(cardState?.done);
  const pending = asCount(cardState?.pending);
  return [
    { value: 'done', label: `已整理 ${done}` },
    { value: 'pending', label: `待整理 ${pending}` },
    { value: 'all', label: `全部 ${done + pending}` },
  ];
}

/**
 * 下一页的 offset。没有更多时返回 null，调用方据此隐藏「加载更多」。
 * 只有已加载条数是 pageSize 的正整数倍、并且还没到总数时才返回。
 * 没加载、已取完、页大小非法、或这一页没取满（offset 对不齐，不能用页码表达）都返回 null。
 */
export function nextPageOffset(loaded: number, total: number, pageSize: number): number | null {
  if (!Number.isFinite(loaded) || !Number.isFinite(total) || !Number.isFinite(pageSize)) return null;
  if (pageSize <= 0) return null;
  const safeLoaded = Math.floor(loaded);
  const safeTotal = Math.floor(total);
  if (safeLoaded <= 0 || safeTotal <= 0) return null;
  if (safeLoaded >= safeTotal) return null;
  if (safeLoaded % pageSize !== 0) return null;
  return safeLoaded;
}

/**
 * 刚装好、还没有理解卡时，资料库不要停在空白的「已整理」。
 * 只在：已整理是 0、还有待整理、用户没动过三选项、当前仍是默认「已整理」时为真。
 */
export function shouldFallbackToAll(
  counts: { cardState?: { done?: number; pending?: number } | null } | null | undefined,
  state: Pick<LibraryUiState, 'cardState'>,
  touched: boolean,
): boolean {
  if (touched) return false;
  if (state.cardState !== 'done') return false;
  const done = asCount(counts?.cardState?.done);
  const pending = asCount(counts?.cardState?.pending);
  return done === 0 && pending > 0;
}

/** 把某个条目的状态改掉。返回新数组，不改原来的。 */
export function applyOptimisticReading<T extends { id: string; readingGroup: ReadingGroup }>(
  items: readonly T[],
  id: string,
  group: ReadingGroup,
): T[] {
  return items.map((item) => (item.id === id ? { ...item, readingGroup: group } : item));
}

/**
 * 当前筛了状态组，而这条的新状态已经对不上时，把它从列表里拿掉。
 * 没有状态筛选，或新状态仍然符合，就原样留下（新数组，不改入参）。
 */
export function removeIfFilteredOut<T extends { id: string; readingGroup: ReadingGroup }>(
  items: readonly T[],
  id: string,
  reading: ReadingGroup | null,
): T[] {
  if (!reading) return items.slice();
  const target = items.find((item) => item.id === id);
  if (!target || target.readingGroup === reading) return items.slice();
  return items.filter((item) => item.id !== id);
}

function asCount(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : 0;
}
