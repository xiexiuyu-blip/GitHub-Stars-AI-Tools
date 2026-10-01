-- PROPOSED 023: 知识图谱。节点 = 已有实体(repository/inbox_item/tool) + 概念节点(capability/problem)；分类走 022。
CREATE TABLE kg_concepts (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('capability','problem')),
  name_zh TEXT NOT NULL CHECK (length(trim(name_zh)) BETWEEN 1 AND 80),
  normalized_name TEXT NOT NULL,
  description_zh TEXT NOT NULL DEFAULT '',
  origin TEXT NOT NULL DEFAULT 'ai' CHECK (origin IN ('ai','user')),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','merged','archived')),
  merged_into_id TEXT REFERENCES kg_concepts(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE(account_id, kind, normalized_name)
);
CREATE TABLE kg_edges (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  src_type TEXT NOT NULL CHECK (src_type IN ('repository','inbox_item','tool','concept')),
  src_id TEXT NOT NULL,
  relation TEXT NOT NULL CHECK (relation IN ('has_capability','solves','part_of','mentions','alternative_of','complements','depends_on','inspired_by','related')),
  dst_type TEXT NOT NULL CHECK (dst_type IN ('repository','inbox_item','tool','concept')),
  dst_id TEXT NOT NULL,
  confidence REAL NOT NULL DEFAULT 1.0 CHECK (confidence BETWEEN 0 AND 1),
  origin TEXT NOT NULL DEFAULT 'ai' CHECK (origin IN ('ai','user','rule','embedding')),
  status TEXT NOT NULL DEFAULT 'suggested' CHECK (status IN ('suggested','confirmed','rejected')),
  evidence_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE(account_id, src_type, src_id, relation, dst_type, dst_id),
  CHECK (NOT (src_type = dst_type AND src_id = dst_id))
);
CREATE INDEX idx_kg_edges_src ON kg_edges(account_id, src_type, src_id);
CREATE INDEX idx_kg_edges_dst ON kg_edges(account_id, dst_type, dst_id);
INSERT INTO schema_migrations(version, name) VALUES('023', 'knowledge_graph');
