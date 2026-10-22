/**
 * 前后端契约：新界面（今日 / 资料库 / 找方案 / 收集）依赖的后端命令返回值。
 *
 * 后端卡（06 场景、07 资料库查询、08 找方案、09 收集）按这里的形状返回 JSON（Rust 端用 serde 的 camelCase）；
 * 界面卡只从这里导入类型，**不许改这个文件**。形状要变，先找总控改契约。
 * 命令名与参数写在每个类型上方的注释里。
 */

/** 维护状态。按最近推送距今多久：active 活跃（≤ 90 天）、slow 放缓（> 90 天且 ≤ 365 天）、stale 疑似停更（> 365 天）。没有推送时间是 null，不算任何一档。 */
export type MaintenanceTier = 'active' | 'slow' | 'stale';

/** 「我的状态」五档。库里存的是上游的 8 个状态值，按组归并：
 *  unseen = unread + later；want = want_to_try + watching；tried = tried + read；in_use = in_use；dropped = deprecated。
 *  界面上五档的中文：没看 / 想试 / 试过 / 在用 / 不要了。 */
export type ReadingGroup = 'unseen' | 'want' | 'tried' | 'in_use' | 'dropped';

/** 命令 `list_scenarios()` → ScenarioSummary[]（按 sortOrder 升序，不含已归档的场景） */
export type ScenarioSummary = {
  id: string;
  slug: string;
  nameZh: string;
  sortOrder: number;
  /** 该场景下有效仓库数（sync_status='active'，且归类状态不是 rejected） */
  count: number;
  /** 「偏少」：count 不超过种子里的 thin_threshold，且场景在 thin_focus 里 */
  thin: boolean;
};

/** 命令 `list_library_view({ filters })` → LibraryPage */
export type LibraryFilters = {
  scenario?: string | null;
  reading?: ReadingGroup | null;
  /** done = 已有理解卡（卡片一句话非空）；pending = 还没有；all = 不筛。默认 done */
  cardState?: 'done' | 'pending' | 'all';
  form?: string | null;
  /** 不传 = 不筛。active / slow / stale 三档互斥。 */
  maintenance?: MaintenanceTier | null;
  keyword?: string;
  /** starred = 按收藏时间新到旧（默认）；stars = 按星数；updated = 按最近更新；relevance = 按相关度（有关键词时用） */
  sort?: 'starred' | 'stars' | 'updated' | 'relevance';
  limit?: number;
  offset?: number;
};

export type LibraryItem = {
  id: string;
  fullName: string;
  htmlUrl: string;
  description: string | null;
  language: string | null;
  starsCount: number;
  /** ISO 时间 */
  starredAt: string;
  pushedAt: string | null;
  /** 理解卡的一句话；还没有卡就是 null */
  oneLiner: string | null;
  /** 列表行第三行「适合：…」用的一句：理解卡里第一条 problems_solved，没有就第一条能力名，再没有就是 null。
   *  后端暂时可能不返回这个字段（所以是可选的）；界面只在它非空且和 oneLiner 不同时才显示这一行。 */
  highlight?: string | null;
  form: string | null;
  scenarioIds: string[];
  /** 库里存的原始状态值（unread / want_to_try / tried / in_use / watching / deprecated / later / read） */
  readingStatus: string;
  readingGroup: ReadingGroup;
  hasCard: boolean;
  confidence: number | null;
  /** 理解卡是否被 Fox 确认过；没有卡为 null */
  cardReview: 'suggested' | 'confirmed' | null;
  /** 命中原因。只有带关键词的相关度排序才有值（项目名、中文说明、英文简介、话题）；其它情况是空数组。 */
  matchReasons?: string[];
  /** 相关度总分。只有带关键词的相关度排序才有值；其它情况是 null。 */
  matchScore?: number | null;
  /** 匹配度档位 1–4，只有带关键词的相关度排序才有值。 */
  matchStars?: number | null;
};

