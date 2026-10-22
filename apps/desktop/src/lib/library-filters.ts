import type { LibraryCounts, LibraryFilters, ReadingGroup, ScenarioSummary } from '@/lib/contracts';

/** 资料库页面上的筛选。默认看全部，按收藏时间新到旧。 */
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
  /** 筛选面板上显示的短名字（技能 / 命令行 / 应用…），和行内小标签是同一套：左栏只有 200px 宽，长写法会被截成半句。 */
  label: string;
  /** 长解释，放进悬停提示（title）。没有解释、或和短名字一样（应用、其他）就是 null。 */
  hint: string | null;
  count: number;
};

export type CardStateOption = {
  value: LibraryUiState['cardState'];
  label: string;
};

/** 类型的短名字：行内小标签、找方案的命中行、复制给 Codex 的正文都用这一套，别把解释写进去。 */
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

/**
 * 类型的长写法（带解释）：筛选面板的悬停提示和详情页的「类型」一行用，行内小标签别用。
 * 技能包不写「装进 Codex 用」：有的技能包是给 claude.ai 用的，会和「怎么试」里的步骤打架。
 */
const FORM_HINT_LABELS: Record<string, string> = {
  skill: '技能包（给 AI 助手装的扩展）',
  mcp: 'MCP 插件（给 AI 接外部工具）',
  cli: '命令行工具（要让 Codex 帮你装）',
  app: '应用',
  library: '开发用代码库（一般用不上）',
  api: '接口（程序之间对接）',
  service: '在线服务',
  collection: '资源清单',
  other: '其他',
};

/** 工具的「类型」写成短名字。没填或者是「其他」（没有信息量）返回 null，界面就不画这个标签；不认识的原样给。 */
export function formLabel(form: string | null | undefined): string | null {
  const id = form?.trim();
  if (!id || id === 'other') return null;
  return FORM_LABELS[id] ?? id;
}

/** 同上，但写成带解释的长写法。没填或「其他」同样返回 null；不认识的原样给。 */
export function formLabelWithHint(form: string | null | undefined): string | null {
  const id = form?.trim();
  if (!id || id === 'other') return null;
  return FORM_HINT_LABELS[id] ?? id;
}

/** 详情页「类型」一行：「类型：应用」。用长写法，因为这里有地方解释；筛选分组标题已经是「类型」，筛选行上不再加这前缀。 */
export function formTagLabel(form: string | null | undefined): string | null {
  const label = formLabelWithHint(form);
  return label ? `类型：${label}` : null;
}

export function createLibraryUiState(): LibraryUiState {
  return {
    scenario: null,
    reading: null,
    form: null,
    // 默认看全部：还没整理的收藏也要能搜到，搜关键词、按场景找都不该漏掉
    cardState: 'all',
    keyword: '',
    sort: 'starred',
  };
}

/** 从别的页面带着条件进资料库（比如今日里的「去资料库看全部待 AI 整理」）。 */
export type LibraryPreset = { cardState?: 'done' | 'pending'; reading?: ReadingGroup | null };

export function withInitial(
  initialKeyword?: string | null,
  initialScenario?: string | null,
  preset?: LibraryPreset | null,
): LibraryUiState {
  const state = createLibraryUiState();
  if (preset?.cardState) state.cardState = preset.cardState;
  if (preset?.reading) state.reading = preset.reading;
  state.keyword = initialKeyword?.trim() ?? '';
  const scenario = initialScenario?.trim() ?? '';
  state.scenario = scenario ? scenario : null;
  return state;
}

/** 空关键词不传。有关键词且排序还是默认的收藏时间时，改发 relevance。offset 是页码乘每页条数。场景、状态、类型为空时不传。 */
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
  // 用户没另选排序时，搜索按相关度。选了星数或最近更新就保持原样。
  if (keyword && state.sort === 'starred') filters.sort = 'relevance';
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

