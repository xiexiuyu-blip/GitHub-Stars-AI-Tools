-- Fox Stars Lab migration 009 inbox_local_documents
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xed4cd1
-- Verify against schema/live_schema_018.sql before use.

PRAGMA foreign_keys = ON;

-- Local files are copied into SQLite at explicit user request. The source
-- filesystem path is intentionally never persisted: the parent card uses an
-- internal, generated URI and this table retains only safe file metadata.
CREATE TABLE IF NOT EXISTS inbox_local_documents (
  inbox_item_id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  file_name TEXT NOT NULL,
  document_kind TEXT NOT NULL CHECK (document_kind IN ('markdown', 'text')),
  content_text TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE,
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE (account_id, content_hash)
);

CREATE TRIGGER IF NOT EXISTS trg_inbox_local_documents_account_insert
BEFORE INSERT ON inbox_local_documents
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM inbox_items
  WHERE id = NEW.inbox_item_id
    AND account_id = NEW.account_id
    AND item_type = 'document'
    AND normalized_url = 'fox-local://document/' || NEW.inbox_item_id
)
BEGIN
  SELECT RAISE(ABORT, 'local inbox document must match local document card');
END;

CREATE TRIGGER IF NOT EXISTS trg_inbox_local_documents_account_update
BEFORE UPDATE OF inbox_item_id, account_id, content_hash ON inbox_local_documents
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM inbox_items
  WHERE id = NEW.inbox_item_id
    AND account_id = NEW.account_id
    AND item_type = 'document'
    AND normalized_url = 'fox-local://document/' || NEW.inbox_item_id
)
BEGIN
  SELECT RAISE(ABORT, 'local inbox document must match local document card');
END;

CREATE INDEX IF NOT EXISTS idx_inbox_local_documents_account_imported
  ON inbox_local_documents(account_id, imported_at DESC);
CREATE INDEX IF NOT EXISTS idx_inbox_local_documents_account_kind
  ON inbox_local_documents(account_id, document_kind, imported_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('009', 'inbox_local_documents');