export type LibraryCounts = {
  /** 当前账号的有效仓库总数（不受筛选影响） */
  total: number;
  /** 各场景下的仓库数（key 是场景 id）；按其他筛选条件（状态、卡片、形态、关键词）算，不按场景本身算 */
  scenarios: Record<string, number>;
  /** 各状态组的数量；按其他筛选条件算，不按状态本身算 */
  reading: Record<ReadingGroup, number>;
  /** 已整理 / 整理中的数量；按其他筛选条件算，不按卡片状态本身算 */
  cardState: { done: number; pending: number };
  forms: Record<string, number>;
  /** 活跃 / 放缓 / 疑似停更的个数。按其他筛选算，不按自己算；没有推送时间的不计。 */
  maintenance: { active: number; slow: number; stale: number };
};

export type LibraryPage = {
  items: LibraryItem[];
  /** 当前筛选下的总数（用于分页） */
  totalCount: number;
  counts: LibraryCounts;
};

/** 命令 `get_library_detail({ id })` → LibraryDetail */
export type LibraryDetail = LibraryItem & {
  /** 理解卡的 what_it_is（先结论） */
  whatItIs: string | null;
  capabilities: string[];
  problemsSolved: string[];
  useWhen: string[];
  notFor: string[];
  howToTry: string | null;
  integratesWith: string[];
  alternativesHint: string[];
  inspirationForFox: string | null;
  risks: string[];
  license: string | null;
  maturity: string | null;
  zhFriendly: boolean | null;
  keywords: string[];
  topics: string[];
  forksCount: number;
  /** README 摘录，最多约 1500 个字符，没有 README 为 null */
  readmeExcerpt: string | null;
};

/** 命令 `set_library_reading({ ids, group })`：把一批仓库设成某个状态组，返回改了多少个。
 *  组到库里状态值的写入映射：unseen→unread，want→want_to_try，tried→tried，in_use→in_use，dropped→deprecated。 */
export type SetReadingRequest = { ids: string[]; group: ReadingGroup };

/** 命令 `get_today_view()` → TodayView */
export type TodayView = {
  /** 今天值得先看：从「还没看、已有理解卡」里，按 Fox 常做的方向（场景排序靠前者）挑最多 3 个 */
  picks: LibraryItem[];
  /** 本周新收藏（近 7 天），最多 8 个，新到旧 */
  weekNew: LibraryItem[];
  counts: {
    total: number;
    unseen: number;
    weekNew: number;
    /** 还没有理解卡（整理中）的数量 */
    cardsPending: number;
    /** 有卡但 Fox 还没确认的数量 */
    cardsSuggested: number;
  };
  /** 最近一次同步时间（ISO），没有就是 null */
  syncedAt: string | null;
};

/** 命令 `solve_v2({ query })` → SolveResult。从 Fox 的收藏里按「意图」找能用的项目。 */
export type SolveHit = {
  id: string;
  fullName: string;
  htmlUrl: string;
  starsCount: number;
  oneLiner: string | null;
  form: string | null;
  scenarioIds: string[];
  /** 这条命中对上了哪些意图词 */
  matched: string[];
  /** 没对上的意图词 */
  missing: string[];
  /** 为什么相关：取理解卡里最相关的一句（能力 / 解决的问题 / 一句话） */
  why: string;
  score: number;
  /** 所有意图都对上了 */
  fullMatch: boolean;
  hasCard: boolean;
};

export type SolveResult = {
  query: string;
  /** 从问题里拆出的意图词（比如「做小红书封面」→ 小红书、封面） */
  intents: string[];
  /** 意图词之外的零散词 */
  extras: string[];
  /** 最多 8 条，按得分从高到低 */
  hits: SolveHit[];
  /** 本地排名 9–16 的后备候选：默认不显示，只在 AI 精选从里面点了名时才拿出来（旧后端没有这一项） */
  moreHits?: SolveHit[];
  /** 默认勾选：只有「所有意图都对上了」的前 3 条；没有就是空数组，不硬凑 */
  preselected: string[];
  fullMatchCount: number;
  /** 所有命中里都没有对上的意图词 */
  gapIntents: string[];
  /** 建议去「发现」看的场景 id（意图词和场景名/关键词对得上的那个），没有就是 null */
  suggestedScenarioId: string | null;
  /** 该场景下已收藏的数量，供「你在这个方向收藏了 N 个」使用 */
  suggestedScenarioCount: number | null;
  /** 还没有理解卡（整理中）、但名称或描述里就对上了全部意图词的仓库数；界面可以提示「另有 N 个还在整理，整理完可能对上」 */
  pendingMatches: number;
};

