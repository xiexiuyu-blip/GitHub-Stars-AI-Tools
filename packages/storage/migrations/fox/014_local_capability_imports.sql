-- Fox Stars Lab migration 014 local_capability_imports
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xee3485
-- Verify against schema/live_schema_018.sql before use.

CREATE TABLE local_capability_imports (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  source_kind TEXT NOT NULL CHECK(source_kind IN ('skill_directory', 'mcp_config')),
  source_label TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  selected_count INTEGER NOT NULL CHECK(selected_count >= 0),
  entry_count INTEGER NOT NULL CHECK(entry_count >= 0),
  warning_count INTEGER NOT NULL CHECK(warning_count >= 0),
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE(account_id, source_kind, content_hash)
);

CREATE TABLE local_capability_entries (
  import_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  entry_key TEXT NOT NULL,
  tool_id TEXT,
  capability_kind TEXT NOT NULL CHECK(capability_kind IN ('skill', 'mcp')),
  source_name TEXT NOT NULL,
  source_description TEXT NOT NULL DEFAULT '',
  transport_kind TEXT,
  safe_endpoint_origin TEXT,
  package_hint TEXT,
  warnings_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY(import_id, entry_key),
  FOREIGN KEY(import_id) REFERENCES local_capability_imports(id) ON DELETE CASCADE,
  FOREIGN KEY(tool_id) REFERENCES tools(id) ON DELETE SET NULL,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);

CREATE TRIGGER trg_local_capability_entries_account_insert
BEFORE INSERT ON local_capability_entries
FOR EACH ROW WHEN
  NOT EXISTS(
    SELECT 1 FROM local_capability_imports i
    WHERE i.id = NEW.import_id AND i.account_id = NEW.account_id
  )
  OR (NEW.tool_id IS NOT NULL AND NOT EXISTS(
    SELECT 1 FROM tools t WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id
  ))
BEGIN
  SELECT RAISE(ABORT, 'local capability provenance must share account');
END;

CREATE TRIGGER trg_local_capability_entries_account_update
BEFORE UPDATE ON local_capability_entries
FOR EACH ROW WHEN
  NOT EXISTS(
    SELECT 1 FROM local_capability_imports i
    WHERE i.id = NEW.import_id AND i.account_id = NEW.account_id
  )
  OR (NEW.tool_id IS NOT NULL AND NOT EXISTS(
    SELECT 1 FROM tools t WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id
  ))
BEGIN
  SELECT RAISE(ABORT, 'local capability provenance must share account');
END;

CREATE INDEX idx_local_capability_imports_account
  ON local_capability_imports(account_id, source_kind, imported_at);
CREATE INDEX idx_local_capability_entries_tool
  ON local_capability_entries(account_id, tool_id);

INSERT INTO schema_migrations(version, name)
VALUES('014', 'local_capability_imports');
