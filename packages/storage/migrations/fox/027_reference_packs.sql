-- PROPOSED 027: “AI 参考包” + “我想解决…”问题历史。
CREATE TABLE reference_packs (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  title_zh TEXT NOT NULL,
  problem_text TEXT NOT NULL DEFAULT '',
  items_json TEXT NOT NULL DEFAULT '[]',
  format TEXT NOT NULL DEFAULT 'markdown' CHECK (format IN ('markdown','json','agents_md','mcp_resource')),
  content TEXT NOT NULL DEFAULT '',
  content_hash TEXT,
  exported_paths_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE saved_questions (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  question TEXT NOT NULL,
  problem_concept_id TEXT REFERENCES kg_concepts(id) ON DELETE SET NULL,
  answer_json TEXT,
  reference_pack_id TEXT REFERENCES reference_packs(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO schema_migrations(version, name) VALUES('027', 'reference_packs');
