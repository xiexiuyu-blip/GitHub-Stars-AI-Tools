-- 030: 业务场景（Fox 用它来做什么）。和 022 的技术分类（它是什么）正交：一个仓库可以属于多个场景。
-- 场景种子见 docs/rebuild/spec/scenarios_seed.json；规则、AI、用户三种来源，用户确认的不被覆盖。
CREATE TABLE scenarios (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  slug TEXT NOT NULL CHECK (slug GLOB '[a-z0-9]*' AND length(slug) BETWEEN 1 AND 64),
  name_zh TEXT NOT NULL CHECK (length(trim(name_zh)) BETWEEN 1 AND 40),
  description_zh TEXT NOT NULL DEFAULT '',
  keywords_json TEXT NOT NULL DEFAULT '[]',
  sort_order INTEGER NOT NULL DEFAULT 0,
  origin TEXT NOT NULL DEFAULT 'seed' CHECK (origin IN ('seed','user')),
  archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0,1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE(account_id, slug)
);
CREATE TABLE entity_scenarios (
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  entity_type TEXT NOT NULL CHECK (entity_type IN ('repository','inbox_item','tool')),
  entity_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
  origin TEXT NOT NULL DEFAULT 'rule' CHECK (origin IN ('rule','ai','user')),
  confidence REAL NOT NULL DEFAULT 0.5 CHECK (confidence BETWEEN 0 AND 1),
  status TEXT NOT NULL DEFAULT 'suggested' CHECK (status IN ('suggested','confirmed','rejected')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY (entity_type, entity_id, scenario_id)
);
CREATE INDEX idx_entity_scenarios_scenario ON entity_scenarios(account_id, scenario_id, status);
CREATE INDEX idx_entity_scenarios_entity ON entity_scenarios(entity_type, entity_id, status);
INSERT INTO schema_migrations(version, name) VALUES('030', 'scenarios');
