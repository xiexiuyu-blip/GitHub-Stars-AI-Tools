/* 隔离验收数据：所有 Tauri 调用在浏览器内响应，不访问实际数据库、账号或 AI。 */
export function installFixture() {
  const today = new Date().toISOString();
  const ago = (days) => new Date(Date.now() - days * 86400000).toISOString();
  const items = ['封面工具', '旧版配图工具', '归档海报工具', '待整理工具'].map((title, index) => ({
    id: `fixture:${index + 1}`, fullName: `fixture/${['cover', 'legacy', 'archived', 'pending'][index]}`,
    htmlUrl: `https://example.invalid/${index}`, description: title, language: 'TypeScript', starsCount: 100 - index,
    starredAt: today, pushedAt: index === 1 ? ago(500) : index === 3 ? null : ago(10), archived: index === 2,
    oneLiner: title, form: 'app', scenarioIds: ['design'], readingStatus: 'unread', readingGroup: 'unseen',
    hasCard: index !== 3, confidence: 0.8, cardReview: 'suggested', matchReasons: ['name'], matchStars: 4,
  }));
  const state = {
    calls: [],
    pendingPacks: [],
    pendingSearches: [],
    pendingDiscoverSearches: [],
    pendingDiscoverFeeds: [],
    delaySearches: false,
    delayDiscoverSearch: false,
    delayDiscoverFeed: false,
    delayPacks: false,
    failPacks: false,
    pagedLibrary: false,
    pagedInserted: false,
    readmeVersion: 1,
    readmeMissing: false,
    clipboard: '',
    listeners: {},
    callbacks: {},
    nextId: 1,
    items,
  };
  window.__FSL_ACCEPTANCE__ = state;
  localStorage.setItem('fsl.solve.ai', 'false');
  Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: async (text) => { state.clipboard = text; }, readText: async () => state.clipboard } });
  const tier = (item) => item.archived ? 'archived' : !item.pushedAt ? null : Date.now() - Date.parse(item.pushedAt) > 365 * 86400000 ? 'stale' : 'active';
  const counts = { total: 4, scenarios: { design: 4 }, reading: { unseen: 4, want: 0, tried: 0, in_use: 0, dropped: 0 }, cardState: { done: 3, pending: 1 }, forms: { app: 4 }, maintenance: { active: 1, slow: 0, stale: 1, archived: 1 } };
  const pagedItems = Array.from({ length: 250 }, (_, idx) => ({
    id: `paged:${idx + 1}`,
    fullName: `fixture/paged-${idx + 1}`,
    htmlUrl: `https://example.invalid/paged/${idx + 1}`,
    description: `分页项目 ${idx + 1}`,
    language: 'TypeScript',
    starsCount: 1000 - idx,
    starredAt: today,
    pushedAt: ago(5),
    archived: false,
    oneLiner: `分页项目 ${idx + 1}`,
    form: 'app',
    scenarioIds: ['design'],
    readingStatus: 'unread',
    readingGroup: 'unseen',
    hasCard: true,
    confidence: 0.9,
    cardReview: 'suggested',
    matchReasons: [],
    matchStars: 0,
  }));
  const scenarios = [{ id: 'design', slug: 'design', nameZh: '图文设计', count: 4, sortOrder: 1, thin: false }];
  const auth = { hasToken: true, user: { id: 1, login: 'fixture-user', name: '验收数据', avatarUrl: null, htmlUrl: null } };
  function emit(event, payload) {
    for (const handler of state.listeners[event] || []) state.callbacks[handler]?.({ event, id: 1, payload });
  }
  state.emit = emit;
  state.resolvePacks = () => { for (const pending of state.pendingPacks.splice(0)) pending.resolve(pending.output); };
  state.rejectPacks = () => { for (const pending of state.pendingPacks.splice(0)) pending.reject('验收：交接单生成失败'); };
  const makeFeedItem = (fullName, description, rank = 1) => ({
    rank,
    fullName,
    htmlUrl: `https://github.com/${fullName}`,
    description,
    language: 'TypeScript',
    starsCount: 500 - rank,
    starsDelta: 50,
    deltaLabel: '今日 +50',
    pushedAt: ago(1),
    createdAt: ago(10),
    topics: ['ai'],
    sourceNote: null,
    mine: false,
    inInbox: false,
    fetchedAt: today,
  });
  const handlers = {
    get_app_settings: () => ({ general: { showWelcomeOnStartup: false }, sync: { autoSync: false, autoSyncOnStartup: false }, ai: { enableAutoSummary: false }, theme: { mode: 'system' } }),
    get_github_auth_state: () => auth,
    get_app_identity: () => ({ productName: 'Fox Stars Lab Dev', identifier: 'com.foxwork.fox-stars-lab.dev', version: '2.1.0-fox.1', isDev: true }),
    get_backend_status: () => ({ backend: '隔离验收', storage: '测试夹具', worker: '未启动', provider: '无真实服务' }),
    has_ai_api_key: () => false, has_embedding_api_key: () => false,
    list_repositories: () => ({ items: [], totalCount: 0, limit: 80, offset: 0 }),
    get_repository_filter_counts: () => ({ totalCount: 0, languageCounts: {}, tagCounts: {} }),
    list_scenarios: () => scenarios,
    get_today_view: () => ({ picks: items.slice(0, 3), weekNew: items, counts: { total: 4, unseen: 4, weekNew: 4, cardsPending: 1, cardsSuggested: 3 }, syncedAt: today }),
    list_library_view: ({ filters = {} }) => {
      if (state.pagedLibrary) {
        const limit = Number(filters.limit) || 80;
        const offset = Number(filters.offset) || 0;
        if (offset > 0 && !state.pagedInserted) {
          state.pagedInserted = true;
          pagedItems.unshift({
            ...pagedItems[0],
            id: 'paged:0',
            fullName: 'fixture/paged-0',
            oneLiner: '新插入的分页项目 0',
          });
        }
        const slice = pagedItems.slice(offset, offset + limit);
        const total = pagedItems.length;
        return {
          items: slice,
          totalCount: total,
          counts: {
            total,
            scenarios: { design: total },
            reading: { unseen: total, want: 0, tried: 0, in_use: 0, dropped: 0 },
            cardState: { done: total, pending: 0 },
            forms: { app: total },
            maintenance: { active: total, slow: 0, stale: 0, archived: 0 },
          },
        };
      }
      let found = items.filter((item) => !filters.maintenance || tier(item) === filters.maintenance);
      if (filters.keyword) found = found.filter((item) => `${item.fullName} ${item.oneLiner}`.includes(filters.keyword));
      if (filters.cardState === 'pending') found = found.filter((item) => !item.hasCard);
      if (filters.cardState === 'done') found = found.filter((item) => item.hasCard);
      return { items: found, totalCount: found.length, counts };
    },
    list_discover: ({ scenarioId = null }) => ({ scenarios: [], selected: scenarioId, candidates: [] }),
    list_discover_feed: ({ feedId = 'trending_daily', language = null }) => {
      const feedKey = language ? `${feedId}:${language}` : feedId;
      const feedItems = feedId === 'trending_daily'
        ? [makeFeedItem('fixture/daily-star', '今日飙升示例项目', 1)]
        : feedId === 'trending_weekly'
          ? [makeFeedItem('fixture/weekly-star', '本周热门示例项目', 1)]
          : [makeFeedItem(`fixture/${feedId}-star`, `${feedId} 示例项目`, 1)];
      const output = { feedId, language, feedKey, lastFetchedAt: today, usedFallback: false, message: null, items: feedItems };
      if (state.delayDiscoverFeed) {
        return new Promise((resolve, reject) => state.pendingDiscoverFeeds.push({ feedId, language, output, resolve, reject }));
      }
      return output;
    },
    search_discover_github: ({ query = '' }) => {
      const output = {
        query,
        totalCount: 1,
        items: [makeFeedItem(`fixture/search-${query}`, `搜索结果 ${query}`, 1)],
        message: null,
        rateLimitResetAt: null,
      };
      if (state.delayDiscoverSearch) {
        return new Promise((resolve, reject) => state.pendingDiscoverSearches.push({ query, output, resolve, reject }));
      }
      return output;
    },
    get_library_detail: ({ id }) => ({ ...items.find((item) => item.id === id), whatItIs: '隔离验收：用于制作封面和海报。', capabilities: ['批量制作封面'], problemsSolved: ['制作封面'], useWhen: ['准备图文素材'], notFor: ['不支持视频剪辑'], howToTry: '打开演示，导入一张图片。', integratesWith: [], alternativesHint: [], inspirationForFox: null, risks: ['请先用测试素材试用'], license: 'MIT', maturity: null, zhFriendly: true, keywords: ['封面'], topics: ['design'], forksCount: 1, readmeExcerpt: null }),
    get_repository_detail: () => ({ readme: state.readmeMissing ? null : { rawMarkdown: `# 隔离项目说明\n\nREADME验收版本${state.readmeVersion}\n\n用于制作封面。`, sourcePath: 'README.md' } }),
    get_ai_automation: () => ({ enabled: false, dailyBudgetTokens: 0, paused: false, pauseReason: null, tokens24h: 0, budgetRemaining: 0, queued: 1, running: 0, failed: 0, providerConfigured: false, model: null, unlimited: false, fullScan: null, lastFullScan: null, activeRepos: 4, needsOrganizing: 1 }),
    get_ai_queue_status: () => ({ succeeded24h: 0 }),
    get_local_api_status: () => ({ enabled: false, host: '127.0.0.1', port: 39217, listening: false, hasToken: false }),
    get_backup_status: () => ({ lastAutoAt: null, nextAutoAt: null, lastRestore: null }),
    solve_v2: ({ query }) => ({ query, intents: ['封面'], extras: [], hits: items.map((item) => ({ ...item, matched: ['封面'], missing: [], why: '可处理封面素材', score: 100, fullMatch: true })), moreHits: [], preselected: [items[0].id, items[1].id], fullMatchCount: 4, gapIntents: [], suggestedScenarioId: 'design', suggestedScenarioCount: 4, pendingMatches: 0 }),
    solve_rerank_v2: () => ({ status: 'refused', items: [], noneFit: false, note: null, reason: 'AI 额度：验收未开启', usage: null }),
    scan_local_capabilities: () => [],
    build_reference_pack_v2: ({ request, format }) => {
      const output = { format, content: format === 'json' ? JSON.stringify({ task: request.problem, items: request.items }) : `# 当前交接单\n\n任务：${request.problem}\n\n格式：${format}\n\n工具：${request.items.map((item) => item.repoId).join(',')}`, itemCount: request.items.length, missing: [], warnings: [] };
      if (state.failPacks) return Promise.reject('验收：交接单生成失败');
      if (state.delayPacks) return new Promise((resolve, reject) => state.pendingPacks.push({ resolve, reject, output }));
      return output;
    },
    search_command_palette: ({ query }) => {
      const output = items.filter((item) => item.fullName.includes(query) || item.oneLiner.includes(query)).map((item) => ({ kind: 'repository', id: item.id, title: item.fullName, subtitle: item.oneLiner }));
      if (state.delaySearches) return new Promise((resolve, reject) => state.pendingSearches.push({ query, output, resolve, reject }));
      return output;
    },
    confirm_library_card: () => null,
    regenerate_library_card: () => Promise.reject('验收：还没配置 AI 服务'),
    sync_github_stars: () => { state.readmeVersion += 1; emit('task-progress', { taskId: 'sync-stars', taskType: 'sync', status: 'succeeded', stage: 'sync', current: 4, total: 4, message: '隔离同步完成', createdAt: today }); return { fetched: 4, inserted: 0, updated: 4, removed: 0, syncedAt: today }; },
    get_repository_last_synced_at: () => today,
    get_repo_community: () => ({ issues: [], discussions: [], fetchedAt: null }),
  };
  window.__TAURI_INTERNALS__ = {
    invoke: async (command, args = {}) => {
      state.calls.push({ command, args });
      if (command === 'plugin:event|listen') { (state.listeners[args.event] ||= []).push(args.handler); return state.nextId++; }
      if (command.startsWith('plugin:')) return null;
      if (handlers[command]) return handlers[command](args);
      if (command.startsWith('list_')) return [];
      return null;
    },
    transformCallback: (callback) => { const id = state.nextId++; state.callbacks[id] = callback; return id; },
    unregisterCallback: (id) => { delete state.callbacks[id]; }, convertFileSrc: (path) => path,
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.isTauri = true;
}
