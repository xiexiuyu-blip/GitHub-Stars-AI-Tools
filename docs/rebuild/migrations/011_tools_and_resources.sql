-- Fox Stars Lab migration 011 tools_and_resources
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xedf730
-- Verify against schema/live_schema_018.sql before use.

-- P3-1: tool cards are account-scoped metadata. Source repositories and
-- inbox bodies remain in their original tables; joins never own a source.
CREATE TABLE tools (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 160),
  normalized_name TEXT NOT NULL CHECK (normalized_name = lower(trim(normalized_name)) AND length(normalized_name) BETWEEN 1 AND 160),
  summary TEXT NOT NULL DEFAULT '' CHECK (length(summary) <= 4000),
  homepage_url TEXT CHECK (homepage_url IS NULL OR (homepage_url GLOB 'http://*' OR homepage_url GLOB 'https://*') AND instr(substr(homepage_url, instr(homepage_url, '//') + 2), '@') = 0),
  selection_status TEXT NOT NULL DEFAULT 'unread' CHECK (selection_status IN ('unread', 'want_to_try', 'tried', 'in_use', 'watching', 'deprecated', 'later', 'read')),
  manual_note TEXT NOT NULL DEFAULT '' CHECK (length(manual_note) <= 12000),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE (account_id, normalized_name)
);

CREATE TABLE tool_forms (
  tool_id TEXT NOT NULL,
  form_kind TEXT NOT NULL CHECK (form_kind IN ('app', 'cli', 'skill', 'mcp', 'api', 'library', 'service', 'other')),
  PRIMARY KEY (tool_id, form_kind),
  FOREIGN KEY (tool_id) REFERENCES tools(id) ON DELETE CASCADE
);

CREATE TABLE tool_repositories (
  tool_id TEXT NOT NULL,
  repository_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  PRIMARY KEY (tool_id, repository_id),
  FOREIGN KEY (tool_id) REFERENCES tools(id) ON DELETE CASCADE,
  FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE CASCADE,
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);

CREATE TABLE tool_inbox_items (
  tool_id TEXT NOT NULL,
  inbox_item_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  PRIMARY KEY (tool_id, inbox_item_id),
  FOREIGN KEY (tool_id) REFERENCES tools(id) ON DELETE CASCADE,
  FOREIGN KEY (inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE,
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);

CREATE INDEX idx_tools_account_updated ON tools(account_id, updated_at DESC);
CREATE INDEX idx_tools_account_status ON tools(account_id, selection_status, updated_at DESC);
CREATE INDEX idx_tool_repositories_account_repository ON tool_repositories(account_id, repository_id);
CREATE INDEX idx_tool_inbox_items_account_inbox ON tool_inbox_items(account_id, inbox_item_id);

CREATE TRIGGER trg_tools_normalized_name_insert
BEFORE INSERT ON tools
FOR EACH ROW WHEN NEW.normalized_name != lower(trim(NEW.name))
BEGIN SELECT RAISE(ABORT, 'tool normalized name must match name'); END;

CREATE TRIGGER trg_tools_normalized_name_update
BEFORE UPDATE OF name, normalized_name ON tools
FOR EACH ROW WHEN NEW.normalized_name != lower(trim(NEW.name))
BEGIN SELECT RAISE(ABORT, 'tool normalized name must match name'); END;

CREATE TRIGGER trg_tool_repositories_account_insert
BEFORE INSERT ON tool_repositories
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM tools t JOIN repositories r ON r.id = NEW.repository_id
  WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id AND r.account_id = NEW.account_id
)
BEGIN SELECT RAISE(ABORT, 'tool repository must share account'); END;

CREATE TRIGGER trg_tool_repositories_account_update
BEFORE UPDATE OF tool_id, repository_id, account_id ON tool_repositories
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM tools t JOIN repositories r ON r.id = NEW.repository_id
  WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id AND r.account_id = NEW.account_id
)
BEGIN SELECT RAISE(ABORT, 'tool repository must share account'); END;

CREATE TRIGGER trg_tool_inbox_items_account_insert
BEFORE INSERT ON tool_inbox_items
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM tools t JOIN inbox_items i ON i.id = NEW.inbox_item_id
  WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id AND i.account_id = NEW.account_id
)
BEGIN SELECT RAISE(ABORT, 'tool inbox item must share account'); END;

CREATE TRIGGER trg_tool_inbox_items_account_update
BEFORE UPDATE OF tool_id, inbox_item_id, account_id ON tool_inbox_items
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM tools t JOIN inbox_items i ON i.id = NEW.inbox_item_id
  WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id AND i.account_id = NEW.account_id
)
BEGIN SELECT RAISE(ABORT, 'tool inbox item must share account'); END;

INSERT INTO schema_migrations(version, name) VALUES ('011', 'tools_and_resources');
