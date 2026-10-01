-- Fox Stars Lab migration 010 inbox_local_pdf_documents
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xee9e1c
-- Verify against schema/live_schema_018.sql before use.

-- 010 is applied by storage.rs inside a BEGIN IMMEDIATE transaction because
-- SQLite needs a table rebuild to extend the historic 009 CHECK constraint.
-- The migration preserves existing local Markdown/text rows and recreates the
-- ownership triggers and indexes after the old table is swapped out.
CREATE TABLE inbox_local_documents_010 (
  inbox_item_id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  file_name TEXT NOT NULL,
  document_kind TEXT NOT NULL CHECK (document_kind IN ('markdown', 'text', 'pdf')),
  content_text TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  page_count INTEGER CHECK (page_count IS NULL OR page_count >= 0),
  content_truncated INTEGER NOT NULL DEFAULT 0 CHECK (content_truncated IN (0, 1)),
  imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE,
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE (account_id, content_hash)
);

INSERT INTO inbox_local_documents_010 (
  inbox_item_id, account_id, file_name, document_kind, content_text,
  content_hash, byte_size, page_count, content_truncated,
  imported_at, created_at, updated_at
)
SELECT inbox_item_id, account_id, file_name, document_kind, content_text,
  content_hash, byte_size, NULL, 0, imported_at, created_at, updated_at
FROM inbox_local_documents;

DROP TABLE inbox_local_documents;
ALTER TABLE inbox_local_documents_010 RENAME TO inbox_local_documents;

CREATE TRIGGER trg_inbox_local_documents_account_insert
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

CREATE TRIGGER trg_inbox_local_documents_account_update
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

CREATE INDEX idx_inbox_local_documents_account_imported
  ON inbox_local_documents(account_id, imported_at DESC);
CREATE INDEX idx_inbox_local_documents_account_kind
  ON inbox_local_documents(account_id, document_kind, imported_at DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('010', 'inbox_local_pdf_documents');