/** 找方案 AI 精选的档位。best 最合适，good 合适，maybe 备选，poor 不太合适。 */
export type RerankFit = 'best' | 'good' | 'maybe' | 'poor';

/** 命令 `solve_rerank_v2` 里的一条判断。 */
export type RerankItem = {
  repoId: string;
  fit: RerankFit;
  reason: string;
};

/** 命令 `solve_rerank_v2({ request })` → RerankResult。本地排序之外，模型再挑一次。 */
export type RerankResult = {
  status: 'ok' | 'cached' | 'skipped' | 'refused' | 'failed';
  /** 按 AI 的顺序；只在 ok / cached 时非空 */
  items: RerankItem[];
  /** 没有任何一个是 best / good */
  noneFit: boolean;
  /** 一句话建议，没有就是 null */
  note: string | null;
  /** skipped / refused / failed 时给界面的原因 */
  message: string | null;
  /** 这次大约花的 token；缓存命中是 0；没调用是 null */
  tokens: number | null;
};

/** 命令 `preview_capture({ text })` → CapturePreview：在入库前先告诉 Fox 每条链接会怎么处理 */
export type CapturePreviewRow = {
  url: string;
  normalizedUrl: string;
  /** github_repo / github / x_post / x / web 等，沿用 extract_links_from_text 的取值 */
  kind: string;
  platform: string;
  owner: string | null;
  name: string | null;
  blockedReason: string | null;
  /** new = 会新收；starred = 已在你的 Stars 里；inbox = 已在收集箱；blocked = 将被拦下 */
  status: 'new' | 'starred' | 'inbox' | 'blocked';
};

export type CapturePreview = {
  rows: CapturePreviewRow[];
  counts: { new: number; starred: number; inbox: number; blocked: number };
};

/** 命令 `list_inbox({ status, limit, offset })` → InboxPage。收集箱里的条目。 */
export type InboxItem = {
  id: string;
  originalUrl: string;
  normalizedUrl: string;
  title: string;
  /** github_repository / webpage / video / document / other */
  itemType: string;
  /** 来源平台：web / github / x / wechat / ... */
  platform: string;
  /** 来源说明，比如「微信群」「手动粘贴」 */
  sourceContext: string;
  userNote: string;
  /** pending 待处理 / ready 已整理 / failed 失败 / archived 已归档 */
  status: 'pending' | 'ready' | 'failed' | 'archived';
  repositoryId: string | null;
  createdAt: string;
  /** 后台读取公开页面的进度（卡 23）：pending 还没读 / done 读到了 / failed 读不到（原因在 fetchError）/ skipped 不需要读（GitHub 仓库、非 https 等）。旧版后台没有下面四项，所以都是可选的。 */
  fetchStatus?: 'pending' | 'done' | 'failed' | 'skipped';
  fetchError?: string;
  /** 页面作者：X 是帖子作者，公众号是号名 */
  pageAuthor?: string;
  /** 页面摘要的前 400 个字符 */
  excerpt?: string;
  /** 收进来时的原话（比如微信群里那条消息）的前 200 个字符；粘贴收集没有，是空串 */
  quote?: string;
  /** 读到的文章里提到的 GitHub 项目（卡 24）。旧版后台没有这一项，所以是可选的 */
  githubRepos?: RepoMention[];
};

/** 文章里提到的一个 GitHub 项目：`starred` 已经在你的收藏里；`inInbox` 已经在收集箱里（含已归档的）。两个都不是就是「新的」。 */
export type RepoMention = { fullName: string; starred: boolean; inInbox: boolean };

/** 命令 `fetch_inbox_pending({ id? })`：联网读取还没读过的收集箱链接，只读公开内容、不登录。带 `id` 时只读这一条（失败过的也会重试）。
 *  一次最多处理 20 条；`remaining` 是还没读的条数（大于 0 说明要再调一次）。 */
export type FetchSummary = { done: number; failed: number; skipped: number; remaining: number };

