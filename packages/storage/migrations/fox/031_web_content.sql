-- 031: 发现候选和社区讨论的本地缓存。只追加，不改已有表。
-- 搜索词在 docs/rebuild/spec/discover_queries.json；界面读缓存，点刷新才联网。
CREATE TABLE IF NOT EXISTS discover_candidates (
  scenario_id TEXT NOT NULL,
  full_name TEXT NOT NULL,
  html_url TEXT NOT NULL,
  description TEXT,
  language TEXT,
  stars_count INTEGER NOT NULL DEFAULT 0,
  pushed_at TEXT,
  topics_json TEXT NOT NULL DEFAULT '[]',
  fetched_at TEXT NOT NULL,
  PRIMARY KEY (scenario_id, full_name)
);
CREATE INDEX IF NOT EXISTS idx_discover_candidates_scenario ON discover_candidates(scenario_id, stars_count DESC);

CREATE TABLE IF NOT EXISTS discover_runs (
  scenario_id TEXT PRIMARY KEY,
  last_fetched_at TEXT NOT NULL,
  queries INTEGER NOT NULL DEFAULT 0,
  failed_queries INTEGER NOT NULL DEFAULT 0,
  message TEXT
);

CREATE TABLE IF NOT EXISTS repo_community_cache (
  repo_id TEXT NOT NULL,
  source TEXT NOT NULL CHECK (source IN ('hn', 'github_issue')),
  payload_json TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  error TEXT,
  PRIMARY KEY (repo_id, source)
);
INSERT INTO schema_migrations(version, name) VALUES('031', 'web_content');
