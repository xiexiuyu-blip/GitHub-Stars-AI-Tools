/* 假的 Tauri 后端：让页面在普通浏览器里也能跑起来，用来做界面验收。
 * 所有 invoke 命令都在这里回答。没写的命令：list_* 返回 []，其余返回 null，并在控制台打一行 [mock] 提示。
 * 想看某个页面的真实数据形状，就在 HANDLERS 里补一个。 */
(function () {
  var calls = [];
  window.__MOCK_CALLS__ = calls;

  var USER = { id: 1, login: 'fox', name: 'Fox', avatarUrl: null, htmlUrl: 'https://github.com/fox' };

  var REPOS = [
    { id: 'r1', fullName: 'anthropics/skills', description: 'Public repository for Agent Skills', language: 'Python', htmlUrl: 'https://github.com/anthropics/skills', starsCount: 24500, readingLabel: '没看', oneLiner: '给 Claude 用的技能包合集' },
    { id: 'r2', fullName: 'openai/codex', description: 'Lightweight coding agent that runs in your terminal', language: 'Rust', htmlUrl: 'https://github.com/openai/codex', starsCount: 41200, readingLabel: '想试', oneLiner: '终端里的编程助手' },
    { id: 'r3', fullName: 'coreyhaines31/marketingskills', description: 'Marketing skills for AI agents', language: 'JavaScript', htmlUrl: 'https://github.com/coreyhaines31/marketingskills', starsCount: 3100, readingLabel: '没看', oneLiner: '营销和增长方向的技能集合' },
  ];

  var HANDLERS = {
    get_app_settings: function () { return { general: { showWelcomeOnStartup: false } }; },
    get_github_auth_state: function () { return { hasToken: true, user: USER }; },
    get_backend_status: function () { return { backend: '本地服务已就绪', storage: '本地数据库已就绪', worker: '后台任务队列已就绪', provider: '假后端（界面验收用）' }; },
    get_app_identity: function () { return { productName: 'Fox Stars Lab Dev', identifier: 'com.foxwork.fox-stars-lab.dev', version: '1.5.3-fox.2', isDev: true }; },
    has_ai_api_key: function () { return false; },
    has_embedding_api_key: function () { return false; },
    get_health_summary: function () { return '假后端：495 个有效仓库，README 495，摘要 178。'; },
    get_today_actions: function () {
      return { activeRepositories: 495, suggestedCards: 86, failedJobs: 0, starredThisWeek: 4, schemaVersion: '029', topLanguages: [{ language: 'Python', count: 120 }, { language: 'TypeScript', count: 96 }] };
    },
    list_workspace_cards: function () { return { items: REPOS, totalCount: REPOS.length }; },
    get_local_api_status: function () { return { enabled: false, host: '127.0.0.1', port: 39217, listening: false, hasToken: false }; },
    get_backup_status: function () { return { lastAutoAt: '2026-10-03T10:17:17Z', nextAutoAt: '2026-10-04T06:17:17Z', lastRestore: null }; },
    list_backups: function () {
      return [{ fileName: 'auto-20261003-101717.sqlite3', kind: 'auto', createdAt: '2026-10-03T10:17:17Z', bytes: 13717504, repositoryCount: 495, schemaVersion: '029' }];
    },
    list_ai_jobs: function () { return []; },
    list_ranking_board: function () { return []; },
    list_repositories: function () { return { items: [], totalCount: 0, limit: 80, offset: 0 }; },
    list_repository_languages: function () { return []; },
    get_repository_filter_counts: function () { return { totalCount: 0, languageCounts: {}, tagCounts: {} }; },
    plugin_updater_check: function () { return null; },
  };

  /* ---- 新界面（今日 / 资料库 / 找方案 / 收集）用到的命令：数据来自原型 data.json（真实的 492 个仓库） ---- */
  var DATA_PROMISE = null;
  function protoData() {
    if (!DATA_PROMISE) { DATA_PROMISE = fetch('/__mock__/data.json').then(function (r) { return r.json(); }); }
    return DATA_PROMISE;
  }
  var GROUP_TO_STATUS = { unseen: 'unread', want: 'want_to_try', tried: 'tried', in_use: 'in_use', dropped: 'deprecated' };
  var READING = {}; // id -> group；没设过就是 unseen
  var PRESET_DONE = false;
  function presetReading(items) {
    if (PRESET_DONE) return; PRESET_DONE = true;
    var withCard = items.filter(function (i) { return i.has; });
    if (withCard[0]) READING[withCard[0].id] = 'want';
    if (withCard[1]) READING[withCard[1].id] = 'want';
    if (withCard[2]) READING[withCard[2].id] = 'in_use';
    if (withCard[3]) READING[withCard[3].id] = 'dropped';
  }
  function groupOf(i) { return READING[i.id] || 'unseen'; }
  function toItem(i, scenMap) {
    var g = groupOf(i);
    return {
      id: i.id, fullName: i.n, htmlUrl: 'https://github.com/' + i.n, description: i.d || null, language: i.l || null,
      starsCount: i.s, starredAt: i.t + 'T08:00:00Z', pushedAt: i.p ? i.p + 'T08:00:00Z' : null,
      oneLiner: i.has ? (i.z || null) : null, form: i.form || null,
      scenarioIds: (i.sc || []).slice(), readingStatus: GROUP_TO_STATUS[g], readingGroup: g,
      hasCard: !!i.has, confidence: i.has ? 0.8 : null, cardReview: i.has ? 'suggested' : null,
    };
  }
  function toDetail(i) {
    var base = toItem(i);
    return Object.assign(base, {
      whatItIs: i.has ? (i.sm || i.z || null) : null, capabilities: i.cap || [], problemsSolved: i.pr || [], useWhen: [], notFor: [],
      howToTry: i.how || null, integratesWith: [], alternativesHint: [], inspirationForFox: null, risks: [],
      license: null, maturity: i.mat || null, zhFriendly: i.cn === 1 ? true : (i.cn === 0 ? false : null),
      keywords: (i.cap || []).slice(0, 5), topics: i.tp || [], forksCount: 0,
      readmeExcerpt: i.has ? null : null,
    });
  }
  function matchesFilters(i, f, skip) {
    if (!(skip === 'scenario') && f.scenario && (i.sc || []).indexOf(f.scenario) < 0) return false;
    if (!(skip === 'reading') && f.reading && groupOf(i) !== f.reading) return false;
    var cs = f.cardState || 'done';
    if (!(skip === 'cardState')) { if (cs === 'done' && !i.has) return false; if (cs === 'pending' && i.has) return false; }
    if (!(skip === 'form') && f.form && (i.form || '') !== f.form) return false;
    if (f.keyword) { var k = String(f.keyword).toLowerCase(); var text = [i.n, i.d, i.z, i.sm, (i.tp || []).join(' ')].join(' ').toLowerCase(); if (text.indexOf(k) < 0) return false; }
    return true;
  }
  var NEW_HANDLERS = {
    list_scenarios: function () {
      return protoData().then(function (D) {
        return D.scen.map(function (s, idx) {
          var count = D.items.filter(function (i) { return (i.sc || []).indexOf(s.id) >= 0; }).length;
          return { id: s.id, slug: s.id, nameZh: s.n, sortOrder: idx + 1, count: count, thin: ['ec', 'seo', 'data'].indexOf(s.id) >= 0 && count <= 14 };
        });
      });
    },
    list_library_view: function (args) {
      var f = (args && args.filters) || {};
      return protoData().then(function (D) {
        presetReading(D.items);
        var all = D.items;
        var list = all.filter(function (i) { return matchesFilters(i, f, null); });
        var sort = f.sort || 'starred';
        list.sort(function (a, b) {
          if (sort === 'stars') return b.s - a.s;
          if (sort === 'updated') return (b.p || '').localeCompare(a.p || '');
          return b.t.localeCompare(a.t);
        });
        var limit = f.limit || 80, offset = f.offset || 0;
        var counts = { total: all.length, scenarios: {}, reading: { unseen: 0, want: 0, tried: 0, in_use: 0, dropped: 0 }, cardState: { done: 0, pending: 0 }, forms: {} };
        D.scen.forEach(function (s) { counts.scenarios[s.id] = all.filter(function (i) { return matchesFilters(i, f, 'scenario') && (i.sc || []).indexOf(s.id) >= 0; }).length; });
        all.filter(function (i) { return matchesFilters(i, f, 'reading'); }).forEach(function (i) { counts.reading[groupOf(i)]++; });
        all.filter(function (i) { return matchesFilters(i, f, 'cardState'); }).forEach(function (i) { counts.cardState[i.has ? 'done' : 'pending']++; });
        all.filter(function (i) { return matchesFilters(i, f, 'form') && i.form; }).forEach(function (i) { counts.forms[i.form] = (counts.forms[i.form] || 0) + 1; });
        return { items: list.slice(offset, offset + limit).map(function (i) { return toItem(i); }), totalCount: list.length, counts: counts };
      });
    },
    get_library_detail: function (args) {
      return protoData().then(function (D) {
        var i = D.items.filter(function (x) { return x.id === (args && args.id); })[0];
        if (!i) throw new Error('找不到这个仓库');
        return toDetail(i);
      });
    },
    set_library_reading: function (args) {
      return protoData().then(function () {
        (args.ids || []).forEach(function (id) { READING[id] = args.group; });
        return (args.ids || []).length;
      });
    },
    get_today_view: function () {
      return protoData().then(function (D) {
        presetReading(D.items);
        var core = ['xhs', 'wx', 'dy', 'img', 'vid', 'ppt', 'wr'];
        var cand = D.items.filter(function (i) { return i.has && groupOf(i) === 'unseen'; });
        var picks = [], used = {};
        cand.forEach(function (i) {
          var sc = (i.sc || []).filter(function (s) { return core.indexOf(s) >= 0; })[0];
          if (sc && !used[sc] && picks.length < 3) { used[sc] = true; picks.push(i); }
        });
        cand.forEach(function (i) { if (picks.length < 3 && picks.indexOf(i) < 0) picks.push(i); });
        var asof = new Date(D.asof + 'T12:00:00Z').getTime();
        var week = D.items.filter(function (i) { return new Date(i.t + 'T00:00:00Z').getTime() >= asof - 7 * 864e5; }).slice(0, 8);
        return {
          picks: picks.map(function (i) { return toItem(i); }), weekNew: week.map(function (i) { return toItem(i); }),
          counts: { total: D.items.length, unseen: D.items.filter(function (i) { return groupOf(i) === 'unseen'; }).length, weekNew: week.length,
            cardsPending: D.items.filter(function (i) { return !i.has; }).length, cardsSuggested: D.items.filter(function (i) { return i.has; }).length },
          syncedAt: D.asof + 'T08:00:00Z',
        };
      });
    },
    copy_item_for_codex: function (args) {
      return protoData().then(function (D) {
        var i = D.items.filter(function (x) { return x.id === (args && (args.repoId || args.id)); })[0];
        if (!i) throw new Error('找不到这个参考项目：' + (args && args.repoId));
        return '请参考这个 GitHub 项目：' + i.n + '\n- 链接：https://github.com/' + i.n + '（★' + i.s + '）\n- 它是什么：' + (i.z || i.d || '') + '\n用法：先阅读项目主页和 README，再按我的需求借鉴它的做法；不要直接复制受限许可证的代码；不确定的地方先问我。';
      });
    },
    confirm_library_card: function () { return null; },
    regenerate_library_card: function () { return null; },
  };
  Object.keys(NEW_HANDLERS).forEach(function (k) { HANDLERS[k] = NEW_HANDLERS[k]; });

  function answer(cmd, args) {
    var name = String(cmd);
    if (name.indexOf('plugin:event|') === 0) { return 1; }
    if (name.indexOf('plugin:') === 0) { return null; }
    if (HANDLERS[name]) { return HANDLERS[name](args || {}); }
    console.info('[mock] 未实现的命令：' + name, args || '');
    return name.indexOf('list_') === 0 ? [] : null;
  }

  var callbackId = 1;
  window.__TAURI_INTERNALS__ = {
    invoke: function (cmd, args) {
      calls.push(String(cmd));
      try { return Promise.resolve(answer(cmd, args)); } catch (error) { return Promise.reject(error); }
    },
    transformCallback: function (callback) {
      var id = callbackId++;
      window['_' + id] = callback;
      return id;
    },
    unregisterCallback: function () {},
    convertFileSrc: function (path) { return path; },
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' }, windows: [{ label: 'main' }], webviews: [{ windowLabel: 'main', label: 'main' }] },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: function () {} };
  window.isTauri = true;

  window.addEventListener('error', function (event) {
    console.error('[页面报错] ' + event.message + ' @ ' + (event.filename || '') + ':' + (event.lineno || ''));
  });
  window.addEventListener('unhandledrejection', function (event) {
    console.error('[未处理的 Promise 拒绝] ' + (event.reason && (event.reason.stack || event.reason.message) || event.reason));
  });
})();