/** 清掉场景、状态、类型和关键词，整理状态回到默认。排序保留。 */
export function clearFilters(state: LibraryUiState): LibraryUiState {
  return {
    scenario: null,
    reading: null,
    form: null,
    cardState: 'all',
    keyword: '',
    sort: state.sort,
  };
}

/** 决定是否显示「清除筛选」。整理状态不是默认的「全部」也算有筛选。 */
export function hasActiveFilters(state: LibraryUiState): boolean {
  return Boolean(state.scenario)
    || Boolean(state.reading)
    || Boolean(state.form)
    || state.keyword.trim() !== ''
    || state.cardState !== 'all';
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

/**
 * 类型按个数从多到少；个数相同按 id 的字符码排序。未知类型原样显示。
 * 筛选面板里显示短名字 + 个数（`label`），长解释放进悬停提示（`hint`）：200px 宽的左栏放不下长写法。
 */
export function formOptions(forms: Readonly<Record<string, number>> | null | undefined): FormFilterRow[] {
  const rows = Object.entries(forms ?? {}).map(([id, count]) => {
    const label = FORM_LABELS[id] ?? id;
    const hint = FORM_HINT_LABELS[id] ?? null;
    return { id, label, hint: hint && hint !== label ? hint : null, count: asCount(count) };
  });
  rows.sort((a, b) => {
    if (a.count !== b.count) return b.count - a.count;
    if (a.id < b.id) return -1;
    if (a.id > b.id) return 1;
    return 0;
  });
  return rows;
}

/** 资料库列表一个都没有时，页面中间说什么。 */
export type LibraryEmptyNote = {
  title: string;
  /** 只有搜索词没搜到时才有：换个说法，或者去「找方案」 */
  description: string | null;
  /** 只有搜索词没搜到时才给「去找方案」按钮：筛选筛空了，去找方案帮不上忙 */
  offerSolve: boolean;
};

const SEARCH_EMPTY: LibraryEmptyNote = {
  title: '没有找到',
  description: '换个说法试试，或去「找方案」说说你想做什么',
  offerSolve: true,
};

/**
 * 一个都没有时说什么：
 * - 搜了关键词：「没有找到，换个说法试试」，并给「去找方案」。
 * - 没搜关键词，只点了「待 AI 整理」：「没有待 AI 整理的收藏，都整理好了。」——这是好消息，不是没搜到。
 * - 没搜关键词，点了别的筛选（想试 / 试过 / 在用 / 不要了、场景、类型、AI 已整理，或几个叠在一起）：「这一类还没有。」
 * - 什么都没筛、也没搜（资料库本身是空的）：保持原来那句。
 */
export function libraryEmptyNote(
  state: Pick<LibraryUiState, 'scenario' | 'reading' | 'form' | 'cardState' | 'keyword'>,
): LibraryEmptyNote {
  if (state.keyword.trim()) return SEARCH_EMPTY;
  const narrowed = Boolean(state.scenario || state.reading || state.form);
  if (state.cardState === 'pending' && !narrowed) {
    return { title: '没有待 AI 整理的收藏，都整理好了。', description: null, offerSolve: false };
  }
  if (narrowed || state.cardState !== 'all') {
    return { title: '这一类还没有。', description: null, offerSolve: false };
  }
  return SEARCH_EMPTY;
}

/** 「全部」是 AI 已整理加待 AI 整理：这两个数本身已经按其他筛选联动过，不按卡片状态再筛一次。 */
export function cardStateOptions(
  cardState: { done?: number; pending?: number } | null | undefined,
): CardStateOption[] {
  const done = asCount(cardState?.done);
  const pending = asCount(cardState?.pending);
  return [
    { value: 'all', label: `全部 ${done + pending}` },
    { value: 'done', label: `AI 已整理 ${done}` },
    { value: 'pending', label: `待 AI 整理 ${pending}` },
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
 * 刚装好、还没有中文说明时，资料库不要停在空白的「AI 已整理」。
 * 只在：AI 已整理是 0、还有待 AI 整理、用户没动过三选项、当前仍是「AI 已整理」时为真。
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