export type InboxPage = {
  items: InboxItem[];
  totalCount: number;
  counts: { pending: number; ready: number; failed: number; archived: number };
};

/** 命令 `set_inbox_status({ id, status })`：归档、恢复等；`set_inbox_note({ id, note })`：写备注（最多 500 字） */
export type SetInboxStatusRequest = { id: string; status: 'pending' | 'ready' | 'archived' };

/** 命令 `confirm_library_card({ id })`：Fox 说「说得对」——把理解卡标为已确认（自动整理不会覆盖它；只有你再点「重新整理这一张」才会重做）。
 *  命令 `regenerate_library_card({ id, confirmed })`：重新整理这一张。必须 `confirmed: true`（界面上点过确认）才开始，
 *  只给这一张任务写 30 分钟授权，每天额度关着也会做这一张；没确认、队列暂停或没配置 AI 时拒绝，数据库不动。成功返回 null。 */
export type CardReviewVerdict = 'confirmed' | 'regenerate';

/* ───────────── 发现（按场景缺口去 GitHub 找还没收藏的） ───────────── */

/** 命令 `list_discover({ scenarioId })` → DiscoverView。只读本地缓存，不联网，所以瞬间返回。
 *  `scenarioId` 为空时取「偏少」里排第一的场景（都不偏少就取排序第一的场景）。 */
export type DiscoverScenario = {
  id: string;
  slug: string;
  nameZh: string;
  sortOrder: number;
  /** Fox 在这个场景下已经收藏的个数（和 `list_scenarios` 的 count 同一口径） */
  myCount: number;
  thin: boolean;
  /** 缓存里这个场景的候选个数（不含已经在 Stars 里的） */
  candidateCount: number;
  /** 上次联网刷新的时间（ISO）；从没刷新过为 null */
  lastFetchedAt: string | null;
};

export type DiscoverCandidate = {
  fullName: string;
  htmlUrl: string;
  /** GitHub 原文描述（英文居多） */
  description: string | null;
  language: string | null;
  starsCount: number;
  pushedAt: string | null;
  topics: string[];
  /** 已经在 Fox 的 Stars 里（界面标「已在你的 Stars」，不给「收进收集箱」） */
  mine: boolean;
  /** 已经在收集箱里（待处理 / 已整理，不含已归档） */
  inInbox: boolean;
  fetchedAt: string;
};

export type DiscoverView = {
  scenarios: DiscoverScenario[];
  selected: string | null;
  candidates: DiscoverCandidate[];
};

/** 命令 `refresh_discover({ scenarioId })` → DiscoverRefreshReport。**联网**（GitHub 搜索接口，不登录，一分钟限 10 次），
 *  可能要几秒到十几秒；成功后把候选写进缓存。限流时 `rateLimitResetAt` 有值，`message` 说人话，已拿到的部分照样写入。 */
export type DiscoverRefreshReport = {
  scenarioId: string;
  queries: number;
  failedQueries: number;
  /** 这次新写入或更新的候选个数（不含已在 Stars 里的） */
  fetched: number;
  skippedMine: number;
  message: string | null;
  rateLimitResetAt: string | null;
};

/* ───────────── 社区：别人怎么用 ───────────── */

/** 命令 `get_repo_community({ id, refresh })` → RepoCommunity。`refresh` 为 false（默认）只读缓存，不联网；
 *  缓存里没有或已过期（24 小时）时 `stale` 为 true，`items` 可能为空。`refresh` 为 true 才联网
 *  （Hacker News 搜索 + GitHub 上评论最多的 Issues），失败写进 `errors`，不抛错。 */
export type CommunityItem = {
  source: 'hn' | 'github_issue';
  title: string;
  url: string;
  /** HN 的分数；GitHub issue 没有为 null */
  points: number | null;
  comments: number | null;
  createdAt: string | null;
  /** issue 的状态 open / closed；HN 为 null */
  state: string | null;
};

export type RepoCommunity = {
  repoId: string;
  fetchedAt: string | null;
  stale: boolean;
  items: CommunityItem[];
  errors: Array<{ source: 'hn' | 'github_issue'; message: string }>;
};
