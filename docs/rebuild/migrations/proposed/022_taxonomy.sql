-- PROPOSED 022: 两级分类树 + 实体归类（AI 建议 / 用户确认）。tags 表保留作自由标签。
CREATE TABLE categories (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  parent_id TEXT REFERENCES categories(id) ON DELETE CASCADE,
  slug TEXT NOT NULL CHECK (slug GLOB '[a-z0-9]*' AND length(slug) BETWEEN 1 AND 64),
  name_zh TEXT NOT NULL CHECK (length(trim(name_zh)) BETWEEN 1 AND 40),
  description_zh TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0,
  origin TEXT NOT NULL DEFAULT 'seed' CHECK (origin IN ('seed','ai','user')),
  archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0,1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE(account_id, slug)
);
CREATE TABLE entity_categories (
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  entity_type TEXT NOT NULL CHECK (entity_type IN ('repository','inbox_item','tool')),
  entity_id TEXT NOT NULL,
  category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
  is_primary INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0,1)),
  confidence REAL NOT NULL DEFAULT 1.0 CHECK (confidence BETWEEN 0 AND 1),
  origin TEXT NOT NULL DEFAULT 'ai' CHECK (origin IN ('ai','user','rule')),
  status TEXT NOT NULL DEFAULT 'suggested' CHECK (status IN ('suggested','confirmed','rejected')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY (entity_type, entity_id, category_id)
);
CREATE INDEX idx_entity_categories_category ON entity_categories(account_id, category_id, status);
CREATE UNIQUE INDEX idx_entity_categories_one_primary ON entity_categories(entity_type, entity_id) WHERE is_primary = 1 AND status <> 'rejected';
INSERT INTO schema_migrations(version, name) VALUES('022', 'taxonomy');
