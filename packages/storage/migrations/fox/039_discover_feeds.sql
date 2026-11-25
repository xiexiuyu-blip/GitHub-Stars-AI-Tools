-- 039: 发现页趋势榜缓存（今日飙升 / 本周热门 / 近期新锐 / HN Show 热门）
CREATE TABLE IF NOT EXISTS discover_feed_items (
  feed_key TEXT NOT NULL,
  rank INTEGER NOT NULL,
  full_name TEXT NOT NULL,
  html_url TEXT NOT NULL,
  description TEXT,
  language TEXT,
  stars_count INTEGER NOT NULL DEFAULT 0,
  stars_delta INTEGER,
  delta_label TEXT,
  pushed_at TEXT,
  created_at TEXT,
  topics_json TEXT NOT NULL DEFAULT '[]',
  source_note TEXT,
  fetched_at TEXT NOT NULL,
  PRIMARY KEY (feed_key, rank)
);
CREATE INDEX IF NOT EXISTS idx_discover_feed_items_key ON discover_feed_items(feed_key, rank ASC);

CREATE TABLE IF NOT EXISTS discover_feed_runs (
  feed_key TEXT PRIMARY KEY,
  last_fetched_at TEXT NOT NULL,
  item_count INTEGER NOT NULL DEFAULT 0,
  used_fallback INTEGER NOT NULL DEFAULT 0,
  message TEXT
);
INSERT INTO schema_migrations(version, name) VALUES('039', 'discover_feeds');
