-- Fox Stars Lab migration 002 upstream_v120
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xed4cd1
-- Verify against schema/live_schema_018.sql before use.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS github_recommendation_candidates (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  full_name TEXT NOT NULL,
  description TEXT,
  language TEXT,
  topics_json TEXT NOT NULL DEFAULT '[]',
  html_url TEXT NOT NULL,
  stars_count INTEGER NOT NULL DEFAULT 0,
  forks_count INTEGER NOT NULL DEFAULT 0,
  pushed_at TEXT,
  status TEXT NOT NULL DEFAULT 'new' CHECK (status IN ('new', 'marked', 'ignored', 'starred')),
  rationale_zh TEXT,
  queries_json TEXT NOT NULL DEFAULT '[]',
  last_seen_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE (account_id, full_name)
);

CREATE INDEX IF NOT EXISTS idx_recommendation_candidates_account_status ON github_recommendation_candidates(account_id, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_recommendation_candidates_account_full_name ON github_recommendation_candidates(account_id, full_name);

CREATE TABLE IF NOT EXISTS github_recommendation_documents (
  account_id TEXT NOT NULL,
  full_name TEXT NOT NULL,
  raw_markdown TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  source_path TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  translation_markdown_zh TEXT,
  translation_model TEXT,
  translation_input_tokens INTEGER,
  translation_output_tokens INTEGER,
  translation_source_char_count INTEGER,
  translation_translated_char_count INTEGER,
  translation_is_truncated INTEGER,
  translation_source_hash TEXT,
  translation_generated_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (account_id, full_name),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_recommendation_documents_account_updated ON github_recommendation_documents(account_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS github_ranking_cache (
  cache_key TEXT PRIMARY KEY,
  payload_json TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_github_ranking_cache_fetched_at ON github_ranking_cache(fetched_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('002', 'upstream_v120');
