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
