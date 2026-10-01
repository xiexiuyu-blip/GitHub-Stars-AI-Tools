-- PROPOSED 028: 连接器注册、事件外发箱、使用记录（MCP 受控写回）。密钥一律放 Keychain，不进库。
CREATE TABLE connectors (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('source','enricher','exporter','action')),
  name_zh TEXT NOT NULL,
  manifest_json TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0,1)),
  last_run_at TEXT, last_error TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE event_outbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  delivered_at TEXT,
  attempts INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_event_outbox_pending ON event_outbox(delivered_at, id);
CREATE TABLE usage_log (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  entity_type TEXT NOT NULL CHECK (entity_type IN ('repository','tool','inbox_item')),
  entity_id TEXT NOT NULL,
  verdict TEXT CHECK (verdict IS NULL OR verdict IN ('useful','meh','useless')),
  note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 4000),
  via TEXT NOT NULL DEFAULT 'ui' CHECK (via IN ('ui','mcp','local_api')),
  client_name TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_usage_log_entity ON usage_log(account_id, entity_type, entity_id, created_at DESC);
INSERT INTO schema_migrations(version, name) VALUES('028', 'extensibility');
