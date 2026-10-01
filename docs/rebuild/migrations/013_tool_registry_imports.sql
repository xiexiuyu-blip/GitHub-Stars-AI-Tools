-- Fox Stars Lab migration 013 tool_registry_imports
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xee11fd
-- Verify against schema/live_schema_018.sql before use.

CREATE TABLE tool_registry_imports (
  id TEXT PRIMARY KEY, account_id TEXT NOT NULL, file_name TEXT NOT NULL,
  content_hash TEXT NOT NULL, format_kind TEXT NOT NULL CHECK(format_kind='markdown_table'),
  imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  selected_count INTEGER NOT NULL, line_count INTEGER NOT NULL, entry_count INTEGER NOT NULL,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE(account_id, content_hash)
);
CREATE TABLE tool_registry_entries (
  import_id TEXT NOT NULL, account_id TEXT NOT NULL, entry_key TEXT NOT NULL,
  tool_id TEXT, source_name TEXT NOT NULL, source_type TEXT NOT NULL,
  source_purpose TEXT NOT NULL, source_status TEXT NOT NULL DEFAULT '',
  warnings_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY(import_id, entry_key),
  FOREIGN KEY(import_id) REFERENCES tool_registry_imports(id) ON DELETE CASCADE,
  FOREIGN KEY(tool_id) REFERENCES tools(id) ON DELETE SET NULL,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE TRIGGER trg_tool_registry_entries_account_insert BEFORE INSERT ON tool_registry_entries
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM tool_registry_imports i WHERE i.id=NEW.import_id AND i.account_id=NEW.account_id)
 OR (NEW.tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tools t WHERE t.id=NEW.tool_id AND t.account_id=NEW.account_id))
BEGIN SELECT RAISE(ABORT,'registry provenance must share account'); END;
CREATE TRIGGER trg_tool_registry_entries_account_update BEFORE UPDATE ON tool_registry_entries
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM tool_registry_imports i WHERE i.id=NEW.import_id AND i.account_id=NEW.account_id)
 OR (NEW.tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tools t WHERE t.id=NEW.tool_id AND t.account_id=NEW.account_id))
BEGIN SELECT RAISE(ABORT,'registry provenance must share account'); END;
CREATE INDEX idx_tool_registry_entries_tool ON tool_registry_entries(account_id,tool_id);
INSERT INTO schema_migrations(version,name) VALUES('013','tool_registry_imports');
