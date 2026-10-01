-- PROPOSED 026: 多来源榜单快照 + 关注视角。upstream github_ranking_cache 保留（6h 缓存）。
CREATE TABLE ranking_sources (
  id TEXT PRIMARY KEY,
  name_zh TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
  requires_key INTEGER NOT NULL DEFAULT 0 CHECK (requires_key IN (0,1)),
  min_interval_minutes INTEGER NOT NULL DEFAULT 360,
  config_json TEXT NOT NULL DEFAULT '{}',
  last_fetched_at TEXT, last_error TEXT
);
CREATE TABLE ranking_snapshots (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL REFERENCES ranking_sources(id) ON DELETE CASCADE,
  list_key TEXT NOT NULL,
  fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  item_count INTEGER NOT NULL DEFAULT 0,
  UNIQUE(source_id, list_key, fetched_at)
);
CREATE TABLE ranking_entries (
  snapshot_id TEXT NOT NULL REFERENCES ranking_snapshots(id) ON DELETE CASCADE,
  rank INTEGER NOT NULL,
  full_name TEXT NOT NULL,
  description TEXT, language TEXT, html_url TEXT,
  stars_count INTEGER, stars_delta INTEGER,
  extra_json TEXT NOT NULL DEFAULT '{}',
  PRIMARY KEY (snapshot_id, rank)
);
CREATE INDEX idx_ranking_entries_name ON ranking_entries(full_name);
CREATE TABLE ranking_watches (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  title_zh TEXT NOT NULL,
  source_ids_json TEXT NOT NULL DEFAULT '[]',
  category_ids_json TEXT NOT NULL DEFAULT '[]',
  min_match_score REAL NOT NULL DEFAULT 0.6,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO ranking_sources(id, name_zh, enabled, requires_key, min_interval_minutes) VALUES
 ('github_search','GitHub 搜索榜（新锐/上升/热门）',1,0,360),
 ('ossinsight','OSS Insight 趋势与主题榜',1,0,360),
 ('hn_show','Hacker News Show HN',1,0,720),
 ('github_trending_html','GitHub Trending 页面（低频抓取，默认关）',0,0,720),
 ('trendshift','Trendshift Signal（需付费 Key，默认关）',0,1,720),
 ('hellogithub','HelloGitHub 月刊（中文精选，默认关）',0,0,10080);
INSERT INTO schema_migrations(version, name) VALUES('026', 'rankings');
