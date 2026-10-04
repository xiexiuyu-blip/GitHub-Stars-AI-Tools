-- 036: 找方案 AI 精选的缓存。同一件事、同一批候选再搜一次直接用上次的结果。
CREATE TABLE solve_rerank_cache (
  cache_key TEXT PRIMARY KEY,
  result_json TEXT NOT NULL,
  model TEXT NOT NULL DEFAULT '',
  tokens INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO schema_migrations(version, name) VALUES('036', 'solve_rerank_cache');
